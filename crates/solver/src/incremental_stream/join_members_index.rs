//! Unbounded private numeric equality buckets. Default/custom/ordered indexes
//! retain original SnapshotMap members. Empty consumed pair activation alone
//! selects direct outer pages and sorted full-ID vectors; no ID is narrowed.
use crate::{SnapshotMap, primitive_rows::PrimitiveRows};
use std::{ops::Bound, sync::Arc};
type Copier<I> = fn(&I) -> I;
pub(super) enum Members<I> {
    Shared(SnapshotMap<I, ()>),
    Numeric(Vec<I>, Copier<I>),
}
impl<I> Clone for Members<I> {
    fn clone(&self) -> Self {
        match self {
            Self::Shared(r) => Self::Shared(r.clone()),
            Self::Numeric(ids, copy) => Self::Numeric(ids.iter().map(copy).collect(), *copy),
        }
    }
}
impl<I> Members<I> {
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Shared(r) => r.len(),
            Self::Numeric(ids, _) => ids.len(),
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn keys(&self) -> impl Iterator<Item = &I> {
        let (shared, numeric) = match self {
            Self::Shared(r) => (Some(r.keys()), None),
            Self::Numeric(ids, _) => (None, Some(ids.iter())),
        };
        shared
            .into_iter()
            .flatten()
            .chain(numeric.into_iter().flatten())
    }
    pub(super) fn for_each<O>(&self, output: &mut Vec<O>, mut emit: impl FnMut(&mut Vec<O>, &I)) {
        match self {
            Self::Shared(r) => {
                for id in r.keys() {
                    emit(output, id);
                }
            }
            Self::Numeric(ids, _) => {
                for id in ids {
                    emit(output, id);
                }
            }
        }
    }
    pub(super) fn shares_root(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Shared(a), Self::Shared(b)) => a.shares_root(b),
            (Self::Numeric(..), Self::Numeric(..)) => std::ptr::eq(self, other),
            _ => false,
        }
    }
    fn shared_map(&self) -> SnapshotMap<I, ()> {
        match self {
            Self::Shared(r) => r.clone(),
            Self::Numeric(ids, copy) => SnapshotMap::from_shared_sorted(
                ids.iter()
                    .map(|id| (Arc::new(copy(id)), Arc::new(())))
                    .collect(),
            ),
        }
    }
}
impl<I: Ord> Members<I> {
    pub(super) fn insert(&mut self, id: I, _unit: ()) {
        match self {
            Self::Shared(r) => r.insert(id, ()),
            Self::Numeric(ids, _) => {
                if let Err(i) = ids.binary_search(&id) {
                    ids.insert(i, id);
                }
            }
        }
    }
    pub(super) fn remove(&mut self, id: &I) {
        match self {
            Self::Shared(r) => {
                r.remove(id);
            }
            Self::Numeric(ids, _) => {
                if let Ok(i) = ids.binary_search(id) {
                    ids.remove(i);
                }
            }
        }
    }
}
pub(super) struct Index<K, I> {
    rows: PrimitiveRows<K, Members<I>>,
    copy: Option<Copier<I>>,
}
impl<K, I> Clone for Index<K, I> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            copy: self.copy,
        }
    }
}
impl<K, I> Index<K, I> {
    pub(super) fn new() -> Self {
        Self {
            rows: PrimitiveRows::new(),
            copy: None,
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub(super) fn empty_members(&self) -> Members<I> {
        match self.copy {
            Some(copy) => Members::Numeric(Vec::new(), copy),
            None => Members::Shared(SnapshotMap::new()),
        }
    }
    pub(super) fn clear(&mut self) {
        self.rows.clear();
    }
    pub(super) fn begin_journal(&mut self) -> usize {
        self.rows.begin_journal()
    }
    pub(super) fn commit_journal(&mut self, t: usize) {
        self.rows.commit_journal(t);
    }
    pub(super) fn rollback_journal(&mut self, t: usize) {
        self.rows.rollback_journal(t);
    }
    pub(super) fn install_snapshot(&mut self, saved: Self) {
        self.rows.install_snapshot(saved.rows);
    }
    pub(super) fn shared_entries(&self) -> SharedEntries<K, I> {
        self.rows
            .shared_entries()
            .into_iter()
            .map(|(k, v)| (k, Arc::new(v.shared_map())))
            .collect()
    }
    pub(super) fn shares_root(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        self.copy.is_none()
            && other.copy.is_none()
            && self
                .rows
                .materialized()
                .shares_root(other.rows.materialized())
    }
}
impl<K: Ord, I> Index<K, I> {
    pub(super) fn get(&self, key: &K) -> Option<&Members<I>> {
        self.rows.get(key)
    }
    pub(super) fn insert(&mut self, key: K, value: Members<I>) {
        self.rows.insert(key, value);
    }
    pub(super) fn remove(&mut self, key: &K) {
        self.rows.remove(key);
    }
    pub(super) fn range<'a>(
        &'a self,
        start: Bound<&'a K>,
        end: Bound<&'a K>,
    ) -> crate::snapshot_map::Range<'a, K, Members<I>> {
        self.rows.materialized().range(start, end)
    }
}
impl<K: 'static, I> Index<K, I> {
    // Activation preflights both key/value certificates and empty sides first.
    pub(super) fn enable_numeric(&mut self, copy: Copier<I>) -> bool {
        if !self.is_empty() || !self.rows.bind_primitive() {
            return false;
        }
        self.copy = Some(copy);
        true
    }
}
#[cfg(test)]
#[allow(clippy::items_after_test_module)] // The trailing alias keeps body locations stable.
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    type Pair = (usize, usize);
    type Expected = BTreeMap<Pair, BTreeSet<Pair>>;
    fn contents(index: &Index<Pair, Pair>) -> Expected {
        index
            .rows
            .iter()
            .map(|(k, v)| (*k, v.keys().copied().collect()))
            .collect()
    }
    #[test]
    fn numeric_join_index_full_ids_member_order_clone_independence_nested_undo_and_full_install() {
        let mut index = Index::<Pair, Pair>::new();
        assert!(index.enable_numeric(|id| *id));
        let empty = index.clone();
        let mut expected = Expected::new();
        let outer = index.begin_journal();
        for step in 0usize..180 {
            let key = (step % 7, if step % 11 == 0 { usize::MAX } else { step % 3 });
            let id = (
                if step % 13 == 0 {
                    usize::MAX
                } else {
                    step % 17
                },
                if step % 5 == 0 { usize::MAX } else { step % 11 },
            );
            let mut members = index
                .get(&key)
                .cloned()
                .unwrap_or_else(|| index.empty_members());
            let child = index.begin_journal();
            if step % 3 == 0 {
                members.remove(&id);
                if let Some(ids) = expected.get_mut(&key) {
                    ids.remove(&id);
                    if ids.is_empty() {
                        expected.remove(&key);
                    }
                }
            } else {
                members.insert(id, ());
                expected.entry(key).or_default().insert(id);
            }
            if members.is_empty() {
                index.remove(&key);
            } else {
                index.insert(key, members);
            }
            index.commit_journal(child);
            assert_eq!(contents(&index), expected);
        }
        let saved = index.clone();
        let saved_contents = contents(&saved);
        index.clear();
        assert!(contents(&index).is_empty());
        assert_eq!(contents(&saved), saved_contents);
        index.rollback_journal(outer);
        assert!(contents(&index).is_empty());
        let parent = index.begin_journal();
        index.install_snapshot(saved);
        assert_eq!(contents(&index), saved_contents);
        index.rollback_journal(parent);
        assert!(contents(&index).is_empty());
        index.install_snapshot(empty);
        assert!(contents(&index).is_empty());
    }
}

// Named cold handoff type;definition follows bodies to retain source locations.
type SharedEntries<K, I> = Vec<(Arc<K>, Arc<SnapshotMap<I, ()>>)>;
