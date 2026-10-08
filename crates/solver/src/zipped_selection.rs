//! Lazy pairing of independently ordered selection streams.
use crate::{SelectionLifecycleEvent, SelectionSource};

/// Pair corresponding occurrences from two independent sources without storing
/// either traversal. Sources may have different payload types and need not be
/// finite or cloneable. Both receive each lifecycle event, left first.
///
/// Opening is lazy: dropping an unconsumed cursor does not open either child.
/// The first next() opens both children. Each pair reads left then right;
/// exhaustion on the right may consume one unmatched left occurrence. Exhaustion
/// on either side fuses the cursor. A fresh open starts fresh child traversals.
/// Callbacks must return promptly; effects and panics are not rolled back.
///
/// Never-ending metadata is true only when both children advertise it. It may
/// still be true for an empty source or an explicitly terminated traversal.
/// Finite size is the shorter exact child size. If either child repeats, exact
/// size is unknown: repeating metadata does not guarantee a nonempty traversal
/// or prevent cooperative termination. A known empty child makes the size zero.
/// Cardinality is metadata, not a scan.
/// Use LimitedSelectionSource before materializing repeating pairs in a cache.
/// Filters on the children run independently before pairing; filters outside
/// this adapter see complete pairs. This distinction matters for pinned values.
pub struct ZippedSelectionSource<L, R> {
    left: L,
    right: R,
}
impl<L, R> ZippedSelectionSource<L, R> {
    pub fn new(left: L, right: R) -> Self {
        Self { left, right }
    }
    pub fn left(&self) -> &L {
        &self.left
    }
    pub fn right(&self) -> &R {
        &self.right
    }
    pub fn left_mut(&mut self) -> &mut L {
        &mut self.left
    }
    pub fn right_mut(&mut self) -> &mut R {
        &mut self.right
    }
}
impl<C, A, B, L: SelectionSource<C, A>, R: SelectionSource<C, B>> SelectionSource<C, (A, B)>
    for ZippedSelectionSource<L, R>
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.left.event(event, context);
        self.right.event(event, context);
    }
    fn is_never_ending(&self) -> bool {
        self.left.is_never_ending() && self.right.is_never_ending()
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        let left = self.left.size(context);
        let right = self.right.size(context);
        if left == Some(0) || right == Some(0) {
            return Some(0);
        }
        match (self.left.is_never_ending(), self.right.is_never_ending()) {
            (true, _) | (_, true) => None,
            (false, false) => left.zip(right).map(|(l, r)| l.min(r)),
        }
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = (A, B)> + 'a>
    where
        (A, B): 'a,
    {
        let left = &mut self.left;
        let right = &mut self.right;
        Box::new(
            std::iter::once_with(move || left.open(context).zip(right.open(context)))
                .flatten()
                .fuse(),
        )
    }
}
