//! Immutable indexed planning ranges and bounded materialization for finite models.
use crate::{Variable, random::SplitMix64};
use std::{cmp::Ordering, error::Error, fmt, iter::FusedIterator, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValueRangeError {
    DescendingBounds,
    ZeroStep,
    CountOverflow,
    MaterializationLimit { size: u128, limit: usize },
}
impl fmt::Display for ValueRangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DescendingBounds => f.write_str("range start must not exceed its end"),
            Self::ZeroStep => f.write_str("range step must be positive"),
            Self::CountOverflow => f.write_str("combined range count exceeds u128"),
            Self::MaterializationLimit { size, limit } => write!(
                f,
                "range has {size} values, exceeding materialization limit {limit}"
            ),
        }
    }
}
impl Error for ValueRangeError {}

/// An immutable, finite, indexed sequence of value occurrences. Equal values may
/// occur more than once; random sampling weights occurrences, not distinct values.
/// Implementations must keep their count/order stable, return a value for each
/// index in 0..len(), and return None outside that interval. Counts use u128.
/// The trait is object safe; iteration/sampling conveniences require a sized handle.
pub trait ValueRange {
    type Value: Clone + Eq;
    fn len(&self) -> u128;
    fn get(&self, index: u128) -> Option<Self::Value>;
    fn contains(&self, value: &Self::Value) -> bool;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Check an assigned value; absence is not a member of a nonoptional range.
    /// For OptionalValueRange, an explicit member is Some(&None).
    fn contains_assignment(&self, value: Option<&Self::Value>) -> bool {
        value.is_some_and(|v| self.contains(v))
    }
    fn iter(&self) -> ValueRangeIter<'_, Self>
    where
        Self: Sized,
    {
        ValueRangeIter {
            range: self,
            front: 0,
            back: self.len(),
        }
    }
    /// Uniform occurrence sampling with replacement; nonempty ranges repeat.
    /// Empty ranges exhaust immediately. Native seeds do not match Java traces.
    fn random_iter(&self, seed: u64) -> impl FusedIterator<Item = Self::Value> + '_
    where
        Self: Sized,
    {
        let mut random = SplitMix64::new(seed);
        self.random_iter_with(move |bound| random.below_wide(bound))
    }
    /// Inject draws in 0..bound. Invalid draws panic. No draws occur when empty.
    fn random_iter_with<'a>(
        &'a self,
        mut draw: impl FnMut(u128) -> u128 + 'a,
    ) -> impl FusedIterator<Item = Self::Value> + 'a
    where
        Self: Sized,
    {
        std::iter::from_fn(move || {
            let size = self.len();
            if size == 0 {
                return None;
            }
            let index = draw(size);
            assert!(index < size, "value-range draw must be less than its bound");
            Some(
                self.get(index)
                    .expect("value range must supply every in-range index"),
            )
        })
        .fuse()
    }
    /// Materialize only after checking an explicit element limit. This limit does
    /// not measure bytes or guarantee allocation success. Duplicate values remain.
    fn materialize(&self, limit: usize) -> Result<Vec<Self::Value>, ValueRangeError>
    where
        Self: Sized,
    {
        if self.len() > limit as u128 {
            return Err(ValueRangeError::MaterializationLimit {
                size: self.len(),
                limit,
            });
        }
        Ok(self.iter().collect())
    }
    /// A stable sorted copy; the source range is unchanged. The comparator may
    /// capture solution facts, replacing a separate comparator-factory abstraction.
    fn sorted_by(
        &self,
        limit: usize,
        compare: impl FnMut(&Self::Value, &Self::Value) -> Ordering,
    ) -> Result<EnumeratedValueRange<Self::Value>, ValueRangeError>
    where
        Self: Sized,
    {
        let mut values = self.materialize(limit)?;
        values.sort_by(compare);
        Ok(EnumeratedValueRange::new(values))
    }
}

/// Lazy double-ended traversal. nth/nth_back skip by rank without visiting values.
/// ExactSizeIterator is intentionally absent: a count may exceed usize::MAX.
pub struct ValueRangeIter<'a, R: ValueRange + ?Sized> {
    range: &'a R,
    front: u128,
    back: u128,
}
impl<R: ValueRange + ?Sized> Iterator for ValueRangeIter<'_, R> {
    type Item = R::Value;
    fn next(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        let index = self.front;
        self.front += 1;
        Some(
            self.range
                .get(index)
                .expect("value range must supply every in-range index"),
        )
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.front += (n as u128).min(self.back - self.front);
        self.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match usize::try_from(self.back - self.front) {
            Ok(n) => (n, Some(n)),
            Err(_) => (usize::MAX, None),
        }
    }
}
impl<R: ValueRange + ?Sized> DoubleEndedIterator for ValueRangeIter<'_, R> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front == self.back {
            return None;
        }
        self.back -= 1;
        Some(
            self.range
                .get(self.back)
                .expect("value range must supply every in-range index"),
        )
    }
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.back -= (n as u128).min(self.back - self.front);
        self.next_back()
    }
}
impl<R: ValueRange + ?Sized> FusedIterator for ValueRangeIter<'_, R> {}

/// Signed 64-bit half-open arithmetic progression. A positive step need not divide
/// the interval: the last value is the final progression member strictly below end.
/// Count/offset arithmetic is widened, including intervals crossing i64::MIN/MAX.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegerValueRange {
    start: i64,
    end: i64,
    step: u64,
    len: u128,
}
impl IntegerValueRange {
    pub fn new(start: i64, end: i64, step: u64) -> Result<Self, ValueRangeError> {
        if start > end {
            return Err(ValueRangeError::DescendingBounds);
        }
        if step == 0 {
            return Err(ValueRangeError::ZeroStep);
        }
        let width = (i128::from(end) - i128::from(start)) as u128;
        Ok(Self {
            start,
            end,
            step,
            len: width.div_ceil(u128::from(step)),
        })
    }
}
impl ValueRange for IntegerValueRange {
    type Value = i64;
    fn len(&self) -> u128 {
        self.len
    }
    fn get(&self, index: u128) -> Option<i64> {
        (index < self.len)
            .then(|| (i128::from(self.start) + (index * u128::from(self.step)) as i128) as i64)
    }
    fn contains(&self, value: &i64) -> bool {
        *value >= self.start
            && *value < self.end
            && (i128::from(*value) - i128::from(self.start)) % (i128::from(self.step)) == 0
    }
}

/// Stable explicit occurrences. Cloning the range shares immutable storage;
/// get/iteration clone only requested values. Empty ranges are valid.
#[derive(Clone, Debug)]
pub struct EnumeratedValueRange<V> {
    values: Arc<[V]>,
}
impl<V> EnumeratedValueRange<V> {
    pub fn new(values: impl IntoIterator<Item = V>) -> Self {
        Self {
            values: values.into_iter().collect(),
        }
    }
    pub fn values(&self) -> &[V] {
        &self.values
    }
}
impl<V: Clone + Eq> ValueRange for EnumeratedValueRange<V> {
    type Value = V;
    fn len(&self) -> u128 {
        self.values.len() as u128
    }
    fn get(&self, index: u128) -> Option<V> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.values.get(i))
            .cloned()
    }
    fn contains(&self, value: &V) -> bool {
        self.values.contains(value)
    }
}

/// False followed by true, suitable for boolean planning variables.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BooleanValueRange;
impl ValueRange for BooleanValueRange {
    type Value = bool;
    fn len(&self) -> u128 {
        2
    }
    fn get(&self, index: u128) -> Option<bool> {
        match index {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }
    fn contains(&self, _: &bool) -> bool {
        true
    }
}

/// Concatenate heterogeneous indexed ranges without flattening their values.
/// Child order and duplicate occurrences are retained. Prefix counts give O(log
/// children) routing, followed by the selected child's own get implementation.
pub struct CompositeValueRange<'a, V: Clone + Eq> {
    children: Vec<Box<dyn ValueRange<Value = V> + 'a>>,
    through: Vec<u128>,
    total: u128,
}
impl<'a, V: Clone + Eq> CompositeValueRange<'a, V> {
    pub fn new(
        children: Vec<Box<dyn ValueRange<Value = V> + 'a>>,
    ) -> Result<Self, ValueRangeError> {
        let mut total = 0u128;
        let mut through = Vec::with_capacity(children.len());
        for child in &children {
            total = total
                .checked_add(child.len())
                .ok_or(ValueRangeError::CountOverflow)?;
            through.push(total);
        }
        Ok(Self {
            children,
            through,
            total,
        })
    }
}
impl<V: Clone + Eq> ValueRange for CompositeValueRange<'_, V> {
    type Value = V;
    fn len(&self) -> u128 {
        self.total
    }
    fn get(&self, index: u128) -> Option<V> {
        let child = self.through.partition_point(|n| *n <= index);
        let range = self.children.get(child)?;
        let before = if child == 0 {
            0
        } else {
            self.through[child - 1]
        };
        range.get(index - before)
    }
    fn contains(&self, value: &V) -> bool {
        self.children.iter().any(|r| r.contains(value))
    }
}

/// Prefix one explicit None member before the child's Some(value) occurrences.
/// Wrapping an empty child gives a singleton None range. Nested wrappers retain
/// distinct Option layers; they do not silently merge separate meanings of None.
#[derive(Clone, Debug)]
pub struct OptionalValueRange<R: ValueRange> {
    child: R,
    len: u128,
}
impl<R: ValueRange> OptionalValueRange<R> {
    pub fn new(child: R) -> Result<Self, ValueRangeError> {
        let len = child
            .len()
            .checked_add(1)
            .ok_or(ValueRangeError::CountOverflow)?;
        Ok(Self { child, len })
    }
    /// Sort child values while retaining the optional member at rank zero, even
    /// for a descending comparator. The limit includes the extra None occurrence.
    pub fn sorted_present_by(
        &self,
        limit: usize,
        compare: impl FnMut(&R::Value, &R::Value) -> Ordering,
    ) -> Result<OptionalValueRange<EnumeratedValueRange<R::Value>>, ValueRangeError> {
        if self.len > limit as u128 {
            return Err(ValueRangeError::MaterializationLimit {
                size: self.len,
                limit,
            });
        }
        OptionalValueRange::new(self.child.sorted_by(limit - 1, compare)?)
    }
}
impl<R: ValueRange> ValueRange for OptionalValueRange<R> {
    type Value = Option<R::Value>;
    fn len(&self) -> u128 {
        self.len
    }
    fn get(&self, index: u128) -> Option<Self::Value> {
        if index == 0 {
            Some(None)
        } else {
            self.child.get(index - 1).map(Some)
        }
    }
    fn contains(&self, value: &Self::Value) -> bool {
        value.as_ref().is_none_or(|v| self.child.contains(v))
    }
}

/// Explicitly materialize a reusable range for the current finite solver.
/// Huge lazy ranges are usable for lookup/sampling but not implicitly expanded.
/// Duplicate occurrences are preserved; FiniteProblem validates domain uniqueness.
///
/// ```
/// use timetuck::*;
/// let domain = IntegerValueRange::new(-2, 3, 1)?;
/// let variable = variable_from_range(&domain, 5)?;
/// assert_eq!(variable.domain(), &[-2, -1, 0, 1, 2]);
/// # Ok::<(), ValueRangeError>(())
/// ```
pub fn variable_from_range<V: Clone + Eq>(
    range: &impl ValueRange<Value = V>,
    maximum_values: usize,
) -> Result<Variable<V>, ValueRangeError> {
    Ok(Variable::new(range.materialize(maximum_values)?))
}
