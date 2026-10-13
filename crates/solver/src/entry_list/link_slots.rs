//! Inspectable links without intrusive pointers or retained neighbor ownership.
use super::*;
impl<T> EntryList<T> {
    /// Current previous/next links, or two empty slots after removal. Foreign
    /// handles are rejected even when removed. Links are resolved from current
    /// list storage; handles never store or retain neighbor pointers. Compaction
    /// and insertion therefore cannot leave a stale link in a retained handle.
    /// Strict previous_entry/next_entry queries still reject removed entries.
    pub fn link_slots(
        &self,
        entry: &ListEntry<T>,
    ) -> Result<[Option<ListEntry<T>>; 2], EntryListError> {
        if !Weak::ptr_eq(&entry.0.owner, &Rc::downgrade(&self.owner)) {
            return Err(EntryListError::ForeignEntry);
        }
        if entry.0.position.get().is_none() {
            return Ok([None, None]);
        }
        Ok([self.previous_entry(entry)?, self.next_entry(entry)?])
    }
}
