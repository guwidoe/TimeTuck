//! Exact coordinates for uniformly ranked contiguous spans, without list payloads.
use std::{error::Error, fmt, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubListRankError {
    ZeroMinimum,
    ReversedBounds { minimum: usize, maximum: usize },
    ListTooShort { available: usize, minimum: usize },
}
impl fmt::Display for SubListRankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroMinimum => write!(f, "minimum span length must be greater than 0"),
            Self::ReversedBounds { minimum, maximum } => write!(
                f,
                "minimum span length ({minimum}) must be less than or equal to maximum ({maximum})"
            ),
            Self::ListTooShort { available, minimum } => write!(
                f,
                "list length ({available}) must be at least the minimum span length ({minimum})"
            ),
        }
    }
}
impl Error for SubListRankError {}

/// Nonempty coordinate index for one list or movable suffix. Every span within
/// the inclusive length bounds receives one rank, ordered by descending length
/// and then ascending start. Length alone determines the index; no values are
/// allocated, cloned or validated. Coordinates are relative to this suffix.
///
/// Maximum length is clamped to available length. Invalid bounds and lists shorter
/// than the minimum are rejected. Counts and arithmetic use u128, including for
/// virtual usize::MAX-sized lists. Lookup uses a short binary search for at most
/// eight lengths, otherwise an exact integer square root and one correction.
/// No floating-point approximation is used.
///
/// ```
/// use timetuck::SubListRankIndex;
/// let spans = SubListRankIndex::new(7, 2, 5)?;
/// assert_eq!(spans.span_count(), 18);
/// assert_eq!(spans.at(0), Some(0..5));
/// assert_eq!(spans.at(17), Some(5..7));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SubListRankIndex {
    available: usize,
    first_row: usize,
    rows: usize,
    count: u128,
}
fn triangle(n: usize) -> u128 {
    let n = n as u128;
    n * (n + 1) / 2
}
impl SubListRankIndex {
    pub fn new(available: usize, minimum: usize, maximum: usize) -> Result<Self, SubListRankError> {
        if minimum == 0 {
            return Err(SubListRankError::ZeroMinimum);
        }
        if minimum > maximum {
            return Err(SubListRankError::ReversedBounds { minimum, maximum });
        }
        if available < minimum {
            return Err(SubListRankError::ListTooShort { available, minimum });
        }
        let first_row = available - maximum.min(available);
        let rows = maximum.min(available) - minimum + 1;
        let before = triangle(first_row);
        let count = triangle(available - minimum + 1) - before;
        Ok(Self {
            available,
            first_row,
            rows,
            count,
        })
    }
    pub fn span_count(&self) -> u128 {
        self.count
    }
    /// Out-of-range ranks return None. Rank zero selects the earliest longest span.
    #[inline]
    pub fn at(&self, rank: u128) -> Option<Range<usize>> {
        (rank < self.count).then(|| self.at_valid_rank(rank))
    }
    /// Internal caller has already checked the containing entity's cumulative bound.
    #[inline]
    pub(crate) fn at_valid_rank(&self, rank: u128) -> Range<usize> {
        debug_assert!(rank < self.count);
        if self.rows <= 8 {
            let width = self.first_row as u128 + 1;
            let prefix = |k: usize| {
                let k = k as u128;
                k * (2 * width + k - 1) / 2
            };
            let (mut low, mut high) = (0, self.rows);
            while low < high {
                let mid = low + (high - low) / 2;
                if prefix(mid + 1) <= rank {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            let start = (rank - prefix(low)) as usize;
            let length = self.available - self.first_row - low;
            return start..start + length;
        }
        let rank = triangle(self.first_row) + rank;
        // The largest valid rank is below T(usize::MAX), so doubling fits u128.
        // floor(sqrt(2r)) is either floor(triangular_root(r)) or one above it.
        let mut row = (2 * rank).isqrt() as usize;
        if triangle(row) > rank {
            row -= 1;
        }
        let start = (rank - triangle(row)) as usize;
        let length = self.available - row;
        start..start + length
    }
    /// Draw uniformly by supplying a uniform rank in 0..span_count. One call is
    /// made with that positive exclusive bound. An out-of-range draw panics.
    pub fn draw_with(&self, mut draw: impl FnMut(u128) -> u128) -> Range<usize> {
        self.at(draw(self.count))
            .expect("sublist draw must be less than its bound")
    }
}
