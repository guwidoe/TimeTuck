//! Solve-wide budgets and immutable per-invocation factory overrides.
use super::{
    AssembledFactoryRun, AssembledSolverFactory, FactoryAssemblyError, FactoryExecutionFailure,
};
use crate::{ContextScoreResource, SolverPhase};
use std::time::{Duration, Instant};

/// Logical evaluations are shared across all entered phases. Time starts before
/// scoring-resource construction, includes mode rebuilds and lifecycle hooks,
/// and is cooperatively enforced by clamping each native phase's remaining time.
/// A running callback cannot be preempted. Initial resource loading is permitted
/// even at zero budget; a zero budget prevents entering any optimization phase.
/// Application bindings must execute the provided bounded phase and return its
/// actual work. Arbitrary work within a binding cannot be preempted; inflated
/// evaluation use is detected before a successful phase is published.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FactoryRunLimits {
    pub max_evaluations: Option<u64>,
    pub max_time: Option<Duration>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactoryRunTermination {
    PhasesCompleted,
    EvaluationLimit,
    TimeLimit,
}
/// The builder reads an override once, before creating its runtime resource.
/// Defaults and cached scoring preparation remain owned by the factory.
pub trait FactoryRunOverride {
    fn factory_run_limits(&self) -> FactoryRunLimits;
}
pub struct AssembledSolverBuilder<'f, 'a, M, P, I, D: ContextScoreResource> {
    pub(super) factory: &'f AssembledSolverFactory<'a, M, P, I, D>,
    pub(super) limits: FactoryRunLimits,
}
impl<'f, 'a, M: 'a, P: 'a, I: 'a, D: ContextScoreResource + 'a>
    AssembledSolverBuilder<'f, 'a, M, P, I, D>
where
    D::Error: 'static,
{
    pub fn with_factory_override(mut self, settings: &impl FactoryRunOverride) -> Self {
        self.limits = settings.factory_run_limits();
        self
    }
    pub fn with_factory_run_limits(mut self, limits: FactoryRunLimits) -> Self {
        self.limits = limits;
        self
    }
    pub fn factory_run_limits(&self) -> FactoryRunLimits {
        self.limits
    }
    pub fn build_assembled_run(
        self,
        input: I,
    ) -> Result<AssembledFactoryRun<'f, 'a, M, P, I, D>, FactoryExecutionFailure<D::Error>> {
        self.factory
            .start_assembled_run_with_limits(input, self.limits)
    }
    pub fn build_observed_assembled_run(
        self,
        input: I,
        callbacks: &mut super::phase_views::RunCallbacks<'_, 'a, M, P, I, D>,
    ) -> Result<AssembledFactoryRun<'f, 'a, M, P, I, D>, FactoryExecutionFailure<D::Error>> {
        self.factory
            .start_observed_assembled_run_with_limits(input, self.limits, callbacks)
    }
}

pub(super) fn validate_reachable_phases<S, E>(
    phases: &[SolverPhase<'_, S>],
    limits: FactoryRunLimits,
) -> Result<(), FactoryAssemblyError<E>> {
    if limits.max_time.is_some() || limits.max_evaluations.is_some() {
        return Ok(());
    }
    for (index, phase) in phases
        .iter()
        .enumerate()
        .take(phases.len().saturating_sub(1))
    {
        if let SolverPhase::LocalSearch(c) = phase
            && c.max_steps == u64::MAX
            && c.max_accepted_moves == u64::MAX
            && c.max_evaluations == u64::MAX
            && c.max_move_evaluations.is_none()
            && c.max_time.is_none()
            && c.max_unimproved_time.is_none()
            && c.max_unimproved_steps.is_none()
            && c.diminished_returns.is_none()
            && c.significant_improvement.is_none()
            && c.step_termination.is_none()
            && c.best_score_limit.is_none()
            && !c.stop_when_feasible
        {
            return Err(FactoryAssemblyError::UnreachablePhase {
                phase: index + 1,
                predecessor: index,
            });
        }
    }
    Ok(())
}

pub(super) fn clamp_phase<'a, S>(
    mut phase: SolverPhase<'a, S>,
    evaluations: Option<u64>,
    time: Option<Duration>,
) -> SolverPhase<'a, S> {
    macro_rules! clamp {
        ($c:expr) => {{
            if let Some(remaining) = evaluations {
                $c.max_evaluations = $c.max_evaluations.min(remaining);
            }
            if let Some(remaining) = time {
                $c.max_time = Some($c.max_time.map_or(remaining, |local| local.min(remaining)));
            }
        }};
    }
    match &mut phase {
        SolverPhase::Construction(c) => clamp!(c),
        SolverPhase::LocalSearch(c) => clamp!(c),
        SolverPhase::RuinRecreate(c) => clamp!(c),
        SolverPhase::Exact(c) => clamp!(c),
        SolverPhase::Custom(c) => clamp!(c),
    }
    phase
}

pub(super) struct RunBudget {
    pub limits: FactoryRunLimits,
    started: Option<Instant>,
    pub termination: Option<FactoryRunTermination>,
}
impl RunBudget {
    pub fn new(limits: FactoryRunLimits) -> Self {
        Self {
            limits,
            started: limits.max_time.map(|_| Instant::now()),
            termination: None,
        }
    }
    pub fn remaining_evaluations(&self, used: u128) -> Option<u64> {
        self.limits
            .max_evaluations
            .map(|max| max.saturating_sub(u64::try_from(used).unwrap_or(u64::MAX)))
    }
    pub fn remaining_time(&self) -> Option<Duration> {
        self.limits.max_time.map(|max| {
            max.saturating_sub(self.started.expect("timed budget has an origin").elapsed())
        })
    }
    pub fn exhausted(&mut self, used: u128) -> bool {
        if self.termination.is_some() {
            return true;
        }
        self.termination = if self.remaining_evaluations(used) == Some(0) {
            Some(FactoryRunTermination::EvaluationLimit)
        } else if self.remaining_time() == Some(Duration::ZERO) {
            Some(FactoryRunTermination::TimeLimit)
        } else {
            None
        };
        self.termination.is_some()
    }
}
