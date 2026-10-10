//! Exact occurrence-weighted selection from borrowed Cartesian blocks.
use crate::{IndexedCandidate, MoveProvider, random::SplitMix64, unique_indices};
use std::{fmt, iter::FusedIterator};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PairCountOverflow;
impl fmt::Display for PairCountOverflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("candidate pair count exceeds u128")
    }
}
impl std::error::Error for PairCountOverflow {}
struct GroupBlock<'a, L> {
    left: &'a [L],
    start: usize,
    stop: usize,
    end: u128,
}
struct RightGroup<'a, R> {
    entries: &'a [R],
    end: u128,
}
struct Block<'a, L, R> {
    left: &'a [L],
    right: &'a [R],
    end: u128,
}

/// A union of borrowed Cartesian blocks. Each block contributes
/// `left.len() * right.len()` pair occurrences. No pairs or payload copies are
/// materialized; metadata is proportional to nonempty blocks. Repeated or
/// overlapping blocks deliberately give repeated pairs additional occurrences.
/// Indexed joins construct disjoint blocks instead. Ordered joins share one
/// right-group table and retain one block per matching left key, instead of one
/// block for every matching key pair. Their rank order remains left key, right
/// key, left occurrence, right occurrence; equality and explicit products keep
/// their original order. A grouped lookup uses two binary searches.
///
/// Rank lookup is logarithmic in block and shared-group counts. Integer ranks give every
/// pair occurrence the same probability, even with uneven group sizes. Unique
/// sampling uses sparse rank remapping proportional to draws, not total pairs.
/// Source mutation is prevented by borrowing. Counts may exceed usize::MAX.
///
/// ```
/// use timetuck::CandidatePairs;
/// let mut pairs = CandidatePairs::new();
/// pairs.push_product(&["a", "b"], &[1, 2, 3])?;
/// pairs.push_product(&["c"], &[4])?;
/// assert_eq!(pairs.len(), 7);
/// assert_eq!(pairs.unique(0).count(), 7);
/// assert_eq!(pairs.repeating(0).take(100).count(), 100);
/// # Ok::<(), timetuck::PairCountOverflow>(())
/// ```
pub struct CandidatePairs<'a, L, R> {
    blocks: Vec<Block<'a, L, R>>,
    grouped: Vec<GroupBlock<'a, L>>,
    grouped_length: u128,
    right_groups: Vec<RightGroup<'a, R>>,
    length: u128,
}
impl<L, R> Default for CandidatePairs<'_, L, R> {
    fn default() -> Self {
        Self {
            blocks: vec![],
            grouped: vec![],
            grouped_length: 0,
            right_groups: vec![],
            length: 0,
        }
    }
}
impl<'a, L, R> CandidatePairs<'a, L, R> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> u128 {
        self.length
    }
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub fn block_count(&self) -> usize {
        self.blocks.len() + self.grouped.len()
    }
    /// Overflow leaves the view unchanged. Empty products allocate no metadata.
    pub fn push_product(&mut self, left: &'a [L], right: &'a [R]) -> Result<(), PairCountOverflow> {
        let size = (left.len() as u128)
            .checked_mul(right.len() as u128)
            .ok_or(PairCountOverflow)?;
        if size == 0 {
            return Ok(());
        }
        let end = self.length.checked_add(size).ok_or(PairCountOverflow)?;
        self.blocks.push(Block { left, right, end });
        self.length = end;
        Ok(())
    }
    /// Build the shared table once. Empty groups are retained so caller key
    /// positions remain valid; they never contribute rank space.
    pub(crate) fn with_right_groups(
        groups: impl IntoIterator<Item = &'a [R]>,
    ) -> Result<Self, PairCountOverflow> {
        let mut out = Self::new();
        let mut end = 0_u128;
        for entries in groups {
            end = end
                .checked_add(entries.len() as u128)
                .ok_or(PairCountOverflow)?;
            out.right_groups.push(RightGroup { entries, end });
        }
        Ok(out)
    }
    fn right_prefix(&self, index: usize) -> u128 {
        if index == 0 {
            0
        } else {
            self.right_groups[index - 1].end
        }
    }
    pub(crate) fn push_group_product(
        &mut self,
        left: &'a [L],
        start: usize,
        end: usize,
    ) -> Result<(), PairCountOverflow> {
        // Grouped blocks are built first; only public direct products can be
        // appended once the joined view is returned to a caller.
        debug_assert!(self.blocks.is_empty());
        debug_assert!(start <= end && end <= self.right_groups.len());
        let width = self.right_prefix(end) - self.right_prefix(start);
        let size = (left.len() as u128)
            .checked_mul(width)
            .ok_or(PairCountOverflow)?;
        if size == 0 {
            return Ok(());
        }
        let next = self.length.checked_add(size).ok_or(PairCountOverflow)?;
        self.grouped.push(GroupBlock {
            left,
            start,
            stop: end,
            end: next,
        });
        self.length = next;
        self.grouped_length = next;
        Ok(())
    }
    #[inline]
    pub fn get(&self, rank: u128) -> Option<(&'a L, &'a R)> {
        if rank >= self.length {
            return None;
        }
        if rank < self.grouped_length {
            return Some(self.get_grouped(rank));
        }
        let index = self.blocks.partition_point(|b| b.end <= rank);
        let start = if index == 0 {
            self.grouped_length
        } else {
            self.blocks[index - 1].end
        };
        let block = &self.blocks[index];
        let local = rank - start;
        let width = block.right.len() as u128;
        Some((
            &block.left[(local / width) as usize],
            &block.right[(local % width) as usize],
        ))
    }
    fn get_grouped(&self, rank: u128) -> (&'a L, &'a R) {
        let index = self.grouped.partition_point(|b| b.end <= rank);
        let start = if index == 0 {
            0
        } else {
            self.grouped[index - 1].end
        };
        let block = &self.grouped[index];
        let local = rank - start;
        let prefix = self.right_prefix(block.start);
        // Every right occurrence contributes left.len() pair ranks. Locate its
        // key group, then restore the original group-major Cartesian order
        // within that group (not flattened row order).
        let occurrence = prefix + local / block.left.len() as u128;
        let group = block.start
            + self.right_groups[block.start..block.stop].partition_point(|g| g.end <= occurrence);
        let preceding = self.right_prefix(group) - prefix;
        let local = local - preceding * block.left.len() as u128;
        let right = self.right_groups[group].entries;
        let width = right.len() as u128;
        (
            &block.left[(local / width) as usize],
            &right[(local % width) as usize],
        )
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&'a L, &'a R)> + FusedIterator + '_ {
        (0..self.length).map(|i| self.get(i).expect("rank inside pair space"))
    }
    /// Uniform draws with explicit retirement and sparse per-traversal state.
    pub fn retiring(&self, seed: u64) -> crate::RetiringCandidatePairs<'_, 'a, L, R> {
        crate::RetiringCandidatePairs::new(self, seed)
    }
    pub fn unique(&self, seed: u64) -> impl FusedIterator<Item = (&'a L, &'a R)> + '_ {
        unique_indices(self.length, seed).map(|i| {
            self.get(i.expect("seeded draw is in bounds"))
                .expect("rank inside pair space")
        })
    }
    /// Nonempty views repeat forever; callers should impose a draw budget.
    pub fn repeating(&self, seed: u64) -> impl FusedIterator<Item = (&'a L, &'a R)> + '_ {
        let mut random = SplitMix64::new(seed);
        std::iter::from_fn(move || {
            if self.is_empty() {
                None
            } else {
                self.get(if self.length == 1 {
                    0
                } else {
                    random.below_wide(self.length)
                })
            }
        })
        .fuse()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PairSampling {
    Unique,
    Repeating,
}

/// A bounded native move provider over an immutable indexed pair view. Mapping
/// receives the current problem, assignment and step; None rejects a proposal.
/// The draw cap counts rejected pairs too, so a rejecting mapper always finishes.
/// A cap or rejection can miss productive moves; empty output is not an
/// exhaustive-neighborhood proof. Duplicate resulting moves are not deduplicated.
///
/// Seeds depend only on the configured seed and completed-step count. Reopening
/// a step repeats its draw order. The index captures keys: rebuild the provider
/// if keys depend on a changed assignment; the mapper may filter changing state.
/// Mapping is lazy, invokes no callbacks on zero budget, and requires no payload
/// Clone. Callback side effects/panics are not rolled back.
pub struct PairMoveProvider<'a, L, R, F> {
    pairs: CandidatePairs<'a, L, R>,
    sampling: PairSampling,
    seed: u64,
    draw_limit: usize,
    mapper: F,
}
impl<'a, L, R, F> PairMoveProvider<'a, L, R, F> {
    pub fn new(
        pairs: CandidatePairs<'a, L, R>,
        sampling: PairSampling,
        seed: u64,
        draw_limit: usize,
        mapper: F,
    ) -> Self {
        Self {
            pairs,
            sampling,
            seed,
            draw_limit,
            mapper,
        }
    }
    pub fn pairs(&self) -> &CandidatePairs<'a, L, R> {
        &self.pairs
    }
}
impl<P, A, M, L, R, F: FnMut(&P, &A, u64, &L, &R) -> Option<M>> MoveProvider<P, A, M>
    for PairMoveProvider<'_, L, R, F>
{
    fn moves<'s>(
        &'s mut self,
        problem: &'s P,
        assignment: &'s A,
        completed_steps: u64,
    ) -> Box<dyn Iterator<Item = M> + 's>
    where
        M: 's,
    {
        let seed = self
            .seed
            .wrapping_add(completed_steps.wrapping_mul(0x9e3779b97f4a7c15));
        let draws: Box<dyn Iterator<Item = (&L, &R)> + 's> = match self.sampling {
            PairSampling::Unique => Box::new(self.pairs.unique(seed)),
            PairSampling::Repeating => Box::new(self.pairs.repeating(seed)),
        };
        let mapper = &mut self.mapper;
        Box::new(
            draws
                .take(self.draw_limit)
                .filter_map(move |(left, right)| {
                    mapper(problem, assignment, completed_steps, left, right)
                }),
        )
    }
}

/// Pair view produced by joining two candidate indexes.
pub type IndexedCandidatePairs<'a, I, L, J, R> =
    CandidatePairs<'a, IndexedCandidate<I, L>, IndexedCandidate<J, R>>;

#[cfg(test)]
mod compact_tests {
    use super::*;
    #[test]
    fn empty_shared_groups_do_not_shift_group_major_ranks() {
        let left = [0, 1];
        let a = [10, 11];
        let b = [12];
        let mut pairs = CandidatePairs::with_right_groups([&[][..], &a, &[], &b, &[]]).unwrap();
        pairs.push_group_product(&left, 0, 5).unwrap();
        pairs.push_group_product(&left, 2, 5).unwrap();
        pairs.push_group_product(&left, 0, 1).unwrap();
        assert_eq!(pairs.block_count(), 2);
        assert_eq!(
            pairs.iter().map(|(a, b)| (*a, *b)).collect::<Vec<_>>(),
            [
                (0, 10),
                (0, 11),
                (1, 10),
                (1, 11),
                (0, 12),
                (1, 12),
                (0, 12),
                (1, 12)
            ]
        );
    }
    #[test]
    #[cfg(target_pointer_width = "64")]
    fn virtual_group_products_check_overflow_before_mutating() {
        let huge = vec![(); usize::MAX];
        let mut pairs = CandidatePairs::with_right_groups([huge.as_slice(); 3]).unwrap();
        let size = (usize::MAX as u128) * (usize::MAX as u128);
        assert_eq!(
            pairs.push_group_product(&huge, 0, 3),
            Err(PairCountOverflow)
        );
        assert!(pairs.is_empty());
        pairs.push_group_product(&huge, 1, 2).unwrap();
        assert_eq!(pairs.len(), size);
        assert!(pairs.get(size - 1).is_some());
        assert_eq!(
            pairs.push_group_product(&[(); 2], 0, 2),
            Err(PairCountOverflow)
        );
        assert_eq!(pairs.len(), size);
        assert_eq!(pairs.block_count(), 1);
    }
}
