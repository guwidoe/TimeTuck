//! Explicitly fallible candidate generation.
use crate::{MoveProvider, MoveProviderEvent};
use std::{cell::RefCell, error::Error, fmt, sync::Arc};

/// A provider failure retaining its original, downcastable source. Clones refer
/// to the same failure; equality compares that identity, not its display text.
#[derive(Clone, Debug)]
pub struct MoveProviderError(Arc<dyn Error + Send + Sync>);
impl MoveProviderError {
    pub fn new(error: impl Error + Send + Sync + 'static) -> Self {
        Self(Arc::new(error))
    }
    pub fn inner(&self) -> &(dyn Error + Send + Sync + 'static) {
        self.0.as_ref()
    }
}
impl PartialEq for MoveProviderError {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for MoveProviderError {}
impl fmt::Display for MoveProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Error for MoveProviderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// A lazy provider whose setup or individual draws may fail. This is a separate
/// contract from [`MoveProvider`]; direct callers always receive the error.
///
/// The engine polls only as required by its selection/buffering policy. An
/// observed error discards the entire current scan, including an earlier saved
/// winner. Previously completed steps and best notifications remain valid; no
/// partial current step is published. Unpolled errors remain unobserved. Candidate
/// buffering and the scorer's budget-exhaustion lookahead may observe an error
/// without scoring another move. Returned errors close ordinary lifecycle scopes. Panics propagate
/// unchanged and do not synthesize lifecycle callbacks during unwinding.
///
/// Lifecycle and construction-completion hooks have the same infallible contract
/// as MoveProvider. Use fallible phase bindings or explicit fallible entry points.
pub trait FallibleMoveProvider<P, A, M> {
    fn event(&mut self, _event: MoveProviderEvent) {}
    fn construction_placement_completed(&mut self, _installed: bool) -> bool {
        false
    }
    fn try_moves<'a>(
        &'a mut self,
        problem: &'a P,
        assignment: &'a A,
        completed_steps: u64,
    ) -> Result<Box<dyn Iterator<Item = Result<M, MoveProviderError>> + 'a>, MoveProviderError>
    where
        M: 'a;
}
impl<P, A, M, F> FallibleMoveProvider<P, A, M> for F
where
    F: FnMut(&P, &A, u64) -> Result<Vec<M>, MoveProviderError>,
{
    fn try_moves<'a>(
        &'a mut self,
        p: &'a P,
        a: &'a A,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = Result<M, MoveProviderError>> + 'a>, MoveProviderError>
    where
        M: 'a,
    {
        Ok(Box::new(self(p, a, step)?.into_iter().map(Ok)))
    }
}

/// An explicit borrowed provider binding, returned when removing either kind.
pub enum MoveProviderBinding<'a, P, A, M> {
    Infallible(&'a mut dyn MoveProvider<P, A, M>),
    Fallible(&'a mut dyn FallibleMoveProvider<P, A, M>),
}
impl<P, A, M> MoveProviderBinding<'_, P, A, M> {
    pub(crate) fn reborrow(&mut self) -> MoveProviderBinding<'_, P, A, M> {
        match self {
            Self::Infallible(p) => MoveProviderBinding::Infallible(*p),
            Self::Fallible(p) => MoveProviderBinding::Fallible(*p),
        }
    }
    pub(crate) fn event(&mut self, event: MoveProviderEvent) {
        match self {
            Self::Infallible(p) => p.event(event),
            Self::Fallible(p) => p.event(event),
        }
    }
    pub(crate) fn construction_placement_completed(&mut self, installed: bool) -> bool {
        match self {
            Self::Infallible(p) => p.construction_placement_completed(installed),
            Self::Fallible(p) => p.construction_placement_completed(installed),
        }
    }
    pub(crate) fn moves<'a>(
        &'a mut self,
        p: &'a P,
        a: &'a A,
        step: u64,
        failure: &'a ProviderFailure,
    ) -> Result<Box<dyn Iterator<Item = M> + 'a>, MoveProviderError>
    where
        M: 'a,
    {
        match self {
            Self::Infallible(provider) => Ok(provider.moves(p, a, step)),
            Self::Fallible(provider) => {
                let mut moves = provider.try_moves(p, a, step)?;
                Ok(Box::new(std::iter::from_fn(move || {
                    if failure.failed() {
                        return None;
                    }
                    match moves.next()? {
                        Ok(m) => Some(m),
                        Err(e) => {
                            *failure.0.borrow_mut() = Some(e);
                            None
                        }
                    }
                })))
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct ProviderFailure(RefCell<Option<MoveProviderError>>);
impl ProviderFailure {
    pub(crate) fn failed(&self) -> bool {
        self.0.borrow().is_some()
    }
    pub(crate) fn check(&self) -> Result<(), MoveProviderError> {
        match self.0.borrow().as_ref() {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}
impl<S> crate::construction::SelectionPolicy<S> for &ProviderFailure {
    fn should_pick(&self, _score: S) -> bool {
        false
    }
    fn can_commit(&self, _evaluation_limit: bool) -> bool {
        !self.failed()
    }
}
