//! Bounded synchronized proposals from live dependent child cursors.
use crate::{DependentPrefix, DependentRow, SelectionLifecycleEvent, SelectionSource};
use std::{marker::PhantomData, num::NonZeroUsize, sync::Arc};

fn never_stop() -> bool {
    false
}

/// Advance each live child once per proposal, in registration order. Unlike the
/// depth-first DependentSelection tree, an infinite suffix does not starve its
/// peers. This is synchronized proposal generation, not Cartesian enumeration or
/// uniform sampling of complete combinations.
///
/// Each child factory receives the current proposal's prefix when that child
/// first opens or exhausts. Its cursor persists until exhaustion, even when an
/// earlier child changes; retain prefix Arcs in the cursor when needed. A factory
/// returns an independent cursor which cannot borrow the factory or temporary
/// prefix. Shared replay channels and borrowed external data are permitted.
/// Payloads need not implement Clone.
///
/// An exhausted cursor is dropped, reopened, and polled once. An empty fresh
/// cursor either rejects the proposal or contributes a None hole, according to
/// ignore_empty. Ignored empty children are retried on later proposals. All-empty
/// proposals are always rejected. At most maximum_failures consecutive rejected
/// proposals are attempted by one next(); reaching that bound fuses the opening.
/// No children produces no proposals. Productive finite children restart, so the
/// source is potentially unbounded and reports no fabricated cardinality.
///
/// Construction and open() invoke no child callbacks. Each next() polls at most
/// twice per visited child per attempt, with no lookahead after a yielded row.
/// Dropping stops work immediately. Use a provider's draw limit or a
/// LimitedSelectionSource to bound successful proposals. with_termination checks
/// before each attempt, factory and child poll; callbacks remain cooperative.
/// A callback panic fuses that opening when caught; a new open starts afresh.
/// Partial proposals never escape, but external callback effects cannot roll back.
/// Lifecycle events are forwarded once to the supplied hook, which owns any child
/// lifecycle routing. Existing cursors cannot coexist with a mutable event call.
pub struct StreamingSelection<C, T, I, F, H, Z = fn() -> bool> {
    children: usize,
    ignore_empty: bool,
    maximum_failures: NonZeroUsize,
    factory: F,
    lifecycle: H,
    termination: Z,
    marker: PhantomData<fn(&C) -> T>,
    cursor: PhantomData<I>,
}
impl<C, T, I, F, H> StreamingSelection<C, T, I, F, H> {
    pub fn new(
        children: usize,
        ignore_empty: bool,
        maximum_failures: NonZeroUsize,
        factory: F,
        lifecycle: H,
    ) -> Self {
        Self {
            children,
            ignore_empty,
            maximum_failures,
            factory,
            lifecycle,
            termination: never_stop,
            marker: PhantomData,
            cursor: PhantomData,
        }
    }
}
impl<C, T, I, F, H, Z> StreamingSelection<C, T, I, F, H, Z> {
    pub fn with_termination<N: FnMut() -> bool>(
        self,
        termination: N,
    ) -> StreamingSelection<C, T, I, F, H, N> {
        StreamingSelection {
            children: self.children,
            ignore_empty: self.ignore_empty,
            maximum_failures: self.maximum_failures,
            factory: self.factory,
            lifecycle: self.lifecycle,
            termination,
            marker: PhantomData,
            cursor: PhantomData,
        }
    }
}
impl<C, T, F, H, Z, I> SelectionSource<C, DependentRow<T>> for StreamingSelection<C, T, I, F, H, Z>
where
    F: FnMut(&C, usize, &DependentPrefix<T>) -> I,
    H: FnMut(SelectionLifecycleEvent, &C),
    Z: FnMut() -> bool,
    I: Iterator<Item = T>,
{
    fn is_never_ending(&self) -> bool {
        self.children != 0
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        (self.children == 0).then_some(0)
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        (self.lifecycle)(event, context);
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = DependentRow<T>> + 'a>
    where
        DependentRow<T>: 'a,
    {
        let mut cursors: Vec<Option<I>> = (0..self.children).map(|_| None).collect();
        let children = self.children;
        let maximum_failures = self.maximum_failures.get();
        let ignore_empty = self.ignore_empty;
        let factory = &mut self.factory;
        let termination = &mut self.termination;
        let mut fused = children == 0;
        Box::new(std::iter::from_fn(move || {
            if fused {
                return None;
            }
            // Set before user callbacks so a caught panic cannot resume a partly
            // consumed proposal. Only successful publication re-enables polling.
            fused = true;
            for _ in 0..maximum_failures {
                if termination() {
                    return None;
                }
                let mut row = Vec::with_capacity(children);
                let mut rejected = false;
                for (child, cursor) in cursors.iter_mut().enumerate() {
                    if termination() {
                        return None;
                    }
                    let mut item = cursor.as_mut().and_then(Iterator::next);
                    if item.is_none() {
                        *cursor = None;
                        if termination() {
                            return None;
                        }
                        *cursor = Some(factory(context, child, &row));
                        if termination() {
                            return None;
                        }
                        item = cursor.as_mut().unwrap().next();
                    }
                    if let Some(item) = item {
                        row.push(Some(Arc::new(item)));
                    } else {
                        *cursor = None;
                        if ignore_empty {
                            row.push(None);
                        } else {
                            rejected = true;
                            break;
                        }
                    }
                }
                if !rejected && row.iter().any(Option::is_some) {
                    fused = false;
                    return Some(row);
                }
            }
            None
        }))
    }
}
