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
    RatioNotRepresentable,
    Policy(DiminishedReturnsError),
}
impl fmt::Display for DiminishedRuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RatioMustBePositiveFinite => {
                f.write_str("runtime improvement ratio must be positive and finite")
            }
            Self::RatioNotRepresentable => f.write_str(
                "runtime improvement ratio requires a numerator or denominator beyond u64",
            ),
            Self::Policy(error) => error.fmt(f),
        }
    }
}
impl Error for DiminishedRuntimeError {}
/// Exact rational value of the supplied IEEE754 number; no decimal rounding or
/// silent approximation. Reject values outside the native u64 fraction envelope.
fn rational(value: f64) -> Result<(u64, u64), DiminishedRuntimeError> {
    if !value.is_finite() || value <= 0.0 {
        return Err(DiminishedRuntimeError::RatioMustBePositiveFinite);
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let mut mantissa = bits & ((1u64 << 52) - 1);
    let mut power = if exponent == 0 {
        -1074
    } else {
        mantissa |= 1u64 << 52;
        exponent - 1023 - 52
    };
    let zeros = mantissa.trailing_zeros();
    mantissa >>= zeros;
    power += zeros as i32;
    let range = DiminishedRuntimeError::RatioNotRepresentable;
    if power >= 0 {
        let factor = 1u64.checked_shl(power as u32).ok_or(range)?;
        Ok((mantissa.checked_mul(factor).ok_or(range)?, 1))
    } else {
        Ok((mantissa, 1u64.checked_shl((-power) as u32).ok_or(range)?))
    }
}
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
        let (n, d) = raw
            .minimum_improvement_ratio
            .map(rational)
            .transpose()?
            .unwrap_or(defaults.ratio());
        let config = DiminishedReturnsConfig::new(
            raw.sliding_window_duration.unwrap_or(defaults.window()),
            n,
            d,
        )
        .map_err(DiminishedRuntimeError::Policy)?;
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
