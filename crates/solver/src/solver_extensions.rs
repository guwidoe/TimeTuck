//! Typed callbacks and move-provider bindings for complete solver plans.
use crate::{CustomPhaseContext, CustomPhaseError, MoveProvider, SolverError, SolverEvent};
use std::collections::BTreeMap;

/// Mutable providers keyed by zero-based index in the full solver phase plan.
/// Construction and local-search phases can be bound. Other phases retain their
/// built-in selection. A construction provider replaces placement/component order
/// with one candidate pool per step. Provider state survives repeated phases/solves;
/// step indices restart at zero in each phase. Callers own provider resets.
pub struct PhaseMoveProviders<'a, P, A, M> {
    /// Maximum raw proposals per bound construction phase, including rejected
    /// moves. Defaults to 100,000. Does not affect local-search providers.
    pub max_construction_scans: u64,
    bindings: BTreeMap<usize, &'a mut dyn MoveProvider<P, A, M>>,
}
impl<P, A, M> Default for PhaseMoveProviders<'_, P, A, M> {
    fn default() -> Self {
        Self {
            bindings: BTreeMap::new(),
            max_construction_scans: 100_000,
        }
    }
}
impl<'a, P, A, M> PhaseMoveProviders<'a, P, A, M> {
    /// Bind or replace a provider. Invalid phase indices are rejected at solve
    /// entry, before graph factories, scoring, commands or events are invoked.
    pub fn bind(
        &mut self,
        phase: usize,
        provider: &'a mut dyn MoveProvider<P, A, M>,
    ) -> Option<&'a mut dyn MoveProvider<P, A, M>> {
        self.bindings.insert(phase, provider)
    }
    pub fn remove(&mut self, phase: usize) -> Option<&'a mut dyn MoveProvider<P, A, M>> {
        self.bindings.remove(&phase)
    }
    pub fn contains(&self, phase: usize) -> bool {
        self.bindings.contains_key(&phase)
    }
    pub(crate) fn get(&mut self, phase: usize) -> Option<&mut (dyn MoveProvider<P, A, M> + 'a)> {
        self.bindings.get_mut(&phase).map(|p| &mut **p)
    }
    pub(crate) fn validate<E>(
        &self,
        phases: impl Iterator<Item = bool>,
    ) -> Result<(), SolverError<E>> {
        let local: Vec<_> = phases.collect();
        for &phase in self.bindings.keys() {
            if local.get(phase) != Some(&true) {
                return Err(SolverError::InvalidMoveProviderPhase { phase });
            }
        }
        Ok(())
    }
}
/// Command callback for an entered custom phase.
pub type SolverCommandCallback<'a, A, M, S, E> = dyn FnMut(usize, &mut CustomPhaseContext<'_, A, M, S, E>) -> Result<(), CustomPhaseError<E>>
    + 'a;
/// Progress and initialization events use the same semantics as solve_with_events.
pub type SolverEventCallback<'a, A, S> = dyn FnMut(SolverEvent<'_, A, S>) + 'a;

/// Independently calculate a phase output's expected score. Return None to skip
/// that phase, or an error to reject the solve. `kind == Initial` with index zero
/// identifies initial-only scoring on an immovable model. Unscored phases and
/// phases that were never entered do not invoke this callback.
///
/// The assignment is the phase output (or retained entry if the phase produced
/// no new assignment). Include structural incompleteness in the expected score.
/// Checks run before phase handoff and initialization notification; best events
/// already emitted during search cannot be retracted. A mismatch or callback
/// error aborts the solve. Panics propagate. Callbacks must not mutate domain
/// facts through interior state. Diagnostic work is excluded from evaluation
/// counters, but its elapsed time belongs to the shared solve deadline.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1_i64,2])])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap_or(0)))?;
/// let mut check = |_, _, a: &Assignment<i64>| {
///     let missing = a.values().iter().filter(|v| v.is_none()).count() as i64;
///     Ok(Some(SimpleScore::new(a.values().iter().flatten().sum()).with_structural(-missing)))
/// };
/// let result = solve_with_extensions(&p, &n, SolverSequence::default(), SolverExtensions {
///     score_check: Some(&mut check), ..Default::default()
/// })?;
/// assert_eq!(result.score, Some(SimpleScore::new(2)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub type PhaseScoreCheck<'a, A, S> =
    dyn FnMut(usize, crate::SolverPhaseKind, &A) -> Result<Option<S>, String> + 'a;

/// Combine per-phase move providers, commands, progress events and score checks.
/// Pass to a `solve*_with_extensions` entry point, including calculators. Existing plan
/// validation, retained-best handoffs, shared budgets and cancellation still apply.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0_i32,1,2]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(i64::from(*v[0].unwrap())))?;
/// let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64|
///     vec![Move::Change {variable: 0, value: Some(2)}];
/// let mut extensions = SolverExtensions::default();
/// // The default plan constructs in phase 0, then searches in phase 1.
/// extensions.move_providers.bind(1, &mut provider);
/// let result = solve_with_extensions(&p, &n, SolverConfig::default(), extensions)?;
/// assert_eq!(result.score, Some(SimpleScore::new(2)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Bind phase zero to customize construction, then use the normal local phase:
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1_i32, 2])])?;
/// let n = ConstraintNetwork::<_, SimpleScore>::new(1);
/// let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64|
///     vec![Move::Change { variable: 0, value: Some(2) }];
/// let mut extensions = SolverExtensions::default();
/// extensions.move_providers.bind(0, &mut provider);
/// extensions.move_providers.max_construction_scans = 100;
/// let r = solve_with_extensions(&p, &n, SolverConfig::default(), extensions)?;
/// assert!(r.complete);
/// assert_eq!(r.assignment.values(), &[Some(2)]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SolverExtensions<'a, P, A, M, S, E> {
    pub move_providers: PhaseMoveProviders<'a, P, A, M>,
    pub command: Option<&'a mut SolverCommandCallback<'a, A, M, S, E>>,
    pub events: Option<&'a mut SolverEventCallback<'a, A, S>>,
    pub score_check: Option<&'a mut PhaseScoreCheck<'a, A, S>>,
}
impl<P, A, M, S, E> Default for SolverExtensions<'_, P, A, M, S, E> {
    fn default() -> Self {
        Self {
            move_providers: PhaseMoveProviders::default(),
            command: None,
            events: None,
            score_check: None,
        }
    }
}

/// Solver extensions for a model with both basic and list variables.
pub type MixedSolverExtensions<'a, B, L, S, E> = SolverExtensions<
    'a,
    crate::MixedProblem<B, L>,
    crate::MixedAssignment<B, L>,
    crate::MixedMove<B, L>,
    S,
    E,
>;
pub(crate) type MixedPhaseProviders<'a, B, L> = PhaseMoveProviders<
    'a,
    crate::MixedProblem<B, L>,
    crate::MixedAssignment<B, L>,
    crate::MixedMove<B, L>,
>;
