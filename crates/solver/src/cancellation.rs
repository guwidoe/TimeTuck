//! Cooperative cancellation shared by one or more search phases.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// A cloneable, thread-safe, one-way cancellation request. Clones share the same
/// local request flag; independently created tokens do not. Linked tokens also
/// observe their parent request sources. There is no reset: create a fresh
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
pub struct CancellationToken {
    requested: Arc<AtomicBool>,
    // Flat immutable sources prevent recursive polling/drop of deeply linked
    // tokens. None keeps ordinary tokens allocation-free beyond their own flag.
    parents: Option<Arc<[Arc<AtomicBool>]>>,
}
impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }
    /// Create an independently cancellable token that also observes every parent.
    /// Cancelling a parent cancels its descendants; cancelling this token does not
    /// cancel its parents or siblings. Clones share this token's local request.
    /// Empty input creates an ordinary independent token. Already cancelled
    /// parents are observed immediately, and dropping parent handles does not
    /// disconnect them. A linked token has its own configuration identity even
    /// when it has only one parent.
    ///
    /// Nested sources are flattened and deduplicated once at construction. Polling
    /// is linear in unique ancestor request sources and uses no allocations, locks,
    /// watcher threads or recursion. Ordinary token polling remains constant time.
    ///
    /// Linking only affects cooperative polls. It does not remove a queued manager
    /// job, wake an idle live daemon, or interrupt user code. Keep using job/manager
    /// cancellation for those lifecycle operations. When a live problem revision
    /// restarts, link its fresh run token instead of reusing a cancelled old link.
    ///
    /// ```
    /// use timetuck::CancellationToken;
    /// let application = CancellationToken::new();
    /// let job = CancellationToken::new();
    /// let search = CancellationToken::linked([&application, &job]);
    /// assert!(job.cancel());
    /// assert!(search.is_cancelled());
    /// assert!(!application.is_cancelled());
    /// // The inherited request does not consume search's own request flag.
    /// assert!(search.cancel());
    /// assert!(!search.cancel());
    /// ```
    pub fn linked<'a>(parents: impl IntoIterator<Item = &'a Self>) -> Self {
        let mut sources = Vec::new();
        for parent in parents {
            sources.push(parent.requested.clone());
            if let Some(ancestors) = &parent.parents {
                sources.extend(ancestors.iter().cloned());
            }
        }
        sources.sort_unstable_by_key(Arc::as_ptr);
        sources.dedup_by(|left, right| Arc::ptr_eq(left, right));
        Self {
            requested: Arc::default(),
            parents: (!sources.is_empty()).then(|| sources.into()),
        }
    }
    /// Request cancellation locally. Returns true only for the first direct request
    /// shared by this token and all of its clones, even when a parent has already
    /// cancelled it. This never propagates a request to parents or sibling tokens.
    pub fn cancel(&self) -> bool {
        !self.requested.swap(true, Ordering::AcqRel)
    }
    pub fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Acquire)
            || self
                .parents
                .as_ref()
                .is_some_and(|sources| sources.iter().any(|source| source.load(Ordering::Acquire)))
    }
}
// Configuration equality compares cancellation identity, not mutable flag state.
impl PartialEq for CancellationToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.requested, &other.requested)
    }
}
impl Eq for CancellationToken {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattened_sources_deduplicate_diamonds_without_retaining_parent_nodes() {
        let root = CancellationToken::new();
        let left = CancellationToken::linked([&root, &root]);
        let right = CancellationToken::linked([&root]);
        let leaf = CancellationToken::linked([&left, &right, &root, &left]);
        assert_eq!(left.parents.as_ref().unwrap().len(), 1);
        assert_eq!(leaf.parents.as_ref().unwrap().len(), 3);
        assert!(CancellationToken::linked([]).parents.is_none());
        drop(left);
        drop(right);
        assert!(!leaf.is_cancelled());
        root.cancel();
        assert!(leaf.is_cancelled());
    }
}
