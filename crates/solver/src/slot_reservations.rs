//! Adaptive remapping of a bounded index space with implicit identity entries.
use std::{error::Error, fmt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotReservationError {
    InvalidSlotCount(i64),
    SignedSlotOutOfBounds { slot: i64, count: usize },
    SignedLogicalIndexOutOfBounds { index: i64, count: usize },
    SlotOutOfBounds { slot: usize, count: usize },
    LogicalIndexOutOfBounds { index: usize, count: usize },
    AllocationFailed,
}
impl fmt::Display for SlotReservationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSlotCount(count) => {
                write!(f, "slotCount ({count}) must be nonnegative and fit usize")
            }
            Self::SignedSlotOutOfBounds { slot, count } => {
                write!(f, "The slot ({slot}) is outside 0..{count}")
            }
            Self::SignedLogicalIndexOutOfBounds { index, count } => {
                write!(f, "The logicalIndex ({index}) is outside 0..{count}")
            }
            Self::SlotOutOfBounds { slot, count } => write!(f, "slot {slot} is outside 0..{count}"),
            Self::LogicalIndexOutOfBounds { index, count } => {
                write!(f, "logical index {index} is outside 0..{count}")
            }
            Self::AllocationFailed => f.write_str("unable to allocate slot reservations"),
        }
    }
}
impl Error for SlotReservationError {}
#[derive(Clone, Debug)]
enum ReservationStorage {
    Sparse(Vec<(usize, usize)>),
    Dense(Vec<usize>),
}

/// Resolve unreserved slots to themselves; reservations can alias logical indices.
/// This map alone does not enforce a permutation or uniqueness.
///
/// Construction allocates nothing. Up to `min(16, slot_count / 2)` nonidentity
/// entries use a small log; the next distinct reservation promotes to an array of
/// `slot_count` indices. Updates/identity reservations do not fill the log. Release
/// and clear restore identity mappings without demoting a dense map. Sparse lookup
/// scans at most 16 entries; dense lookup is constant time.
///
/// A promotion may allocate the entire index space. Use a sparse map instead for
/// very large spaces with few remappings. Indices/counts are unsigned; negative
/// inputs can use the signed boundary methods before conversion. All index validation
/// precedes mutation, and failed allocation leaves the mappings unchanged.
///
/// ```
/// use timetuck::*;
/// let mut map = SlotReservationMap::new(4);
/// assert_eq!(map.reserve(1, 3)?, 1);
/// assert_eq!(map.resolve(1)?, 3);
/// assert_eq!(map.release(1)?, 3);
/// assert_eq!(map.resolve(1)?, 1);
/// # Ok::<(), SlotReservationError>(())
/// ```
#[derive(Clone, Debug)]
pub struct SlotReservationMap {
    slot_count: usize,
    reservations: usize,
    storage: ReservationStorage,
}
impl SlotReservationMap {
    /// Validate an external signed count before constructing any storage.
    pub fn try_with_slot_count(slot_count: i64) -> Result<Self, SlotReservationError> {
        usize::try_from(slot_count)
            .map(Self::new)
            .map_err(|_| SlotReservationError::InvalidSlotCount(slot_count))
    }
    pub fn new(slot_count: usize) -> Self {
        Self {
            slot_count,
            reservations: 0,
            storage: ReservationStorage::Sparse(Vec::new()),
        }
    }
    pub fn slot_count(&self) -> usize {
        self.slot_count
    }
    fn signed_slot(&self, slot: i64) -> Result<usize, SlotReservationError> {
        usize::try_from(slot)
            .ok()
            .filter(|&slot| slot < self.slot_count)
            .ok_or(SlotReservationError::SignedSlotOutOfBounds {
                slot,
                count: self.slot_count,
            })
    }
    pub fn resolve_signed(&self, slot: i64) -> Result<usize, SlotReservationError> {
        self.resolve(self.signed_slot(slot)?)
    }
    /// Validate both external arguments before any sparse/dense mutation.
    pub fn reserve_signed(&mut self, slot: i64, index: i64) -> Result<usize, SlotReservationError> {
        let slot = self.signed_slot(slot)?;
        let index = usize::try_from(index)
            .ok()
            .filter(|&index| index < self.slot_count)
            .ok_or(SlotReservationError::SignedLogicalIndexOutOfBounds {
                index,
                count: self.slot_count,
            })?;
        self.reserve(slot, index)
    }
    pub fn release_signed(&mut self, slot: i64) -> Result<usize, SlotReservationError> {
        self.release(self.signed_slot(slot)?)
    }
    /// Number of nonidentity entries, independent of allocated capacity.
    pub fn len(&self) -> usize {
        self.reservations
    }
    pub fn is_empty(&self) -> bool {
        self.reservations == 0
    }
    pub fn is_dense(&self) -> bool {
        matches!(self.storage, ReservationStorage::Dense(_))
    }
    fn check_slot(&self, slot: usize) -> Result<(), SlotReservationError> {
        if slot < self.slot_count {
            Ok(())
        } else {
            Err(SlotReservationError::SlotOutOfBounds {
                slot,
                count: self.slot_count,
            })
        }
    }
    pub fn resolve(&self, slot: usize) -> Result<usize, SlotReservationError> {
        self.check_slot(slot)?;
        Ok(match &self.storage {
            ReservationStorage::Sparse(entries) => entries
                .iter()
                .find(|(s, _)| *s == slot)
                .map_or(slot, |(_, v)| *v),
            ReservationStorage::Dense(entries) => entries[slot],
        })
    }
    /// Install a mapping and return its previous resolution. Identity removes it.
    pub fn reserve(&mut self, slot: usize, index: usize) -> Result<usize, SlotReservationError> {
        self.check_slot(slot)?;
        if index >= self.slot_count {
            return Err(SlotReservationError::LogicalIndexOutOfBounds {
                index,
                count: self.slot_count,
            });
        }
        match &mut self.storage {
            ReservationStorage::Dense(entries) => {
                let previous = std::mem::replace(&mut entries[slot], index);
                self.reservations -= usize::from(previous != slot);
                self.reservations += usize::from(index != slot);
                Ok(previous)
            }
            ReservationStorage::Sparse(entries) => {
                if let Some(at) = entries.iter().position(|(s, _)| *s == slot) {
                    let previous = entries[at].1;
                    if index == slot {
                        entries.swap_remove(at);
                        self.reservations -= 1;
                    } else {
                        entries[at].1 = index;
                    }
                    return Ok(previous);
                }
                if index == slot {
                    return Ok(slot);
                }
                let limit = 16.min(self.slot_count / 2);
                if entries.len() < limit {
                    entries
                        .try_reserve(1)
                        .map_err(|_| SlotReservationError::AllocationFailed)?;
                    entries.push((slot, index));
                } else {
                    let mut dense = Vec::new();
                    dense
                        .try_reserve_exact(self.slot_count)
                        .map_err(|_| SlotReservationError::AllocationFailed)?;
                    dense.extend(0..self.slot_count);
                    for &(s, v) in entries.iter() {
                        dense[s] = v;
                    }
                    dense[slot] = index;
                    self.storage = ReservationStorage::Dense(dense);
                }
                self.reservations += 1;
                Ok(slot)
            }
        }
    }
    /// Restore identity and return the previous resolution, even if unreserved.
    pub fn release(&mut self, slot: usize) -> Result<usize, SlotReservationError> {
        self.reserve(slot, slot)
    }
    /// Restore every identity mapping, retaining the current representation.
    pub fn clear(&mut self) {
        match &mut self.storage {
            ReservationStorage::Sparse(entries) => entries.clear(),
            ReservationStorage::Dense(entries) => {
                for (slot, index) in entries.iter_mut().enumerate() {
                    *index = slot;
                }
            }
        }
        self.reservations = 0;
    }
}
impl fmt::Display for SlotReservationMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut entries: Vec<_> = match &self.storage {
            ReservationStorage::Sparse(entries) => entries.clone(),
            ReservationStorage::Dense(entries) => entries
                .iter()
                .copied()
                .enumerate()
                .filter(|(s, v)| s != v)
                .collect(),
        };
        entries.sort_unstable_by_key(|(s, _)| *s);
        f.write_str("{")?;
        for (i, (slot, index)) in entries.into_iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?;
            }
            write!(f, "{slot}={index}")?;
        }
        f.write_str("}")
    }
}
