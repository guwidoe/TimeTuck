//! Fluent native phase configuration consumed directly by existing local search.
use crate::{Acceptance, LocalSearchConfig, MoveSelection, SolverPhase};
use std::num::NonZeroU64;

pub struct LocalSearchPhasePlan<'a, S = crate::SimpleScore> {
    config: LocalSearchConfig<'a, S>,
}
impl<S> Default for LocalSearchPhasePlan<'_, S> {
    fn default() -> Self {
        Self::new(LocalSearchConfig::default())
    }
}
impl<'a, S> LocalSearchPhasePlan<'a, S> {
    pub fn new(config: LocalSearchConfig<'a, S>) -> Self {
        Self { config }
    }
    /// Explicit native variable tabu with best-accepted scanning replaces Java
    /// preset tuning. Kernel validation and acceptance remain authoritative.
    pub fn with_tabu_search(mut self, tenure: usize, aspiration: bool) -> Self {
        self.config.acceptance = Acceptance::VariableTabu { tenure, aspiration };
        self.config.selection = MoveSelection::BestAccepted;
        self
    }
    /// Phase-local retained-best feasibility, using the score's existing semantics.
    pub fn with_feasible_stop(mut self, enabled: bool) -> Self {
        self.config.stop_when_feasible = enabled;
        self
    }
    /// Accepted candidates per neighborhood scan; None clears a previous cap.
    /// This is separate from phase-wide accepted-move/step budgets.
    pub fn with_accepted_candidate_limit(mut self, limit: Option<NonZeroU64>) -> Self {
        self.config.accepted_candidate_limit = limit;
        self
    }
    pub fn config(&self) -> &LocalSearchConfig<'a, S> {
        &self.config
    }
    pub fn into_config(self) -> LocalSearchConfig<'a, S> {
        self.config
    }
    pub fn into_phase(self) -> SolverPhase<'a, S> {
        SolverPhase::LocalSearch(self.config)
    }
}
