//! Sparse retirement over indexed candidate spaces, including virtual pair sets.
use crate::{CandidatePairs, LazyConcat, LazyConcatError, NoDrawToRetire, random::SplitMix64};
use std::{collections::BTreeMap, iter::FusedIterator};

/// Uniform draws with replacement over live ranks. `retire` removes only the
/// most recent draw, using sparse swap-removal. Equal payloads at distinct ranks
/// remain distinct occurrences. No source materialization is needed, even for a
/// `u128::MAX`-sized space. Storage is O(retirements). Small prefixes use sparse
/// lookup; after retiring at least 1/32 of a usize-sized space, remaining ranks
/// become a dense vector with constant-time lookup/removal. Promotion preserves
/// the exact draw order and can retain up to 31 times the retirement count.
/// Drawing without retirement does not grow storage. Cloning forks all state.
///
/// No draw is available to retire before the first draw, after a retirement, or
/// after exhaustion. Drawing again replaces the pending retirement target.
/// Native seeded traces are not Java-compatible.
#[derive(Clone, Debug)]
pub struct RetiringIndices {
    remaining: u128,
    moved: BTreeMap<u128, u128>,
    dense: Option<Vec<u128>>,
    length: u128,
    last: Option<u128>,
    random: SplitMix64,
}
impl RetiringIndices {
    pub fn new(length: u128, seed: u64) -> Self {
        Self {
            remaining: length,
            moved: BTreeMap::new(),
            dense: None,
            length,
            last: None,
            random: SplitMix64::new(seed),
        }
    }
    pub fn remaining(&self) -> u128 {
        self.remaining
    }
    /// Retained logical slots (sparse remappings or dense active ranks).
    pub fn stored_indices(&self) -> usize {
        self.dense.as_ref().map_or(self.moved.len(), Vec::len)
    }
    pub fn is_empty(&self) -> bool {
        self.remaining == 0
    }
    pub fn draw(&mut self) -> Option<u128> {
        if self.is_empty() {
            self.last = None;
            return None;
        }
        let rank = if self.remaining == 1 {
            0
        } else {
            self.random.below_wide(self.remaining)
        };
        self.last = Some(rank);
        Some(match &self.dense {
            Some(values) => values[rank as usize],
            None => self.moved.get(&rank).copied().unwrap_or(rank),
        })
    }
    pub fn retire(&mut self) -> Result<(), NoDrawToRetire> {
        let rank = self.last.take().ok_or(NoDrawToRetire)?;
        let end = self.remaining - 1;
        if let Some(values) = &mut self.dense {
            values.swap_remove(rank as usize);
        } else {
            let replacement = self.moved.remove(&end).unwrap_or(end);
            self.moved.remove(&rank);
            if rank != end && replacement != rank {
                self.moved.insert(rank, replacement);
            }
        }
        self.remaining = end;
        if self.dense.is_none()
            && end > 0
            && self.length - end >= self.length.div_ceil(32)
            && let Ok(count) = usize::try_from(end)
        {
            // Allocation failure leaves the sparse representation usable. Fill
            // in active-rank order, so promotion consumes no random numbers and
            // preserves the exact same sequence as continued sparse sampling.
            let mut values = Vec::new();
            if values.try_reserve_exact(count).is_ok() {
                values.extend(0..end);
                for (&slot, &value) in &self.moved {
                    values[slot as usize] = value;
                }
                self.dense = Some(values);
                self.moved.clear();
            }
        }
        Ok(())
    }
}
impl Iterator for RetiringIndices {
    type Item = u128;
    fn next(&mut self) -> Option<u128> {
        self.draw()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.is_empty() {
            (0, Some(0))
        } else {
            (usize::MAX, None)
        }
    }
}
impl FusedIterator for RetiringIndices {}

/// Borrowed retiring draws from concatenated groups. Deferred groups load only
/// when selected. Supplier errors retain their normal cached behavior and may be
/// retired like successful draws. A supplier panic leaves its drawn rank pending
/// for retirement; no supplier effect is rolled back. Retiring never mutates the
/// view or its underlying index. Other iterators remain independent.
pub struct RetiringCandidates<'q, 'a, T> {
    view: &'q LazyConcat<'a, T>,
    indices: RetiringIndices,
}
impl<'q, 'a, T> RetiringCandidates<'q, 'a, T> {
    pub fn new(view: &'q LazyConcat<'a, T>, seed: u64) -> Self {
        Self {
            view,
            indices: RetiringIndices::new(view.len() as u128, seed),
        }
    }
    pub fn remaining(&self) -> u128 {
        self.indices.remaining()
    }
    pub fn stored_indices(&self) -> usize {
        self.indices.stored_indices()
    }
    pub fn retire(&mut self) -> Result<(), NoDrawToRetire> {
        self.indices.retire()
    }
}
impl<'q, T> Iterator for RetiringCandidates<'q, '_, T> {
    type Item = Result<&'q T, LazyConcatError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.indices.next().map(|i| self.view.get(i as usize))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}
impl<T> FusedIterator for RetiringCandidates<'_, '_, T> {}

/// Retire selected pair occurrences without building their Cartesian product.
/// Repeating/overlapping blocks retain occurrence weights. Payloads are borrowed
/// without Clone. The underlying view and all other traversals remain unchanged.
pub struct RetiringCandidatePairs<'q, 'a, L, R> {
    view: &'q CandidatePairs<'a, L, R>,
    indices: RetiringIndices,
}
impl<'q, 'a, L, R> RetiringCandidatePairs<'q, 'a, L, R> {
    pub fn new(view: &'q CandidatePairs<'a, L, R>, seed: u64) -> Self {
        Self {
            view,
            indices: RetiringIndices::new(view.len(), seed),
        }
    }
    pub fn remaining(&self) -> u128 {
        self.indices.remaining()
    }
    pub fn stored_indices(&self) -> usize {
        self.indices.stored_indices()
    }
    pub fn retire(&mut self) -> Result<(), NoDrawToRetire> {
        self.indices.retire()
    }
}
impl<'a, L, R> Iterator for RetiringCandidatePairs<'_, 'a, L, R> {
    type Item = (&'a L, &'a R);
    fn next(&mut self) -> Option<Self::Item> {
        self.indices
            .next()
            .map(|i| self.view.get(i).expect("live pair rank"))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}
impl<L, R> FusedIterator for RetiringCandidatePairs<'_, '_, L, R> {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sparse_retirement_matches_dense_swap_removal_at_every_step() {
        for n in [0, 1, 2, 3, 16, 65, 200] {
            for seed in 0..32 {
                let mut sparse = RetiringIndices::new(n, seed);
                let mut dense: Vec<_> = (0..n).collect();
                let mut random = SplitMix64::new(seed);
                assert_eq!(sparse.retire(), Err(NoDrawToRetire));
                for step in 0..n * 4 {
                    let i = if dense.len() == 1 {
                        0
                    } else {
                        random.below_wide(dense.len() as u128) as usize
                    };
                    assert_eq!(sparse.draw(), Some(dense[i]));
                    if step % 4 == 3 {
                        sparse.retire().unwrap();
                        dense.swap_remove(i);
                        assert_eq!(sparse.retire(), Err(NoDrawToRetire));
                    }
                    assert_eq!(sparse.remaining(), dense.len() as u128);
                }
                assert_eq!(sparse.draw(), None);
                assert_eq!(sparse.draw(), None);
                assert_eq!(sparse.stored_indices(), 0);
            }
        }
    }
}
