//! Ordered, gapped storage with stable entry identities and detached cursors.
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::{Rc, Weak},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntryListError {
    Removed,
    ForeignEntry,
    IndexOutOfBounds { index: usize, len: usize },
    SlotOutOfBounds { slot: usize, slot_count: usize },
    ForeignCursor,
    CursorInvalidated,
    NoCurrentEntry,
}
impl fmt::Display for EntryListError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Removed => f.write_str("entry was already removed"),
            Self::ForeignEntry => f.write_str("entry belongs to another list"),
            Self::IndexOutOfBounds { index, len } => {
                write!(f, "index {index} exceeds list length {len}")
            }
            Self::SlotOutOfBounds { slot, slot_count } => {
                write!(f, "slot {slot} exceeds slotCount {slot_count}")
            }
            Self::ForeignCursor => f.write_str("cursor belongs to another list"),
            Self::CursorInvalidated => f.write_str("list changed structurally outside this cursor"),
            Self::NoCurrentEntry => f.write_str("cursor has no current entry"),
        }
    }
}
impl std::error::Error for EntryListError {}

struct EntryState<T> {
    owner: Weak<()>,
    position: Cell<Option<usize>>,
    value: RefCell<Rc<T>>,
}
/// Stable identity; retains its last value even after removal or list destruction.
/// Values are shared without requiring `T: Clone`. Handles are local to one thread.
pub struct ListEntry<T>(Rc<EntryState<T>>);
impl<T> Clone for ListEntry<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T> ListEntry<T> {
    pub fn value(&self) -> Rc<T> {
        self.0.value.borrow().clone()
    }
    pub fn position(&self) -> Option<usize> {
        self.0.owner.upgrade().and_then(|_| self.0.position.get())
    }
    pub fn is_removed(&self) -> bool {
        self.position().is_none()
    }
    pub fn same_entry(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<T: fmt::Display> fmt::Display for ListEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.position() {
            Some(p) => write!(f, "{}@{p}", self.value()),
            None => f.write_str("null"),
        }
    }
}
impl<T: fmt::Debug> fmt::Debug for ListEntry<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListEntry")
            .field("position", &self.position())
            .field("value", &self.value())
            .finish()
    }
}

/// Appending and removal by handle avoid scanning the list. Removal compacts
/// when holes exceed one quarter of the live length. Indexed reads scan holes;
/// insertion and explicit compaction take linear time. Slot count is bounded,
/// but allocation capacity may retain a previous high-water mark.
///
/// ```
/// use timetuck::EntryList;
/// let mut list = EntryList::new();
/// let first = list.push("a");
/// let last = list.push("c");
/// list.insert(1, "b").unwrap();
/// list.remove(&first).unwrap();
/// assert_eq!(*last.value(), "c");
/// assert_eq!(last.position(), Some(1));
/// ```
pub struct EntryList<T> {
    owner: Rc<()>,
    revision: Rc<()>,
    slots: Vec<Option<ListEntry<T>>>,
    len: usize,
}
impl<T> Default for EntryList<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> EntryList<T> {
    pub fn new() -> Self {
        Self {
            owner: Rc::new(()),
            revision: Rc::new(()),
            slots: vec![],
            len: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }
    /// Live entries in encounter order, without cloning their payloads. The
    /// iterator borrows the list; cloned handles remain valid across later edits.
    pub fn entries(
        &self,
    ) -> impl DoubleEndedIterator<Item = ListEntry<T>> + std::iter::FusedIterator + '_ {
        self.slots.iter().flatten().cloned()
    }
    pub fn first_entry(&self) -> Option<ListEntry<T>> {
        self.entries().next()
    }
    pub fn last_entry(&self) -> Option<ListEntry<T>> {
        self.entries().next_back()
    }
    fn live_slot(&self, entry: &ListEntry<T>) -> Result<usize, EntryListError> {
        if !Weak::ptr_eq(&entry.0.owner, &Rc::downgrade(&self.owner)) {
            return Err(EntryListError::ForeignEntry);
        }
        entry.0.position.get().ok_or(EntryListError::Removed)
    }
    /// Next live entry, skipping gaps. Rejects removed or foreign handles.
    pub fn next_entry(&self, entry: &ListEntry<T>) -> Result<Option<ListEntry<T>>, EntryListError> {
        let slot = self.live_slot(entry)?;
        Ok(self.slots[slot + 1..].iter().flatten().next().cloned())
    }
    /// Previous live entry, skipping gaps. Rejects removed or foreign handles.
    pub fn previous_entry(
        &self,
        entry: &ListEntry<T>,
    ) -> Result<Option<ListEntry<T>>, EntryListError> {
        let slot = self.live_slot(entry)?;
        Ok(self.slots[..slot].iter().rev().flatten().next().cloned())
    }
    /// Insert before the first live entry. Like indexed insertion, this compacts
    /// gaps and takes linear time; existing entry identities are preserved.
    pub fn push_front(&mut self, value: T) -> ListEntry<T> {
        self.insert(0, value)
            .expect("zero is a valid insertion index")
    }
    /// Insert immediately after a live entry, preserving existing identities.
    /// Invalid handles leave the list and its detached cursors unchanged.
    pub fn insert_after(
        &mut self,
        entry: &ListEntry<T>,
        value: T,
    ) -> Result<ListEntry<T>, EntryListError> {
        let slot = self.live_slot(entry)?;
        let rank = self.slots[..=slot].iter().flatten().count();
        self.insert(rank, value)
    }
    /// Remove all entries and invalidate their handles and detached cursors.
    /// Retained handles keep their last payload. Clearing an empty list is a no-op.
    pub fn clear(&mut self) {
        if self.is_empty() {
            return;
        }
        for entry in self.slots.iter().flatten() {
            entry.0.position.set(None);
        }
        self.len = 0;
        self.changed();
        // Install empty storage before dropping user payloads.
        let removed = std::mem::take(&mut self.slots);
        drop(removed);
    }
    /// Visit each entry present at the start, in encounter order, and remove it
    /// after its callback if still owned by this list. The callback may remove
    /// this or other entries, clear the list, or insert new entries. Every
    /// original entry is visited once,
    /// even if already removed; newly inserted entries remain and are not visited.
    /// If the callback replaces or swaps the list, entries now owned by another
    /// list are left there; callbacks still visit the original captured entries.
    /// A callback panic stops traversal: completed removals remain, while the
    /// current and remaining entries stay unless the callback removed them.
    /// Captures O(n) handles without cloning payloads; repeated removal compacts
    /// gaps using the same policy as ordinary removal.
    pub fn clear_with(&mut self, mut visitor: impl FnMut(&mut Self, ListEntry<T>)) {
        let entries: Vec<_> = self.entries().collect();
        for entry in entries {
            visitor(self, entry.clone());
            if self.live_slot(&entry).is_ok() {
                self.remove(&entry)
                    .expect("captured entry belongs to this list");
            }
        }
    }
    fn changed(&mut self) {
        self.revision = Rc::new(());
    }
    fn entry(&self, position: usize, value: T) -> ListEntry<T> {
        ListEntry(Rc::new(EntryState {
            owner: Rc::downgrade(&self.owner),
            position: Cell::new(Some(position)),
            value: RefCell::new(Rc::new(value)),
        }))
    }
    pub fn push(&mut self, value: T) -> ListEntry<T> {
        let entry = self.entry(self.slots.len(), value);
        self.slots.push(Some(entry.clone()));
        self.len += 1;
        self.changed();
        entry
    }
    /// Invalid indexes leave both the collection and existing cursors unchanged.
    pub fn insert(&mut self, index: usize, value: T) -> Result<ListEntry<T>, EntryListError> {
        if index > self.len {
            return Err(EntryListError::IndexOutOfBounds {
                index,
                len: self.len,
            });
        }
        self.slots.reserve(1);
        self.pack();
        let entry = self.entry(index, value);
        self.slots.insert(index, Some(entry.clone()));
        self.len += 1;
        for (i, entry) in self.slots.iter().enumerate().skip(index + 1) {
            entry.as_ref().unwrap().0.position.set(Some(i));
        }
        self.changed();
        Ok(entry)
    }
    pub fn remove(&mut self, entry: &ListEntry<T>) -> Result<Rc<T>, EntryListError> {
        let position = self.live_slot(entry)?;
        debug_assert!(self.slots[position].as_ref().unwrap().same_entry(entry));
        self.slots[position] = None;
        entry.0.position.set(None);
        self.len -= 1;
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
        }
        if self.slots.len() - self.len > self.len / 4 {
            self.pack();
        }
        self.changed();
        Ok(entry.value())
    }
    fn pack(&mut self) {
        self.slots.retain(Option::is_some);
        for (i, entry) in self.slots.iter().enumerate() {
            entry.as_ref().unwrap().0.position.set(Some(i));
        }
    }
    /// Relocation invalidates detached cursors; entry handles remain valid.
    pub fn compact(&mut self) {
        if self.slots.len() != self.len {
            self.pack();
            self.changed();
        }
    }
    fn slot_of(&self, index: usize) -> Option<usize> {
        if index >= self.len {
            return None;
        }
        if self.slots.len() == self.len {
            return Some(index);
        }
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_some())
            .nth(index)
            .map(|(i, _)| i)
    }
    pub fn get(&self, index: usize) -> Option<Rc<T>> {
        self.slot_of(index)
            .map(|slot| self.slots[slot].as_ref().unwrap().value())
    }
    /// Distinguishes an absent slot from a present entry whose payload is `None`.
    /// Slot reads never compact or mutate the list.
    pub fn entry_at(&self, slot: usize) -> Result<Option<ListEntry<T>>, EntryListError> {
        self.slots
            .get(slot)
            .cloned()
            .ok_or(EntryListError::SlotOutOfBounds {
                slot,
                slot_count: self.slots.len(),
            })
    }
    pub fn snapshot(&self) -> Vec<Rc<T>> {
        self.slots.iter().flatten().map(ListEntry::value).collect()
    }
    /// Compacts before visiting. Callback panics leave a valid, compact list.
    pub fn for_each(&mut self, mut visitor: impl FnMut(&T)) {
        self.compact();
        for entry in self.slots.iter().flatten() {
            visitor(&entry.value());
        }
    }
    pub fn cursor(&self, index: usize) -> Result<EntryCursor<T>, EntryListError> {
        if index > self.len {
            return Err(EntryListError::IndexOutOfBounds {
                index,
                len: self.len,
            });
        }
        Ok(EntryCursor {
            owner: Rc::downgrade(&self.owner),
            revision: Rc::downgrade(&self.revision),
            rank: index,
            next_slot: self.slot_of(index).unwrap_or(self.slots.len()),
            current: None,
        })
    }
}

/// Detached bidirectional cursor. Pass its owning list to each operation.
/// Structural edits through another cursor or directly on the list invalidate it;
/// replacing a value does not. Exhaustion leaves the last returned entry current.
///
/// ```
/// use timetuck::EntryList;
/// let mut list = EntryList::new();
/// list.push(1);
/// list.push(2);
/// let mut cursor = list.cursor(0).unwrap();
/// assert_eq!(*cursor.next(&list).unwrap().unwrap(), 1);
/// cursor.remove(&mut list).unwrap();
/// cursor.add(&mut list, 3).unwrap();
/// assert_eq!(*cursor.previous(&list).unwrap().unwrap(), 3);
/// ```
pub struct EntryCursor<T> {
    owner: Weak<()>,
    revision: Weak<()>,
    rank: usize,
    next_slot: usize,
    current: Option<(ListEntry<T>, bool)>,
}
impl<T> EntryCursor<T> {
    fn check(&self, list: &EntryList<T>) -> Result<(), EntryListError> {
        if !Weak::ptr_eq(&self.owner, &Rc::downgrade(&list.owner)) {
            return Err(EntryListError::ForeignCursor);
        }
        if !Weak::ptr_eq(&self.revision, &Rc::downgrade(&list.revision)) {
            return Err(EntryListError::CursorInvalidated);
        }
        Ok(())
    }
    fn refresh(&mut self, list: &EntryList<T>) {
        self.revision = Rc::downgrade(&list.revision);
        self.next_slot = list.slot_of(self.rank).unwrap_or(list.slots.len());
    }
    pub fn has_next(&self, list: &EntryList<T>) -> Result<bool, EntryListError> {
        self.check(list)?;
        Ok(self.rank < list.len)
    }
    pub fn has_previous(&self, list: &EntryList<T>) -> Result<bool, EntryListError> {
        self.check(list)?;
        Ok(self.rank > 0)
    }
    pub fn next(&mut self, list: &EntryList<T>) -> Result<Option<Rc<T>>, EntryListError> {
        self.check(list)?;
        while self.next_slot < list.slots.len() {
            let slot = self.next_slot;
            self.next_slot += 1;
            if let Some(entry) = &list.slots[slot] {
                self.rank += 1;
                self.current = Some((entry.clone(), true));
                return Ok(Some(entry.value()));
            }
        }
        Ok(None)
    }
    pub fn previous(&mut self, list: &EntryList<T>) -> Result<Option<Rc<T>>, EntryListError> {
        self.check(list)?;
        while self.next_slot > 0 {
            self.next_slot -= 1;
            if let Some(entry) = &list.slots[self.next_slot] {
                self.rank -= 1;
                self.current = Some((entry.clone(), false));
                return Ok(Some(entry.value()));
            }
        }
        Ok(None)
    }
    pub fn remove(&mut self, list: &mut EntryList<T>) -> Result<Rc<T>, EntryListError> {
        self.check(list)?;
        let (entry, forward) = self
            .current
            .as_ref()
            .ok_or(EntryListError::NoCurrentEntry)?;
        let value = list.remove(entry)?;
        if *forward {
            self.rank -= 1;
        }
        self.current = None;
        self.refresh(list);
        Ok(value)
    }
    /// Replaces the current payload; previously returned `Rc` values are snapshots.
    pub fn set(&mut self, list: &mut EntryList<T>, value: T) -> Result<Rc<T>, EntryListError> {
        self.check(list)?;
        let (entry, _) = self
            .current
            .as_ref()
            .ok_or(EntryListError::NoCurrentEntry)?;
        let old = entry.0.value.replace(Rc::new(value));
        Ok(old)
    }
    pub fn add(
        &mut self,
        list: &mut EntryList<T>,
        value: T,
    ) -> Result<ListEntry<T>, EntryListError> {
        self.check(list)?;
        let entry = list.insert(self.rank, value)?;
        self.rank += 1;
        self.current = None;
        self.refresh(list);
        Ok(entry)
    }
}
