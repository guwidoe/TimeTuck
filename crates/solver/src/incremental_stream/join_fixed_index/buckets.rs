//! Fixed equality joins may locate primitive bucket keys through the directory.
//! Members and their canonical Arc owners keep the original replacement policy.
use super::{Members, SnapshotMap};
use crate::primitive_rows::PrimitiveRows;
use std::{ops::Bound, sync::Arc};

pub(super) enum Buckets<K> {
    Tree(SnapshotMap<K, Members>),
    Paged(Box<PrimitiveRows<K, Members>>),
}
impl<K> Clone for Buckets<K> {
    fn clone(&self) -> Self {
        match self {
            Self::Tree(rows) => Self::Tree(rows.clone()),
            Self::Paged(rows) => Self::Paged(rows.clone()),
        }
    }
}
impl<K> Buckets<K> {
    pub(super) fn tree(&self) -> &SnapshotMap<K, Members> {
        match self {
            Self::Tree(rows) => rows,
            Self::Paged(_) => unreachable!("tree fixed bucket operation"),
        }
    }
    pub(super) fn is_paged(&self) -> bool {
        matches!(self, Self::Paged(_))
    }
    pub(super) fn shared_entries(&self) -> Vec<(Arc<K>, Arc<Members>)> {
        match self {
            Self::Tree(rows) => rows.shared_entries(),
            Self::Paged(rows) => rows.shared_entries(),
        }
    }
    pub(super) fn clear(&mut self) {
        match self {
            Self::Tree(rows) => rows.clear(),
            Self::Paged(rows) => rows.clear(),
        }
    }
    pub(super) fn begin_journal(&mut self) -> usize {
        match self {
            Self::Tree(rows) => rows.begin_journal(),
            Self::Paged(rows) => rows.begin_journal(),
        }
    }
    pub(super) fn commit_journal(&mut self, token: usize) {
        match self {
            Self::Tree(rows) => rows.commit_journal(token),
            Self::Paged(rows) => rows.commit_journal(token),
        }
    }
    pub(super) fn rollback_journal(&mut self, token: usize) {
        match self {
            Self::Tree(rows) => rows.rollback_journal(token),
            Self::Paged(rows) => rows.rollback_journal(token),
        }
    }
    pub(super) fn install_snapshot(&mut self, saved: Self) {
        match (self, saved) {
            (Self::Tree(rows), Self::Tree(saved)) => rows.install_snapshot(saved),
            (Self::Paged(rows), Self::Paged(saved)) => rows.install_snapshot(*saved),
            // A full snapshot can predate private graph activation. The working
            // binding stays unchanged while exact saved key/value owners transfer.
            (Self::Paged(rows), Self::Tree(saved)) => {
                rows.install_snapshot(PrimitiveRows::from_snapshot_map(saved));
            }
            (Self::Tree(rows), Self::Paged(saved)) => {
                rows.install_snapshot(saved.materialized().clone());
            }
        }
    }
}
impl<K: Ord> Buckets<K> {
    #[inline]
    pub(super) fn get(&self, key: &K) -> Option<&Members> {
        match self {
            Self::Tree(rows) => rows.get(key),
            Self::Paged(rows) => rows.get(key),
        }
    }
    pub(super) fn insert(&mut self, key: K, value: Members) {
        match self {
            Self::Tree(rows) => rows.insert(key, value),
            Self::Paged(rows) => rows.insert(key, value),
        }
    }
    pub(super) fn remove(&mut self, key: &K) {
        match self {
            Self::Tree(rows) => {
                rows.remove(key);
            }
            Self::Paged(rows) => {
                rows.remove(key);
            }
        }
    }
    pub(super) fn range<'a>(
        &'a self,
        start: Bound<&'a K>,
        end: Bound<&'a K>,
    ) -> crate::snapshot_map::Range<'a, K, Members> {
        match self {
            Self::Tree(rows) => rows.range(start, end),
            // Graph bindings only select pages for equality. Keep cold internal
            // inspection of another relation valid without changing row owners.
            Self::Paged(rows) => rows.materialized().range(start, end),
        }
    }
}
impl<K: 'static> Buckets<K> {
    pub(super) fn bind_primitive(&mut self) -> bool {
        if self.is_paged() {
            return true;
        }
        let Self::Tree(rows) = std::mem::replace(self, Self::Tree(SnapshotMap::new())) else {
            unreachable!()
        };
        let mut rows = PrimitiveRows::from_snapshot_map(rows);
        let bound = rows.bind_primitive();
        *self = if bound {
            Self::Paged(Box::new(rows))
        } else {
            Self::Tree(rows.materialized().clone())
        };
        bound
    }
}
