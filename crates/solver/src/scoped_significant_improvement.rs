//! Distinct solve and phase timing for initialized score-threshold searches.
use crate::{
    Score, SignificantImprovement, SignificantImprovementConfig, SolverPhaseKind,
    TerminationProgressScope,
};
use std::time::Duration;

/// Runtime used by the ordered solver's score-aware time profile. Construction
/// consumes no stagnation interval. Its completion arms the solve interval;
/// entering improvement search arms a distinct phase interval. Scores are seeded
/// from the first initialized best at search entry, independently of time origins.
/// Threshold improvements update both active intervals, and late improvements
/// cannot revive expired ones. A later search phase receives a fresh phase timer
/// while the solve timer continues. Timestamps are elapsed monotonic durations.
#[derive(Clone, Debug)]
pub struct ScopedSignificantImprovement<S> {
    config: SignificantImprovementConfig<S>,
    solver: SignificantImprovement<S>,
    phase: SignificantImprovement<S>,
    kind: Option<SolverPhaseKind>,
    solver_origin: Option<Duration>,
    phase_origin: Duration,
}
impl<S: Score> ScopedSignificantImprovement<S> {
    pub fn new(config: SignificantImprovementConfig<S>) -> Self {
        Self {
            solver: SignificantImprovement::new(config.clone()),
            phase: SignificantImprovement::new(config.clone()),
            config,
            kind: None,
            solver_origin: None,
            phase_origin: Duration::ZERO,
        }
    }
    pub fn phase_started(&mut self, kind: SolverPhaseKind, now: Duration) {
        self.kind = Some(kind);
        self.phase_origin = now;
        self.phase = SignificantImprovement::new(self.config.clone());
    }
    pub fn phase_ended(&mut self, kind: SolverPhaseKind, now: Duration) {
        if kind == SolverPhaseKind::Construction && self.kind == Some(kind) {
            self.solver_origin = Some(now);
            self.solver = SignificantImprovement::new(self.config.clone());
        }
    }
    fn active(&self) -> bool {
        self.kind
            .is_some_and(|kind| kind != SolverPhaseKind::Construction)
    }
    pub fn is_started(&self) -> bool {
        self.active() && self.solver.is_started()
    }
    pub fn observe_best(&mut self, now: Duration, best: &S) {
        if !self.active() {
            return;
        }
        if !self.solver.is_started() {
            self.solver
                .start(self.solver_origin.unwrap_or(now), best.clone());
        }
        if !self.phase.is_started() {
            self.phase.start(self.phase_origin, best.clone());
        }
        self.solver.is_terminated(now, best);
        self.phase.is_terminated(now, best);
    }
    pub fn is_terminated(&mut self, scope: TerminationProgressScope, now: Duration) -> bool {
        if !self.active() {
            return false;
        }
        match scope {
            TerminationProgressScope::Solve => self.solver.expired(now),
            TerminationProgressScope::Phase => self.phase.expired(now),
        }
    }
    pub fn progress(&mut self, scope: TerminationProgressScope, now: Duration) -> f64 {
        if !self.active() {
            return 0.0;
        }
        match scope {
            TerminationProgressScope::Solve => self.solver.progress(now),
            TerminationProgressScope::Phase => self.phase.progress(now),
        }
    }
}
