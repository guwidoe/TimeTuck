//! Compose finite occurrence sources into lazy ordered pair selections.
use crate::{
    ProductSnapshot, SelectionLifecycleEvent, SelectionProduct, SelectionProductError,
    SelectionSource,
};
use std::{iter::FusedIterator, sync::Arc};

/// Owned handles to two source occurrences. Equal payloads retain multiplicity.
pub type SelectionPair<T> = (Arc<T>, Arc<T>);

/// Pair one shared source in positional order i<j, or two independent sources in
/// Cartesian order (right varies fastest). Shared mode opens and forwards events
/// to its child once; separate mode does so once per child, including empty ones.
/// This distinguishes a shared source from two sources with equal payloads.
/// No payload equality, entity legality, pin or no-op filtering is implied.
///
/// Collection uses SelectionProduct's finite-source checks, exact declared sizes
/// and per-child limit. Only source occurrences are stored; pairs remain lazy.
/// Neither the context nor payload needs Clone. Returned pair handles survive
/// source edits and lifecycle ends. size() is exact when child sizes are known
/// and representable, otherwise None. A known empty dimension implies size zero.
///
/// Use directly or inside CachedMoveProvider to drive real solver moves and
/// automatic lifecycle. Source callbacks are cooperative. try_pairs returns
/// collection errors; SelectionSource::open panics on them, never pretends the
/// selection is empty. Snapshot collection does not roll back external effects.
pub struct PairSelectionSource<C, T, S> {
    product: SelectionProduct<C, T, S>,
    shared: bool,
}
impl<C, T, S: SelectionSource<C, T>> PairSelectionSource<C, T, S> {
    pub fn shared(source: S, maximum_per_source: usize) -> Self {
        Self {
            product: SelectionProduct::new(vec![source], false, maximum_per_source),
            shared: true,
        }
    }
    pub fn separate(left: S, right: S, maximum_per_source: usize) -> Self {
        Self {
            product: SelectionProduct::new(vec![left, right], false, maximum_per_source),
            shared: false,
        }
    }
    pub fn sources(&self) -> &[S] {
        self.product.children()
    }
    pub fn sources_mut(&mut self) -> &mut [S] {
        self.product.children_mut()
    }
    pub fn try_pairs(
        &mut self,
        context: &C,
    ) -> Result<PairSelectionIter<T>, SelectionProductError> {
        let snapshot = self.product.snapshot(context)?;
        let left = snapshot.child_pool(0).map_or(0, <[Arc<T>]>::len);
        let count = if self.shared {
            (left as u128)
                .checked_mul(left.saturating_sub(1) as u128)
                .ok_or(SelectionProductError::CardinalityOverflow)?
                / 2
        } else {
            snapshot.cardinality()
        };
        Ok(PairSelectionIter {
            snapshot,
            shared: self.shared,
            left: 0,
            right: usize::from(self.shared),
            remaining: count,
        })
    }
}
impl<C, T, S: SelectionSource<C, T>> SelectionSource<C, SelectionPair<T>>
    for PairSelectionSource<C, T, S>
{
    fn is_never_ending(&self) -> bool {
        self.product
            .children()
            .iter()
            .any(SelectionSource::is_never_ending)
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.product.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        let children = self.product.children_mut();
        let left = children[0].size(context);
        if self.shared {
            let n = left? as u128;
            usize::try_from(n.checked_mul(n.saturating_sub(1))? / 2).ok()
        } else {
            let right = children[1].size(context);
            if left == Some(0) || right == Some(0) {
                Some(0)
            } else {
                left?.checked_mul(right?)
            }
        }
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = SelectionPair<T>> + 'a>
    where
        SelectionPair<T>: 'a,
    {
        Box::new(
            self.try_pairs(context)
                .unwrap_or_else(|error| panic!("pair selection source: {error}")),
        )
    }
}
/// Owns only the independent source snapshots, never an expanded list of pairs.
pub struct PairSelectionIter<T> {
    snapshot: ProductSnapshot<T>,
    shared: bool,
    left: usize,
    right: usize,
    remaining: u128,
}
impl<T> PairSelectionIter<T> {
    pub fn remaining(&self) -> u128 {
        self.remaining
    }
}
impl<T> Iterator for PairSelectionIter<T> {
    type Item = SelectionPair<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let left = self.snapshot.child_pool(0).expect("nonempty left pool");
        let right = if self.shared {
            left
        } else {
            self.snapshot.child_pool(1).expect("nonempty right pool")
        };
        let pair = (left[self.left].clone(), right[self.right].clone());
        self.remaining -= 1;
        self.right += 1;
        if self.right == right.len() {
            self.left += 1;
            self.right = if self.shared { self.left + 1 } else { 0 };
        }
        Some(pair)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match usize::try_from(self.remaining) {
            Ok(n) => (n, Some(n)),
            Err(_) => (usize::MAX, None),
        }
    }
}
impl<T> FusedIterator for PairSelectionIter<T> {}
