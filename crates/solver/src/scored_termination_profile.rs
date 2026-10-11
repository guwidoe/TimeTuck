//! Score-aware configuration validated once before scoped solver binding.
use crate::{
    MixedSolverSequence, Score, SignificantImprovementConfig, SignificantImprovementError,
    SolverLimits, SolverSequence, TerminationProfile,
};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoredTerminationProfileError {
    MissingUnimprovedTime,
    NoFeasibleLevels,
    InvalidThreshold(SignificantImprovementError),
}
impl fmt::Display for ScoredTerminationProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingUnimprovedTime => f.write_str(
                "score difference threshold can only be used if an unimproved*SpentLimit is configured",
            ),
            Self::NoFeasibleLevels => f.write_str(
                "feasibility stopping can only be used with a score type that has at least 1 feasible level",
            ),
            Self::InvalidThreshold(error) => error.fmt(f),
        }
    }
}
impl Error for ScoredTerminationProfileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidThreshold(error) => Some(error),
            _ => None,
        }
    }
}
/// Add score-dependent settings to an existing stopping profile. A configured
/// stagnation budget selects either ordinary or significant-improvement timing;
/// it replaces both inherited variants, so the ordinary timer cannot mask the
/// requested threshold. Missing settings preserve the corresponding controls.
/// Feasibility is explicit: None inherits, Some(false) clears, Some(true) enables.
/// Validation and threshold preparation add no work to score evaluation loops.
///
/// ```
/// use timetuck::*;
/// use std::time::Duration;
/// let time = TerminationProfile::new(None, Some(Duration::from_secs(30)),
///     None, None, None, None)?;
/// let profile = ScoredTerminationProfile::new(time,
///     Some(HardSoftScore::new(1, 0)), Some(true))?;
/// profile.with_limits(SolverLimits::default(), |limits| {
///     assert!(limits.stop_when_feasible);
///     assert!(limits.max_unimproved_time.is_none());
///     assert_eq!(limits.significant_improvement.unwrap().window(), Duration::from_secs(30));
/// });
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredTerminationProfile<S> {
    profile: TerminationProfile,
    significant: Option<SignificantImprovementConfig<S>>,
    feasible: Option<bool>,
}
impl<S: Score> ScoredTerminationProfile<S> {
    pub fn new(
        profile: TerminationProfile,
        minimum_improvement: Option<S>,
        feasible: Option<bool>,
    ) -> Result<Self, ScoredTerminationProfileError> {
        if feasible == Some(true) && S::HARD_LEVEL_COUNT == 0 {
            return Err(ScoredTerminationProfileError::NoFeasibleLevels);
        }
        let significant = minimum_improvement
            .map(|minimum| {
                let window = profile
                    .unimproved_spent_limit()
                    .ok_or(ScoredTerminationProfileError::MissingUnimprovedTime)?;
                SignificantImprovementConfig::new(window, minimum)
                    .map_err(ScoredTerminationProfileError::InvalidThreshold)
            })
            .transpose()?;
        Ok(Self {
            profile,
            significant,
            feasible,
        })
    }
    pub fn profile(&self) -> &TerminationProfile {
        &self.profile
    }
    pub fn significant_improvement(&self) -> Option<&SignificantImprovementConfig<S>> {
        self.significant.as_ref()
    }
    pub fn feasibility(&self) -> Option<bool> {
        self.feasible
    }
    fn overlay<'a>(&'a self, mut limits: SolverLimits<'a, S>) -> SolverLimits<'a, S> {
        if self.profile.unimproved_spent_limit().is_some() {
            limits.significant_improvement = self.significant.as_ref();
            if self.significant.is_some() {
                limits.max_unimproved_time = None;
            }
        }
        if let Some(feasible) = self.feasible {
            limits.stop_when_feasible = feasible;
        }
        limits
    }
    /// Bind shared controls for one call. Returned owned results may outlive the
    /// profile; borrowed temporary expressions remain scoped to the callback.
    pub fn with_limits<R>(
        &self,
        limits: SolverLimits<'_, S>,
        run: impl FnOnce(SolverLimits<'_, S>) -> R,
    ) -> R {
        self.profile
            .with_limits(limits, |limits| run(self.overlay(limits)))
    }
    /// Bind shared score controls and the underlying profile's local tuning.
    pub fn with_sequence<'a, R>(
        &self,
        sequence: impl Into<SolverSequence<'a, S>>,
        run: impl FnOnce(SolverSequence<'_, S>) -> R,
    ) -> R
    where
        S: 'a,
    {
        self.profile.with_sequence(sequence, |sequence| {
            run(SolverSequence {
                limits: self.overlay(sequence.limits),
                phases: sequence.phases,
            })
        })
    }
    pub fn with_mixed_sequence<'a, R>(
        &self,
        sequence: impl Into<MixedSolverSequence<'a, S>>,
        run: impl FnOnce(MixedSolverSequence<'_, S>) -> R,
    ) -> R
    where
        S: 'a,
    {
        self.profile.with_mixed_sequence(sequence, |sequence| {
            run(MixedSolverSequence {
                limits: self.overlay(sequence.limits),
                phases: sequence.phases,
            })
        })
    }
}
