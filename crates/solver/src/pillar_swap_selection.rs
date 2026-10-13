//! Optional policy assembly and bounded weighted snapshots of real pillar swaps.
use crate::{
    Assignment, FiniteProblem, ModelError, Move, PillarChangeMode, PillarMoveProvider,
    PillarSelectionPolicy, WeightedSelection, WeightedSelectionError,
};
use std::{error::Error, fmt};
#[derive(Debug)]
pub enum PillarSwapSelectionError {
    Model(ModelError),
    Weight(WeightedSelectionError),
}
impl fmt::Display for PillarSwapSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Weight(e) => e.fmt(f),
        }
    }
}
impl Error for PillarSwapSelectionError {}
/// Builder retaining validated primary/secondary selection policies and a fixed
/// probability weight. `select` consumes these through actual live grouping and
/// ordinary range/pin checks, then the native weighted distribution. Snapshot
/// capture is explicitly bounded; this optional path does not change solver
/// defaults. Reopen after assignment changes to avoid stale grouping or weights.
#[derive(Clone, Debug)]
pub struct PillarSwapSelection {
    primary: PillarSelectionPolicy,
    secondary: PillarSelectionPolicy,
    weight: f64,
}
impl Default for PillarSwapSelection {
    fn default() -> Self {
        Self {
            primary: PillarSelectionPolicy::whole(),
            secondary: PillarSelectionPolicy::whole(),
            weight: 1.0,
        }
    }
}
impl PillarSwapSelection {
    pub fn with_primary(mut self, policy: PillarSelectionPolicy) -> Self {
        self.primary = policy;
        self
    }
    pub fn with_secondary(mut self, policy: PillarSelectionPolicy) -> Self {
        self.secondary = policy;
        self
    }
    pub fn with_probability_weight(mut self, weight: f64) -> Result<Self, WeightedSelectionError> {
        WeightedSelection::new(vec![((), weight)])?;
        self.weight = weight;
        Ok(self)
    }
    pub fn primary(&self) -> &PillarSelectionPolicy {
        &self.primary
    }
    pub fn secondary(&self) -> &PillarSelectionPolicy {
        &self.secondary
    }
    pub fn probability_weight(&self) -> f64 {
        self.weight
    }
    pub fn select<V: Eq + Clone>(
        &self,
        p: &FiniteProblem<V>,
        a: &Assignment<V>,
        maximum: usize,
    ) -> Result<WeightedSelection<Move<V>>, PillarSwapSelectionError> {
        let provider = PillarMoveProvider {
            left: self.primary.clone(),
            right: self.secondary.clone(),
            changes: PillarChangeMode::Disabled,
            swaps: true,
        };
        let moves = provider
            .select(p, a)
            .map_err(PillarSwapSelectionError::Model)?;
        WeightedSelection::from_bounded_iter(moves.map(|m| (m, self.weight)), maximum)
            .map_err(PillarSwapSelectionError::Weight)
    }
}
