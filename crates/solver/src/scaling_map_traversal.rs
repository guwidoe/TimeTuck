//! Optional diagnostic traversal that follows the live map representation.
use crate::ScalingMap;
impl<K: Ord, V> ScalingMap<K, V> {
    /// Follow physical sorted-array encounter order while array-backed, ignoring
    /// `reversed`; after promotion, follow the requested tree order. This is a
    /// backend-sensitive diagnostic traversal, not ordinary semantic ordering.
    /// `iter()` and `range()` always retain their standard double-ended contract.
    /// Borrows the actual storage without copying keys/values or requiring Clone;
    /// reopen after edits to observe promotion, removals and changed membership.
    pub fn storage_traversal(&self, reversed: bool) -> impl Iterator<Item = (&K, &V)> {
        let reverse = reversed && !self.is_array_based();
        let mut entries = self.iter();
        std::iter::from_fn(move || {
            if reverse {
                entries.next_back()
            } else {
                entries.next()
            }
        })
    }
}
