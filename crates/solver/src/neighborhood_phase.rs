//! Explicit Neighborhoods compatibility requests compiled into native phases.
use crate::{
    LocalSearchConfig, MoveProvider, MoveProviderBindingError, NeighborhoodUnion,
    PhaseMoveProviders, SolverPhase, SolverSequence,
};
use std::{error::Error, fmt};
/// Opt-in applies to this explicit compatibility builder. Ordinary low-level
/// native move providers keep their existing behavior and require no preview flag.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NeighborhoodApi {
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NeighborhoodApiError {
    FeatureDisabled,
    DescentUnsupported,
}
impl fmt::Display for NeighborhoodApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::FeatureDisabled => {
                "NEIGHBORHOODS must be enabled before using an explicit Neighborhoods provider"
            }
            Self::DescentUnsupported => {
                "variable-neighborhood descent does not support the Neighborhoods API"
            }
        })
    }
}
impl Error for NeighborhoodApiError {}
/// Construct the actual algorithm before validating the compatibility opt-in.
/// Descent compilation installs its ordered union and strict first-improvement
/// controls. Limits, assertions and termination settings remain caller supplied.
/// Prepared phases own the validated config and borrow an optional real provider.
pub struct NeighborhoodPhase<'a, S> {
    config: LocalSearchConfig<'a, S>,
    descent: bool,
}
impl<'a, S> NeighborhoodPhase<'a, S> {
    pub fn local_search(config: LocalSearchConfig<'a, S>) -> Self {
        Self {
            config,
            descent: false,
        }
    }
    pub fn variable_descent(
        union: &'a NeighborhoodUnion<'a>,
        mut config: LocalSearchConfig<'a, S>,
    ) -> Result<Self, crate::DescentRequiresSequentialOrder> {
        let template = crate::variable_neighborhood_descent(union)?;
        config.neighborhood_union = template.neighborhood_union;
        config.acceptance = template.acceptance;
        config.move_order = template.move_order;
        config.selection = template.selection;
        config.rejected_moves = template.rejected_moves;
        // Additional acceptors/ranking cannot quietly change the descent policy.
        config.additional_acceptance = &[];
        config.ranking = crate::FinalistRanking::HighestScore;
        Ok(Self {
            config,
            descent: true,
        })
    }
    pub fn prepare<P, A, M>(
        self,
        api: NeighborhoodApi,
        provider: Option<&'a mut dyn MoveProvider<P, A, M>>,
    ) -> Result<PreparedNeighborhoodPhase<'a, P, A, M, S>, NeighborhoodApiError> {
        if provider.is_some() && !api.enabled {
            return Err(NeighborhoodApiError::FeatureDisabled);
        }
        if api.enabled && self.descent {
            return Err(NeighborhoodApiError::DescentUnsupported);
        }
        Ok(PreparedNeighborhoodPhase {
            config: self.config,
            provider,
        })
    }
}
pub struct PreparedNeighborhoodPhase<'a, P, A, M, S> {
    config: LocalSearchConfig<'a, S>,
    provider: Option<&'a mut dyn MoveProvider<P, A, M>>,
}
impl<'a, P, A, M, S> PreparedNeighborhoodPhase<'a, P, A, M, S> {
    pub fn config(&self) -> &LocalSearchConfig<'a, S> {
        &self.config
    }
    /// Append atomically at the actual sequence index and install its provider.
    /// Existing provider bindings are never silently overwritten.
    pub fn append_to(
        self,
        sequence: &mut SolverSequence<'a, S>,
        providers: &mut PhaseMoveProviders<'a, P, A, M>,
    ) -> Result<usize, MoveProviderBindingError> {
        let phase = sequence.phases.len();
        if self.provider.is_some() && providers.contains(phase) {
            return Err(MoveProviderBindingError::AlreadyBound { phase });
        }
        sequence.phases.push(SolverPhase::LocalSearch(self.config));
        if let Some(provider) = self.provider {
            providers.bind(phase, provider);
        }
        Ok(phase)
    }
}
