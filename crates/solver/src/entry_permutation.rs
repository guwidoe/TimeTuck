//! Optional fair permutations of current stable list entries.
use crate::{EntryList, ListEntry, UniqueIndices, UniqueSampleError, unique_indices};
use std::iter::FusedIterator;
impl<T> EntryList<T> {
    /// Capture live handles in encounter order, then visit without replacement.
    /// The borrow prevents structural edits until traversal ends. Removed entries
    /// are omitted on reopening. Payloads need not implement Clone; equal payloads
    /// retain distinct entry identities. Storage is linear in live membership.
    /// Uniform bounded draws give each permutation equal probability. Invalid
    /// draws yield one error then fuse; singleton/empty lists draw nothing.
    /// Native swap-table order differs from Java mock draw traces.
    pub fn randomized_entries_with<'a>(
        &'a self,
        draw: impl FnMut(u128) -> u128 + 'a,
    ) -> impl FusedIterator<Item = Result<ListEntry<T>, UniqueSampleError>> + 'a {
        let entries: Vec<_> = self.entries().collect();
        UniqueIndices::with_draw(entries.len() as u128, draw)
            .map(move |i| i.map(|i| entries[i as usize].clone()))
    }
    /// Seeded native fair traversal using the same bounded wide generator as
    /// unique candidate/range sampling. Reopening the same membership and seed
    /// reproduces the same identity order, without changing list encounter order.
    pub fn randomized_entries(
        &self,
        seed: u64,
    ) -> impl FusedIterator<Item = Result<ListEntry<T>, UniqueSampleError>> + '_ {
        let entries: Vec<_> = self.entries().collect();
        unique_indices(entries.len() as u128, seed)
            .map(move |i| i.map(|i| entries[i as usize].clone()))
    }
}
