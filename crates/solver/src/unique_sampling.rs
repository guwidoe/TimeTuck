//! Lazy uniform sampling of indexed occurrences without replacement.
use crate::{ValueRange, random::SplitMix64};
use std::{collections::HashMap, error::Error, fmt, iter::FusedIterator};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UniqueSampleError {
    InvalidDraw {
        index: u128,
        bound: u128,
    },
    /// The range violated its indexed-access contract for a sampled occurrence.
    MissingValue {
        index: u128,
    },
}
impl fmt::Display for UniqueSampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unique sample: {self:?}")
    }
}
impl Error for UniqueSampleError {}

/// Lazy permutation of 0..length. Every successful draw removes one occurrence.
/// Storage grows only with consumed draws, never with the declared range length.
/// A sparse swap table represents the still-live prefix: a draw chooses a position
/// in that prefix, then the last live occurrence fills its vacated position.
/// Uniform independent draws therefore yield a uniform permutation.
///
/// has_next and remaining are passive. Singleton/empty pools consume no random
/// draw. Invalid draws emit one error and permanently exhaust this cursor. A draw
/// callback panic happens before any mapping mutation; its own effects cannot be
/// rolled back. Hash-table iteration order never participates in selection.
/// No Java PRNG trace parity is claimed. Bounds and counts may exceed usize::MAX.
pub struct UniqueIndices<D> {
    remaining: u128,
    moved: HashMap<u128, u128>,
    draw: D,
}
impl<D: FnMut(u128) -> u128> UniqueIndices<D> {
    pub fn with_draw(length: u128, draw: D) -> Self {
        Self {
            remaining: length,
            moved: HashMap::new(),
            draw,
        }
    }
    /// Number of occurrences still available; zero after an invalid draw.
    pub fn remaining(&self) -> u128 {
        self.remaining
    }
    pub fn has_next(&self) -> bool {
        self.remaining != 0
    }
    /// Number of sparse remapping entries, not the source cardinality. Allocated
    /// table capacity may remain larger after entries have been removed.
    pub fn stored_indices(&self) -> usize {
        self.moved.len()
    }
    fn exhaust(&mut self) {
        self.remaining = 0;
        self.moved.clear();
    }
}
impl<D: FnMut(u128) -> u128> Iterator for UniqueIndices<D> {
    type Item = Result<u128, UniqueSampleError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let bound = self.remaining;
        let index = if bound == 1 { 0 } else { (self.draw)(bound) };
        if index >= bound {
            self.exhaust();
            return Some(Err(UniqueSampleError::InvalidDraw { index, bound }));
        }
        let last = bound - 1;
        let selected = self.moved.remove(&index).unwrap_or(index);
        let replacement = self.moved.remove(&last).unwrap_or(last);
        if index != last {
            self.moved.insert(index, replacement);
        }
        self.remaining = last;
        Some(Ok(selected))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        // A bad injected draw can end iteration after a single error.
        (
            usize::from(self.has_next()),
            usize::try_from(self.remaining).ok(),
        )
    }
}
impl<D: FnMut(u128) -> u128> FusedIterator for UniqueIndices<D> {}
/// Seeded unique index traversal. Taking k indices needs at most k remapping
/// entries even when the source has u128::MAX occurrences.
pub fn unique_indices(length: u128, seed: u64) -> UniqueIndices<impl FnMut(u128) -> u128> {
    let mut random = SplitMix64::new(seed);
    UniqueIndices::with_draw(length, move |bound| random.below_wide(bound))
}

/// Borrowed unique occurrence sampling without cloning payloads. Equal payloads
/// at different positions are distinct occurrences and can both be returned.
pub struct UniqueSliceSamples<'a, T, D> {
    values: &'a [T],
    indices: UniqueIndices<D>,
}
impl<'a, T, D: FnMut(u128) -> u128> UniqueSliceSamples<'a, T, D> {
    pub fn with_draw(values: &'a [T], draw: D) -> Self {
        Self {
            values,
            indices: UniqueIndices::with_draw(values.len() as u128, draw),
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
impl<'a, T, D: FnMut(u128) -> u128> Iterator for UniqueSliceSamples<'a, T, D> {
    type Item = Result<&'a T, UniqueSampleError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.indices
            .next()
            .map(|r| r.map(|i| &self.values[i as usize]))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}
impl<T, D: FnMut(u128) -> u128> FusedIterator for UniqueSliceSamples<'_, T, D> {}
pub fn unique_slice_samples<T>(
    values: &[T],
    seed: u64,
) -> UniqueSliceSamples<'_, T, impl FnMut(u128) -> u128> {
    let mut random = SplitMix64::new(seed);
    UniqueSliceSamples::with_draw(values, move |bound| random.below_wide(bound))
}

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
