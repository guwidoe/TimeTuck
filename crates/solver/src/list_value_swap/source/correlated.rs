use super::{ListSwapSourceLimits, ListValueSwapSource, ValueSwapSelection, next_selectable};
use crate::{ListMove, ListPosition, ListSelectionView, SelectionLifecycleEvent, SelectionSource};

/// Open partners for one canonical registered left value. Position is the cached
/// position in this immutable opening, or None for an optional missing value.
/// The cursor may borrow context or canonical left, but must be independent of
/// mutable factory storage: own or share any factory data it needs. It survives
/// successive outputs until exhausted or its rejected-read budget is reached.
/// Lifecycle defaults to no action; owned-iterator closures implement this trait.
pub trait ListSwapPartnerFactory<C, V> {
    fn event(&mut self, _event: SelectionLifecycleEvent, _context: &C) {}
    fn open<'a>(
        &mut self,
        context: &'a C,
        left: &'a V,
        position: Option<ListPosition>,
    ) -> Box<dyn Iterator<Item = V> + 'a>;
}
impl<C, V, F, I> ListSwapPartnerFactory<C, V> for F
where
    F: FnMut(&C, &V, Option<ListPosition>) -> I,
    I: Iterator<Item = V> + 'static,
{
    fn open<'a>(
        &mut self,
        context: &'a C,
        left: &'a V,
        position: Option<ListPosition>,
    ) -> Box<dyn Iterator<Item = V> + 'a> {
        Box::new(self(context, left, position))
    }
}
impl<L, F> ListValueSwapSource<L, F, true> {
    /// Enumerate partners for each movable left occurrence. The left and its
    /// factory cursor persist across successful outputs until the cursor exhausts.
    /// Repeated lefts open fresh cursors and preserve occurrence multiplicity.
    /// Pinned/required-missing lefts never invoke the factory. Pinned partners,
    /// invalid receiver ranges, dependencies and disallowed no-ops are skipped.
    /// Empty or exhausted branches advance left. No lookahead follows an output.
    ///
    /// Per-output limits bound partner reads for each branch and branches visited;
    /// each new movable-left search also has its own reads_per_side bound. A branch
    /// that yields no accepted partner within its read limit is discarded before
    /// advancing left. Left exhaustion/search-limit or the overall branch limit
    /// fuses this opening. These limits bound rejected work, not successful output;
    /// a productive infinite partner cursor can starve later lefts. Use a finite
    /// provider draw limit or LimitedSelectionSource to bound successful proposals.
    ///
    /// First next prepares ONE assignment/checker index and opens the left source;
    /// no child/factory is called by open itself. Partners are never materialized.
    /// All identity, pin, range and dependency checks still apply. Unknown values
    /// and callback panics fuse when caught. Exhausted/discarded cursor destruction
    /// finishes before another branch or output; retained cursors may fail on a
    /// later poll/drop without revoking already returned moves. External callback
    /// effects cannot roll back. Reopen to capture fresh assignment positions.
    /// Events forward left then factory; size only knows an advertised empty left.
    /// Never-ending metadata is conservative (a factory can return infinite data),
    /// so this mode advertises potential unboundedness even for finite lefts.
    ///
    /// Use owned ListSelectionView snapshots with existing ConfiguredMoveProvider.
    /// No Java mimic, random-trace or singleton NoChange identity is promised.
    pub fn correlated(
        left: L,
        factory: F,
        limits: ListSwapSourceLimits,
        include_noops: bool,
    ) -> Self {
        Self {
            left,
            right: factory,
            limits,
            include_noops,
        }
    }
}
impl<
    V: Clone + Eq,
    C: ListSelectionView<V>,
    L: SelectionSource<C, V>,
    F: ListSwapPartnerFactory<C, V>,
> SelectionSource<C, ListMove<V>> for ListValueSwapSource<L, F, true>
{
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.left.event(event, context);
        self.right.event(event, context);
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        (self.left.size(context) == Some(0)).then_some(0)
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        let left = &mut self.left;
        let factory = &mut self.right;
        let limits = self.limits;
        let include_noops = self.include_noops;
        Box::new(
            std::iter::once_with(move || {
                let mut selection =
                    ValueSwapSelection::new(context.problem(), context.assignment())
                        .expect("correlated swap source requires valid assignment");
                let mut left = left.open(context);
                let mut ended = false;
                let mut branch = None;
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    ended = true;
                    for _ in 0..limits.pairs_per_item.get() {
                        if branch.is_none() {
                            let l = next_selectable(
                                &selection,
                                &mut *left,
                                limits.reads_per_side.get(),
                            )?;
                            let partners = factory.open(
                                context,
                                &selection.problem.values()[l],
                                selection.locations[l],
                            );
                            branch = Some((l, partners));
                        }
                        let (l, partners) = branch.as_mut().unwrap();
                        for _ in 0..limits.reads_per_side.get() {
                            let Some(value) = partners.next() else {
                                break;
                            };
                            let r = selection
                                .problem
                                .value_index(&value)
                                .expect("correlated swap source requires registered partners");
                            // Owned input destruction is user code too; it must finish
                            // while panic fusion is armed, before publishing a move.
                            drop(value);
                            if !selection.selectable[r] {
                                continue;
                            }
                            if let Some(movement) = selection.movement(*l, r, include_noops) {
                                ended = false;
                                return Some(movement);
                            }
                        }
                        // Remove before dropping: a panicking user destructor cannot
                        // leave a reusable branch or publish the next branch's output.
                        drop(branch.take());
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
