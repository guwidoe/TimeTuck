//! Private join captured rows. Shared/borrowed/public routes keep all owners.
//! Only empty ownership-consumed numeric pair sides with certified builtin
//! captured values select copied cells. Index representatives remain unchanged.
use crate::{
    SnapshotMap,
    primitive_rows::{PrimitiveRows, RowsCapture, consumed_scores},
};
use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};
pub(super) enum Rows<I, V> {
    Shared(PrimitiveRows<I, V>),
    Copied(Box<consumed_scores::Rows<I, V>>),
}
pub(super) enum Capture<I, V> {
    Shared(RowsCapture<I, V>),
    Copied(consumed_scores::Capture<I, V>),
}
impl<I, V> Clone for Rows<I, V> {
    fn clone(&self) -> Self {
        match self {
            Self::Shared(r) => Self::Shared(r.clone()),
            Self::Copied(r) => Self::Copied(Box::new((**r).clone())),
        }
    }
}
impl<I, V> Rows<I, V> {
    pub(super) fn new() -> Self {
        Self::Shared(PrimitiveRows::new())
    }
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Shared(r) => r.len(),
            Self::Copied(r) => r.len(),
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn clear(&mut self) {
        match self {
            Self::Shared(r) => r.clear(),
            Self::Copied(r) => r.clear(),
        }
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
        let (shared, copied) = match self {
            Self::Shared(r) => (Some(r), None),
            Self::Copied(r) => (None, Some(r)),
        };
        shared
            .into_iter()
            .flat_map(|r| r.iter())
            .chain(copied.into_iter().flat_map(|r| r.iter()))
    }
    pub(super) fn shared_entries(&self) -> Vec<(Arc<I>, Arc<V>)> {
        match self {
            Self::Shared(r) => r.shared_entries(),
            Self::Copied(r) => r.shared_captured_entries(),
        }
    }
    pub(super) fn begin_journal(&mut self) -> usize {
        match self {
            Self::Shared(r) => r.begin_journal(),
            Self::Copied(r) => r.begin(),
        }
    }
    pub(super) fn commit_journal(&mut self, t: usize) {
        match self {
            Self::Shared(r) => r.commit_journal(t),
            Self::Copied(r) => r.commit(t),
        }
    }
    pub(super) fn rollback_journal(&mut self, t: usize) {
        match self {
            Self::Shared(r) => r.rollback_journal(t),
            Self::Copied(r) => r.rollback(t),
        }
    }
    pub(super) fn capture_current(&self, t: usize) -> Capture<I, V> {
        match self {
            Self::Shared(r) => Capture::Shared(r.capture_current(t)),
            Self::Copied(r) => Capture::Copied(r.capture(t)),
        }
    }
    pub(super) fn install_capture(&mut self, c: Capture<I, V>) {
        match (self, c) {
            (Self::Shared(r), Capture::Shared(c)) => r.install_capture(c),
            (Self::Copied(r), Capture::Copied(c)) => r.install_capture(c),
            _ => panic!("join captured row binding changed"),
        }
    }
    pub(super) fn install_snapshot(&mut self, saved: Self) {
        match (self, saved) {
            (Self::Shared(r), Self::Shared(s)) => r.install_snapshot(s),
            (Self::Copied(r), Self::Copied(s)) => r.install(*s),
            (Self::Copied(r), Self::Shared(s)) => {
                r.install_shared_captured_entries(s.shared_entries())
            }
            (Self::Shared(r), Self::Copied(s)) => {
                r.install_snapshot(PrimitiveRows::from_snapshot_map(
                    SnapshotMap::from_shared_sorted(s.shared_captured_entries()),
                ))
            }
        }
    }
    pub(super) fn journal_transaction(&mut self) -> Transaction<'_, I, V> {
        let token = self.begin_journal();
        Transaction {
            rows: self,
            token: Some(token),
        }
    }
    #[cfg(test)]
    pub(super) fn is_primitive(&self) -> bool {
        match self {
            Self::Shared(r) => r.is_primitive(),
            Self::Copied(_) => true,
        }
    }
    #[cfg(test)]
    pub(super) fn is_copied(&self) -> bool {
        matches!(self, Self::Copied(_))
    }
    #[cfg(test)]
    pub(super) fn materialized(&self) -> &SnapshotMap<I, V> {
        match self {
            Self::Shared(r) => r.materialized(),
            Self::Copied(_) => panic!("copied private join rows have no shared materialized view"),
        }
    }
}
impl<I: Ord, V> Rows<I, V> {
    pub(super) fn get(&self, id: &I) -> Option<&V> {
        match self {
            Self::Shared(r) => r.get(id),
            Self::Copied(r) => r.get_captured(id),
        }
    }
    pub(super) fn insert(&mut self, id: I, value: V) {
        match self {
            Self::Shared(r) => r.insert(id, value),
            Self::Copied(r) => r.insert_captured(id, value),
        }
    }
    pub(super) fn remove(&mut self, id: &I) {
        match self {
            Self::Shared(r) => {
                r.remove(id);
            }
            Self::Copied(r) => r.remove_captured(id),
        }
    }
}
impl<I: 'static, V> Rows<I, V> {
    pub(super) fn bind_primitive(&mut self) -> bool {
        match self {
            Self::Shared(r) => r.bind_primitive(),
            Self::Copied(_) => true,
        }
    }
}
impl<I: Clone + 'static, V: Clone + 'static> Rows<I, V> {
    // The only caller has checked the existing exact captured-value certificate
    // and empty whole-graph activation before any mutation of either side.
    pub(super) fn enable_copied(&mut self, copy: fn(&V) -> V) -> bool {
        if !self.is_empty() {
            return false;
        }
        let Some(rows) = consumed_scores::Rows::new_captured(copy) else {
            return false;
        };
        *self = Self::Copied(Box::new(rows));
        true
    }
}
pub(super) struct Transaction<'a, I, V> {
    rows: &'a mut Rows<I, V>,
    token: Option<usize>,
}
impl<I, V> Deref for Transaction<'_, I, V> {
    type Target = Rows<I, V>;
    fn deref(&self) -> &Self::Target {
        self.rows
    }
}
impl<I, V> DerefMut for Transaction<'_, I, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.rows
    }
}
impl<I, V> Transaction<'_, I, V> {
    pub(super) fn commit(mut self) {
        self.rows.commit_journal(self.token.take().unwrap());
    }
}
impl<I, V> Drop for Transaction<'_, I, V> {
    fn drop(&mut self) {
        if let Some(t) = self.token.take() {
            self.rows.rollback_journal(t);
        }
    }
}
