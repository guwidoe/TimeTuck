//! Checked fixed-layout access, supplied reference buffers and bidirectional editing.
use super::{SegmentedSlice, SegmentedSliceMut};
use std::{collections::BTreeMap, error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SegmentAccessError {
    Slot { index: i128, length: usize },
    MissingValue(String),
    OwnerCount { owners: usize, segments: usize },
    DuplicateOwner,
    StaleOwnerPosition,
    NoForwardSlot,
    NoBackwardSlot,
    FixedLayout,
}
impl fmt::Display for SegmentAccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slot { index, length } => write!(
                f,
                "Index ({index}) out of bounds for a list of size ({length})"
            ),
            Self::MissingValue(value) => {
                write!(f, "Value ({value}) is not contained in any entity list")
            }
            _ => write!(f, "segmented list access: {self:?}"),
        }
    }
}
impl Error for SegmentAccessError {}
fn signed_slot(index: isize, length: usize) -> Result<usize, SegmentAccessError> {
    usize::try_from(index)
        .ok()
        .filter(|&i| i < length)
        .ok_or(SegmentAccessError::Slot {
            index: index as i128,
            length,
        })
}
/// Structural commands are explicitly rejected by fixed-layout borrowed views.
/// Payloads are borrowed, so rejection neither clones nor consumes them.
#[derive(Debug)]
pub enum FixedSegmentRequest<'a, T> {
    Append(&'a T),
    Insert { index: isize, value: &'a T },
    Extend(&'a [T]),
    InsertAll { index: isize, values: &'a [T] },
    Clear,
    RemoveValue(&'a T),
    RemoveAt(isize),
    RemoveAll(&'a [T]),
    RetainAll(&'a [T]),
}
/// Either the original supplied slice, or a fresh exactly-sized reference vector.
/// A sufficiently long buffer is filled in place; the first spare slot becomes
/// None and any later tail stays unchanged. An undersized buffer is untouched.
/// Values are borrowed from the original segments, never cloned.
#[derive(Debug)]
pub enum SegmentReferenceBuffer<'b, 'a, T> {
    Supplied(&'b mut [Option<&'a T>]),
    Allocated(Vec<Option<&'a T>>),
}
impl<'b, 'a, T> SegmentReferenceBuffer<'b, 'a, T> {
    pub fn segment_references(&self) -> &[Option<&'a T>] {
        match self {
            Self::Supplied(slice) => slice,
            Self::Allocated(values) => values,
        }
    }
    pub fn uses_supplied_storage(&self) -> bool {
        matches!(self, Self::Supplied(_))
    }
}
impl<'a, T> SegmentedSlice<'a, T> {
    pub fn segment_slot(&self, index: isize) -> Result<&'a T, SegmentAccessError> {
        Ok(self
            .get(signed_slot(index, self.len())?)
            .expect("checked segment slot"))
    }
    pub fn write_segment_references<'b>(
        &self,
        supplied: &'b mut [Option<&'a T>],
    ) -> SegmentReferenceBuffer<'b, 'a, T> {
        if supplied.len() < self.len() {
            return SegmentReferenceBuffer::Allocated(self.iter().map(Some).collect());
        }
        for (slot, value) in supplied.iter_mut().zip(self.iter()) {
            *slot = Some(value);
        }
        if supplied.len() > self.len() {
            supplied[self.len()] = None;
        }
        SegmentReferenceBuffer::Supplied(supplied)
    }
}
impl<'a, T> SegmentedSliceMut<'a, T> {
    pub fn replace_segment_slot(
        &mut self,
        index: isize,
        value: T,
    ) -> Result<T, SegmentAccessError> {
        let index = signed_slot(index, self.len())?;
        Ok(self.replace(index, value).expect("checked segment slot"))
    }
    pub fn reject_segment_resize(
        &mut self,
        _request: FixedSegmentRequest<'_, T>,
    ) -> Result<(), SegmentAccessError> {
        Err(SegmentAccessError::FixedLayout)
    }
    /// A gap cursor at start..=len. step_next returns the slot at the gap, then
    /// advances; step_previous retreats first. Replacement targets the upcoming
    /// slot at the current gap, including before the first step. This deliberately
    /// differs from Java ListIterator's last-returned-element set contract.
    /// No structural edit or payload clone is performed. Sequential traversal
    /// touches segment boundaries directly, with no binary lookup per step.
    pub fn segment_cursor_at(
        &mut self,
        start: usize,
    ) -> Result<SegmentedListCursor<'_, 'a, T>, SegmentAccessError> {
        if start > self.len() {
            return Err(SegmentAccessError::Slot {
                index: start as i128,
                length: self.len(),
            });
        }
        let (segment, offset) = self
            .layout
            .locate(start)
            .unwrap_or((self.segments.len(), 0));
        Ok(SegmentedListCursor {
            view: self,
            position: start,
            segment,
            offset,
        })
    }
}
/// An exclusive cursor over the actual mutable segments. Returned references
/// borrow the cursor until the next mutation; it cannot be a standard Iterator
/// yielding independently retained references alongside mutable replacements.
/// A successful forward/backward reversal visits the same slot again.
/// End probes and failed writes preserve cursor position and all segments.
///
/// ```
/// use timetuck::SegmentedSliceMut;
/// let mut a=[1,2]; let mut b=[3];
/// let mut view=SegmentedSliceMut::new([a.as_mut_slice(),b.as_mut_slice()]);
/// let mut cursor=view.segment_cursor_at(2).unwrap();
/// assert_eq!(*cursor.step_previous().unwrap(),2);
/// assert_eq!(cursor.replace_upcoming_slot(7).unwrap(),2);
/// assert_eq!(*cursor.step_next().unwrap(),7);
/// drop(cursor); drop(view);
/// assert_eq!(a,[1,7]);
/// ```
pub struct SegmentedListCursor<'v, 'a, T> {
    view: &'v mut SegmentedSliceMut<'a, T>,
    position: usize,
    segment: usize,
    offset: usize,
}
impl<'v, 'a, T> SegmentedListCursor<'v, 'a, T> {
    pub fn cursor_position(&self) -> usize {
        self.position
    }
    pub fn has_forward_slot(&self) -> bool {
        self.position < self.view.len()
    }
    pub fn has_backward_slot(&self) -> bool {
        self.position > 0
    }
    /// Position of the gap after the next step, not Java nextIndex semantics.
    pub fn after_next_position(&self) -> Option<usize> {
        self.has_forward_slot().then(|| self.position + 1)
    }
    pub fn previous_slot_index(&self) -> Option<usize> {
        self.position.checked_sub(1)
    }
    pub fn step_next(&mut self) -> Result<&T, SegmentAccessError> {
        if !self.has_forward_slot() {
            return Err(SegmentAccessError::NoForwardSlot);
        }
        let (segment, offset) = (self.segment, self.offset);
        self.position += 1;
        self.offset += 1;
        while self.segment < self.view.segments.len()
            && self.offset == self.view.segments[self.segment].len()
        {
            self.segment += 1;
            self.offset = 0;
        }
        Ok(&self.view.segments[segment][offset])
    }
    pub fn step_previous(&mut self) -> Result<&T, SegmentAccessError> {
        if !self.has_backward_slot() {
            return Err(SegmentAccessError::NoBackwardSlot);
        }
        while self.offset == 0 {
            self.segment -= 1;
            self.offset = self.view.segments[self.segment].len();
        }
        self.offset -= 1;
        self.position -= 1;
        Ok(&self.view.segments[self.segment][self.offset])
    }
    pub fn replace_upcoming_slot(&mut self, value: T) -> Result<T, SegmentAccessError> {
        if !self.has_forward_slot() {
            return Err(SegmentAccessError::NoForwardSlot);
        }
        Ok(std::mem::replace(
            &mut self.view.segments[self.segment][self.offset],
            value,
        ))
    }
    pub fn reject_cursor_insert(&mut self, value: &T) -> Result<(), SegmentAccessError> {
        self.view
            .reject_segment_resize(FixedSegmentRequest::Insert {
                index: self.position as isize,
                value,
            })
    }
    pub fn reject_cursor_remove(&mut self) -> Result<(), SegmentAccessError> {
        Err(SegmentAccessError::FixedLayout)
    }
}
/// Canonical owner-to-segment lookup, prepared once without cloning owners/values.
/// The immutable view is borrowed, so edits/repartitioning require a new lookup.
/// Callback positions are checked against actual segment contents, rejecting stale
/// owners, local indices or payload metadata instead of trusting an external cache.
pub struct SegmentedOwnerLookup<'v, 'a, E, T> {
    view: &'v SegmentedSlice<'a, T>,
    owners: BTreeMap<&'v E, usize>,
}
impl<'v, 'a, E: Ord, T> SegmentedOwnerLookup<'v, 'a, E, T> {
    pub fn new(
        view: &'v SegmentedSlice<'a, T>,
        owners: &'v [E],
    ) -> Result<Self, SegmentAccessError> {
        if owners.len() != view.segment_count() {
            return Err(SegmentAccessError::OwnerCount {
                owners: owners.len(),
                segments: view.segment_count(),
            });
        }
        let mut indexed = BTreeMap::new();
        for (i, owner) in owners.iter().enumerate() {
            if indexed.insert(owner, i).is_some() {
                return Err(SegmentAccessError::DuplicateOwner);
            }
        }
        Ok(Self {
            view,
            owners: indexed,
        })
    }
    pub fn resolve_assigned_value(
        &self,
        value: &T,
        lookup: impl FnOnce(&T) -> Option<(E, usize)>,
    ) -> Result<usize, SegmentAccessError>
    where
        T: PartialEq + fmt::Display,
    {
        let (owner, index) =
            lookup(value).ok_or_else(|| SegmentAccessError::MissingValue(value.to_string()))?;
        let segment = *self
            .owners
            .get(&owner)
            .ok_or_else(|| SegmentAccessError::MissingValue(value.to_string()))?;
        let flat = self
            .view
            .flat_index(segment, index)
            .ok_or(SegmentAccessError::StaleOwnerPosition)?;
        if self.view.get(flat) != Some(value) {
            return Err(SegmentAccessError::StaleOwnerPosition);
        }
        Ok(flat)
    }
}
