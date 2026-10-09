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
struct Block<'a, L, R> {
    left: &'a [L],
    right: &'a [R],
    end: u128,
}

/// A union of borrowed Cartesian blocks. Each block contributes
/// `left.len() * right.len()` pair occurrences. No pairs or payload copies are
/// materialized; metadata is proportional to nonempty blocks. Repeated or
/// overlapping blocks deliberately give repeated pairs additional occurrences.
/// Indexed joins construct disjoint blocks instead.
///
/// Rank lookup is logarithmic in block count. Uniform integer ranks give every
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
    length: u128,
}
impl<L, R> Default for CandidatePairs<'_, L, R> {
    fn default() -> Self {
        Self {
            blocks: vec![],
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
        self.blocks.len()
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
    pub fn get(&self, rank: u128) -> Option<(&'a L, &'a R)> {
        if rank >= self.length {
            return None;
        }
        let index = self.blocks.partition_point(|b| b.end <= rank);
        let start = if index == 0 {
            0
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
