//! Borrowed flat views over disjoint list segments and value-preserving repartitioning.
pub(crate) mod segment_access;
use std::{error::Error, fmt, ops::Range};

#[derive(Debug)]
struct SegmentLayout {
    ends: Vec<usize>,
}
impl SegmentLayout {
    fn new(lengths: impl IntoIterator<Item = usize>) -> Self {
        let mut total = 0usize;
        let ends = lengths
            .into_iter()
            .map(|n| {
                total = total
                    .checked_add(n)
                    .expect("segmented slice length overflow");
                total
            })
            .collect();
        Self { ends }
    }
    fn len(&self) -> usize {
        self.ends.last().copied().unwrap_or(0)
    }
    fn start(&self, segment: usize) -> usize {
        if segment == 0 {
            0
        } else {
            self.ends[segment - 1]
        }
    }
    fn locate(&self, index: usize) -> Option<(usize, usize)> {
        if index >= self.len() {
            return None;
        }
        let segment = self.ends.partition_point(|&end| end <= index);
        Some((segment, index - self.start(segment)))
    }
    fn flat_index(&self, segment: usize, index: usize) -> Option<usize> {
        let end = *self.ends.get(segment)?;
        let start = self.start(segment);
        (index < end - start).then(|| start + index)
    }
    fn valid_range(&self, range: &Range<usize>) -> bool {
        range.start <= range.end && range.end <= self.len()
    }
}

/// A concatenated view that borrows values instead of copying them. Empty and
/// repeated segments are supported. Flat lookup uses binary search over segment
/// boundaries; traversal visits the original slices directly. Construction stores
/// only segment references and boundaries, and panics on total-length overflow.
///
/// ```
/// use timetuck::SegmentedSlice;
/// let a = [1, 2]; let b = [3, 4];
/// let view = SegmentedSlice::new([a.as_slice(), &[], b.as_slice()]);
/// assert_eq!(view.locate(2), Some((2, 0)));
/// assert_eq!(view.range(1..3).unwrap().iter().copied().collect::<Vec<_>>(), [2, 3]);
/// ```
#[derive(Debug)]
pub struct SegmentedSlice<'a, T> {
    segments: Vec<&'a [T]>,
    layout: SegmentLayout,
}
impl<'a, T> SegmentedSlice<'a, T> {
    pub fn new(segments: impl IntoIterator<Item = &'a [T]>) -> Self {
        let segments: Vec<_> = segments.into_iter().collect();
        let layout = SegmentLayout::new(segments.iter().map(|s| s.len()));
        Self { segments, layout }
    }
    pub fn len(&self) -> usize {
        self.layout.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }
    pub fn segment(&self, index: usize) -> Option<&'a [T]> {
        self.segments.get(index).copied()
    }
    pub fn offsets(&self) -> impl ExactSizeIterator<Item = usize> + '_ {
        (0..self.segment_count()).map(|i| self.layout.start(i))
    }
    pub fn locate(&self, index: usize) -> Option<(usize, usize)> {
        self.layout.locate(index)
    }
    pub fn flat_index(&self, segment: usize, index: usize) -> Option<usize> {
        self.layout.flat_index(segment, index)
    }
    pub fn get(&self, index: usize) -> Option<&'a T> {
        let (segment, offset) = self.locate(index)?;
        Some(&self.segments[segment][offset])
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &'a T> + '_ {
        self.segments.iter().flat_map(|segment| segment.iter())
    }
    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.position(value).is_some()
    }
    pub fn position(&self, value: &T) -> Option<usize>
    where
        T: PartialEq,
    {
        self.iter().position(|v| v == value)
    }
    pub fn rposition(&self, value: &T) -> Option<usize>
    where
        T: PartialEq,
    {
        self.iter()
            .rev()
            .position(|v| v == value)
            .map(|n| self.len() - 1 - n)
    }
    /// A borrowed subview; empty intersections are omitted. Flat indices start at
    /// zero in the new view, and segment indices refer to its retained segments.
    pub fn range(&self, range: Range<usize>) -> Option<Self> {
        if !self.layout.valid_range(&range) {
            return None;
        }
        Some(Self::new(self.segments.iter().enumerate().filter_map(
            |(i, s)| {
                let start = self.layout.start(i);
                let from = range.start.saturating_sub(start).min(s.len());
                let to = range.end.saturating_sub(start).min(s.len());
                (from < to).then(|| &s[from..to])
            },
        )))
    }
}

/// A mutable flat view with fixed segment lengths. Borrowing prevents structural
/// changes or aliasing while the view exists. Replacements and swaps immediately
/// update the underlying slices. No operation clones a value. Out-of-bounds
/// access returns None/false and leaves every segment unchanged.
#[derive(Debug)]
pub struct SegmentedSliceMut<'a, T> {
    segments: Vec<&'a mut [T]>,
    layout: SegmentLayout,
}
impl<'a, T> SegmentedSliceMut<'a, T> {
    pub fn new(segments: impl IntoIterator<Item = &'a mut [T]>) -> Self {
        let segments: Vec<_> = segments.into_iter().collect();
        let layout = SegmentLayout::new(segments.iter().map(|s| s.len()));
        Self { segments, layout }
    }
    pub fn len(&self) -> usize {
        self.layout.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn view(&self) -> SegmentedSlice<'_, T> {
        SegmentedSlice::new(self.segments.iter().map(|s| &**s))
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        let (segment, offset) = self.layout.locate(index)?;
        Some(&self.segments[segment][offset])
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        let (segment, offset) = self.layout.locate(index)?;
        Some(&mut self.segments[segment][offset])
    }
    pub fn replace(&mut self, index: usize, value: T) -> Option<T> {
        self.get_mut(index)
            .map(|slot| std::mem::replace(slot, value))
    }
    pub fn swap(&mut self, left: usize, right: usize) -> bool {
        let (Some(mut a), Some(mut b)) = (self.layout.locate(left), self.layout.locate(right))
        else {
            return false;
        };
        if a.0 == b.0 {
            self.segments[a.0].swap(a.1, b.1);
        } else {
            if a.0 > b.0 {
                std::mem::swap(&mut a, &mut b);
            }
            let (left, right) = self.segments.split_at_mut(b.0);
            std::mem::swap(&mut left[a.0][a.1], &mut right[0][b.1]);
        }
        true
    }
    pub fn reverse(&mut self, range: Range<usize>) -> bool {
        if !self.layout.valid_range(&range) {
            return false;
        }
        for n in 0..(range.end - range.start) / 2 {
            self.swap(range.start + n, range.end - 1 - n);
        }
        true
    }
    pub fn range_mut(&mut self, range: Range<usize>) -> Option<SegmentedSliceMut<'_, T>> {
        if !self.layout.valid_range(&range) {
            return None;
        }
        Some(SegmentedSliceMut::new(
            self.segments.iter_mut().enumerate().filter_map(|(i, s)| {
                let start = self.layout.start(i);
                let from = range.start.saturating_sub(start).min(s.len());
                let to = range.end.saturating_sub(start).min(s.len());
                (from < to).then(|| &mut s[from..to])
            }),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepartitionError {
    SegmentCount,
    LengthOverflow,
    TotalLength,
    Allocation,
}
impl fmt::Display for RepartitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid list repartition: {self:?}")
    }
}
impl Error for RepartitionError {}

/// Redistribute owned values across existing lists without changing their combined
/// order or cloning payloads. Every list receives its requested length, including
/// zero. Shape/overflow/allocation errors occur before any value moves. This is a
/// general collection operation; solver pin/range validation belongs to ListProblem.
pub fn repartition_lists<T>(
    lists: &mut [Vec<T>],
    lengths: &[usize],
) -> Result<(), RepartitionError> {
    if lengths.len() != lists.len() {
        return Err(RepartitionError::SegmentCount);
    }
    let total = lengths
        .iter()
        .try_fold(0usize, |sum, &n| sum.checked_add(n))
        .ok_or(RepartitionError::LengthOverflow)?;
    let current_total = lists
        .iter()
        .try_fold(0usize, |sum, list| sum.checked_add(list.len()))
        .ok_or(RepartitionError::LengthOverflow)?;
    if total != current_total {
        return Err(RepartitionError::TotalLength);
    }
    if lists.iter().zip(lengths).all(|(list, &n)| list.len() == n) {
        return Ok(());
    }
    let mut target = Vec::new();
    target
        .try_reserve_exact(lists.len())
        .map_err(|_| RepartitionError::Allocation)?;
    for &n in lengths {
        let mut list = Vec::new();
        list.try_reserve_exact(n)
            .map_err(|_| RepartitionError::Allocation)?;
        target.push(list);
    }
    let mut destination = 0;
    for value in lists.iter_mut().flat_map(|list| list.drain(..)) {
        while target[destination].len() == lengths[destination] {
            destination += 1;
        }
        target[destination].push(value);
    }
    for (list, mut replacement) in lists.iter_mut().zip(target) {
        std::mem::swap(list, &mut replacement);
    }
    Ok(())
}
