//! Composable source filters and explicit per-step candidate work limits.
use crate::{MoveProvider, SelectionLifecycleEvent, SelectionSource};
use std::{iter::FusedIterator, num::NonZeroUsize};

fn never_stop() -> bool {
    false
}

/// Limit each opened traversal independently. Events are forwarded unchanged.
/// Apply this before a filter to cap inspected candidates; after a filter to cap
/// accepted candidates. The latter alone cannot bound an endlessly rejecting filter.
pub struct LimitedSelectionSource<S> {
    source: S,
    maximum: usize,
}
impl<S> LimitedSelectionSource<S> {
    pub fn new(source: S, maximum: usize) -> Self {
        Self { source, maximum }
    }
    pub fn source(&self) -> &S {
        &self.source
    }
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }
}
impl<C, T, S: SelectionSource<C, T>> SelectionSource<C, T> for LimitedSelectionSource<S> {
    fn is_never_ending(&self) -> bool {
        false
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.source.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        let size = self.source.size(context);
        if self.maximum == 0 || size == Some(0) {
            Some(0)
        } else if self.source.is_never_ending() {
            Some(self.maximum)
        } else {
            size.map(|size| size.min(self.maximum))
        }
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        if self.maximum == 0 {
            return Box::new(std::iter::empty());
        }
        Box::new(self.source.open(context).take(self.maximum).fuse())
    }
}

/// Lazily reject source occurrences using the current open() context. Source
/// lifecycle events are forwarded unchanged. This does not materialize candidates.
/// Filtered size is unknown until consumed; upper_bound() reports only the child's
/// finite count, never a fabricated exact accepted count. A SelectionCache around
/// this source materializes matches at its configured boundary and knows that size.
///
/// Filtering alone does not bound rejected work. Use a finite source, or wrap the
/// child in LimitedSelectionSource first. For retryable bounded probes use the
/// separate BoundedFilter API; for solver providers use FilteredMoveProvider.
/// `with_termination` also polls inside rejected runs, after each configured
/// number of inspected occurrences (accepted or rejected), before inspecting
/// another. There is no initial poll; use interval one for the shortest delay.
/// Polling resets on every open. A true result fuses that traversal; a later
/// open starts afresh. Callbacks must return promptly; this is cooperative.
/// Never-ending metadata describes the child, not a guarantee against stopping.
pub struct FilteredSelectionSource<S, F, Z = fn() -> bool> {
    source: S,
    predicate: F,
    termination: Z,
    poll_interval: NonZeroUsize,
    stats: CandidateFilterStats,
}
impl<S, F> FilteredSelectionSource<S, F> {
    pub fn new(source: S, predicate: F) -> Self {
        Self {
            source,
            predicate,
            termination: never_stop,
            poll_interval: NonZeroUsize::new(usize::MAX).unwrap(),
            stats: CandidateFilterStats::default(),
        }
    }
}
impl<S, F, Z> FilteredSelectionSource<S, F, Z> {
    pub fn with_termination<N: FnMut() -> bool>(
        self,
        interval: NonZeroUsize,
        termination: N,
    ) -> FilteredSelectionSource<S, F, N> {
        FilteredSelectionSource {
            source: self.source,
            predicate: self.predicate,
            termination,
            poll_interval: interval,
            stats: self.stats,
        }
    }
    pub fn stats(&self) -> CandidateFilterStats {
        self.stats
    }
    pub fn source(&self) -> &S {
        &self.source
    }
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }
    pub fn upper_bound<C, T>(&mut self, context: &C) -> Option<usize>
    where
        S: SelectionSource<C, T>,
    {
        let size = self.source.size(context);
        if self.source.is_never_ending() {
            None
        } else {
            size
        }
    }
}
impl<C, T, S: SelectionSource<C, T>, F: FnMut(&C, &T) -> bool, Z: FnMut() -> bool>
    SelectionSource<C, T> for FilteredSelectionSource<S, F, Z>
{
    fn is_never_ending(&self) -> bool {
        self.source.is_never_ending()
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.source.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        let _ = self.upper_bound(context);
        None
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        self.stats = CandidateFilterStats::default();
        let predicate = &mut self.predicate;
        let source = &mut self.source;
        Box::new(Screened {
            source: std::iter::once_with(move || source.open(context)).flatten(),
            predicate: move |item: &T| predicate(context, item),
            scan_limit: usize::MAX,
            selection_limit: usize::MAX,
            stats: &mut self.stats,
            termination: &mut self.termination,
            poll_interval: self.poll_interval.get(),
            until_poll: self.poll_interval.get(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateFilterStop {
    Terminated,
    SourceExhausted,
    ScanLimit,
    SelectionLimit,
}
/// Last opened source or provider traversal. Until a terminating next() call,
/// stop is None;
/// a consumer may drop its cursor early, which does not prove source exhaustion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CandidateFilterStats {
    pub scanned: usize,
    pub rejected: usize,
    pub yielded: usize,
    pub stop: Option<CandidateFilterStop>,
}

/// Filter any native move provider with separate total inspected/yielded limits
/// per step. Counts reset on moves(), predicates receive the current immutable
/// problem/assignment and completed-step number. Child factories are lazy and
/// zero budgets never open them. Counters are inspectable after the cursor drops.
///
/// Reaching either cap ends this traversal, not a proof that no eligible moves
/// exist. The solver treats an empty traversal as no proposals for this step.
/// Optional `with_termination` polls a caller's cancellation/deadline predicate
/// after each configured number of inspected candidates, before the next read.
/// Poll cadence spans yielded candidates and resets per traversal. Limits take
/// precedence over a coincident poll. There is no initial poll and no automatic
/// access to solver controls: pass the same cancellation token in both places
/// when the solver must also report cancellation. Without a callback, only the
/// work caps stop rejected runs. Child next()/predicate calls must return promptly.
/// Panics and external callback effects are not rolled back. Duplicate accepted
/// occurrences remain.
pub struct FilteredMoveProvider<Q, F, Z = fn() -> bool> {
    provider: Q,
    predicate: F,
    scan_limit: usize,
    selection_limit: usize,
    stats: CandidateFilterStats,
    termination: Z,
    poll_interval: NonZeroUsize,
}
impl<Q, F> FilteredMoveProvider<Q, F> {
    pub fn new(provider: Q, scan_limit: usize, selection_limit: usize, predicate: F) -> Self {
        Self {
            provider,
            predicate,
            scan_limit,
            selection_limit,
            stats: CandidateFilterStats::default(),
            termination: never_stop,
            poll_interval: NonZeroUsize::new(usize::MAX).unwrap(),
        }
    }
}
impl<Q, F, Z> FilteredMoveProvider<Q, F, Z> {
    pub fn with_termination<N: FnMut() -> bool>(
        self,
        interval: NonZeroUsize,
        termination: N,
    ) -> FilteredMoveProvider<Q, F, N> {
        FilteredMoveProvider {
            provider: self.provider,
            predicate: self.predicate,
            scan_limit: self.scan_limit,
            selection_limit: self.selection_limit,
            stats: self.stats,
            termination,
            poll_interval: interval,
        }
    }
    pub fn provider(&self) -> &Q {
        &self.provider
    }
    pub fn provider_mut(&mut self) -> &mut Q {
        &mut self.provider
    }
    pub fn stats(&self) -> CandidateFilterStats {
        self.stats
    }
}
struct Screened<'a, I, F, Z> {
    source: I,
    predicate: F,
    scan_limit: usize,
    selection_limit: usize,
    stats: &'a mut CandidateFilterStats,
    termination: Z,
    poll_interval: usize,
    until_poll: usize,
}
impl<I: Iterator, F: FnMut(&I::Item) -> bool, Z: FnMut() -> bool> Iterator
    for Screened<'_, I, F, Z>
{
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        if self.stats.stop.is_some() {
            return None;
        }
        loop {
            if self.stats.yielded == self.selection_limit {
                self.stats.stop = Some(CandidateFilterStop::SelectionLimit);
                return None;
            }
            if self.stats.scanned == self.scan_limit {
                self.stats.stop = Some(CandidateFilterStop::ScanLimit);
                return None;
            }
            if self.until_poll == 0 {
                if (self.termination)() {
                    self.stats.stop = Some(CandidateFilterStop::Terminated);
                    return None;
                }
                self.until_poll = self.poll_interval;
            }
            let Some(item) = self.source.next() else {
                self.stats.stop = Some(CandidateFilterStop::SourceExhausted);
                return None;
            };
            self.stats.scanned += 1;
            self.until_poll -= 1;
            if (self.predicate)(&item) {
                self.stats.yielded += 1;
                return Some(item);
            }
            self.stats.rejected += 1;
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.stats.stop.is_some() {
            return (0, Some(0));
        }
        let remaining =
            (self.scan_limit - self.stats.scanned).min(self.selection_limit - self.stats.yielded);
        (
            0,
            Some(
                self.source
                    .size_hint()
                    .1
                    .map_or(remaining, |n| n.min(remaining)),
            ),
        )
    }
}
impl<I: Iterator, F: FnMut(&I::Item) -> bool, Z: FnMut() -> bool> FusedIterator
    for Screened<'_, I, F, Z>
{
}
impl<P, A, M, Q: MoveProvider<P, A, M>, F: FnMut(&P, &A, u64, &M) -> bool, Z: FnMut() -> bool>
    MoveProvider<P, A, M> for FilteredMoveProvider<Q, F, Z>
{
    fn event(&mut self, event: crate::MoveProviderEvent) {
        self.provider.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.provider.construction_placement_completed(installed)
    }

    fn moves<'a>(
        &'a mut self,
        problem: &'a P,
        assignment: &'a A,
        completed_steps: u64,
    ) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.stats = CandidateFilterStats::default();
        let provider = &mut self.provider;
        let predicate = &mut self.predicate;
        let source =
            std::iter::once_with(move || provider.moves(problem, assignment, completed_steps))
                .flatten();
        Box::new(Screened {
            source,
            predicate: move |item: &M| predicate(problem, assignment, completed_steps, item),
            scan_limit: self.scan_limit,
            selection_limit: self.selection_limit,
            stats: &mut self.stats,
            termination: &mut self.termination,
            poll_interval: self.poll_interval.get(),
            until_poll: self.poll_interval.get(),
        })
    }
}
