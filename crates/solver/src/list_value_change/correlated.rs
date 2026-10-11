use super::{ListSelectionView, ListValueChange, ValueChangeSelection};
use crate::{ListDestinationIndex, ListPosition, SelectionLifecycleEvent, SelectionSource};
use std::num::NonZeroUsize;

/// Opens destinations for one canonical registered value occurrence. The cursor
/// may borrow the factory, immutable context, value or shared destination index.
/// It is dropped before yielding a candidate or trying the next value. Lifecycle
/// events default to no action. Closures returning owned cursors implement this
/// trait directly; use a custom implementation for borrowed cursors.
pub trait ListDestinationFactory<C, V> {
    fn event(&mut self, _event: SelectionLifecycleEvent, _context: &C) {}
    fn open<'a>(
        &'a mut self,
        context: &'a C,
        value: &'a V,
        destinations: &'a ListDestinationIndex<'_, V>,
    ) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a>;
}
impl<C, V, F, I> ListDestinationFactory<C, V> for F
where
    F: FnMut(&C, &V, &ListDestinationIndex<'_, V>) -> I,
    I: Iterator<Item = Option<ListPosition>> + 'static,
{
    fn open<'a>(
        &'a mut self,
        context: &'a C,
        value: &'a V,
        destinations: &'a ListDestinationIndex<'_, V>,
    ) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a> {
        Box::new(self(context, value, destinations))
    }
}

/// Per-output work bounds, including successful and exhausted reads. At most
/// values_per_item raw values are read and each movable occurrence opens one
/// cursor with at most destinations_per_value reads. Reaching the value limit
/// without output fuses the opening. Successful output resets both budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CorrelatedListChangeLimits {
    pub values_per_item: NonZeroUsize,
    pub destinations_per_value: NonZeroUsize,
}

/// Lazy correlated list changes with at most ONE accepted destination per value
/// occurrence. Repeat values deliberately to request further destinations. Every
/// movable occurrence opens a fresh factory cursor, including duplicate values;
/// pinned values never invoke it. Empty, exhausted or rejected destination branches
/// advance to the next value. Unused destinations are dropped after acceptance.
///
/// First next() validates the assignment, prepares one shared destination index,
/// and opens the value child. The index follows ListDestinationIndex's model and
/// range costs; it does not materialize live source cursors. Factories can use its
/// for_value selection or provide their own PRE-removal positions. All proposals
/// still pass native pin, range and dependency checks; actual moves normalize
/// same-list indices to POST-removal positions. include_noops only controls valid
/// unchanged proposals. Reopen with a new immutable context after committed edits.
///
/// Each next() performs at most values_per_item value reads and that many times
/// destinations_per_value destination reads, plus index/identity/validation work.
/// User callbacks must return cooperatively. Exhaustion, work-limit exhaustion,
/// malformed metadata or a caught callback panic fuses this opening. External
/// callback side effects cannot be rolled back. Events forward values then factory.
/// Size is unknown except an advertised empty value source. Never-ending metadata
/// describes the value child's potential unboundedness, not guaranteed output.
///
/// ConfiguredMoveProvider can capture an owned ListSelectionView and map candidates
/// via movement().clone(); its draw limit bounds successful proposals as usual.
/// This native contract does not promise Java mimic lifecycle or random traces.
pub struct CorrelatedListValueChangeSource<S, F> {
    values: S,
    factory: F,
    limits: CorrelatedListChangeLimits,
    include_noops: bool,
}
impl<S, F> CorrelatedListValueChangeSource<S, F> {
    pub fn new(
        values: S,
        factory: F,
        limits: CorrelatedListChangeLimits,
        include_noops: bool,
    ) -> Self {
        Self {
            values,
            factory,
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
    pub fn factory(&self) -> &F {
        &self.factory
    }
    pub fn factory_mut(&mut self) -> &mut F {
        &mut self.factory
    }
}
impl<V, C, S, F> SelectionSource<C, ListValueChange<V>> for CorrelatedListValueChangeSource<S, F>
where
    V: Clone + Eq,
    C: ListSelectionView<V>,
    S: SelectionSource<C, V>,
    F: ListDestinationFactory<C, V>,
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.values.event(event, context);
        self.factory.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        (self.values.size(context) == Some(0)).then_some(0)
    }
    fn is_never_ending(&self) -> bool {
        self.values.is_never_ending()
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = ListValueChange<V>> + 'a>
    where
        ListValueChange<V>: 'a,
    {
        let values = &mut self.values;
        let factory = &mut self.factory;
        let limits = self.limits;
        let include_noops = self.include_noops;
        Box::new(
            std::iter::once_with(move || {
                let selection = ValueChangeSelection::new(context.problem(), context.assignment())
                    .expect("correlated list change source requires a valid assignment");
                let index = ListDestinationIndex::new(
                    context.problem(),
                    context.assignment(),
                    context.problem().allows_unassigned(),
                )
                .expect("correlated list change source requires a valid destination index");
                let mut values = values.open(context);
                let mut ended = false;
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    ended = true;
                    for _ in 0..limits.values_per_item.get() {
                        let value = values.next()?;
                        let id = selection
                            .problem
                            .value_index(&value)
                            .expect("correlated list change source requires registered values");
                        if selection.locations[id].is_some_and(|p| !selection.movable(p)) {
                            continue;
                        }
                        let mut destinations =
                            factory.open(context, &selection.problem.values()[id], &index);
                        for _ in 0..limits.destinations_per_value.get() {
                            let Some(destination) = destinations.next() else {
                                break;
                            };
                            if !selection
                                .destination_selectable(destination)
                                .expect("correlated list change source requires valid destinations")
                            {
                                continue;
                            }
                            if let Some(candidate) =
                                selection.candidate(id, destination, include_noops)
                            {
                                // User cursor destruction may unwind too. Publish only
                                // after it completes while the opening is still armed.
                                drop(destinations);
                                ended = false;
                                return Some(candidate);
                            }
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
