use super::{ListValueChange, ValueChangeSelection};
use crate::{
    ListAssignment, ListPosition, ListProblem, ListSelectionContext, SelectionLifecycleEvent,
    SelectionSource,
};
use std::{num::NonZeroUsize, sync::Arc};

/// Immutable model and assignment view for list-change selection. The source
/// validates the assignment on first pull. An owned `(Arc<ListProblem<V>>,
/// ListAssignment<V>)` context can be captured by ConfiguredMoveProvider: share
/// the immutable model and clone the current assignment per capture. That
/// snapshot copies row containers and clones assigned values; it does not
/// materialize candidate pools.
/// Borrowed ListSelectionContext keeps its existing validation/borrowing contract.
pub trait ListSelectionView<V> {
    fn problem(&self) -> &ListProblem<V>;
    fn assignment(&self) -> &ListAssignment<V>;
}
impl<V: Clone + Eq> ListSelectionView<V> for ListSelectionContext<'_, V> {
    fn problem(&self) -> &ListProblem<V> {
        self.problem()
    }
    fn assignment(&self) -> &ListAssignment<V> {
        self.assignment()
    }
}
impl<V> ListSelectionView<V> for (Arc<ListProblem<V>>, ListAssignment<V>) {
    fn problem(&self) -> &ListProblem<V> {
        &self.0
    }
    fn assignment(&self) -> &ListAssignment<V> {
        &self.1
    }
}

/// Limits apply separately to every yielded item, including successful reads.
/// At most `reads_per_side` values and destinations are read for one attempted
/// pair; at most `pairs_per_item` completed pairs are validated. Exhausting either
/// limit fuses the opening. Successful output resets the limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ListChangeSourceLimits {
    pub reads_per_side: NonZeroUsize,
    pub pairs_per_item: NonZeroUsize,
}

/// Lazily pair live value and PRE-removal destination sources into list changes.
/// Pinned values are skipped before any destination is read. The selected value
/// is retained while pinned destinations are skipped. Receiver ranges and list
/// dependencies filter completed pairs, consuming both occurrences on rejection.
/// Missing values can be assigned even for required lists; unassignment requires
/// an optional model. Duplicate occurrences retain their order and multiplicity.
/// `include_noops` retains adjacent same-list slots and missing-to-unassigned
/// candidates, without bypassing pins, range or dependency validation.
///
/// Construction/opening do not traverse either child. First next() builds one
/// assignment index and opens both children, values first. No source pool or
/// Cartesian product is materialized. Index preparation follows native Eq/indexed
/// model costs. Each next() performs at most twice reads_per_side times
/// pairs_per_item child reads, plus identity lookup and candidate validation.
/// All positions belong to the borrowed assignment; reopen after committing edits.
///
/// Events forward once to each child, values first. Size is unknown unless a
/// child advertises zero. Never-ending metadata requires both children to advertise
/// it; this means potentially unbounded generation, not guaranteed output. Child
/// exhaustion, rejected-work exhaustion or a caught callback panic fuses the
/// opening. Unknown values and malformed destinations panic when encountered,
/// consistently with SelectionSource's infallible API. External callback effects
/// cannot be rolled back; a fresh opening retries from the children's own state.
///
/// Use ConfiguredMoveProvider with a finite draw limit and map each candidate
/// with `candidate.movement().clone()` (or use ListValueChange::into_move
/// when consuming the source directly). Returned candidates retain the selected
/// PRE-removal metadata and normalize their actual move to POST-removal indices.
/// Destination streams are independent; per-value factories/mimic replay are not
/// implied by this pairing contract.
pub struct ListValueChangeSource<S, D> {
    values: S,
    destinations: D,
    limits: ListChangeSourceLimits,
    include_noops: bool,
}
impl<S, D> ListValueChangeSource<S, D> {
    pub fn new(
        values: S,
        destinations: D,
        limits: ListChangeSourceLimits,
        include_noops: bool,
    ) -> Self {
        Self {
            values,
            destinations,
            limits,
            include_noops,
        }
    }
    pub fn values(&self) -> &S {
        &self.values
    }
    pub fn values_mut(&mut self) -> &mut S {
        &mut self.values
    }
    pub fn destinations(&self) -> &D {
        &self.destinations
    }
    pub fn destinations_mut(&mut self) -> &mut D {
        &mut self.destinations
    }
}
impl<
    V: Clone + Eq,
    C: ListSelectionView<V>,
    S: SelectionSource<C, V>,
    D: SelectionSource<C, Option<ListPosition>>,
> SelectionSource<C, ListValueChange<V>> for ListValueChangeSource<S, D>
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.values.event(event, context);
        self.destinations.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        let values = self.values.size(context);
        let destinations = self.destinations.size(context);
        (values == Some(0) || destinations == Some(0)).then_some(0)
    }
    fn is_never_ending(&self) -> bool {
        self.values.is_never_ending() && self.destinations.is_never_ending()
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = ListValueChange<V>> + 'a>
    where
        ListValueChange<V>: 'a,
    {
        let values = &mut self.values;
        let destinations = &mut self.destinations;
        let limits = self.limits;
        let include_noops = self.include_noops;
        Box::new(
            std::iter::once_with(move || {
                let selection = ValueChangeSelection::new(context.problem(), context.assignment())
                    .expect("list change source requires a valid assignment");
                let mut values = values.open(context);
                let mut destinations = destinations.open(context);
                let mut ended = false;
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    // Arm before invoking user code: unwinding must not resume a
                    // half-consumed pair after the caller catches the panic.
                    ended = true;
                    for _ in 0..limits.pairs_per_item.get() {
                        let mut selected = None;
                        for _ in 0..limits.reads_per_side.get() {
                            let value = values.next()?;
                            let id = selection
                                .problem
                                .value_index(&value)
                                .expect("list change source requires registered values");
                            if selection.locations[id].is_none_or(|p| selection.movable(p)) {
                                selected = Some(id);
                                break;
                            }
                        }
                        let id = selected?;
                        let mut selected = None;
                        for _ in 0..limits.reads_per_side.get() {
                            let destination = destinations.next()?;
                            if selection
                                .destination_selectable(destination)
                                .expect("list change source requires valid destinations")
                            {
                                selected = Some(destination);
                                break;
                            }
                        }
                        let destination = selected?;
                        if let Some(candidate) = selection.candidate(id, destination, include_noops)
                        {
                            ended = false;
                            return Some(candidate);
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
