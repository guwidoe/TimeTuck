//! Checked signed input at collection boundaries; native unsigned paths stay unchanged.
use crate::{EntryList, EntryListError, LazyConcat, LazyConcatError, ListEntry, ValueRange};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignedIndexError {
    Index { index: i128, length: usize },
    Slot { slot: i128, slot_count: usize },
    Lazy(LazyConcatError),
    Entry(EntryListError),
}
impl fmt::Display for SignedIndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Index { index, length } => write!(f, "Index: {index}, Size: {length}"),
            Self::Slot { slot, slot_count } => {
                write!(f, "slot {slot} exceeds slotCount {slot_count}")
            }
            Self::Lazy(error) => error.fmt(f),
            Self::Entry(error) => error.fmt(f),
        }
    }
}
impl Error for SignedIndexError {}

impl<T> LazyConcat<'_, T> {
    /// Reject negative or unrepresentable indices before materializing a group.
    pub fn get_signed(&self, index: i128) -> Result<&T, SignedIndexError> {
        let native = usize::try_from(index).map_err(|_| SignedIndexError::Index {
            index,
            length: self.len(),
        })?;
        self.get(native).map_err(SignedIndexError::Lazy)
    }
}
impl<T> EntryList<T> {
    /// Validate signed external indices before ordinary insertion and compaction.
    pub fn insert_signed(
        &mut self,
        index: i128,
        value: T,
    ) -> Result<ListEntry<T>, SignedIndexError> {
        let native = usize::try_from(index).map_err(|_| SignedIndexError::Index {
            index,
            length: self.len(),
        })?;
        self.insert(native, value).map_err(SignedIndexError::Entry)
    }
    /// Read a physical slot without compaction, preserving holes as None.
    pub fn entry_at_signed(&self, slot: i128) -> Result<Option<ListEntry<T>>, SignedIndexError> {
        let native = usize::try_from(slot).map_err(|_| SignedIndexError::Slot {
            slot,
            slot_count: self.slot_count(),
        })?;
        self.entry_at(native).map_err(SignedIndexError::Entry)
    }
}
/// Optional signed boundary for every finite range, including dynamically sized
/// ranges. Negative indices are absent; valid indices retain ordinary get semantics.
pub trait SignedValueRange: ValueRange {
    fn get_signed(&self, index: i128) -> Option<Self::Value> {
        u128::try_from(index).ok().and_then(|index| self.get(index))
    }
}
impl<R: ValueRange + ?Sized> SignedValueRange for R {}
