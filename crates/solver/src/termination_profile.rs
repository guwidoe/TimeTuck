//! Validated stopping settings with fallback merging and scoped solver binding.
use crate::{
    DiminishedReturnsConfig, MixedSolverPhase, MixedSolverSequence, SearchLimit, SearchTermination,
    SolverLimits, SolverPhase, SolverSequence, default_diminished_returns_config,
};
use std::{borrow::Cow, error::Error, fmt, time::Duration};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TerminationProfileError {
    NegativeCount { field: &'static str, value: i64 },
    InvalidRatio,
    ZeroWindow,
    ConflictingLimits { count: &'static str },
    InapplicableToConstruction { setting: &'static str },
}
impl fmt::Display for TerminationProfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegativeCount { field, value } => {
                write!(f, "{field} cannot be negative: {value}")
            }
            Self::InvalidRatio => f.write_str(
                "minimumImprovementRatio numerator and denominator must be strictly positive",
            ),
            Self::ZeroWindow => f.write_str("slidingWindowDuration must be strictly positive"),
            Self::InapplicableToConstruction { setting } => {
                write!(
                    f,
                    "construction termination setting {setting} is not applicable"
                )
            }
            Self::ConflictingLimits { count } => {
                write!(f, "unimprovedSpentLimit cannot be combined with {count}")
            }
        }
    }
}
impl Error for TerminationProfileError {}
/// Service-style configuration policy: stagnation time cannot be combined with
/// step/move count settings. Lower-level native solver limits remain unrestricted.
/// Ratios are exact signed-numerator/unsigned-denominator inputs, not floats.
/// Missing fields inherit independently; merges are revalidated atomically.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TerminationProfile {
    spent: Option<Duration>,
    unimproved: Option<Duration>,
    steps: Option<u64>,
    unimproved_steps: Option<u64>,
    moves: Option<u64>,
    window: Option<Duration>,
    ratio: Option<(u64, u64)>,
}
impl TerminationProfile {
    pub fn new(
        spent: Option<Duration>,
        unimproved: Option<Duration>,
        steps: Option<i64>,
        moves: Option<i64>,
        window: Option<Duration>,
        ratio: Option<(i64, u64)>,
    ) -> Result<Self, TerminationProfileError> {
        fn count(
            value: Option<i64>,
            field: &'static str,
        ) -> Result<Option<u64>, TerminationProfileError> {
            value
                .map(|n| {
                    u64::try_from(n)
                        .map_err(|_| TerminationProfileError::NegativeCount { field, value: n })
                })
                .transpose()
        }
        let steps = count(steps, "stepCountLimit")?;
        let moves = count(moves, "moveCountLimit")?;
        let ratio = ratio
            .map(|(n, d)| {
                if n <= 0 || d == 0 {
                    Err(TerminationProfileError::InvalidRatio)
                } else {
                    Ok((n as u64, d))
                }
            })
            .transpose()?;
        let result = Self {
            spent,
            unimproved,
            steps,
            unimproved_steps: None,
            moves,
            window,
            ratio,
        };
        result.validate()?;
        Ok(result)
    }
    fn validate(&self) -> Result<(), TerminationProfileError> {
        if self.window.is_some_and(|w| w.is_zero()) {
            return Err(TerminationProfileError::ZeroWindow);
        }
        if self.unimproved.is_some() {
            if self.steps.is_some() {
                return Err(TerminationProfileError::ConflictingLimits {
                    count: "stepCountLimit",
                });
            }
            if self.unimproved_steps.is_some() {
                return Err(TerminationProfileError::ConflictingLimits {
                    count: "unimprovedStepCountLimit",
                });
            }
            if self.moves.is_some() {
                return Err(TerminationProfileError::ConflictingLimits {
                    count: "moveCountLimit",
                });
            }
        }
        Ok(())
    }
    pub fn spent_limit(&self) -> Option<Duration> {
        self.spent
    }
    pub fn unimproved_spent_limit(&self) -> Option<Duration> {
        self.unimproved
    }
    pub fn step_count_limit(&self) -> Option<u64> {
        self.steps
    }
    /// Configure completed search steps without a strict retained-best improvement.
    /// None clears the setting; zero is an active limit. This service-style count
    /// policy cannot coexist with stagnation time; native solver configs can.
    pub fn with_unimproved_step_limit(
        mut self,
        limit: Option<i64>,
    ) -> Result<Self, TerminationProfileError> {
        self.unimproved_steps = limit
            .map(|value| {
                u64::try_from(value).map_err(|_| TerminationProfileError::NegativeCount {
                    field: "unimprovedStepCountLimit",
                    value,
                })
            })
            .transpose()?;
        self.validate()?;
        Ok(self)
    }
    pub fn unimproved_step_count_limit(&self) -> Option<u64> {
        self.unimproved_steps
    }
    pub fn move_count_limit(&self) -> Option<u64> {
        self.moves
    }
    pub fn sliding_window(&self) -> Option<Duration> {
        self.window
    }
    pub fn minimum_improvement_ratio(&self) -> Option<(u64, u64)> {
        self.ratio
    }
    /// None borrows this profile without cloning. A supplied fallback fills only
    /// absent fields; conflicts in the merged policy leave both inputs untouched.
    pub fn with_fallback<'a>(
        &'a self,
        fallback: Option<&Self>,
    ) -> Result<Cow<'a, Self>, TerminationProfileError> {
        let Some(b) = fallback else {
            return Ok(Cow::Borrowed(self));
        };
        let result = Self {
            spent: self.spent.or(b.spent),
            unimproved: self.unimproved.or(b.unimproved),
            steps: self.steps.or(b.steps),
            unimproved_steps: self.unimproved_steps.or(b.unimproved_steps),
            moves: self.moves.or(b.moves),
            window: self.window.or(b.window),
            ratio: self.ratio.or(b.ratio),
        };
        result.validate()?;
        Ok(Cow::Owned(result))
    }
    /// Tuning is inactive when both fields are missing. Otherwise missing tuning
    /// uses the native Community defaults (30 seconds and 1/10000).
    pub fn diminished_returns(&self) -> Option<DiminishedReturnsConfig> {
        if self.window.is_none() && self.ratio.is_none() {
            return None;
        }
        let defaults = default_diminished_returns_config();
        let (n, d) = self.ratio.unwrap_or(defaults.ratio());
        Some(
            DiminishedReturnsConfig::new(self.window.unwrap_or(defaults.window()), n, d)
                .expect("validated profile"),
        )
    }
    /// Bind phase-local construction budgets, preserving unspecified settings,
    /// cancellation, scoring assertions and candidate interruption policy.
    /// Stagnation and diminished returns require an improvement phase and are
    /// rejected before any solve starts. Explicit zero limits remain active.
    pub fn bind_construction<'a>(
        &self,
        mut config: crate::ConstructionConfig<'a>,
    ) -> Result<crate::ConstructionConfig<'a>, TerminationProfileError> {
        if self.unimproved.is_some() {
            return Err(TerminationProfileError::InapplicableToConstruction {
                setting: "UnimprovedTimeMillisSpent",
            });
        }
        if self.unimproved_steps.is_some() {
            return Err(TerminationProfileError::InapplicableToConstruction {
                setting: "UnimprovedStepCount",
            });
        }
        if self.diminished_returns().is_some() {
            return Err(TerminationProfileError::InapplicableToConstruction {
                setting: "DiminishedReturns",
            });
        }
        if let Some(limit) = self.spent {
            config.max_time = Some(limit);
        }
        if let Some(limit) = self.steps {
            config.max_steps = Some(limit);
        }
        if let Some(limit) = self.moves {
            config.max_move_evaluations = Some(limit);
        }
        Ok(config)
    }
    /// Bind one local-search phase. Specified settings replace their respective
    /// phase controls; missing settings and score-evaluation safety caps survive.
    /// Unlike with_limits, this step counter resets at every phase invocation.
    pub fn bind_local_search<'a, S>(
        &self,
        mut config: crate::LocalSearchConfig<'a, S>,
    ) -> crate::LocalSearchConfig<'a, S> {
        if let Some(limit) = self.spent {
            config.max_time = Some(limit);
        }
        if let Some(limit) = self.unimproved {
            config.max_unimproved_time = Some(limit);
        }
        if let Some(limit) = self.steps {
            config.max_steps = limit;
        }
        if let Some(limit) = self.unimproved_steps {
            config.max_unimproved_steps = Some(limit);
        }
        if let Some(limit) = self.moves {
            config.max_move_evaluations = Some(limit);
        }
        if let Some(policy) = self.diminished_returns() {
            config.diminished_returns = Some(policy);
        }
        config
    }
    /// Apply specified shared budgets and OR a solve-wide search-step cap with
    /// any existing expression. The callback scopes the borrowed expression;
    /// it may return owned solve results but cannot retain the temporary limits.
    /// Steps exclude construction; move evaluations use the native candidate
    /// counter. Existing unspecified controls and evaluation safety caps survive.
    pub fn with_limits<S, R>(
        &self,
        mut limits: SolverLimits<'_, S>,
        run: impl FnOnce(SolverLimits<'_, S>) -> R,
    ) -> R {
        if let Some(time) = self.spent {
            limits.max_time = Some(time);
        }
        if let Some(time) = self.unimproved {
            limits.max_unimproved_time = Some(time);
        }
        if let Some(moves) = self.moves {
            limits.max_move_evaluations = Some(moves);
        }
        if let Some(steps) = self.unimproved_steps {
            limits.max_unimproved_steps = Some(steps);
        }
        let Some(steps) = self.steps else {
            return run(limits);
        };
        let inherited = limits
            .termination
            .copied()
            .unwrap_or(SearchTermination::Any(&[]));
        let children = [
            inherited,
            SearchTermination::Condition(SearchLimit::Steps(steps)),
        ];
        let combined = SearchTermination::Any(&children);
        run(SolverLimits {
            termination: Some(&combined),
            ..limits
        })
    }
    /// Bind ordinary basic/list solver configurations or sequences for one call.
    /// Supplied diminished tuning replaces local-phase tuning; absent tuning
    /// preserves it. Construction, custom, repair and exact settings are retained.
    pub fn with_sequence<'a, S: 'a, R>(
        &self,
        sequence: impl Into<SolverSequence<'a, S>>,
        run: impl FnOnce(SolverSequence<'_, S>) -> R,
    ) -> R {
        let mut sequence = sequence.into();
        if let Some(policy) = self.diminished_returns() {
            for phase in &mut sequence.phases {
                if let SolverPhase::LocalSearch(local) = phase {
                    local.diminished_returns = Some(policy);
                }
            }
        }
        self.with_limits(sequence.limits, |limits| {
            run(SolverSequence {
                limits,
                phases: sequence.phases,
            })
        })
    }
    /// Mixed-model counterpart of with_sequence, preserving component order.
    pub fn with_mixed_sequence<'a, S: 'a, R>(
        &self,
        sequence: impl Into<MixedSolverSequence<'a, S>>,
        run: impl FnOnce(MixedSolverSequence<'_, S>) -> R,
    ) -> R {
        let mut sequence = sequence.into();
        if let Some(policy) = self.diminished_returns() {
            for phase in &mut sequence.phases {
                if let MixedSolverPhase::LocalSearch(local) = phase {
                    local.search.diminished_returns = Some(policy);
                }
            }
        }
        self.with_limits(sequence.limits, |limits| {
            run(MixedSolverSequence {
                limits,
                phases: sequence.phases,
            })
        })
    }
}
