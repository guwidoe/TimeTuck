//! Explicit managed-context construction and ownership transfer.
use super::*;
impl<'p, 'c, V: Eq + Clone, S: AdditiveScore> ScoreDirector<'p, 'c, V, S> {
    /// The shared immutable working model, retained across managed phase changes.
    pub fn working_model(&self) -> &'p FiniteProblem<V> {
        self.problem
    }

    /// Manage actual phase work on this cache. On a mode change, a fresh cache
    /// is prepared before transferring the assignment Vec and local weights.
    /// Scoring errors leave the existing cache and assignment intact.
    pub fn into_context(
        self,
        global: crate::EnvironmentMode,
        phases: Vec<crate::EnvironmentMode>,
        metrics: crate::ContextMetrics,
    ) -> crate::ScoringContextManager<Self, crate::scoring_context::ContextRebuilder<Self>> {
        crate::ScoringContextManager::new(
            self,
            global,
            phases,
            metrics,
            Self::rebuild_context as crate::scoring_context::ContextRebuilder<Self>,
        )
    }
    fn rebuild_context(&mut self, _: crate::ContextSettings) -> Result<Self, ScoringError> {
        let contributions = (0..self.network.constraint_count())
            .map(|i| {
                self.network.contribution_weighted(
                    i,
                    &self.assignment,
                    self.weights
                        .get(i, self.network.constraints[i].weight.as_ref()),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let sum = self
            .network
            .arithmetic
            .sum(&contributions)
            .map_err(|_| ScoringError::ScoreOverflow { constraint: None })?;
        let score = with_structure(sum, self.problem, &self.assignment);
        // All fallible work and user callbacks precede ownership transfer.
        let assignment = std::mem::replace(&mut self.assignment, Assignment::new(Vec::new()));
        Ok(Self {
            problem: self.problem,
            network: self.network,
            assignment,
            weights: self.weights.clone(),
            contributions,
            score,
            retained_changes: None,
        })
    }
}
