//! Bounded neighborhood assertions over live, shared working state.
use std::{cell::RefCell, rc::Rc};

/// A failed neighborhood observation. A limit bounds the number of candidates
/// pulled, not elapsed time or work performed inside the provider's next call.
#[derive(Debug, PartialEq, Eq)]
pub enum NeighborhoodAssertionError<M, E> {
    Provider(E),
    Missing { moves: Vec<M>, inspected: usize },
    Unexpected { movement: M, inspected: usize },
}
impl<M: std::fmt::Debug, E: std::fmt::Display> std::fmt::Display
    for NeighborhoodAssertionError<M, E>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider(e) => write!(f, "neighborhood provider failed: {e}"),
            Self::Missing { moves, inspected } => {
                write!(f, "missing moves {moves:?} after {inspected} candidates")
            }
            Self::Unexpected {
                movement,
                inspected,
            } => write!(f, "unexpected move {movement:?} at candidate {inspected}"),
        }
    }
}
impl<M: std::fmt::Debug, E: std::error::Error + 'static> std::error::Error
    for NeighborhoodAssertionError<M, E>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Provider(e) => Some(e),
            _ => None,
        }
    }
}
type Provider<'a, S, M, E> =
    dyn for<'s> Fn(&'s S) -> Result<Box<dyn Iterator<Item = M> + 's>, E> + 'a;

/// Test a neighborhood against its current working state. Each assertion creates
/// a fresh lazy iterator. Derived contexts share state and provider, but retain
/// independent limits. No candidate cache can survive a permanent state change.
///
/// Use update to execute existing model moves or score-director sessions. Temporary
/// execution uses their existing rollback guards; this tester does not implement
/// move semantics or score maintenance. Inspection and updates are synchronous.
/// Reentrant updates/observations while state is mutably borrowed panic, as with
/// RefCell; this single-threaded helper is not a concurrent solver coordinator.
///
/// ```
/// use timetuck::NeighborhoodTester;
/// let tester = NeighborhoodTester::new(vec![1,2], |rows| {
///     Ok::<_, std::convert::Infallible>(Box::new(rows.iter().copied()))
/// });
/// tester.produces_all_of([1,2]).unwrap();
/// let short = tester.within(1);
/// assert!(short.produces_all_of([2]).is_err());
/// tester.update(|rows| rows.push(3));
/// tester.produces_all_of([3]).unwrap();
/// ```
pub struct NeighborhoodTester<'a, S, M, E> {
    state: Rc<RefCell<S>>,
    provider: Rc<Provider<'a, S, M, E>>,
    limit: usize,
}
impl<'a, S, M, E> NeighborhoodTester<'a, S, M, E> {
    /// Native default limit: 10,000 candidates per assertion.
    pub fn new(
        state: S,
        provider: impl for<'s> Fn(&'s S) -> Result<Box<dyn Iterator<Item = M> + 's>, E> + 'a,
    ) -> Self {
        Self {
            state: Rc::new(RefCell::new(state)),
            provider: Rc::new(provider),
            limit: 10_000,
        }
    }
    /// Share the same working state without changing this context's limit.
    /// A zero limit observes no candidates.
    pub fn within(&self, limit: usize) -> Self {
        Self {
            state: self.state.clone(),
            provider: self.provider.clone(),
            limit,
        }
    }
    pub fn iteration_limit(&self) -> usize {
        self.limit
    }
    pub fn inspect<R>(&self, inspect: impl FnOnce(&S) -> R) -> R {
        inspect(&self.state.borrow())
    }
    pub fn update<R>(&self, update: impl FnOnce(&mut S) -> R) -> R {
        update(&mut self.state.borrow_mut())
    }
}
impl<S, M: PartialEq, E> NeighborhoodTester<'_, S, M, E> {
    /// Presence, not occurrence counts: repeated expected moves need only appear
    /// once. Stops as soon as all requested moves have appeared. Empty requests
    /// succeed without invoking the provider.
    pub fn produces_all_of(
        &self,
        expected: impl IntoIterator<Item = M>,
    ) -> Result<(), NeighborhoodAssertionError<M, E>> {
        let mut missing: Vec<_> = expected.into_iter().collect();
        if missing.is_empty() {
            return Ok(());
        }
        let state = self.state.borrow();
        let candidates = (self.provider)(&state).map_err(NeighborhoodAssertionError::Provider)?;
        let mut inspected = 0;
        for movement in candidates.take(self.limit) {
            inspected += 1;
            missing.retain(|m| *m != movement);
            if missing.is_empty() {
                return Ok(());
            }
        }
        Err(NeighborhoodAssertionError::Missing {
            moves: missing,
            inspected,
        })
    }
    /// Reject forbidden moves within the observed prefix. Success for a limited
    /// or infinite provider is not proof of absence beyond that prefix.
    pub fn produces_none_of(
        &self,
        forbidden: impl IntoIterator<Item = M>,
    ) -> Result<(), NeighborhoodAssertionError<M, E>> {
        let forbidden: Vec<_> = forbidden.into_iter().collect();
        if forbidden.is_empty() {
            return Ok(());
        }
        let state = self.state.borrow();
        let candidates = (self.provider)(&state).map_err(NeighborhoodAssertionError::Provider)?;
        for (index, movement) in candidates.take(self.limit).enumerate() {
            if forbidden.contains(&movement) {
                return Err(NeighborhoodAssertionError::Unexpected {
                    movement,
                    inspected: index + 1,
                });
            }
        }
        Ok(())
    }
}
