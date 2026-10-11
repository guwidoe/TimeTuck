//! Invocation-local composed stopping conditions for individual phases.
use crate::{
    MoveEvaluationCounts, Score, ScoredSearchTermination, SearchProgress, SearchTermination,
    StepProgress,
};
use std::time::Duration;

/// Bind one phase in [`crate::SolverExtensions::phase_terminations`]. Indices are
/// zero-based in the complete plan; duplicate and missing indices are rejected
/// before clock, lifecycle or scoring callbacks. A binding stops only its phase;
/// independent global limits can still stop the solve.
///
/// Counters and elapsed time start anew immediately before phase-start callbacks
/// and preparation. Initial scoring counts as a score, not a move. Complete
/// retained input seeds scored goals; partial input never does. Construction
/// contributes its current complete score only at retained placement boundaries.
/// Search stagnation arms at the kernel's start hook (after local/repair/custom
/// initialization, before exact enumeration); construction never arms it.
///
/// Expressions are polled cooperatively inside the usual kernel boundaries,
/// including reconstruction scans. Expiry latches for this phase and is removed
/// before handoff. Existing hard limits keep their precedence. An initial-only
/// immovable solve enters no phase and therefore does not use phase bindings.
#[derive(Debug)]
pub struct PhaseTerminationBinding<'a, S = crate::SimpleScore> {
    pub(crate) phase: usize,
    pub(crate) search: Option<&'a SearchTermination<'a>>,
    pub(crate) scored: Option<&'a ScoredSearchTermination<'a, S>>,
    pub(crate) stateful: Option<&'a crate::StatefulSearchTermination<'a, S>>,
}
impl<'a, S> PhaseTerminationBinding<'a, S> {
    pub fn search(phase: usize, condition: &'a SearchTermination<'a>) -> Self {
        Self {
            phase,
            search: Some(condition),
            scored: None,
            stateful: None,
        }
    }
    pub fn scored(phase: usize, condition: &'a ScoredSearchTermination<'a, S>) -> Self {
        Self {
            phase,
            search: None,
            scored: Some(condition),
            stateful: None,
        }
    }
    pub fn stateful(phase: usize, condition: &'a crate::StatefulSearchTermination<'a, S>) -> Self {
        Self {
            phase,
            search: None,
            scored: None,
            stateful: Some(condition),
        }
    }
    pub fn phase(&self) -> usize {
        self.phase
    }
}
impl<S: Score> PhaseTerminationBinding<'_, S> {
    pub(crate) fn needs_scores(&self) -> bool {
        self.search
            .is_some_and(SearchTermination::needs_score_counts)
            || self
                .scored
                .is_some_and(ScoredSearchTermination::needs_score_counts)
            || self
                .stateful
                .is_some_and(crate::StatefulSearchTermination::needs_scores)
    }
    pub(crate) fn needs_moves(&self) -> bool {
        self.search
            .is_some_and(SearchTermination::needs_move_counts)
            || self
                .scored
                .is_some_and(ScoredSearchTermination::needs_move_counts)
            || self
                .stateful
                .is_some_and(crate::StatefulSearchTermination::needs_moves)
    }
    pub(crate) fn needs_best(&self) -> bool {
        self.scored
            .is_some_and(ScoredSearchTermination::needs_best_score)
            || self
                .stateful
                .is_some_and(crate::StatefulSearchTermination::needs_best)
    }
}
pub(crate) struct PhaseControl<'a, S> {
    pub(crate) binding: &'a PhaseTerminationBinding<'a, S>,
    origin: Duration,
    stateful: Option<crate::StatefulTerminationSession<'a, S>>,
    scores: u64,
    moves: MoveEvaluationCounts,
    pub(crate) steps: StepProgress,
    pub(crate) construction_steps: u64,
    pub(crate) last_improvement: Option<Duration>,
    best: Option<S>,
    pub(crate) expired: bool,
}
impl<'a, S: Score> PhaseControl<'a, S> {
    pub(crate) fn new(
        binding: &'a PhaseTerminationBinding<'a, S>,
        origin: Duration,
        scores: u64,
        moves: MoveEvaluationCounts,
        best: Option<&S>,
    ) -> Self {
        Self {
            binding,
            origin,
            stateful: binding.stateful.map(crate::StatefulTerminationSession::new),
            scores,
            moves,
            steps: StepProgress::default(),
            construction_steps: 0,
            last_improvement: None,
            best: if binding.needs_best() {
                best.cloned()
            } else {
                None
            },
            expired: false,
        }
    }
    pub(crate) fn needs_history(&self) -> bool {
        self.binding
            .stateful
            .is_some_and(crate::StatefulSearchTermination::needs_history)
    }
    pub(crate) fn start_history(&mut self, now: Duration) {
        if let Some(session) = &mut self.stateful {
            session.start_search(now.saturating_sub(self.origin));
        }
    }
    pub(crate) fn observe_history(&mut self, now: Duration, score: &S) {
        if let Some(session) = &mut self.stateful {
            session.observe_best(now.saturating_sub(self.origin), score);
        }
    }
    pub(crate) fn observe_best(&mut self, score: &S) {
        if self.binding.needs_best()
            && score.structural() >= 0
            && self.best.as_ref().is_none_or(|best| score > best)
        {
            self.best = Some(score.clone());
        }
    }
    pub(crate) fn reached(
        &mut self,
        now: Duration,
        scores: u64,
        moves: MoveEvaluationCounts,
        construction_score: Option<&S>,
    ) -> bool {
        let progress = SearchProgress {
            elapsed: now.saturating_sub(self.origin),
            completed_steps: self.steps.completed_steps(),
            construction_steps: self.construction_steps,
            score_evaluations: scores.saturating_sub(self.scores),
            move_evaluations: moves.since(self.moves),
            unimproved_steps: self.last_improvement.map(|_| self.steps.unimproved_steps()),
            unimproved_time: self.last_improvement.map(|last| now.saturating_sub(last)),
        };
        let best = self
            .best
            .as_ref()
            .into_iter()
            .chain(construction_score)
            .max();
        // Evaluate sessions before combining: no history update is short-circuited.
        let stateful_reached = self
            .stateful
            .as_mut()
            .is_some_and(|session| session.status(progress, best).is_some_and(|s| s.reached));
        self.expired |= self
            .binding
            .search
            .is_some_and(|c| c.status(progress).is_some_and(|s| s.reached))
            || self.binding.scored.is_some_and(|c| {
                c.status(
                    progress,
                    self.best
                        .as_ref()
                        .into_iter()
                        .chain(construction_score)
                        .max(),
                )
                .is_some_and(|s| s.reached)
            });
        self.expired |= stateful_reached;
        self.expired
    }
}
