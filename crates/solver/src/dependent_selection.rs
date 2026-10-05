//! Prefix-dependent candidate trees with bounded source work.
use crate::SelectionLifecycleEvent;
use std::{error::Error, fmt, iter::FusedIterator, marker::PhantomData, sync::Arc};

pub type DependentRow<T> = Vec<Option<Arc<T>>>;
/// Already selected levels, with explicit holes for ignored empty children.
pub type DependentPrefix<T> = [Option<Arc<T>>];
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependentStop {
    Exhausted,
    PollLimit,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DependentSelectionStats {
    pub polls: usize,
    pub opened: usize,
    pub yielded: usize,
    pub stop: Option<DependentStop>,
}
struct Frame<I> {
    iterator: I,
    had_item: bool,
    exhausted: bool,
    omitted: bool,
}

/// Depth-first traversal with last-level-fastest order. Factories receive the
/// current prefix; None preserves the level of an explicitly ignored empty child.
/// Each suffix is reopened after an earlier choice changes. A rejected empty
/// suffix prunes that branch, never all remaining earlier choices. Zero levels
/// and branches with every level omitted produce no row.
///
/// Factories return independent cursors: clone Arc prefix entries if a cursor
/// needs to retain them. Payloads need not implement Clone. Work is bounded by a
/// total number of child next() calls, including exhaustion probes. Limit-driven
/// termination is distinct in stats(); it is not proof of exhaustive enumeration.
/// Factories/next() remain cooperative, and infinite suffixes can starve ancestors.
pub struct DependentSelectionIter<F, I: Iterator> {
    factory: F,
    levels: usize,
    ignore_empty: bool,
    poll_limit: usize,
    frames: Vec<Frame<I>>,
    prefix: DependentRow<I::Item>,
    stats: DependentSelectionStats,
}
impl<F, I: Iterator> DependentSelectionIter<F, I>
where
    F: FnMut(usize, &DependentPrefix<I::Item>) -> I,
{
    pub fn new(levels: usize, ignore_empty: bool, poll_limit: usize, factory: F) -> Self {
        Self {
            factory,
            levels,
            ignore_empty,
            poll_limit,
            frames: vec![],
            prefix: vec![],
            stats: DependentSelectionStats::default(),
        }
    }
    pub fn stats(&self) -> DependentSelectionStats {
        self.stats
    }
    fn stop(&mut self, reason: DependentStop) {
        self.stats.stop = Some(reason);
        self.frames.clear();
        self.prefix.clear();
    }
}
impl<F, I: Iterator> Iterator for DependentSelectionIter<F, I>
where
    F: FnMut(usize, &DependentPrefix<I::Item>) -> I,
{
    type Item = DependentRow<I::Item>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stats.stop.is_some() {
            return None;
        }
        if self.levels == 0 {
            self.stop(DependentStop::Exhausted);
            return None;
        }
        loop {
            if self.prefix.len() == self.levels {
                let row = self.prefix.clone();
                self.prefix.pop();
                if row.iter().any(Option::is_some) {
                    self.stats.yielded = self.stats.yielded.saturating_add(1);
                    return Some(row);
                }
                continue;
            }
            let depth = self.prefix.len();
            if self.frames.len() == depth {
                if self.stats.polls == self.poll_limit {
                    self.stop(DependentStop::PollLimit);
                    return None;
                }
                self.stats.opened = self.stats.opened.saturating_add(1);
                let iterator = (self.factory)(depth, &self.prefix);
                self.frames.push(Frame {
                    iterator,
                    had_item: false,
                    exhausted: false,
                    omitted: false,
                });
            }
            let frame = &mut self.frames[depth];
            if frame.exhausted {
                if self.ignore_empty && !frame.had_item && !frame.omitted {
                    frame.omitted = true;
                    self.prefix.push(None);
                } else {
                    self.frames.pop();
                    if self.prefix.pop().is_none() {
                        self.stop(DependentStop::Exhausted);
                        return None;
                    }
                }
                continue;
            }
            if self.stats.polls == self.poll_limit {
                self.stop(DependentStop::PollLimit);
                return None;
            }
            self.stats.polls += 1;
            match frame.iterator.next() {
                Some(item) => {
                    frame.had_item = true;
                    self.prefix.push(Some(Arc::new(item)));
                }
                None => frame.exhausted = true,
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.stats.stop.map(|_| 0))
    }
}
impl<F, I: Iterator> FusedIterator for DependentSelectionIter<F, I> where
    F: FnMut(usize, &DependentPrefix<I::Item>) -> I
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependentSampleError {
    PollLimit {
        depth: usize,
    },
    InvalidDraw {
        depth: usize,
        index: usize,
        bound: usize,
    },
}
impl fmt::Display for DependentSampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dependent sample: {self:?}")
    }
}
impl Error for DependentSampleError {}

/// Own a prefix-aware factory and a lifecycle hook. The hook can forward events
/// to its entity/value sources exactly once; the factory receives current context
/// every time a suffix opens. Returned cursors may own cloned prefix Arcs or borrow
/// external data, but cannot borrow the transient prefix slice itself.
/// This API has no fabricated product size: dependent branch cardinalities vary.
pub struct DependentSelection<C, T, F, H> {
    levels: usize,
    ignore_empty: bool,
    factory: F,
    lifecycle: H,
    marker: PhantomData<fn(&C) -> T>,
}
impl<C, T, F, H: FnMut(SelectionLifecycleEvent, &C)> DependentSelection<C, T, F, H> {
    pub fn new(levels: usize, ignore_empty: bool, factory: F, lifecycle: H) -> Self {
        Self {
            levels,
            ignore_empty,
            factory,
            lifecycle,
            marker: PhantomData,
        }
    }
    pub fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        (self.lifecycle)(event, context);
    }
    pub fn iter<'a, I: Iterator<Item = T>>(
        &'a mut self,
        context: &'a C,
        poll_limit: usize,
    ) -> DependentSelectionIter<impl FnMut(usize, &DependentPrefix<T>) -> I + 'a, I>
    where
        F: FnMut(&C, usize, &DependentPrefix<T>) -> I,
    {
        DependentSelectionIter::new(
            self.levels,
            self.ignore_empty,
            poll_limit,
            move |depth, prefix| (self.factory)(context, depth, prefix),
        )
    }
    /// Sample one path, materializing only each current conditional child under a
    /// total poll limit. None means this path is empty, not that the whole tree is
    /// empty; callers bound retry attempts explicitly. Choosing uniformly within
    /// each child is not uniform over complete rows when branch widths differ.
    /// Draw callbacks receive depth and a positive exclusive bound. Every choice
    /// uses this attempt's current prefix, with no previous-row replay state.
    /// Source/draw/lifecycle side effects are not rolled back on errors or panics.
    pub fn sample_with<I: Iterator<Item = T>>(
        &mut self,
        context: &C,
        poll_limit: usize,
        mut draw: impl FnMut(usize, usize) -> usize,
    ) -> Result<Option<DependentRow<T>>, DependentSampleError>
    where
        F: FnMut(&C, usize, &DependentPrefix<T>) -> I,
    {
        let mut prefix = vec![];
        let mut polls = 0;
        for depth in 0..self.levels {
            if polls == poll_limit {
                return Err(DependentSampleError::PollLimit { depth });
            }
            let mut source = (self.factory)(context, depth, &prefix);
            let mut items = vec![];
            loop {
                if polls == poll_limit {
                    return Err(DependentSampleError::PollLimit { depth });
                }
                polls += 1;
                match source.next() {
                    Some(item) => items.push(Arc::new(item)),
                    None => break,
                }
            }
            if items.is_empty() {
                if !self.ignore_empty {
                    return Ok(None);
                }
                prefix.push(None);
                continue;
            }
            let index = draw(depth, items.len());
            let value = items
                .get(index)
                .ok_or(DependentSampleError::InvalidDraw {
                    depth,
                    index,
                    bound: items.len(),
                })?
                .clone();
            prefix.push(Some(value));
        }
        Ok(prefix.iter().any(Option::is_some).then_some(prefix))
    }
}
