//! Explicit, bounded construction placement queues over phase-scoped keys.
use crate::{
    CachedSelectionOrder, SelectionCache, SelectionCacheError, SelectionCacheScope,
    SelectionCacheState, SelectionLifecycleEvent, SelectionSource, WeightedSelection,
    WeightedSelectionError,
};
use std::{error::Error, fmt};

/// Candidate source for one recorded entity/value key. Each opening sees the
/// current context; only the key list is snapshotted at phase start. Callbacks
/// must return promptly. Some(size) promises an exact count, not an upper bound.
pub trait PlacementSource<C, K, M> {
    fn event(&mut self, _event: SelectionLifecycleEvent, _context: &C) {}
    fn is_never_ending(&self) -> bool {
        false
    }
    fn size(&mut self, context: &C, key: &K) -> Option<usize>;
    fn open<'a>(&'a mut self, context: &'a C, key: &'a K) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a;
}
impl<C, K, M, T: PlacementSource<C, K, M> + ?Sized> PlacementSource<C, K, M> for Box<T> {
    fn event(&mut self, e: SelectionLifecycleEvent, c: &C) {
        (**self).event(e, c);
    }
    fn is_never_ending(&self) -> bool {
        (**self).is_never_ending()
    }
    fn size(&mut self, c: &C, k: &K) -> Option<usize> {
        (**self).size(c, k)
    }
    fn open<'a>(&'a mut self, c: &'a C, k: &'a K) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        (**self).open(c, k)
    }
}
/// Convenience adapter for callbacks returning an owned candidate vector. Vector
/// allocation happens inside the callback; implement PlacementSource for lazy input.
pub struct PlacementFn<F>(pub F);
impl<C, K, M, F: FnMut(&C, &K) -> Vec<M>> PlacementSource<C, K, M> for PlacementFn<F> {
    fn size(&mut self, _: &C, _: &K) -> Option<usize> {
        None
    }
    fn open<'a>(&'a mut self, c: &'a C, k: &'a K) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        Box::new((self.0)(c, k).into_iter())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlacementOrder {
    /// Visit every child for the first key, then every child for the next key.
    Once,
    /// Repeat the same key/child order until the caller stops. Empty key/child
    /// collections remain exhausted. Empty candidate pools are still placements.
    Cycle,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlacementError {
    Cache(SelectionCacheError),
    UnboundedChild {
        child: usize,
    },
    CandidateLimit {
        child: usize,
        maximum: usize,
    },
    SizeMismatch {
        child: usize,
        declared: usize,
        actual: usize,
    },
    /// A failed or panicking callback invalidated this phase's queue.
    Unavailable,
}
impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "placement queue: {self:?}")
    }
}
impl Error for PlacementError {}
impl From<SelectionCacheError> for PlacementError {
    fn from(e: SelectionCacheError) -> Self {
        Self::Cache(e)
    }
}
/// Owned immutable placement; held snapshots survive subsequent steps/phases.
/// Neither keys nor moves need Clone to share a snapshot or traverse it repeatedly.
pub struct PlacementSnapshot<K, M> {
    keys: WeightedSelection<K>,
    key: usize,
    child: usize,
    moves: WeightedSelection<M>,
}
impl<K, M> Clone for PlacementSnapshot<K, M> {
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            key: self.key,
            child: self.child,
            moves: self.moves.clone(),
        }
    }
}
impl<K, M> PlacementSnapshot<K, M> {
    pub fn key(&self) -> &K {
        &self.keys.items()[self.key]
    }
    pub fn child_index(&self) -> usize {
        self.child
    }
    pub fn moves(&self) -> &[M] {
        self.moves.items()
    }
    pub fn iter(&self) -> std::slice::Iter<'_, M> {
        self.moves().iter()
    }
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }
    pub fn len(&self) -> usize {
        self.moves.len()
    }
}
type KeyCache<C, K, S> = SelectionCache<C, K, S, fn(&C, &K) -> f64>;
/// Key-major placement scheduling with explicit balanced lifecycle events.
/// Keys are collected once per phase; one child is opened per next_placement call.
/// Each finite child pool is collected under its limit before the queue advances.
/// There is no scoring, legality filtering or automatic solver lifecycle wiring.
///
/// Events are forwarded once to the key source and each child on success. Invalid
/// transitions forward nothing. Callback failures/panics invalidate the queue for
/// the rest of the phase; close scopes normally and start a new phase to recover.
/// Earlier snapshots survive. Callback side effects and partial event delivery
/// cannot be rolled back. Source edits affect the next opening; key edits wait
/// for the next phase. A provider may clone moves from a placement into its pool.
pub struct PlacementQueue<C, K, M, KS, S> {
    keys: KeyCache<C, K, KS>,
    children: Vec<S>,
    order: PlacementOrder,
    maximum: usize,
    key: usize,
    child: usize,
    failed: bool,
    marker: std::marker::PhantomData<fn() -> M>,
}
impl<C, K, M, KS: SelectionSource<C, K>, S: PlacementSource<C, K, M>>
    PlacementQueue<C, K, M, KS, S>
{
    pub fn new(
        keys: KS,
        children: Vec<S>,
        order: PlacementOrder,
        maximum_keys: usize,
        maximum_moves: usize,
    ) -> Result<Self, PlacementError> {
        Ok(Self {
            keys: SelectionCache::uniform(
                keys,
                SelectionCacheScope::Phase,
                CachedSelectionOrder::Original,
                maximum_keys,
            )?,
            children,
            order,
            maximum: maximum_moves,
            key: 0,
            child: 0,
            failed: false,
            marker: std::marker::PhantomData,
        })
    }
    pub fn key_source(&self) -> &KS {
        self.keys.source()
    }
    pub fn key_source_mut(&mut self) -> &mut KS {
        self.keys.source_mut()
    }
    pub fn children(&self) -> &[S] {
        &self.children
    }
    /// Editing children cannot change the number or order of queue lanes.
    pub fn children_mut(&mut self) -> &mut [S] {
        &mut self.children
    }
    pub fn state(&self) -> SelectionCacheState {
        self.keys.state()
    }
    pub fn order(&self) -> PlacementOrder {
        self.order
    }
    /// Number of key/child occurrences in one complete pass. Available while
    /// the phase key cache is live; counts empty candidate pools as placements.
    pub fn placement_count(&self) -> Result<u128, PlacementError> {
        if self.failed {
            return Err(PlacementError::Unavailable);
        }
        Ok(self.keys.size()? as u128 * self.children.len() as u128)
    }
    pub fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), PlacementError> {
        use SelectionCacheState as State;
        use SelectionLifecycleEvent as Event;
        let state = self.state();
        if !matches!(
            (state, event),
            (State::Idle, Event::SolveStarted)
                | (State::Solving, Event::PhaseStarted)
                | (State::Phase, Event::StepStarted)
                | (State::Step, Event::StepEnded)
                | (State::Phase, Event::PhaseEnded)
                | (State::Solving, Event::SolveEnded)
        ) {
            return Err(SelectionCacheError::InvalidTransition { state, event }.into());
        }
        let failed = self.failed;
        self.failed = true;
        let key_result = self.keys.event(event, context);
        // A bounded key-build error still entered its scope. Keep healthy child
        // scopes balanced so the caller can close and retry the phase normally.
        for child in &mut self.children {
            child.event(event, context);
        }
        key_result?;
        if event == Event::PhaseStarted {
            self.key = 0;
            self.child = 0;
            self.failed = false;
        } else {
            self.failed = failed;
        }
        Ok(())
    }
    /// Passive availability: does not open a source or claim a pool is nonempty.
    pub fn has_next(&self) -> Result<bool, PlacementError> {
        if self.failed {
            return Err(PlacementError::Unavailable);
        }
        let size = self.keys.size()?;
        Ok(!self.children.is_empty() && self.key < size)
    }
    pub fn next_placement(
        &mut self,
        context: &C,
    ) -> Result<Option<PlacementSnapshot<K, M>>, PlacementError> {
        if !self.has_next()? {
            return Ok(None);
        }
        let keys = self.keys.snapshot()?;
        let key = &keys.items()[self.key];
        self.failed = true;
        let child = &mut self.children[self.child];
        if child.is_never_ending() {
            return Err(PlacementError::UnboundedChild { child: self.child });
        }
        let declared = child.size(context, key);
        if declared.is_some_and(|n| n > self.maximum) {
            return Err(PlacementError::CandidateLimit {
                child: self.child,
                maximum: self.maximum,
            });
        }
        let moves = WeightedSelection::from_bounded_iter(
            child.open(context, key).map(|m| (m, 1.0)),
            self.maximum,
        )
        .map_err(|e| match e {
            WeightedSelectionError::CandidateLimit { .. } => PlacementError::CandidateLimit {
                child: self.child,
                maximum: self.maximum,
            },
            e => PlacementError::Cache(SelectionCacheError::Weights(e)),
        })?;
        if let Some(declared) = declared
            && declared != moves.len()
        {
            return Err(PlacementError::SizeMismatch {
                child: self.child,
                declared,
                actual: moves.len(),
            });
        }
        let result = PlacementSnapshot {
            keys,
            key: self.key,
            child: self.child,
            moves,
        };
        self.child += 1;
        if self.child == self.children.len() {
            self.child = 0;
            self.key += 1;
            if self.key == result.keys.len() && self.order == PlacementOrder::Cycle {
                self.key = 0;
            }
        }
        self.failed = false;
        Ok(Some(result))
    }
}
/// One unit key, for repeated ungrouped candidate pools.
pub struct SinglePlacementKey;
impl<C> SelectionSource<C, ()> for SinglePlacementKey {
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = ()> + 'a>
    where
        (): 'a,
    {
        Box::new(std::iter::once(()))
    }
}
/// Adapt an ordinary candidate source to the unit-key placement interface.
pub struct UngroupedPlacement<S>(pub S);
impl<C, M, S: SelectionSource<C, M>> PlacementSource<C, (), M> for UngroupedPlacement<S> {
    fn event(&mut self, e: SelectionLifecycleEvent, c: &C) {
        self.0.event(e, c);
    }
    fn is_never_ending(&self) -> bool {
        self.0.is_never_ending()
    }
    fn size(&mut self, c: &C, _: &()) -> Option<usize> {
        self.0.size(c)
    }
    fn open<'a>(&'a mut self, c: &'a C, _: &'a ()) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.0.open(c)
    }
}
pub type PooledPlacements<C, M, S> =
    PlacementQueue<C, (), M, SinglePlacementKey, UngroupedPlacement<S>>;
/// Reopen the same finite source for each placement, using the current context.
/// Availability remains true in an active phase even when a candidate pool is empty.
pub fn pooled_placements<C, M, S: SelectionSource<C, M>>(
    source: S,
    maximum_moves: usize,
) -> Result<PooledPlacements<C, M, S>, PlacementError> {
    PlacementQueue::new(
        SinglePlacementKey,
        vec![UngroupedPlacement(source)],
        PlacementOrder::Cycle,
        1,
        maximum_moves,
    )
}
