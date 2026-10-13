//! Lifecycle observers attached to actual native child acceptance states.
use crate::{
    AcceptanceError, Score, SolverLifecycleCallback, SolverLifecycleEvent,
    TerminationProgressScope, ValueAcceptor,
};
pub struct ObservedAcceptorChild<'a, S, M, K> {
    state: ValueAcceptor<S, M, K>,
    lifecycle: Box<SolverLifecycleCallback<'a>>,
}
impl<'a, S, M, K> ObservedAcceptorChild<'a, S, M, K> {
    pub fn new(
        state: ValueAcceptor<S, M, K>,
        lifecycle: impl FnMut(SolverLifecycleEvent) + 'a,
    ) -> Self {
        Self {
            state,
            lifecycle: Box::new(lifecycle),
        }
    }
}
/// Ordered conjunction of live child acceptors, each with its own observer.
/// Candidate acceptance short-circuits. Lifecycle broadcasts and selected-step
/// state updates visit every child in registration order. Lifecycle notifications
/// describe the caller's solve/phase; they do not reset policy memory. Construct
/// fresh child states at the desired policy boundary, as native search does.
/// Notifications are explicit, compatible with SolverLifecycleEvent callbacks;
/// step_started/step_ended update acceptance state and do not emit duplicate
/// lifecycle events. Inline observer panics stop later callbacks and propagate;
/// external observer effects are not rolled back. Ordinary native acceptors need
/// no observers and keep their existing allocation/execution paths.
pub struct ObservedCompositeAcceptor<'a, S, M, K> {
    children: Vec<ObservedAcceptorChild<'a, S, M, K>>,
}
impl<'a, S: Score, M: Eq + Clone, K: Eq + Clone> ObservedCompositeAcceptor<'a, S, M, K> {
    pub fn new(children: Vec<ObservedAcceptorChild<'a, S, M, K>>) -> Self {
        Self { children }
    }
    pub fn lifecycle(&mut self, event: SolverLifecycleEvent) {
        for child in &mut self.children {
            (child.lifecycle)(event);
        }
    }
    pub fn child_mut(&mut self, index: usize) -> Option<&mut ValueAcceptor<S, M, K>> {
        self.children.get_mut(index).map(|child| &mut child.state)
    }
    pub fn accepts(
        &mut self,
        current: S,
        candidate: S,
        variables: &[usize],
        movement: &M,
        values: &[K],
    ) -> bool {
        self.children.iter_mut().all(|child| {
            child.state.accepts(
                current.clone(),
                candidate.clone(),
                variables,
                movement,
                values,
            )
        })
    }
    pub fn step_started(&mut self, elapsed: std::time::Duration) {
        for child in &mut self.children {
            child.state.step_started(elapsed);
        }
    }
    pub fn termination_step_started(
        &mut self,
        scope: TerminationProgressScope,
        progress: Option<f64>,
    ) -> Result<(), AcceptanceError> {
        if progress.is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p)) {
            return Err(AcceptanceError::InvalidGradient);
        }
        for child in &mut self.children {
            child.state.termination_step_started(scope, progress)?;
        }
        Ok(())
    }
    pub fn step_ended(&mut self, score: S, variables: &[usize], movement: &M, values: &[K]) {
        for child in &mut self.children {
            child
                .state
                .step_ended(score.clone(), variables, movement, values);
        }
    }
}
