//! History-dependent stopping expressions, separate from stateless score queries.
use crate::{
    Score, ScoredSearchTermination, SearchProgress, SignificantImprovement,
    SignificantImprovementConfig, TerminationExpression, TerminationState,
};
use std::{error::Error, fmt, time::Duration};

/// A stateless subtree or an independent score-difference stagnation history.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatefulSearchLimit<'a, S = crate::SimpleScore> {
    Static(&'a ScoredSearchTermination<'a, S>),
    /// Uses the validated threshold's exact comparison and strictly-after-window
    /// expiry. Every occurrence has its own anchor, including repeated leaves.
    SignificantImprovement(SignificantImprovementConfig<S>),
    /// Weighted target/feasibility progress from this scope's initialized score.
    ScoreGoal(crate::ScoreGoalProgressConfig<S>),
    /// Independent rate history, applicable only to local-search phase bindings.
    /// Unknown progress until expiry; solve-wide use is rejected before callbacks.
    DiminishedReturns(crate::PhaseDiminishedReturnsConfig<S>),
}
/// AND/OR composition with retained-score history. Bind solve-wide through
/// [`crate::SolverExtensions::stateful_termination`] or to an individual phase
/// with [`crate::PhaseTerminationBinding::stateful`]. All history occurrences
/// receive score observations and elapsed samples, even if another branch has
/// already decided the expression's Boolean result.
///
/// Score-difference stagnation arms on the first complete retained score after search starts.
/// Initial preparation and construction cannot arm it. Once armed, elapsed time
/// continues through later preparation and construction, but construction scores
/// do not update its anchor. Phase bindings create fresh independent histories.
/// DiminishedReturns is restricted to local-search phase bindings. It arms at
/// the first actual step after setup and a complete search-best observation.
/// Only strict best improvements update its score; worsening samples are ignored.
/// Each window is polled regardless of the expression outcome and latches expiry.
/// Static subtrees retain the usual solve/phase counter and complete-score scope.
/// ScoreGoal baselines instead capture initialized retained scores, including
/// construction; see [`crate::ScoreGoalProgressConfig`] for the distinct policy.
///
/// ```
/// use timetuck::*;
/// use std::time::Duration;
/// let moves = ScoredSearchTermination::Condition(
///     ScoredSearchLimit::Search(SearchLimit::MoveEvaluations(10_000)));
/// let children = [
///     StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&moves)),
///     StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(
///         SignificantImprovementConfig::new(Duration::from_secs(30), SimpleScore::new(100))?)),
/// ];
/// let stop = StatefulSearchTermination::All(&children);
/// assert_eq!(stop.try_status(SearchProgress::default(), None),
///     Err(StatefulTerminationError::HistoryRequired));
/// // Pass Some(&stop) in SolverExtensions::stateful_termination, or bind a phase.
/// let binding = PhaseTerminationBinding::stateful(0, &stop);
/// assert_eq!(binding.phase(), 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type StatefulSearchTermination<'a, S = crate::SimpleScore> =
    TerminationExpression<'a, StatefulSearchLimit<'a, S>>;

/// A stateless query cannot evaluate a history-dependent expression.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatefulTerminationError {
    HistoryRequired,
}
impl fmt::Display for StatefulTerminationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("termination requires a StatefulTerminationSession")
    }
}
impl Error for StatefulTerminationError {}
impl<S: Score> StatefulSearchTermination<'_, S> {
    pub fn try_status(
        &self,
        progress: SearchProgress,
        best: Option<&S>,
    ) -> Result<Option<TerminationState>, StatefulTerminationError> {
        if self.needs_history() {
            return Err(StatefulTerminationError::HistoryRequired);
        }
        Ok(self.evaluate(|leaf| match leaf {
            StatefulSearchLimit::Static(condition) => condition.status(progress, best),
            StatefulSearchLimit::SignificantImprovement(_)
            | StatefulSearchLimit::ScoreGoal(_)
            | StatefulSearchLimit::DiminishedReturns(_) => {
                unreachable!("checked history")
            }
        }))
    }
    fn any_leaf(&self, predicate: impl Fn(&StatefulSearchLimit<'_, S>) -> bool + Copy) -> bool {
        match self {
            Self::Condition(leaf) => predicate(leaf),
            Self::All(children) | Self::Any(children) => {
                children.iter().any(|c| c.any_leaf(predicate))
            }
        }
    }
    pub(crate) fn needs_local_phase(&self) -> bool {
        self.any_leaf(|leaf| matches!(leaf, StatefulSearchLimit::DiminishedReturns(_)))
    }
    pub(crate) fn needs_history(&self) -> bool {
        self.any_leaf(|leaf| !matches!(leaf, StatefulSearchLimit::Static(_)))
    }
    pub(crate) fn needs_best(&self) -> bool {
        self.any_leaf(|leaf| match leaf {
            StatefulSearchLimit::Static(c) => c.needs_best_score(),
            StatefulSearchLimit::ScoreGoal(_) => true,
            _ => false,
        })
    }
    pub(crate) fn needs_scores(&self) -> bool {
        self.any_leaf(
            |leaf| matches!(leaf, StatefulSearchLimit::Static(c) if c.needs_score_counts()),
        )
    }
    pub(crate) fn needs_moves(&self) -> bool {
        self.any_leaf(
            |leaf| matches!(leaf, StatefulSearchLimit::Static(c) if c.needs_move_counts()),
        )
    }
}

type GoalCapture<'a, S> = (
    &'a crate::ScoreGoalProgressConfig<S>,
    Option<crate::goal_progress::GoalProgress<S>>,
);
/// One invocation's score histories. The expression remains immutable and can
/// be reused by other independent sessions. Call `start_search` after setup,
/// then `observe_best` for complete retained scores from search phases only.
/// `status` polls history expiry but never treats its static best-score argument
/// as a new history observation. This separates construction's retained goals
/// from search improvement anchors without rescoring or assignment snapshots.
///
/// Elapsed readings clamp monotonically. Each history's expiry latches; the
/// expression itself is evaluated afresh (integrated solver/phase controls latch
/// the whole expression). `reset` discards all history and search arming.
#[derive(Debug)]
pub struct StatefulTerminationSession<'a, S = crate::SimpleScore> {
    expression: &'a StatefulSearchTermination<'a, S>,
    histories: Vec<SignificantImprovement<S>>,
    goals: Vec<GoalCapture<'a, S>>,
    rates: Vec<crate::DiminishedReturns<S>>,
    rate_best: Option<S>,
    started: bool,
    first_step: Option<Duration>,
    last: Duration,
}
impl<'a, S: Score> StatefulTerminationSession<'a, S> {
    pub fn new(expression: &'a StatefulSearchTermination<'a, S>) -> Self {
        fn collect<'a, S: Score>(
            expression: &'a StatefulSearchTermination<'a, S>,
            histories: &mut Vec<SignificantImprovement<S>>,
            goals: &mut Vec<GoalCapture<'a, S>>,
            rates: &mut Vec<crate::DiminishedReturns<S>>,
        ) {
            match expression {
                TerminationExpression::Condition(StatefulSearchLimit::SignificantImprovement(
                    c,
                )) => histories.push(SignificantImprovement::new(c.clone())),
                TerminationExpression::Condition(StatefulSearchLimit::ScoreGoal(c)) => {
                    goals.push((c, None))
                }
                TerminationExpression::Condition(StatefulSearchLimit::DiminishedReturns(c)) => {
                    rates.push(c.capture());
                }
                TerminationExpression::Condition(_) => {}
                TerminationExpression::All(children) | TerminationExpression::Any(children) => {
                    for child in *children {
                        collect(child, histories, goals, rates);
                    }
                }
            }
        }
        let mut histories = Vec::new();
        let mut goals = Vec::new();
        let mut rates = Vec::new();
        collect(expression, &mut histories, &mut goals, &mut rates);
        Self {
            expression,
            histories,
            goals,
            rates,
            rate_best: None,
            started: false,
            first_step: None,
            last: Duration::ZERO,
        }
    }
    /// Capture each score goal's baseline once. This does not update best-score
    /// retention or arm stagnation. Supply only complete initialized observations;
    /// negative structural scores are ignored. Status queries never capture it.
    pub fn observe_initialized_score(&mut self, score: &S) {
        if score.structural() < 0 {
            return;
        }
        for (config, state) in &mut self.goals {
            if state.is_none() {
                *state = Some(config.capture(score));
            }
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new(self.expression);
    }
    pub fn start_search(&mut self, elapsed: Duration) {
        self.last = self.last.max(elapsed);
        self.started = true;
    }
    pub(crate) fn needs_first_step(&self) -> bool {
        !self.rates.is_empty() && self.first_step.is_none()
    }
    /// Arm phase rate windows at the actual first step, after initialization.
    /// Repeated steps do not restart grace; all rate occurrences keep their own histories.
    pub fn start_step(&mut self, elapsed: Duration) {
        self.last = self.last.max(elapsed);
        if !self.started || self.first_step.is_some() {
            return;
        }
        self.first_step = Some(self.last);
        if let Some(best) = &self.rate_best {
            for rate in &mut self.rates {
                rate.start(self.last, best.clone());
            }
        }
    }
    /// Call only for complete retained search results. Structurally flawed scores
    /// are ignored; accepted worsening states never reset a threshold anchor.
    pub fn observe_best(&mut self, elapsed: Duration, score: &S) {
        self.last = self.last.max(elapsed);
        if !self.started || score.structural() < 0 {
            return;
        }
        if !self.rates.is_empty() && self.rate_best.as_ref().is_none_or(|best| score > best) {
            self.rate_best = Some(score.clone());
            for rate in &mut self.rates {
                if self.first_step.is_some() && !rate.grace_started() {
                    rate.start(self.last, score.clone());
                } else {
                    rate.step(self.last, score);
                }
            }
        }
        for history in &mut self.histories {
            if history.is_started() {
                history.is_terminated(self.last, score);
            } else {
                history.start(self.last, score.clone());
            }
        }
    }
    pub fn status(
        &mut self,
        mut progress: SearchProgress,
        best: Option<&S>,
    ) -> Option<TerminationState> {
        self.last = self.last.max(progress.elapsed);
        progress.elapsed = self.last;
        let mut histories = self.histories.iter_mut();
        let mut goals = self.goals.iter();
        let mut rates = self.rates.iter_mut();
        self.expression.evaluate(|leaf| match leaf {
            StatefulSearchLimit::Static(condition) => condition.status(progress, best),
            StatefulSearchLimit::DiminishedReturns(_) => {
                let rate = rates.next().expect("one rate per occurrence");
                let reached = self.first_step.is_some()
                    && self
                        .rate_best
                        .as_ref()
                        .is_some_and(|best| rate.is_terminated(self.last, best));
                Some(TerminationState {
                    reached,
                    progress: reached.then_some(1.0),
                })
            }
            StatefulSearchLimit::ScoreGoal(_) => {
                let (_, state) = goals.next().expect("one goal per occurrence");
                Some(
                    match (state, best.filter(|score| score.structural() >= 0)) {
                        (Some(state), Some(best)) => state.status(best),
                        _ => TerminationState {
                            reached: false,
                            progress: None,
                        },
                    },
                )
            }
            StatefulSearchLimit::SignificantImprovement(_) => {
                let history = histories.next().expect("one history per occurrence");
                let reached = history.expired(self.last);
                Some(TerminationState {
                    reached,
                    progress: history.is_started().then(|| history.progress(self.last)),
                })
            }
        })
    }
}
