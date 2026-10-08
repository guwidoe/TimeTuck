//! Explicit shared snapshot leases and step-scoped source views.
use crate::{
    ProductSnapshot, SelectionCacheError, SelectionCacheState, SelectionLifecycleEvent,
    SelectionProduct, SelectionProductError, SelectionSource,
};
use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    fmt,
    hash::Hash,
    rc::{Rc, Weak},
    sync::Arc,
};

struct Entry<T> {
    value: Arc<T>,
}
type Entries<K, T> = RefCell<HashMap<K, Weak<Entry<T>>>>;

/// Thread-local registry of active snapshot demands. Equal keys must describe the
/// same computation and immutable input revision. Include source identity and
/// configuration in the key when a registry serves different sources.
/// Builders run outside registry borrows; the first still-live publication wins
/// even if a builder reenters the pool. Errors/panics publish no partial value.
/// Values need not be Clone. Dropping the last lease removes its registry entry;
/// independently held Arc snapshots remain valid but do not keep a demand active.
pub struct SharedSelectionPool<K, T> {
    entries: Rc<Entries<K, T>>,
}
impl<K, T> Default for SharedSelectionPool<K, T> {
    fn default() -> Self {
        Self {
            entries: Rc::new(RefCell::new(HashMap::new())),
        }
    }
}
impl<K, T> Clone for SharedSelectionPool<K, T> {
    fn clone(&self) -> Self {
        Self {
            entries: self.entries.clone(),
        }
    }
}
impl<K: Clone + Eq + Hash, T> SharedSelectionPool<K, T> {
    pub fn active_count(&self, key: &K) -> usize {
        self.entries.borrow().get(key).map_or(0, Weak::strong_count)
    }
    pub fn len(&self) -> usize {
        self.entries.borrow().len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn try_acquire<E>(
        &self,
        key: K,
        build: impl FnOnce() -> Result<T, E>,
    ) -> Result<SharedSelectionLease<K, T>, E> {
        let existing = self.entries.borrow().get(&key).and_then(Weak::upgrade);
        let entry = if let Some(entry) = existing {
            entry
        } else {
            let value = Arc::new(build()?);
            let existing = self.entries.borrow().get(&key).and_then(Weak::upgrade);
            if let Some(entry) = existing {
                entry
            } else {
                let entry = Rc::new(Entry { value });
                self.entries
                    .borrow_mut()
                    .insert(key.clone(), Rc::downgrade(&entry));
                entry
            }
        };
        Ok(SharedSelectionLease {
            key,
            entry,
            pool: Rc::downgrade(&self.entries),
        })
    }
}
pub struct SharedSelectionLease<K: Eq + Hash, T> {
    key: K,
    entry: Rc<Entry<T>>,
    pool: Weak<Entries<K, T>>,
}
impl<K: Clone + Eq + Hash, T> Clone for SharedSelectionLease<K, T> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            entry: self.entry.clone(),
            pool: self.pool.clone(),
        }
    }
}
impl<K: Eq + Hash, T> SharedSelectionLease<K, T> {
    pub fn value(&self) -> &T {
        &self.entry.value
    }
    pub fn snapshot(&self) -> Arc<T> {
        self.entry.value.clone()
    }
}
impl<K: Eq + Hash, T> Drop for SharedSelectionLease<K, T> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.entry) == 1
            && let Some(pool) = self.pool.upgrade()
        {
            pool.borrow_mut().remove(&self.key);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedSelectionError {
    Lifecycle(SelectionCacheError),
    Collection(SelectionProductError),
}
impl fmt::Display for SharedSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "shared selection: {self:?}")
    }
}
impl Error for SharedSelectionError {}
impl From<SelectionCacheError> for SharedSelectionError {
    fn from(e: SelectionCacheError) -> Self {
        Self::Lifecycle(e)
    }
}
impl From<SelectionProductError> for SharedSelectionError {
    fn from(e: SelectionProductError) -> Self {
        Self::Collection(e)
    }
}

/// Share one bounded finite source collection between step-scoped views. Each
/// view owns its child and forwards that child's lifecycle; only the first view
/// with a given key opens/collects its child. Equal keys promise equivalent child
/// outputs for the same input revision. A key must change if source configuration,
/// order or grouping facts change while another view still holds a lease.
///
/// StepStarted acquires a lease after forwarding the child event. StepEnded
/// releases it before forwarding. PhaseEnded closes an unfinished step, and
/// SolveEnded closes unfinished inner scopes. Every child receives a balanced
/// sequence, including synthesized end events. Dropping a view releases its
/// demand without calling user lifecycle callbacks. Held snapshots are independent
/// of active demand counts. Sources/contexts/payloads do not need Clone.
///
/// Invalid events change nothing; failed starts remain entered but unavailable,
/// so end events can clean them up before retrying. Panics propagate. Source edits
/// take effect on the next acquisition and must respect key identity. A source
/// can be GroupedSelectionSource, and this view can feed CachedMoveProvider for
/// automatic engine events. Both outer and shared caches should use Step scope.
pub struct SharedStepSelectionSource<C, K: Eq + Hash, T, S, F> {
    input: SelectionProduct<C, T, S>,
    pool: SharedSelectionPool<K, ProductSnapshot<T>>,
    key: F,
    lease: Option<SharedSelectionLease<K, ProductSnapshot<T>>>,
    state: SelectionCacheState,
}
impl<C, K: Clone + Eq + Hash, T, S: SelectionSource<C, T>, F: FnMut(&C) -> K>
    SharedStepSelectionSource<C, K, T, S, F>
{
    pub fn new(
        pool: SharedSelectionPool<K, ProductSnapshot<T>>,
        source: S,
        maximum: usize,
        key: F,
    ) -> Self {
        Self {
            input: SelectionProduct::new(vec![source], false, maximum),
            pool,
            key,
            lease: None,
            state: SelectionCacheState::Idle,
        }
    }
    pub fn source(&self) -> &S {
        &self.input.children()[0]
    }
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.input.children_mut()[0]
    }
    pub fn state(&self) -> SelectionCacheState {
        self.state
    }
    pub fn snapshot(&self) -> Result<ProductSnapshot<T>, SharedSelectionError> {
        self.lease
            .as_ref()
            .map(|l| l.value().clone())
            .ok_or_else(|| SelectionCacheError::NotReady.into())
    }
    pub fn try_event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), SharedSelectionError> {
        use SelectionCacheState as S;
        use SelectionLifecycleEvent as E;
        if matches!(event, E::PhaseEnded | E::SolveEnded) && self.state == S::Step {
            self.try_event(E::StepEnded, context)?;
        }
        if event == E::SolveEnded && self.state == S::Phase {
            self.try_event(E::PhaseEnded, context)?;
        }
        let next = match (self.state, event) {
            (S::Idle, E::SolveStarted) => S::Solving,
            (S::Solving, E::PhaseStarted) => S::Phase,
            (S::Phase, E::StepStarted) => S::Step,
            (S::Step, E::StepEnded) => S::Phase,
            (S::Phase, E::PhaseEnded) => S::Solving,
            (S::Solving, E::SolveEnded) => S::Idle,
            _ => {
                return Err(SelectionCacheError::InvalidTransition {
                    state: self.state,
                    event,
                }
                .into());
            }
        };
        self.state = next;
        if event == E::StepEnded {
            self.lease = None;
        }
        self.input.event(event, context);
        if event == E::StepStarted {
            let key = (self.key)(context);
            self.lease = Some(
                self.pool
                    .try_acquire(key, || self.input.snapshot(context))?,
            );
        }
        Ok(())
    }
}
impl<C, K: Clone + Eq + Hash, T, S: SelectionSource<C, T>, F: FnMut(&C) -> K>
    SelectionSource<C, Arc<T>> for SharedStepSelectionSource<C, K, T, S, F>
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.try_event(event, context)
            .unwrap_or_else(|e| panic!("{e}"));
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(
            self.snapshot()
                .unwrap_or_else(|e| panic!("{e}"))
                .child_pool(0)
                .map_or(0, <[_]>::len),
        )
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = Arc<T>> + 'a>
    where
        Arc<T>: 'a,
    {
        let snapshot = self.snapshot().unwrap_or_else(|e| panic!("{e}"));
        let n = snapshot.child_pool(0).map_or(0, <[_]>::len);
        Box::new((0..n).map(move |i| snapshot.child_pool(0).unwrap()[i].clone()))
    }
}
