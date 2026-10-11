use super::ValueSwapSelection;
use crate::{ListMove, ListSelectionView, SelectionLifecycleEvent, SelectionSource};
use std::num::NonZeroUsize;

/// Cooperative work limits for one yielded swap. Each attempted pair reads at
/// most `reads_per_side` values from each child, including the successful read.
/// At most `pairs_per_item` complete pairs are checked before giving up. A
/// successful output resets both limits; opening again starts fresh traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListSwapSourceLimits {
    pub reads_per_side: NonZeroUsize,
    pub pairs_per_item: NonZeroUsize,
}

/// Lazily pair two live value streams into list swaps. Pins and required missing
/// values are filtered independently BEFORE pairing; receiver ranges and list
/// dependencies are checked AFTER pairing. Rejected complete pairs consume both
/// occurrences. Duplicate and reverse pairs retain their order and multiplicity.
/// Optional assigned/missing pairs become atomic unassign/assign replacements.
/// `include_noops` retains equal or two-missing pairs as empty composites; it
/// does not bypass pins, receiver ranges or dependency validation.
///
/// Opening is lazy: the first next() builds one assignment index and opens both
/// children, left first. No Cartesian product or child traversal is collected.
/// Locations are snapshots; reopen after committing assignment changes. Index
/// preparation follows the model's native Eq/indexed lookup costs. Each next()
/// performs at most twice reads_per_side times pairs_per_item child reads, plus
/// identity lookup and candidate validation. Child callbacks remain cooperative.
///
/// Child exhaustion or either work limit fuses that opening. The right side may
/// leave one unmatched left occurrence consumed. Never-ending metadata requires
/// both children to advertise it and means potentially unbounded generation,
/// not guaranteed infinite output. Exact size is unknown unless a child declares
/// zero. Events are forwarded once to each child, left first. External callback
/// effects cannot be rolled back. A caught child or validation panic fuses the
/// opening; a fresh opening retries from the children's own state. An unknown streamed value panics
/// when encountered, consistently with SelectionSource's infallible open API.
///
/// ListSelectionView supports borrowed contexts and owned model/assignment
/// snapshots captured by ConfiguredMoveProvider. Map each move with clone() and
/// set a finite provider draw limit; candidates remain streamed. Owned capture
/// copies assignment row containers and values once per capture. Alternatively,
/// bound outputs with take() or LimitedSelectionSource before collecting moves.
/// Receiver normalization preserves selected left/right
/// order for assigned swaps; assigned/missing replacement always unassigns the
/// assigned member before assigning the missing value at that same position.
pub struct ListValueSwapSource<L, R> {
    left: L,
    right: R,
    limits: ListSwapSourceLimits,
    include_noops: bool,
}
impl<L, R> ListValueSwapSource<L, R> {
    pub fn new(left: L, right: R, limits: ListSwapSourceLimits, include_noops: bool) -> Self {
        Self {
            left,
            right,
            limits,
            include_noops,
        }
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

fn next_selectable<V: Clone + Eq>(
    selection: &ValueSwapSelection<'_, V>,
    values: &mut dyn Iterator<Item = V>,
    limit: usize,
) -> Option<usize> {
    for _ in 0..limit {
        let value = values.next()?;
        let id = selection
            .problem
            .value_index(&value)
            .expect("list swap source requires registered values");
        if selection.selectable[id] {
            return Some(id);
        }
    }
    None
}
impl<V: Clone + Eq, C: ListSelectionView<V>, L: SelectionSource<C, V>, R: SelectionSource<C, V>>
    SelectionSource<C, ListMove<V>> for ListValueSwapSource<L, R>
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
        (left == Some(0) || right == Some(0)).then_some(0)
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        let left = &mut self.left;
        let right = &mut self.right;
        let limits = self.limits;
        let include_noops = self.include_noops;
        Box::new(
            std::iter::once_with(move || {
                let mut selection =
                    ValueSwapSelection::new(context.problem(), context.assignment())
                        .expect("list swap source requires a valid assignment");
                let mut left = left.open(context);
                let mut right = right.open(context);
                let mut ended = false;
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    // A caught panic must not resume after consuming half a pair.
                    ended = true;
                    for _ in 0..limits.pairs_per_item.get() {
                        let l =
                            next_selectable(&selection, &mut *left, limits.reads_per_side.get())?;
                        let r =
                            next_selectable(&selection, &mut *right, limits.reads_per_side.get())?;
                        if let Some(movement) = selection.movement(l, r, include_noops) {
                            ended = false;
                            return Some(movement);
                        }
                    }
                    None
                })
                .fuse()
            })
            .flatten()
            .fuse(),
        )
    }
}
