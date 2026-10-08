//! Composable sublist proposals from independently selected spans and destinations.
use crate::{
    ListMove, ListPosition, ListRange, ListSelectionContext, SelectionLifecycleEvent,
    SelectionSource, SubListSelection, SubListSize, SubListSource,
};

/// Number of span coordinates in the model catalog, independent of how often a
/// source repeats them or restricts its owner sequence. This is not traversal length.
pub trait SubListCardinality<C>: SelectionSource<C, ListRange> {
    fn span_count(&mut self, context: &C) -> u128;
}
impl<V: Eq + Clone> SubListCardinality<ListSelectionContext<'_, V>> for SubListSource {
    fn span_count(&mut self, c: &ListSelectionContext<'_, V>) -> u128 {
        SubListSelection::new(c.problem(), c.assignment(), self.bounds())
            .expect("validated list context")
            .len()
    }
}

type SubListProposalDraw = Box<dyn FnMut(u128) -> u128>;
fn seeded_sublist_proposal_draw(seed: u64) -> SubListProposalDraw {
    let mut random = crate::random::SplitMix64::new(seed);
    Box::new(move |bound| random.below_wide(bound))
}
fn draw_sublist_proposal_rank(draw: &mut impl FnMut(u128) -> u128, bound: u128) -> u128 {
    let value = draw(bound);
    assert!(value < bound, "sublist draw must be less than its bound");
    value
}

/// SubListProposalDraw a span within each selected entity. Missing, pinned, empty and too-short
/// entities consume reads but no randomness. The read budget bounds the entire
/// opening, including rejections; an empty span catalog never opens the child.
/// Rebuilds its coordinate index on each opening, so committed changes are visible.
/// Lifecycle events pass to the entity source once. Random state continues across
/// openings; construct a new source to restart a seed. No list payloads are cloned.
pub struct EntitySubListSource<E, D = SubListProposalDraw> {
    entities: E,
    bounds: SubListSize,
    maximum_reads: usize,
    draw: D,
}
impl<E> EntitySubListSource<E> {
    pub fn new(entities: E, bounds: SubListSize, maximum_reads: usize, seed: u64) -> Self {
        Self::with_draw(
            entities,
            bounds,
            maximum_reads,
            seeded_sublist_proposal_draw(seed),
        )
    }
}
impl<E, D> EntitySubListSource<E, D> {
    pub fn with_draw(entities: E, bounds: SubListSize, maximum_reads: usize, draw: D) -> Self {
        Self {
            entities,
            bounds,
            maximum_reads,
            draw,
        }
    }
    pub fn entities(&self) -> &E {
        &self.entities
    }
    pub fn entities_mut(&mut self) -> &mut E {
        &mut self.entities
    }
}
impl<
    'p,
    V: Eq + Clone,
    E: SelectionSource<ListSelectionContext<'p, V>, usize>,
    D: FnMut(u128) -> u128,
> SubListCardinality<ListSelectionContext<'p, V>> for EntitySubListSource<E, D>
{
    fn span_count(&mut self, c: &ListSelectionContext<'p, V>) -> u128 {
        SubListSelection::new(c.problem(), c.assignment(), self.bounds)
            .expect("validated list context")
            .len()
    }
}
impl<
    'p,
    V: Eq + Clone,
    E: SelectionSource<ListSelectionContext<'p, V>, usize>,
    D: FnMut(u128) -> u128,
> SelectionSource<ListSelectionContext<'p, V>, ListRange> for EntitySubListSource<E, D>
{
    fn event(&mut self, event: SelectionLifecycleEvent, c: &ListSelectionContext<'p, V>) {
        self.entities.event(event, c);
    }
    fn is_never_ending(&self) -> bool {
        false
    } // Every opening has a read budget.
    fn size(&mut self, c: &ListSelectionContext<'p, V>) -> Option<usize> {
        if self.maximum_reads == 0 || self.span_count(c) == 0 {
            Some(0)
        } else {
            None
        }
    }
    fn open<'a>(
        &'a mut self,
        c: &'a ListSelectionContext<'p, V>,
    ) -> Box<dyn Iterator<Item = ListRange> + 'a>
    where
        ListRange: 'a,
    {
        let index = SubListSelection::new(c.problem(), c.assignment(), self.bounds)
            .expect("validated list context");
        if index.is_empty() || self.maximum_reads == 0 {
            return Box::new(std::iter::empty());
        }
        let mut entities = self.entities.open(c).take(self.maximum_reads);
        let draw = &mut self.draw;
        Box::new(
            std::iter::from_fn(move || {
                entities.by_ref().find_map(|entity| {
                    let n = index.entity_len(entity);
                    if n == 0 {
                        None
                    } else {
                        index.at_entity(entity, draw_sublist_proposal_rank(draw, n))
                    }
                })
            })
            .fuse(),
        )
    }
}

/// Raw snapshot proposal, retaining the selected PRE-removal destination. It may
/// be non-doable (pins, receiver ranges, dependencies, optional unassignment).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubListChangeProposal {
    pub source: ListRange,
    pub destination: Option<ListPosition>,
    pub reverse: bool,
}
impl SubListChangeProposal {
    /// Convert to native POST-removal coordinates. An insertion strictly inside
    /// the removed span has no native relocation and returns None. Boundary
    /// destinations both map to its original start (possibly reversing in place).
    /// The problem must still validate the resulting move before scoring it.
    pub fn into_move<V>(self) -> Option<ListMove<V>> {
        let s = self.source;
        let length = s.end.checked_sub(s.start).filter(|n| *n > 0)?;
        Some(match self.destination {
            None => ListMove::Unassign {
                entity: s.entity,
                start: s.start,
                end: s.end,
            },
            Some(mut destination) => {
                if destination.entity == s.entity {
                    if destination.index > s.start && destination.index < s.end {
                        return None;
                    }
                    if destination.index >= s.end {
                        destination.index -= length;
                    }
                }
                ListMove::SubListChange {
                    entity: s.entity,
                    start: s.start,
                    end: s.end,
                    destination,
                    reverse: self.reverse,
                }
            }
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubListSwapProposal {
    pub left: ListRange,
    pub right: ListRange,
    pub reverse: bool,
}
impl SubListSwapProposal {
    /// Retains overlaps and no-ops; native move validation decides doability.
    pub fn into_move<V>(self) -> ListMove<V> {
        ListMove::SubListSwap {
            left: self.left,
            right: self.right,
            reverse: self.reverse,
        }
    }
}

/// Pair selected spans and PRE-removal destinations lazily. Exhausting either
/// child fuses the cursor. Unassignment consumes no reversal draw. Sources receive
/// lifecycle events left then right; callbacks and their effects are not rolled back.
/// proposal_count is the Cartesian catalog size, not traversal length, and includes
/// invalid/no-op pairs. size is only exact for empty catalogs; other lengths are
/// unknown because child selection can be truncated or filtered.
pub struct SubListChangeSource<L, R, D = SubListProposalDraw> {
    left: L,
    right: R,
    reversing: bool,
    draw: D,
}
pub struct SubListSwapSource<L, R, D = SubListProposalDraw> {
    left: L,
    right: R,
    reversing: bool,
    draw: D,
}
macro_rules! sublist_source_constructors {
    ($name:ident) => {
        impl<L, R> $name<L, R> {
            pub fn new(left: L, right: R, reversing: bool, seed: u64) -> Self {
                Self::with_draw(left, right, reversing, seeded_sublist_proposal_draw(seed))
            }
        }
        impl<L, R, D> $name<L, R, D> {
            pub fn with_draw(left: L, right: R, reversing: bool, draw: D) -> Self {
                Self {
                    left,
                    right,
                    reversing,
                    draw,
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
            pub fn selects_reversing(&self) -> bool {
                self.reversing
            }
        }
    };
}
sublist_source_constructors!(SubListChangeSource);
sublist_source_constructors!(SubListSwapSource);
impl<L, R, D> SubListChangeSource<L, R, D> {
    pub fn proposal_count<C>(&mut self, c: &C) -> Option<u128>
    where
        L: SubListCardinality<C>,
        R: SelectionSource<C, Option<ListPosition>>,
    {
        let n = self.left.span_count(c);
        if n == 0 {
            return Some(0);
        }
        n.checked_mul(self.right.size(c)? as u128)?
            .checked_mul(if self.reversing { 2 } else { 1 })
    }
}
impl<L, R, D> SubListSwapSource<L, R, D> {
    pub fn proposal_count<C>(&mut self, c: &C) -> Option<u128>
    where
        L: SubListCardinality<C>,
        R: SubListCardinality<C>,
    {
        self.left
            .span_count(c)
            .checked_mul(self.right.span_count(c))?
            .checked_mul(if self.reversing { 2 } else { 1 })
    }
}
impl<
    C,
    L: SelectionSource<C, ListRange>,
    R: SelectionSource<C, Option<ListPosition>>,
    D: FnMut(u128) -> u128,
> SelectionSource<C, SubListChangeProposal> for SubListChangeSource<L, R, D>
{
    fn event(&mut self, e: SelectionLifecycleEvent, c: &C) {
        self.left.event(e, c);
        self.right.event(e, c);
    }
    fn is_never_ending(&self) -> bool {
        self.left.is_never_ending() && self.right.is_never_ending()
    }
    fn size(&mut self, c: &C) -> Option<usize> {
        if self.left.size(c) == Some(0) || self.right.size(c) == Some(0) {
            Some(0)
        } else {
            None
        }
    }
    fn open<'a>(&'a mut self, c: &'a C) -> Box<dyn Iterator<Item = SubListChangeProposal> + 'a>
    where
        SubListChangeProposal: 'a,
    {
        let (left, right, draw, reversing) = (
            &mut self.left,
            &mut self.right,
            &mut self.draw,
            self.reversing,
        );
        Box::new(
            std::iter::once_with(move || {
                left.open(c)
                    .zip(right.open(c))
                    .map(move |(source, destination)| {
                        let reverse = reversing
                            && destination.is_some()
                            && draw_sublist_proposal_rank(draw, 2) != 0;
                        SubListChangeProposal {
                            source,
                            destination,
                            reverse,
                        }
                    })
            })
            .flatten()
            .fuse(),
        )
    }
}
impl<C, L: SelectionSource<C, ListRange>, R: SelectionSource<C, ListRange>, D: FnMut(u128) -> u128>
    SelectionSource<C, SubListSwapProposal> for SubListSwapSource<L, R, D>
{
    fn event(&mut self, e: SelectionLifecycleEvent, c: &C) {
        self.left.event(e, c);
        self.right.event(e, c);
    }
    fn is_never_ending(&self) -> bool {
        self.left.is_never_ending() && self.right.is_never_ending()
    }
    fn size(&mut self, c: &C) -> Option<usize> {
        if self.left.size(c) == Some(0) || self.right.size(c) == Some(0) {
            Some(0)
        } else {
            None
        }
    }
    fn open<'a>(&'a mut self, c: &'a C) -> Box<dyn Iterator<Item = SubListSwapProposal> + 'a>
    where
        SubListSwapProposal: 'a,
    {
        let (left, right, draw, reversing) = (
            &mut self.left,
            &mut self.right,
            &mut self.draw,
            self.reversing,
        );
        Box::new(
            std::iter::once_with(move || {
                left.open(c)
                    .zip(right.open(c))
                    .map(move |(mut left, mut right)| {
                        if left.entity == right.entity && left.start > right.start {
                            std::mem::swap(&mut left, &mut right);
                        }
                        SubListSwapProposal {
                            left,
                            right,
                            reverse: reversing && draw_sublist_proposal_rank(draw, 2) != 0,
                        }
                    })
            })
            .flatten()
            .fuse(),
        )
    }
}
