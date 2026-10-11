use super::{
    SubListChangeProposal, SubListChangeSource, SubListProposalDraw, draw_sublist_proposal_rank,
    seeded_sublist_proposal_draw,
};
use crate::{
    ListMove, ListPosition, ListRange, ListSelectionView, SelectionLifecycleEvent, SelectionSource,
};
use std::num::NonZeroUsize;

/// Per-output rejection bounds, including successful reads and exhaustion probes.
/// Each visited span branch may search at most reads_per_side source spans and
/// read at most reads_per_side destinations. Successful output resets budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubListChangeSourceLimits {
    pub reads_per_side: NonZeroUsize,
    pub spans_per_item: NonZeroUsize,
}

/// Open PRE-removal destinations for one nonempty movable span in the immutable
/// context. The cursor may borrow context, but cannot borrow mutable factory
/// storage. It persists across outputs; own/share any retained factory data.
/// None requests optional unassignment. Lifecycle defaults to no action.
pub trait SubListDestinationFactory<C> {
    fn event(&mut self, _event: SelectionLifecycleEvent, _context: &C) {}
    fn open<'a>(
        &mut self,
        context: &'a C,
        span: ListRange,
    ) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a>;
}
impl<C, F, I> SubListDestinationFactory<C> for F
where
    F: FnMut(&C, ListRange) -> I,
    I: Iterator<Item = Option<ListPosition>> + 'static,
{
    fn open<'a>(
        &mut self,
        context: &'a C,
        span: ListRange,
    ) -> Box<dyn Iterator<Item = Option<ListPosition>> + 'a> {
        Box::new(self(context, span))
    }
}
impl<L, F> SubListChangeSource<L, F, (SubListChangeSourceLimits, SubListProposalDraw), true> {
    /// Retain each movable span while lazily enumerating its dependent destinations.
    /// Unlike independent raw proposals, this mode yields only valid, changing
    /// ListMoves, through one shared checker per opening. Invalid/empty/pinned
    /// source spans are skipped before the factory; invalid destinations, no-ops,
    /// receiver-range/dependency violations and required unassignment are skipped.
    /// PRE-removal positions are normalized exactly once through the proposal.
    ///
    /// Each next visits at most spans_per_item branches, with reads_per_side for
    /// each new source search and each branch's destinations. Empty, exhausted or
    /// wholly rejected destination branches advance to the next span. Exhausted
    /// source/search/branch budgets fuse the opening. Productive infinite branches
    /// can starve later spans; bound successful outputs with a provider draw limit.
    ///
    /// open is lazy; first next validates the snapshot and opens the span child.
    /// Reversal consumes one bound-2 draw for every read Some destination, even
    /// if later rejected; None consumes no draw. Random state continues on reopen.
    /// No lookahead after output. Reopen captures changed rows/positions. Events
    /// forward span source then factory. Cardinality is unknown except empty left;
    /// never-ending metadata conservatively accounts for unbounded factories.
    ///
    /// Caught callback/draw/cursor-drop panics fuse the opening. Exhausted cursor
    /// destruction precedes advancing; retained cursors can fail later without
    /// revoking prior outputs. External callback effects cannot be rolled back.
    /// Owned ListSelectionView contexts work through ConfiguredMoveProvider.
    pub fn correlated(
        left: L,
        factory: F,
        limits: SubListChangeSourceLimits,
        reversing: bool,
        seed: u64,
    ) -> Self {
        Self::correlated_with_draw(
            left,
            factory,
            limits,
            reversing,
            seeded_sublist_proposal_draw(seed),
        )
    }
}
impl<L, F, D> SubListChangeSource<L, F, (SubListChangeSourceLimits, D), true> {
    /// Correlated selection with injected rank draws; see correlated for ordering.
    pub fn correlated_with_draw(
        left: L,
        factory: F,
        limits: SubListChangeSourceLimits,
        reversing: bool,
        draw: D,
    ) -> Self {
        Self {
            left,
            right: factory,
            reversing,
            draw: (limits, draw),
        }
    }
    pub fn left(&self) -> &L {
        &self.left
    }
    pub fn left_mut(&mut self) -> &mut L {
        &mut self.left
    }
    pub fn right(&self) -> &F {
        &self.right
    }
    pub fn right_mut(&mut self) -> &mut F {
        &mut self.right
    }
    pub fn selects_reversing(&self) -> bool {
        self.reversing
    }
}
impl<
    V: Eq + Clone,
    C: ListSelectionView<V>,
    L: SelectionSource<C, ListRange>,
    F: SubListDestinationFactory<C>,
    D: FnMut(u128) -> u128,
> SelectionSource<C, ListMove<V>>
    for SubListChangeSource<L, F, (SubListChangeSourceLimits, D), true>
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
        let (left, factory, (limits, draw), reversing) = (
            &mut self.left,
            &mut self.right,
            &mut self.draw,
            self.reversing,
        );
        let limits = *limits;
        Box::new(
            std::iter::once_with(move || {
                let problem = context.problem();
                let assignment = context.assignment();
                let checker = problem
                    .move_checker(assignment)
                    .expect("correlated sublist source requires valid assignment");
                let mut left = left.open(context);
                let mut branch = None;
                let mut ended = false;
                std::iter::from_fn(move || {
                    if ended {
                        return None;
                    }
                    ended = true;
                    for _ in 0..limits.spans_per_item.get() {
                        if branch.is_none() {
                            let span =
                                left.by_ref().take(limits.reads_per_side.get()).find(|s| {
                                    problem.entities().get(s.entity).is_some_and(|meta| {
                                        !meta.is_pinned()
                                            && s.start >= meta.pinned_prefix()
                                            && s.start < s.end
                                            && s.end <= assignment.lists()[s.entity].len()
                                    })
                                })?;
                            branch = Some((span, factory.open(context, span)));
                        }
                        let (span, destinations) = branch.as_mut().unwrap();
                        for _ in 0..limits.reads_per_side.get() {
                            let Some(destination) = destinations.next() else {
                                break;
                            };
                            let reverse = reversing
                                && destination.is_some()
                                && draw_sublist_proposal_rank(draw, 2) != 0;
                            if destination.is_none() && !problem.allows_unassigned() {
                                continue;
                            }
                            let Some(movement) = (SubListChangeProposal {
                                source: *span,
                                destination,
                                reverse,
                            })
                            .into_move() else {
                                continue;
                            };
                            if checker.is_doable(&movement).unwrap_or(false) {
                                ended = false;
                                return Some(movement);
                            }
                        }
                        // User destructor must finish with fusion armed before another output.
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
