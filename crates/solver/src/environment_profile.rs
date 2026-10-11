//! Global and per-phase verification configuration, before runtime binding.
use crate::SolverPhaseKind;
use std::{error::Error, fmt};
type PhaseCheckResult<S> = Result<Option<S>, String>;

/// Requested diagnostic policy. Ordering is the configuration eligibility order,
/// not a guarantee that one mode detects every error detected by another.
/// These values alone do not install runtime checks or control random seeds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum EnvironmentMode {
    NonReproducible,
    NoAssert,
    #[default]
    PhaseAssert,
    StepAssert,
    NonIntrusiveFullAssert,
    FullAssert,
    TrackedFullAssert,
}
impl EnvironmentMode {
    pub fn is_reproducible(self) -> bool {
        self != Self::NonReproducible
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EnvironmentProfileError {
    NonReproducibleOverride {
        phase: usize,
        requested: EnvironmentMode,
    },
    WeakerPhase {
        phase: usize,
        global: EnvironmentMode,
        requested: EnvironmentMode,
    },
}
impl fmt::Display for EnvironmentProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonReproducibleOverride { phase, requested } => write!(
                f,
                "phase {phase} environmentMode {requested:?} is only possible when global environmentMode is reproducible"
            ),
            Self::WeakerPhase {
                phase,
                global,
                requested,
            } => write!(
                f,
                "phase {phase} environmentMode {requested:?} must have an assertion level higher than or equal to the global environment level {global:?}"
            ),
        }
    }
}
impl Error for EnvironmentProfileError {}

/// Preserve declarations separately from their inherited values. Queries remain
/// available even when validation fails; reading does not prepare or mutate.
/// The global policy governs work outside phases and is never inferred from them.
/// Runtime mode scheduling, score-director replacement, diagnostics and random
/// reseeding are separate capabilities; this profile does not implement them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct EnvironmentProfile {
    global: Option<EnvironmentMode>,
    phases: Vec<Option<EnvironmentMode>>,
}
impl EnvironmentProfile {
    pub fn new(global: Option<EnvironmentMode>, phases: Vec<Option<EnvironmentMode>>) -> Self {
        Self { global, phases }
    }
    pub fn declared_global(&self) -> Option<EnvironmentMode> {
        self.global
    }
    pub fn declared_phases(&self) -> &[Option<EnvironmentMode>] {
        &self.phases
    }
    pub fn global_mode(&self) -> EnvironmentMode {
        self.global.unwrap_or_default()
    }
    pub fn phase_modes(&self) -> impl ExactSizeIterator<Item = EnvironmentMode> + '_ {
        self.phases
            .iter()
            .map(|mode| mode.unwrap_or(self.global_mode()))
    }
    /// Only genuine overrides, retaining the original phase index. An explicit
    /// restatement of the global mode remains declared but is not an override.
    pub fn overrides(&self) -> impl Iterator<Item = (usize, EnvironmentMode)> + '_ {
        self.phases.iter().enumerate().filter_map(|(phase, mode)| {
            mode.filter(|mode| *mode != self.global_mode())
                .map(|mode| (phase, mode))
        })
    }
    pub fn validate(&self) -> Result<(), EnvironmentProfileError> {
        let global = self.global_mode();
        for (phase, requested) in self.overrides() {
            if !global.is_reproducible() {
                return Err(EnvironmentProfileError::NonReproducibleOverride { phase, requested });
            }
            if requested < global {
                return Err(EnvironmentProfileError::WeakerPhase {
                    phase,
                    global,
                    requested,
                });
            }
        }
        Ok(())
    }
    /// Validate the entire declaration before allocating a resolved snapshot.
    /// The snapshot retains both the global mode and every phase's effective mode.
    ///
    /// ```
    /// use timetuck::{EnvironmentMode as M, EnvironmentProfile};
    /// let declaration = EnvironmentProfile::new(None, vec![None, Some(M::FullAssert)]);
    /// let prepared = declaration.prepare()?;
    /// assert_eq!(prepared.global_mode(), M::PhaseAssert);
    /// assert_eq!(prepared.phase_mode(0), Some(M::PhaseAssert));
    /// assert_eq!(prepared.phase_mode(1), Some(M::FullAssert));
    /// assert_eq!(declaration.declared_global(), None);
    /// # Ok::<(), timetuck::EnvironmentProfileError>(())
    /// ```
    pub fn prepare(&self) -> Result<PreparedEnvironmentProfile, EnvironmentProfileError> {
        self.validate()?;
        Ok(PreparedEnvironmentProfile {
            global: self.global_mode(),
            phases: self.phase_modes().collect(),
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedEnvironmentProfile {
    global: EnvironmentMode,
    phases: Vec<EnvironmentMode>,
}
impl PreparedEnvironmentProfile {
    pub fn global_mode(&self) -> EnvironmentMode {
        self.global
    }
    pub fn phase_modes(&self) -> &[EnvironmentMode] {
        &self.phases
    }
    pub fn phase_mode(&self, phase: usize) -> Option<EnvironmentMode> {
        self.phases.get(phase).copied()
    }
    /// Bind selective phase checks to `SolverExtensions::score_check`. The
    /// supplied scorer must independently calculate the full score, including
    /// structural incompleteness, without mutating domain facts. Diagnostic
    /// calculations keep the extension callback's existing counter/time rules.
    /// NoAssert skips this callback; PhaseAssert invokes it at phase handoff.
    /// Synthetic initial-only scoring uses the global mode. Other native checks
    /// configured separately in solver limits are left intact.
    ///
    /// Reject unsupported modes before returning a callback, even for phases
    /// that a solve might never enter. This does not reseed randomness or install
    /// step/undo/tracked diagnostics. Phase indices must match the solver plan.
    pub fn score_check<'a, A, S>(
        &'a self,
        mut fresh: impl FnMut(&A) -> Result<S, String> + 'a,
    ) -> Result<
        impl FnMut(usize, SolverPhaseKind, &A) -> PhaseCheckResult<S> + 'a,
        EnvironmentBindingError,
    > {
        for (phase, mode) in std::iter::once((None, self.global)).chain(
            self.phases
                .iter()
                .copied()
                .enumerate()
                .map(|(i, m)| (Some(i), m)),
        ) {
            if !matches!(
                mode,
                EnvironmentMode::NoAssert | EnvironmentMode::PhaseAssert
            ) {
                return Err(EnvironmentBindingError { phase, mode });
            }
        }
        Ok(move |phase: usize, kind: SolverPhaseKind, assignment: &A| {
            let mode = if kind == SolverPhaseKind::Initial {
                self.global
            } else {
                self.phase_mode(phase)
                    .ok_or_else(|| format!("verification profile has no phase {phase}"))?
            };
            if mode == EnvironmentMode::PhaseAssert {
                fresh(assignment).map(Some)
            } else {
                Ok(None)
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvironmentBindingError {
    /// None identifies the global mode; Some identifies the first unsupported phase.
    pub phase: Option<usize>,
    pub mode: EnvironmentMode,
}
impl fmt::Display for EnvironmentBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "phase score-check binding does not implement {:?} for ",
            self.mode
        )?;
        match self.phase {
            Some(phase) => write!(f, "phase {phase}"),
            None => f.write_str("the global mode"),
        }
    }
}
impl Error for EnvironmentBindingError {}
