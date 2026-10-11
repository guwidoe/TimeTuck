//! Retained single-phase basic search entry points.
use super::*;
impl<'p, 'c, V: Eq + Clone, S: AdditiveScore> ScoreDirector<'p, 'c, V, S> {
    /// Run one native construction phase on the retained contribution cache.
    pub fn run_construction(
        &mut self,
        config: crate::ConstructionConfig,
    ) -> Result<crate::ConstructionResult<V, S>, ScoringError> {
        self.run_construction_monitored(None, None, None, config)
    }
    /// Run one native local-search phase and retain its best assignment/cache.
    pub fn run_local_search(
        &mut self,
        config: crate::LocalSearchConfig<'_, S>,
    ) -> Result<crate::LocalSearchResult<V, S>, crate::LocalSearchError> {
        self.run_local_search_monitored(None, None, None, config, None)
    }
}
