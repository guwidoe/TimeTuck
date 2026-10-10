//! Retirement adapter for indexed Cartesian candidate pairs.
use crate::{CandidatePairs, NoDrawToRetire, RetiringIndices};
use std::iter::FusedIterator;

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
