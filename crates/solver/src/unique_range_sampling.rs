//! Indexed planning-range adapter for unique occurrence sampling.
use crate::{UniqueIndices, UniqueSampleError, ValueRange, random::SplitMix64};
use std::iter::FusedIterator;

/// Unique occurrences from an immutable indexed range. Only selected values are
/// fetched. The range's count/order must remain stable throughout iteration.
/// A missing in-range value emits one error and exhausts the cursor. Equal values
/// at different indices remain separate candidates; this is not value deduplication.
pub struct UniqueRangeSamples<'a, R: ValueRange + ?Sized, D> {
    range: &'a R,
    indices: UniqueIndices<D>,
}
impl<'a, R: ValueRange + ?Sized, D: FnMut(u128) -> u128> UniqueRangeSamples<'a, R, D> {
    pub fn with_draw(range: &'a R, draw: D) -> Self {
        Self {
            range,
            indices: UniqueIndices::with_draw(range.len(), draw),
        }
    }
    pub fn remaining(&self) -> u128 {
        self.indices.remaining()
    }
    pub fn has_next(&self) -> bool {
        self.indices.has_next()
    }
    pub fn stored_indices(&self) -> usize {
        self.indices.stored_indices()
    }
}
impl<R: ValueRange + ?Sized, D: FnMut(u128) -> u128> Iterator for UniqueRangeSamples<'_, R, D> {
    type Item = Result<R::Value, UniqueSampleError>;
    fn next(&mut self) -> Option<Self::Item> {
        let index = match self.indices.next()? {
            Ok(index) => index,
            Err(e) => return Some(Err(e)),
        };
        match self.range.get(index) {
            Some(value) => Some(Ok(value)),
            None => {
                self.indices.exhaust();
                Some(Err(UniqueSampleError::MissingValue { index }))
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}
impl<R: ValueRange + ?Sized, D: FnMut(u128) -> u128> FusedIterator
    for UniqueRangeSamples<'_, R, D>
{
}
/// Sample an indexed planning range without allocating its full permutation.
///
/// ```
/// use timetuck::*;
/// let range = EnumeratedValueRange::new(vec!["red", "green", "blue"]);
/// let mut values = unique_range_samples(&range, 42).collect::<Result<Vec<_>, _>>()?;
/// values.sort();
/// assert_eq!(values, ["blue", "green", "red"]);
/// # Ok::<(), UniqueSampleError>(())
/// ```
pub fn unique_range_samples<R: ValueRange + ?Sized>(
    range: &R,
    seed: u64,
) -> UniqueRangeSamples<'_, R, impl FnMut(u128) -> u128> {
    let mut random = SplitMix64::new(seed);
    UniqueRangeSamples::with_draw(range, move |bound| random.below_wide(bound))
}
