//! Owned runtime declarations bound to actual local-search stopping policies.
use crate::{
    DiminishedReturnsConfig, DiminishedReturnsError, MixedSolverSequence, SolverSequence,
    apply_diminished_returns_default, apply_mixed_diminished_returns_default,
    default_diminished_returns_config,
};
use std::{error::Error, fmt, time::Duration};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeDiminishedReturns {
    pub enabled: Option<bool>,
    pub sliding_window_duration: Option<Duration>,
    pub minimum_improvement_ratio: Option<f64>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeTerminationProperties {
    pub diminished_returns: Option<RuntimeDiminishedReturns>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeSolverProperties {
    pub termination: Option<RuntimeTerminationProperties>,
}
/// Unresolved observations retain unset fields even when execution uses defaults.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiminishedReturnsDeclaration {
    pub sliding_window_duration: Option<Duration>,
    pub minimum_improvement_ratio: Option<f64>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RuntimeSolverTermination {
    pub diminished_returns: Option<DiminishedReturnsDeclaration>,
}
/// Receipt from applying declarations to a real sequence. Solver-level defaults
/// are resolved into each missing local phase, not a cross-phase timer. Existing
/// explicit policies and all other phase/solve controls win and remain intact.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiminishedRuntimeBinding {
    pub termination: RuntimeSolverTermination,
    pub resolved: Option<DiminishedReturnsConfig>,
    pub applied_local_phases: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiminishedRuntimeError {
    RatioMustBePositiveFinite,
    Policy(DiminishedReturnsError),
}
impl fmt::Display for DiminishedRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RatioMustBePositiveFinite => {
                f.write_str("runtime improvement ratio must be positive and finite")
            }
            Self::Policy(error) => error.fmt(f),
        }
    }
}
impl Error for DiminishedRuntimeError {}
impl RuntimeSolverProperties {
    fn resolve(self) -> Result<DiminishedRuntimeBinding, DiminishedRuntimeError> {
        let off = DiminishedRuntimeBinding {
            termination: RuntimeSolverTermination::default(),
            resolved: None,
            applied_local_phases: 0,
        };
        let Some(raw) = self.termination.and_then(|t| t.diminished_returns) else {
            return Ok(off);
        };
        // Explicit false suppresses these runtime defaults; it does not delete
        // unrelated declarations or already configured phase policies.
        if raw.enabled == Some(false)
            || (raw.enabled != Some(true)
                && raw.sliding_window_duration.is_none()
                && raw.minimum_improvement_ratio.is_none())
        {
            return Ok(off);
        }
        let defaults = default_diminished_returns_config();
        let window = raw.sliding_window_duration.unwrap_or(defaults.window());
        let config = match raw.minimum_improvement_ratio {
            Some(ratio) => DiminishedReturnsConfig::from_f64(window, ratio),
            None => DiminishedReturnsConfig::new(window, 1, 10_000),
        }
        .map_err(|error| match error {
            DiminishedReturnsError::RatioMustBePositiveFinite => {
                DiminishedRuntimeError::RatioMustBePositiveFinite
            }
            error => DiminishedRuntimeError::Policy(error),
        })?;
        Ok(DiminishedRuntimeBinding {
            termination: RuntimeSolverTermination {
                diminished_returns: Some(DiminishedReturnsDeclaration {
                    sliding_window_duration: raw.sliding_window_duration,
                    minimum_improvement_ratio: raw.minimum_improvement_ratio,
                }),
            },
            resolved: Some(config),
            applied_local_phases: 0,
        })
    }
    /// Stage validation before changing any phase. Only missing local policies
    /// are filled; phases added later need a fresh application.
    pub fn bind<S>(
        self,
        sequence: &mut SolverSequence<'_, S>,
    ) -> Result<DiminishedRuntimeBinding, DiminishedRuntimeError> {
        let mut binding = self.resolve()?;
        if let Some(policy) = binding.resolved {
            binding.applied_local_phases = apply_diminished_returns_default(sequence, policy);
        }
        Ok(binding)
    }
    pub fn bind_mixed<S>(
        self,
        sequence: &mut MixedSolverSequence<'_, S>,
    ) -> Result<DiminishedRuntimeBinding, DiminishedRuntimeError> {
        let mut binding = self.resolve()?;
        if let Some(policy) = binding.resolved {
            binding.applied_local_phases = apply_mixed_diminished_returns_default(sequence, policy);
        }
        Ok(binding)
    }
}
