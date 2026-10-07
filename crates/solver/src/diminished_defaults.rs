//! Apply a default stopping policy to existing local-search phases.
use crate::{
    DiminishedReturnsConfig, MixedSolverPhase, MixedSolverSequence, SolverPhase, SolverSequence,
};
use std::time::Duration;

/// Community's default diminished-returns settings: a thirty-second sliding
/// window and an exact minimum rate ratio of one ten-thousandth.
pub fn default_diminished_returns_config() -> DiminishedReturnsConfig {
    DiminishedReturnsConfig::new(Duration::from_secs(30), 1, 10_000)
        .expect("positive built-in window and ratio")
}

/// Fill only missing local-search policies in the current sequence. Explicit
/// phase settings win. Other phase kinds and shared solve limits are unchanged.
/// Returns the number of phases changed. Each local phase starts its own timer
/// through the ordinary search engine; this creates no cross-phase time budget.
/// Phases added later require another call. No score/plan cloning is required.
pub fn apply_diminished_returns_default<S>(
    sequence: &mut SolverSequence<'_, S>,
    policy: DiminishedReturnsConfig,
) -> usize {
    let mut changed = 0;
    for phase in &mut sequence.phases {
        if let SolverPhase::LocalSearch(config) = phase
            && config.diminished_returns.is_none()
        {
            config.diminished_returns = Some(policy);
            changed += 1;
        }
    }
    changed
}

/// The same default application for mixed-model local-search phases. Component
/// order and all existing local-search controls are retained.
pub fn apply_mixed_diminished_returns_default<S>(
    sequence: &mut MixedSolverSequence<'_, S>,
    policy: DiminishedReturnsConfig,
) -> usize {
    let mut changed = 0;
    for phase in &mut sequence.phases {
        if let MixedSolverPhase::LocalSearch(config) = phase
            && config.search.diminished_returns.is_none()
        {
            config.search.diminished_returns = Some(policy);
            changed += 1;
        }
    }
    changed
}
