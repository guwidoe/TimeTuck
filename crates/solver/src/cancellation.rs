//! Cooperative cancellation shared by one or more search phases.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A cloneable, thread-safe, one-way cancellation request. Clones share the same
/// flag; independently created tokens do not. There is no reset: create a fresh
/// token for a new independently cancellable run. This does not interrupt an
/// active score callback or roll back a successfully selected candidate.
///
/// ```
/// use timetuck::CancellationToken;
/// let token = CancellationToken::new();
/// let controller = token.clone();
/// assert!(controller.cancel());
/// assert!(token.is_cancelled());
/// assert!(!token.cancel());
/// ```
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    /// Request cancellation. Returns true only for the first request shared by
    /// this token and all of its clones.
    pub fn cancel(&self) -> bool {
        !self.0.swap(true, Ordering::AcqRel)
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
// Configuration equality compares cancellation identity, not mutable flag state.
impl PartialEq for CancellationToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for CancellationToken {}
