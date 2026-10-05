//! Direct random access to list spans, without materializing a quadratic catalog.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, ListRange, SubListChangeConfig,
    SubListSize, random::SplitMix64,
};
use std::iter::FusedIterator;

#[derive(Clone, Debug)]
struct SpanBlock {
    entity: usize,
    prefix: usize,
    longest: usize,
    lengths: usize,
    first_width: u128,
    before: u128,
    through: u128,
}
impl SpanBlock {
    fn count_lengths(&self, lengths: usize) -> u128 {
        let k = lengths as u128;
        k * (2 * self.first_width + k - 1) / 2
    }
}

/// An immutable coordinate index of the movable spans in one assignment.
/// Ranks follow entity order, then descending length, then ascending start.
/// Whole pins and pinned prefixes are excluded; unassigned values have no span.
/// Every eligible span gets one rank, independent of its length or entity.
///
/// Only per-entity metadata is retained. After ordinary model validation, building
/// the index takes O(entities) space/time and rank lookup takes O(log entities +
/// log list length). Counts use u128; no list values are cloned. Coordinates remain
/// tied to the original snapshot: build a new index after changing an assignment.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])])?;
/// let spans = SubListSelection::new(&p, &p.initial_assignment(), SubListSize::new(1, 3)?)?;
/// assert_eq!(spans.len(), 6);
/// assert_eq!(spans.at(0), Some(ListRange { entity: 0, start: 0, end: 3 }));
/// assert_eq!(spans.random_iter(42).take(100).count(), 100);
/// # Ok::<(), ListError>(())
/// ```
#[derive(Clone, Debug)]
pub struct SubListSelection {
    blocks: Vec<SpanBlock>,
    total: u128,
}
impl SubListSelection {
    pub fn new<V: Eq + Clone>(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        size: SubListSize,
    ) -> Result<Self, ListError> {
        problem.validate(assignment)?;
        let mut blocks = Vec::new();
        let mut total = 0;
        for (entity, metadata) in problem.entities().iter().enumerate() {
            if metadata.is_pinned() {
                continue;
            }
            let prefix = metadata.pinned_prefix();
            let available = assignment.lists()[entity].len() - prefix;
            let longest = available.min(size.maximum());
            if longest < size.minimum() {
                continue;
            }
            let mut block = SpanBlock {
                entity,
                prefix,
                longest,
                lengths: longest - size.minimum() + 1,
                first_width: (available - longest + 1) as u128,
                before: total,
                through: 0,
            };
            total += block.count_lengths(block.lengths);
            block.through = total;
            blocks.push(block);
        }
        Ok(Self { blocks, total })
    }
    pub fn len(&self) -> u128 {
        self.total
    }
    pub fn is_empty(&self) -> bool {
        self.total == 0
    }
    /// Return a span by rank; out-of-range ranks have no span.
    pub fn at(&self, rank: u128) -> Option<ListRange> {
        let block = self
            .blocks
            .get(self.blocks.partition_point(|b| b.through <= rank))?;
        let offset = rank - block.before;
        let (mut lo, mut hi) = (0, block.lengths);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if block.count_lengths(mid + 1) <= offset {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let length = block.longest - lo;
        let start = block.prefix + (offset - block.count_lengths(lo)) as usize;
        Some(ListRange {
            entity: block.entity,
            start,
            end: start + length,
        })
    }
    /// Uniform draws with replacement. Nonempty selections repeat indefinitely;
    /// empty selections immediately exhaust. Consumers must bound repeated draws.
    /// Seeds reproduce native traces, not Java PRNG traces.
    pub fn random_iter(&self, seed: u64) -> impl FusedIterator<Item = ListRange> + '_ {
        let mut random = SplitMix64::new(seed);
        self.random_iter_with(move |bound| random.below_wide(bound))
    }
    /// Inject rank draws for replay or another random source. The callback receives
    /// the positive exclusive bound, and is never called for an empty selection.
    /// Panics if the callback returns a rank outside 0..bound.
    pub fn random_iter_with<'a>(
        &'a self,
        mut draw: impl FnMut(u128) -> u128 + 'a,
    ) -> impl FusedIterator<Item = ListRange> + 'a {
        std::iter::from_fn(move || {
            if self.is_empty() {
                return None;
            }
            let rank = draw(self.total);
            assert!(
                rank < self.total,
                "sublist draw must be less than its bound"
            );
            self.at(rank)
        })
        .fuse()
    }
}

/// Sample relocation/reversal/unassignment proposals for one immutable snapshot.
/// A draw chooses a span uniformly, then a post-removal insertion slot uniformly
/// across movable entities (plus one unassignment slot when enabled), then a
/// reversal bit when enabled. Incompatible receiving ranges and no-ops consume an
/// attempt but yield nothing. Duplicates are retained. This is not uniform over
/// legal moves: different spans can have different destination counts.
///
/// At most `attempts` proposals are considered, so an impossible neighborhood
/// terminates. Source indexing is eager, candidate drawing is lazy. Per attempt,
/// destination lookup scans entities and range checks scan only the chosen span.
/// No complete move catalog or cloned assignment is built. Reopen per committed
/// step and advance the seed when using this through a MoveProvider.
pub fn sampled_sublist_change_moves<'a, V: Eq + Clone>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    config: SubListChangeConfig,
    seed: u64,
    attempts: u64,
) -> Result<impl FusedIterator<Item = ListMove<V>> + 'a, ListError> {
    config.validate(problem)?;
    let selection = SubListSelection::new(problem, assignment, config.size())?;
    let unassignment = config.unassignment().unwrap_or(problem.allows_unassigned());
    let mut random = SplitMix64::new(seed);
    let attempts = if selection.is_empty() { 0 } else { attempts };
    Ok((0..attempts).filter_map(move |_| {
        let span = selection.at(random.below_wide(selection.len())).unwrap();
        let length = span.end - span.start;
        let width = |entity: usize| {
            let metadata = &problem.entities()[entity];
            if metadata.is_pinned() {
                0
            } else {
                let available = assignment.lists()[entity].len() - metadata.pinned_prefix();
                (available - if entity == span.entity { length } else { 0 }) as u128 + 1
            }
        };
        let slots: u128 = (0..problem.entities().len()).map(width).sum();
        let mut rank = random.below_wide(slots + u128::from(unassignment));
        if rank == slots {
            return Some(ListMove::Unassign {
                entity: span.entity,
                start: span.start,
                end: span.end,
            });
        }
        let entity = (0..problem.entities().len())
            .find(|&entity| {
                let count = width(entity);
                if rank < count {
                    true
                } else {
                    rank -= count;
                    false
                }
            })
            .expect("the movable source has an insertion slot");
        let index = problem.entities()[entity].pinned_prefix() + rank as usize;
        let reverse = config.reversing() && random.below(2) != 0;
        if (!reverse && entity == span.entity && index == span.start)
            || !assignment.lists()[span.entity][span.start..span.end]
                .iter()
                .all(|v| problem.accepts(entity, v))
        {
            return None;
        }
        Some(ListMove::SubListChange {
            entity: span.entity,
            start: span.start,
            end: span.end,
            destination: ListPosition { entity, index },
            reverse,
        })
    }))
}

/// Sample exchanges of two spans, with independent side-length bounds.
/// Each side is drawn uniformly from its eligible span index. Overlap and receiving
/// range violations consume an attempt without yielding a move. If both sides fit
/// both length bounds, their order is canonicalized; otherwise the asymmetric side
/// assignment is retained. Reversal is a fair bit when enabled and at least one
/// side has length greater than one. Duplicates across attempts are retained.
///
/// Sampling is lazy and bounded by `attempts`, including rejected pairs. Build a
/// fresh iterator for each committed assignment. Pins, ranges, same-list exchanges
/// and unequal lengths use the ordinary native ListMove application semantics.
pub fn sampled_sublist_swap_moves<'a, V: Eq + Clone>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    config: crate::SubListSwapConfig,
    seed: u64,
    attempts: u64,
) -> Result<impl FusedIterator<Item = ListMove<V>> + 'a, ListError> {
    let left = SubListSelection::new(problem, assignment, config.left_size())?;
    let right = SubListSelection::new(problem, assignment, config.right_size())?;
    let mut random = SplitMix64::new(seed);
    let attempts = if left.is_empty() || right.is_empty() {
        0
    } else {
        attempts
    };
    Ok((0..attempts).filter_map(move |_| {
        let mut l = left.at(random.below_wide(left.len())).unwrap();
        let mut r = right.at(random.below_wide(right.len())).unwrap();
        if (l.entity == r.entity && l.start < r.end && r.start < l.end)
            || !assignment.lists()[l.entity][l.start..l.end]
                .iter()
                .all(|v| problem.accepts(r.entity, v))
            || !assignment.lists()[r.entity][r.start..r.end]
                .iter()
                .all(|v| problem.accepts(l.entity, v))
        {
            return None;
        }
        let contains = |size: SubListSize, n| (size.minimum()..=size.maximum()).contains(&n);
        if contains(config.left_size(), r.end - r.start)
            && contains(config.right_size(), l.end - l.start)
            && (l.entity, l.start) > (r.entity, r.start)
        {
            std::mem::swap(&mut l, &mut r);
        }
        let reverse = config.reversing()
            && (l.end - l.start > 1 || r.end - r.start > 1)
            && random.below(2) != 0;
        Some(ListMove::SubListSwap {
            left: l,
            right: r,
            reverse,
        })
    }))
}
