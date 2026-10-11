//! Typed callbacks and move-provider bindings for complete solver plans.
use crate::{CustomPhaseContext, CustomPhaseError, MoveProvider, SolverError, SolverEvent};
use std::{collections::BTreeMap, error::Error, fmt};

/// Mutable providers keyed by zero-based index in the full solver phase plan.
/// Construction and local-search phases can be bound. Other phases retain their
/// built-in selection. A construction provider replaces placement/component order
/// with one candidate pool per step. Provider state survives repeated phases/solves;
/// step indices restart at zero in each phase. Engine lifecycle events are automatic;
/// providers choose how to reset, or use PhaseResetMoveProvider for an explicit policy.
pub struct PhaseMoveProviders<'a, P, A, M> {
    /// Maximum raw proposals per bound construction phase, including rejected
    /// moves. Defaults to 100,000. Does not affect local-search providers.
    pub max_construction_scans: u64,
    bindings: BTreeMap<usize, usize>,
    providers: Vec<Option<&'a mut dyn MoveProvider<P, A, M>>>,
}
impl<P, A, M> Default for PhaseMoveProviders<'_, P, A, M> {
    fn default() -> Self {
        Self {
            bindings: BTreeMap::new(),
            providers: Vec::new(),
            max_construction_scans: 100_000,
        }
    }
}
impl<'a, P, A, M> PhaseMoveProviders<'a, P, A, M> {
    /// Bind or replace one phase. Returns its old provider only when that was the
    /// last phase bound to it; a shared provider stays borrowed by its other phases.
    /// Invalid phase indices are rejected before solve callbacks.
    pub fn bind(
        &mut self,
        phase: usize,
        provider: &'a mut dyn MoveProvider<P, A, M>,
    ) -> Option<&'a mut dyn MoveProvider<P, A, M>> {
        let previous = self.remove(phase);
        let slot = self.store(provider);
        self.bindings.insert(phase, slot);
        previous
    }
    /// Borrow one provider once and reuse it in every selected phase. Duplicate
    /// indices are deduplicated. Empty lists and existing bindings are rejected
    /// atomically; use remove() explicitly to replace a shared binding.
    pub fn bind_shared(
        &mut self,
        phases: impl IntoIterator<Item = usize>,
        provider: &'a mut dyn MoveProvider<P, A, M>,
    ) -> Result<(), MoveProviderBindingError> {
        let phases: std::collections::BTreeSet<_> = phases.into_iter().collect();
        if phases.is_empty() {
            return Err(MoveProviderBindingError::EmptyPhases);
        }
        if let Some(&phase) = phases.iter().find(|&&phase| self.contains(phase)) {
            return Err(MoveProviderBindingError::AlreadyBound { phase });
        }
        let slot = self.store(provider);
        for phase in phases {
            self.bindings.insert(phase, slot);
        }
        Ok(())
    }
    fn store(&mut self, provider: &'a mut dyn MoveProvider<P, A, M>) -> usize {
        if let Some(slot) = self.providers.iter().position(Option::is_none) {
            self.providers[slot] = Some(provider);
            slot
        } else {
            let slot = self.providers.len();
            self.providers.push(Some(provider));
            slot
        }
    }
    /// Remove a phase binding. Returns the provider only after its last binding
    /// is removed, because a shared provider cannot be mutably borrowed twice.
    pub fn remove(&mut self, phase: usize) -> Option<&'a mut dyn MoveProvider<P, A, M>> {
        let slot = self.bindings.remove(&phase)?;
        if self.bindings.values().any(|&other| other == slot) {
            None
        } else {
            self.providers[slot].take()
        }
    }
    pub fn contains(&self, phase: usize) -> bool {
        self.bindings.contains_key(&phase)
    }
    pub(crate) fn get(&mut self, phase: usize) -> Option<&mut (dyn MoveProvider<P, A, M> + 'a)> {
        let slot = *self.bindings.get(&phase)?;
        self.providers[slot].as_deref_mut()
    }
    fn event(&mut self, event: crate::MoveProviderEvent) {
        for provider in self.providers.iter_mut().flatten() {
            provider.event(event);
        }
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
/// Invalid shared provider registration. No binding changes on error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveProviderBindingError {
    EmptyPhases,
    AlreadyBound { phase: usize },
}
impl fmt::Display for MoveProviderBindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "move provider binding: {self:?}")
    }
}
impl Error for MoveProviderBindingError {}

/// A real solver invocation owns one lifecycle for every distinct binding.
/// Start lazily at its first scoring kernel, after all shared plan validation.
/// Diagnostics reuse the same kernel closure and cannot open a second lifecycle.
pub(crate) struct ProviderSolve<'r, 'p, P, A, M> {
    providers: Option<&'r mut PhaseMoveProviders<'p, P, A, M>>,
    started: bool,
}
impl<'r, 'p, P, A, M> ProviderSolve<'r, 'p, P, A, M> {
    pub(crate) fn new(providers: Option<&'r mut PhaseMoveProviders<'p, P, A, M>>) -> Self {
        Self {
            providers,
            started: false,
        }
    }
    pub(crate) fn start(&mut self) {
        if !self.started {
            self.started = true;
            if let Some(providers) = &mut self.providers {
                providers.event(crate::MoveProviderEvent::SolveStarted);
            }
        }
    }
}
impl<'r, 'p, P, A, M> std::ops::Deref for ProviderSolve<'r, 'p, P, A, M> {
    type Target = Option<&'r mut PhaseMoveProviders<'p, P, A, M>>;
    fn deref(&self) -> &Self::Target {
        &self.providers
    }
}
impl<P, A, M> std::ops::DerefMut for ProviderSolve<'_, '_, P, A, M> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.providers
    }
}
impl<P, A, M> Drop for ProviderSolve<'_, '_, P, A, M> {
    fn drop(&mut self) {
        if self.started
            && !std::thread::panicking()
            && let Some(providers) = &mut self.providers
        {
            providers.event(crate::MoveProviderEvent::SolveEnded);
        }
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

/// Failure of a started solve, borrowing the last successfully retained handoff.
/// Before the first scored handoff, this is the original initial assignment with
/// no score. A failed phase's candidates and any output rejected by score checks
/// are excluded. Work and reports include successfully committed phases only;
/// no counters are invented for a failed kernel.
///
/// `phase` identifies the most recently entered phase (or the synthetic Initial
/// scoring kernel for an immovable model). A handoff error may name a following
/// phase in `error` that was never entered. The error is borrowed through Debug
/// for diagnostics; the unchanged typed error is returned by the solve function.
#[derive(Debug)]
pub struct SolverFailure<'a, A, S> {
    pub assignment: &'a A,
    pub score: Option<&'a S>,
    pub complete: bool,
    pub work: crate::SolverWork,
    pub phases: &'a [crate::PhaseReport<S>],
    pub phase: Option<crate::SolverEventProducer>,
    pub error: &'a dyn fmt::Debug,
}
/// Called once for a normal error after SolveStarted, before SolveEnded and
/// release of still-owned retained sessions/providers. A fresh phase kernel may
/// already have dropped its local resources while returning its error. This
/// callback never exposes the failed kernel's working state.
///
/// Preflight errors and unwinding panics do not notify. Like other synchronous
/// observers, callback panics propagate instead of returning the original error;
/// there is no recursive failure notification or further lifecycle notification
/// during unwind. Returning normally preserves the original error. No snapshots
/// are cloned and no scoring is performed for this callback.
pub type SolverFailureCallback<'a, A, S> = dyn FnMut(SolverFailure<'_, A, S>) + 'a;

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
    /// Optional common clock for solve and phase budgets; see [`crate::SolverClock`].
    pub clock: Option<&'a crate::SolverClock>,
    /// Composed phase-local limits; see [`crate::PhaseTerminationBinding`].
    pub phase_terminations: &'a [crate::PhaseTerminationBinding<'a, S>],
    pub events: Option<&'a mut SolverEventCallback<'a, A, S>>,
    /// Optional solve/phase/step notifications. Omit on a later solve to detach.
    pub lifecycle: Option<&'a mut crate::SolverLifecycleCallback<'a>>,
    /// Optional live metrics; cloned handles can observe concurrent invocations.
    pub metrics: Option<&'a crate::SolverMetrics<S>>,
    pub score_check: Option<&'a mut PhaseScoreCheck<'a, A, S>>,
    /// Observe a started solve's error and last retained handoff before solve teardown.
    pub failure: Option<&'a mut SolverFailureCallback<'a, A, S>>,
}
impl<P, A, M, S, E> Default for SolverExtensions<'_, P, A, M, S, E> {
    fn default() -> Self {
        Self {
            move_providers: PhaseMoveProviders::default(),
            command: None,
            events: None,
            clock: None,
            phase_terminations: &[],
            score_check: None,
            failure: None,
            lifecycle: None,
            metrics: None,
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
