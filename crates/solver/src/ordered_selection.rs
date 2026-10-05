//! Stable sorting and step-wise sampling without replacement over scoped caches.
use crate::{
    CachedSelectionOrder, SelectionCache, SelectionCacheError, SelectionCacheScope,
    SelectionCacheState, SelectionLifecycleEvent, SelectionSource, WeightedSelection,
    random::SplitMix64,
};
use std::{cmp::Ordering, error::Error, fmt, iter::FusedIterator, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrderedSelectionError {
    Cache(SelectionCacheError),
    InvalidDraw { bound: usize, index: usize },
}
impl From<SelectionCacheError> for OrderedSelectionError {
    fn from(error: SelectionCacheError) -> Self {
        Self::Cache(error)
    }
}
impl fmt::Display for OrderedSelectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cache(error) => error.fmt(f),
            Self::InvalidDraw { bound, index } => {
                write!(f, "shuffle index {index} is outside 0..{bound}")
            }
        }
    }
}
impl Error for OrderedSelectionError {}

/// Immutable occurrence order over shared candidates. Equal candidates remain
/// distinct occurrences. Cloning a snapshot never clones its items. Held snapshots
/// remain usable after the owner's next shuffle, rebuild, or cache release.
pub struct OrderedSelection<T> {
    candidates: WeightedSelection<T>,
    indices: Arc<[usize]>,
}
impl<T> Clone for OrderedSelection<T> {
    fn clone(&self) -> Self {
        Self {
            candidates: self.candidates.clone(),
            indices: self.indices.clone(),
        }
    }
}
impl<T> OrderedSelection<T> {
    pub fn len(&self) -> usize {
        self.indices.len()
    }
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.indices
            .get(index)
            .map(|&i| &self.candidates.items()[i])
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &T> + ExactSizeIterator + FusedIterator {
        self.indices.iter().map(|&i| &self.candidates.items()[i])
    }
    /// Own the snapshot and clone only items actually yielded, allowing bounded
    /// move providers to stop without cloning the unused tail.
    pub fn into_items(
        self,
    ) -> impl DoubleEndedIterator<Item = T> + ExactSizeIterator + FusedIterator
    where
        T: Clone,
    {
        (0..self.len()).map(move |i| self.get(i).unwrap().clone())
    }
}

type Compare<C, T> = Box<dyn FnMut(&C, &T, &T) -> Ordering>;
enum OrderingMode<C, T> {
    Sorted(Compare<C, T>),
    Shuffled(Box<dyn FnMut(usize) -> usize>),
}
type UniformCache<C, T, S> = SelectionCache<C, T, S, fn(&C, &T) -> f64>;

/// Finite candidate ordering with explicit solve/phase/step lifecycle delivery.
/// Sorting is stable and happens once per configured cache build, using that
/// boundary's context. Shuffling happens at every step start, starting with the
/// previous permutation until the source cache rebuilds. Repeated iteration in
/// one step returns the same order; every occurrence is yielded exactly once.
///
/// Sources must be finite and are collected under an explicit maximum. Callbacks
/// are owned; comparators must give a consistent total ordering. A valid event
/// advances lifecycle state before callbacks. Build/order failure or unwinding
/// makes the current order unavailable until the next configured cache rebuild;
/// close the active scope normally. Already held snapshots remain valid. External
/// callback effects cannot be undone. Parent lifecycle events are caller-driven.
pub struct OrderedSelectionCache<C, T, S> {
    cache: UniformCache<C, T, S>,
    scope: SelectionCacheScope,
    mode: OrderingMode<C, T>,
    current: Option<OrderedSelection<T>>,
}
impl<C, T, S: SelectionSource<C, T>> OrderedSelectionCache<C, T, S> {
    pub fn sorted_by(
        source: S,
        scope: SelectionCacheScope,
        maximum: usize,
        compare: impl FnMut(&C, &T, &T) -> Ordering + 'static,
    ) -> Result<Self, OrderedSelectionError> {
        Self::new(
            source,
            scope,
            maximum,
            OrderingMode::Sorted(Box::new(compare)),
        )
    }
    pub fn shuffled(
        source: S,
        scope: SelectionCacheScope,
        maximum: usize,
        seed: u64,
    ) -> Result<Self, OrderedSelectionError> {
        let mut random = SplitMix64::new(seed);
        Self::shuffled_with(source, scope, maximum, move |bound| {
            random.below_wide(bound as u128) as usize
        })
    }
    /// Inject a draw returning an index below its exclusive positive bound.
    /// An n-item shuffle requests bounds n,n-1,...,2, once each. Empty/singleton
    /// sources consume no draws. Native seeded randomness need not match Java.
    pub fn shuffled_with(
        source: S,
        scope: SelectionCacheScope,
        maximum: usize,
        draw: impl FnMut(usize) -> usize + 'static,
    ) -> Result<Self, OrderedSelectionError> {
        Self::new(
            source,
            scope,
            maximum,
            OrderingMode::Shuffled(Box::new(draw)),
        )
    }
    fn new(
        source: S,
        scope: SelectionCacheScope,
        maximum: usize,
        mode: OrderingMode<C, T>,
    ) -> Result<Self, OrderedSelectionError> {
        Ok(Self {
            cache: SelectionCache::uniform(source, scope, CachedSelectionOrder::Original, maximum)?,
            scope,
            mode,
            current: None,
        })
    }
    pub fn source(&self) -> &S {
        self.cache.source()
    }
    /// Mutations become visible at the next configured source-cache rebuild.
    pub fn source_mut(&mut self) -> &mut S {
        self.cache.source_mut()
    }
    pub fn state(&self) -> SelectionCacheState {
        self.cache.state()
    }
    pub fn is_never_ending(&self) -> bool {
        false
    }
    pub fn snapshot(&self) -> Result<OrderedSelection<T>, OrderedSelectionError> {
        self.current
            .clone()
            .ok_or(SelectionCacheError::NotReady.into())
    }
    pub fn size(&self) -> Result<usize, OrderedSelectionError> {
        Ok(self.snapshot()?.len())
    }
    pub fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), OrderedSelectionError> {
        use SelectionCacheScope as Scope;
        use SelectionLifecycleEvent as Event;
        let rebuild = matches!(
            (self.scope, event),
            (Scope::Solve, Event::SolveStarted)
                | (Scope::Phase, Event::PhaseStarted)
                | (Scope::Step, Event::StepStarted)
        );
        let release = matches!(
            (self.scope, event),
            (Scope::Solve, Event::SolveEnded)
                | (Scope::Phase, Event::PhaseEnded)
                | (Scope::Step, Event::StepEnded)
        );
        let shuffle = matches!(self.mode, OrderingMode::Shuffled(_)) && event == Event::StepStarted;
        let changes_order = rebuild || release || shuffle;
        let previous = if changes_order {
            self.current.take()
        } else {
            None
        };
        if let Err(error) = self.cache.event(event, context) {
            if matches!(error, SelectionCacheError::InvalidTransition { .. }) && changes_order {
                self.current = previous;
            }
            return Err(error.into());
        }
        if release || !changes_order {
            return Ok(());
        }
        let (candidates, mut indices) = if rebuild {
            let candidates = self.cache.snapshot()?;
            let indices = (0..candidates.len()).collect::<Vec<_>>();
            (candidates, indices)
        } else {
            let previous = previous.ok_or(SelectionCacheError::NotReady)?;
            (previous.candidates, previous.indices.to_vec())
        };
        match &mut self.mode {
            OrderingMode::Sorted(compare) => {
                indices.sort_by(|&a, &b| {
                    compare(context, &candidates.items()[a], &candidates.items()[b])
                });
            }
            OrderingMode::Shuffled(draw) if shuffle => {
                for bound in (2..=indices.len()).rev() {
                    let index = draw(bound);
                    if index >= bound {
                        return Err(OrderedSelectionError::InvalidDraw { bound, index });
                    }
                    indices.swap(bound - 1, index);
                }
            }
            OrderingMode::Shuffled(_) => {}
        }
        self.current = Some(OrderedSelection {
            candidates,
            indices: indices.into(),
        });
        Ok(())
    }
}
