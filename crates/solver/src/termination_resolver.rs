//! Resolve platform defaults and per-request stopping policies before solving.
use crate::{
    DiminishedReturnsConfig, MixedSolverPhase, MixedSolverSequence, SolverLimits, SolverPhase,
    SolverSequence, TerminationProfile, TerminationProfileError, default_diminished_returns_config,
};
use std::time::Duration;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminationResolver {
    platform: TerminationProfile,
}
/// Effective shared limits and the chosen local-search slowdown policy. Native
/// hard limits are OR-composed: the first reached budget stops the solve.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTermination {
    profile: TerminationProfile,
    diminished: bool,
}
impl TerminationResolver {
    /// Structured native durations replace external configuration strings.
    /// Platform slowdown tuning is deliberately absent; core defaults apply
    /// unless the request supplies tuning. Counts must be nonnegative.
    pub fn new(
        spent: Duration,
        unimproved: Option<Duration>,
        steps: Option<i64>,
        moves: Option<i64>,
    ) -> Result<Self, TerminationProfileError> {
        Ok(Self {
            platform: TerminationProfile::new(Some(spent), unimproved, steps, moves, None, None)?,
        })
    }
    pub fn platform(&self) -> &TerminationProfile {
        &self.platform
    }
    /// Request stagnation suppresses platform counts. Any request count suppresses
    /// platform stagnation, while the other count may still inherit. Missing
    /// request limits inherit platform limits; explicit zero remains present.
    /// Stagnation/count limits disable diminished returns, ignoring request tuning.
    /// Otherwise slowdown is enabled, with missing tuning left to core defaults.
    pub fn resolve(&self, request: Option<&TerminationProfile>) -> ResolvedTermination {
        let empty = TerminationProfile::default();
        let r = request.unwrap_or(&empty);
        let p = &self.platform;
        let (unimproved, steps, moves) = if r.unimproved_spent_limit().is_some() {
            (r.unimproved_spent_limit(), None, None)
        } else if r.step_count_limit().is_some() || r.move_count_limit().is_some() {
            (
                None,
                r.step_count_limit().or(p.step_count_limit()),
                r.move_count_limit().or(p.move_count_limit()),
            )
        } else {
            (
                p.unimproved_spent_limit(),
                p.step_count_limit(),
                p.move_count_limit(),
            )
        };
        let diminished = unimproved.is_none() && steps.is_none() && moves.is_none();
        // All integer values originate in validated signed profile inputs.
        let profile = TerminationProfile::new(
            r.spent_limit().or(p.spent_limit()),
            unimproved,
            steps.map(|n| n as i64),
            moves.map(|n| n as i64),
            if diminished { r.sliding_window() } else { None },
            if diminished {
                r.minimum_improvement_ratio().map(|(n, d)| (n as i64, d))
            } else {
                None
            },
        )
        .expect("resolved validated profiles cannot conflict");
        ResolvedTermination {
            profile,
            diminished,
        }
    }
}
impl ResolvedTermination {
    /// Effective limits and explicit tuning. Missing tuning stays None here even
    /// when slowdown is enabled; diminished_returns() materializes core defaults.
    pub fn profile(&self) -> &TerminationProfile {
        &self.profile
    }
    pub fn uses_diminished_returns(&self) -> bool {
        self.diminished
    }
    pub fn diminished_returns(&self) -> Option<DiminishedReturnsConfig> {
        self.diminished.then(|| {
            self.profile
                .diminished_returns()
                .unwrap_or_else(default_diminished_returns_config)
        })
    }
    fn shared<S>(&self, limits: &mut SolverLimits<'_, S>) {
        // Resolution is authoritative for these shared fields, including clearing
        // conflicting pre-existing values. Other native safeguards remain intact.
        limits.max_time = self.profile.spent_limit();
        limits.max_unimproved_time = self.profile.unimproved_spent_limit();
        limits.max_move_evaluations = self.profile.move_count_limit();
    }
    /// Bind effective shared limits and replace each local phase's slowdown
    /// policy, including clearing old tuning when disabled. Explicit phase-local
    /// budgets, other phase kinds and existing shared expressions are preserved.
    pub fn with_sequence<'a, S: 'a, R>(
        &self,
        sequence: impl Into<SolverSequence<'a, S>>,
        run: impl FnOnce(SolverSequence<'_, S>) -> R,
    ) -> R {
        let mut sequence = sequence.into();
        self.shared(&mut sequence.limits);
        let policy = self.diminished_returns();
        for phase in &mut sequence.phases {
            if let SolverPhase::LocalSearch(local) = phase {
                local.diminished_returns = policy;
            }
        }
        self.profile.with_limits(sequence.limits, |limits| {
            run(SolverSequence {
                limits,
                phases: sequence.phases,
            })
        })
    }
    /// Mixed-model binding with the same policy and shared-limit precedence.
    pub fn with_mixed_sequence<'a, S: 'a, R>(
        &self,
        sequence: impl Into<MixedSolverSequence<'a, S>>,
        run: impl FnOnce(MixedSolverSequence<'_, S>) -> R,
    ) -> R {
        let mut sequence = sequence.into();
        self.shared(&mut sequence.limits);
        let policy = self.diminished_returns();
        for phase in &mut sequence.phases {
            if let MixedSolverPhase::LocalSearch(local) = phase {
                local.search.diminished_returns = policy;
            }
        }
        self.profile.with_limits(sequence.limits, |limits| {
            run(MixedSolverSequence {
                limits,
                phases: sequence.phases,
            })
        })
    }
}
