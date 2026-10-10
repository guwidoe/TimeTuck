//! Retirement adapter for lazily concatenated candidate groups.
use crate::{LazyConcat, LazyConcatError, NoDrawToRetire, RetiringIndices};
use std::iter::FusedIterator;

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
