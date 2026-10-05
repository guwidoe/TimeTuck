//! Explicit selection lifecycle and immutable caches at solve/phase/step scope.
use crate::{WeightedSelection, WeightedSelectionError, random::SplitMix64};
use std::{error::Error, fmt, iter::FusedIterator, marker::PhantomData};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionCacheScope {
    Uncached,
    Solve,
    Phase,
    Step,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CachedSelectionOrder {
    Original,
    Random,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionCacheState {
    Idle,
    Solving,
    Phase,
    Step,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionLifecycleEvent {
    SolveStarted,
    PhaseStarted,
    StepStarted,
    StepEnded,
    PhaseEnded,
    SolveEnded,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionCacheError {
    UncachedScope,
    UnboundedSource,
    InvalidTransition {
        state: SelectionCacheState,
        event: SelectionLifecycleEvent,
    },
    NotReady,
    RandomListAccess,
    IndexOutOfBounds {
        index: usize,
        len: usize,
    },
    SizeMismatch {
        declared: usize,
        actual: usize,
    },
    Weights(WeightedSelectionError),
}
impl fmt::Display for SelectionCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selection cache: {self:?}")
    }
}
impl Error for SelectionCacheError {}
impl From<WeightedSelectionError> for SelectionCacheError {
    fn from(error: WeightedSelectionError) -> Self {
        Self::Weights(error)
    }
}

/// A finite candidate source and its lifecycle hook. Every valid event is forwarded
/// exactly once, including events below the configured cache scope. size() and
/// open() run only when rebuilding. Some(size) promises an exact count; None permits
/// an unknown finite count. The caller-supplied collection limit remains enforced.
/// Source callbacks are cooperative and their side effects cannot be rolled back.
pub trait SelectionSource<C, T> {
    fn is_never_ending(&self) -> bool {
        false
    }
    fn event(&mut self, _event: SelectionLifecycleEvent, _context: &C) {}
    fn size(&mut self, context: &C) -> Option<usize>;
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a;
}

/// Lifecycle-owned candidate cache. The source must be finite even when selection
/// repeats randomly. Solve/phase/step start builds that scope's snapshot; its end
/// releases it. All lifecycle events are forwarded to the child, cached or not.
///
/// Callers explicitly deliver balanced events; this does not automatically wire
/// Java lifecycle objects or global solver listeners. A move provider may use a
/// step-scoped cache and retain its captured snapshot while evaluating candidates.
/// Solve/phase caches intentionally retain their earlier candidates/weights until
/// their next start. Choose a short enough scope for changing assignment facts.
///
/// Invalid transitions forward nothing. A valid event advances the state before
/// source callbacks. If a build fails or unwinds, that scope remains active but
/// its cache is unavailable; end it normally before retrying. No stale snapshot is
/// installed. Existing externally held snapshots remain owned and usable.
/// Weights are evaluated only for Random order; Original preserves source order.
pub struct SelectionCache<C, T, S, W> {
    source: S,
    weight: W,
    scope: SelectionCacheScope,
    order: CachedSelectionOrder,
    maximum: usize,
    state: SelectionCacheState,
    snapshot: Option<WeightedSelection<T>>,
    context: PhantomData<fn(&C)>,
}
impl<C, T, S: SelectionSource<C, T>> SelectionCache<C, T, S, fn(&C, &T) -> f64> {
    pub fn uniform(
        source: S,
        scope: SelectionCacheScope,
        order: CachedSelectionOrder,
        maximum: usize,
    ) -> Result<Self, SelectionCacheError> {
        Self::new(source, scope, order, maximum, |_, _| 1.0)
    }
}
impl<C, T, S: SelectionSource<C, T>, W: FnMut(&C, &T) -> f64> SelectionCache<C, T, S, W> {
    pub fn new(
        source: S,
        scope: SelectionCacheScope,
        order: CachedSelectionOrder,
        maximum: usize,
        weight: W,
    ) -> Result<Self, SelectionCacheError> {
        if scope == SelectionCacheScope::Uncached {
            return Err(SelectionCacheError::UncachedScope);
        }
        if source.is_never_ending() {
            return Err(SelectionCacheError::UnboundedSource);
        }
        Ok(Self {
            source,
            weight,
            scope,
            order,
            maximum,
            state: SelectionCacheState::Idle,
            snapshot: None,
            context: PhantomData,
        })
    }
    pub fn source(&self) -> &S {
        &self.source
    }
    /// Source changes become visible at the next configured cache boundary.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }
    pub fn state(&self) -> SelectionCacheState {
        self.state
    }
    /// Random selection has repeating semantics even if its current cache is empty.
    pub fn is_never_ending(&self) -> bool {
        self.order == CachedSelectionOrder::Random
    }
    pub fn size(&self) -> Result<usize, SelectionCacheError> {
        Ok(self.current()?.len())
    }
    pub fn snapshot(&self) -> Result<WeightedSelection<T>, SelectionCacheError> {
        Ok(self.current()?.clone())
    }
    fn current(&self) -> Result<&WeightedSelection<T>, SelectionCacheError> {
        self.snapshot.as_ref().ok_or(SelectionCacheError::NotReady)
    }
    pub fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), SelectionCacheError> {
        use SelectionCacheState as State;
        use SelectionLifecycleEvent as Event;
        let next = match (self.state, event) {
            (State::Idle, Event::SolveStarted) => State::Solving,
            (State::Solving, Event::PhaseStarted) => State::Phase,
            (State::Phase, Event::StepStarted) => State::Step,
            (State::Step, Event::StepEnded) => State::Phase,
            (State::Phase, Event::PhaseEnded) => State::Solving,
            (State::Solving, Event::SolveEnded) => State::Idle,
            _ => {
                return Err(SelectionCacheError::InvalidTransition {
                    state: self.state,
                    event,
                });
            }
        };
        let build = matches!(
            (self.scope, event),
            (SelectionCacheScope::Solve, Event::SolveStarted)
                | (SelectionCacheScope::Phase, Event::PhaseStarted)
                | (SelectionCacheScope::Step, Event::StepStarted)
        );
        let release = matches!(
            (self.scope, event),
            (SelectionCacheScope::Solve, Event::SolveEnded)
                | (SelectionCacheScope::Phase, Event::PhaseEnded)
                | (SelectionCacheScope::Step, Event::StepEnded)
        );
        self.state = next;
        if build || release {
            self.snapshot = None;
        }
        self.source.event(event, context);
        if build {
            let declared = self.source.size(context);
            if declared.is_some_and(|n| n > self.maximum) {
                return Err(WeightedSelectionError::CandidateLimit {
                    maximum: self.maximum,
                }
                .into());
            }
            let weights = &mut self.weight;
            let order = self.order;
            let entries = self.source.open(context).map(|item| {
                let weight = if order == CachedSelectionOrder::Original {
                    1.0
                } else {
                    weights(context, &item)
                };
                (item, weight)
            });
            let snapshot = WeightedSelection::from_bounded_iter(entries, self.maximum)?;
            if let Some(declared) = declared
                && snapshot.len() != declared
            {
                return Err(SelectionCacheError::SizeMismatch {
                    declared,
                    actual: snapshot.len(),
                });
            }
            self.snapshot = Some(snapshot);
        }
        Ok(())
    }
    pub fn iter(&self, seed: u64) -> Result<CachedSelectionIter<'_, T>, SelectionCacheError> {
        let mut random = SplitMix64::new(seed);
        self.iter_with(move || random.unit())
    }
    pub fn iter_with<'a>(
        &'a self,
        draw: impl FnMut() -> f64 + 'a,
    ) -> Result<CachedSelectionIter<'a, T>, SelectionCacheError> {
        Ok(CachedSelectionIter {
            snapshot: self.current()?,
            order: self.order,
            cursor: 0,
            draw: Box::new(draw),
        })
    }
    pub fn list_iter(&self) -> Result<std::slice::Iter<'_, T>, SelectionCacheError> {
        self.list_iter_from(0)
    }
    pub fn list_iter_from(
        &self,
        index: usize,
    ) -> Result<std::slice::Iter<'_, T>, SelectionCacheError> {
        if self.order == CachedSelectionOrder::Random {
            return Err(SelectionCacheError::RandomListAccess);
        }
        let items = self.current()?.items();
        Ok(items
            .get(index..)
            .ok_or(SelectionCacheError::IndexOutOfBounds {
                index,
                len: items.len(),
            })?
            .iter())
    }
}

pub struct CachedSelectionIter<'a, T> {
    snapshot: &'a WeightedSelection<T>,
    order: CachedSelectionOrder,
    cursor: usize,
    draw: Box<dyn FnMut() -> f64 + 'a>,
}
impl<T> CachedSelectionIter<'_, T> {
    /// Inspect availability without advancing the source or drawing randomness.
    pub fn has_next(&self) -> bool {
        match self.order {
            CachedSelectionOrder::Original => self.cursor < self.snapshot.len(),
            CachedSelectionOrder::Random => self.snapshot.active_len() > 0,
        }
    }
}
impl<'a, T> Iterator for CachedSelectionIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.has_next() {
            return None;
        }
        match self.order {
            CachedSelectionOrder::Original => {
                let item = &self.snapshot.items()[self.cursor];
                self.cursor += 1;
                Some(item)
            }
            CachedSelectionOrder::Random => self
                .snapshot
                .choose((self.draw)())
                .expect("cached selector requires draws in [0,1)"),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.order == CachedSelectionOrder::Original {
            let left = self.snapshot.len() - self.cursor;
            (left, Some(left))
        } else if self.has_next() {
            (usize::MAX, None)
        } else {
            (0, Some(0))
        }
    }
}
impl<T> FusedIterator for CachedSelectionIter<'_, T> {}
