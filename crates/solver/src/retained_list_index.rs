//! Director-owned registry positions for committed ordered-list layouts.
use crate::{ListAssignment, ListPosition, ListProblem};
use std::{
    collections::BTreeSet,
    fmt,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetainedListIndexSnapshot {
    pub positions: Vec<Option<ListPosition>>,
    pub synchronizations: u64,
    pub refreshed_entities: u64,
}
struct Data {
    positions: Vec<Option<ListPosition>>,
    rows: Vec<Vec<usize>>,
    all_dirty: bool,
    dirty_rows: BTreeSet<usize>,
    synchronizations: u64,
    refreshed_entities: u64,
}
struct Inner {
    dirty: AtomicBool,
    data: Mutex<Data>,
}
/// A real reusable position table, with no working values or director retained.
/// Copies share the same table. Snapshots describe committed state; temporary
/// scored guards retain the prior committed table. A commit invalidates it until
/// the director's next index request synchronizes touched rows. Closing/replacing
/// the assignment invalidates it too. None denotes stale, not an empty layout.
/// Assignment clones are independent and never inherit this cache connection.
#[derive(Clone)]
pub struct RetainedListIndex {
    inner: Arc<Inner>,
}
impl RetainedListIndex {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                dirty: AtomicBool::new(true),
                data: Mutex::new(Data {
                    positions: vec![],
                    rows: vec![],
                    all_dirty: true,
                    dirty_rows: BTreeSet::new(),
                    synchronizations: 0,
                    refreshed_entities: 0,
                }),
            }),
        }
    }
    pub fn same_instance(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
    pub fn snapshot(&self) -> Option<RetainedListIndexSnapshot> {
        if self.inner.dirty.load(Ordering::Acquire) {
            return None;
        }
        let data = self.inner.data.lock().expect("native list index lock");
        if self.inner.dirty.load(Ordering::Acquire) {
            return None;
        }
        Some(RetainedListIndexSnapshot {
            positions: data.positions.clone(),
            synchronizations: data.synchronizations,
            refreshed_entities: data.refreshed_entities,
        })
    }
    pub fn location(&self, registry_id: usize) -> Option<Option<ListPosition>> {
        if self.inner.dirty.load(Ordering::Acquire) {
            return None;
        }
        let data = self.inner.data.lock().expect("native list index lock");
        if self.inner.dirty.load(Ordering::Acquire) {
            return None;
        }
        data.positions.get(registry_id).copied()
    }
    fn assigned_mask(&self) -> Vec<bool> {
        self.inner
            .data
            .lock()
            .expect("native list index lock")
            .positions
            .iter()
            .map(Option::is_some)
            .collect()
    }
    pub(crate) fn synchronize<V: Eq + Clone>(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) {
        assignment.index_owner.attach(self);
        if !self.inner.dirty.load(Ordering::Acquire) {
            return;
        }
        let (all, entities) = {
            let data = self.inner.data.lock().expect("native list index lock");
            (
                data.all_dirty,
                if data.all_dirty {
                    (0..assignment.lists().len()).collect::<Vec<_>>()
                } else {
                    data.dirty_rows.iter().copied().collect()
                },
            )
        };
        // Equality/hash callbacks and allocation finish before cache publication.
        // A panic leaves the original table stale and the next request retryable.
        let updates: Vec<_> = entities
            .iter()
            .map(|&entity| {
                (
                    entity,
                    assignment.lists()[entity]
                        .iter()
                        .map(|value| {
                            problem
                                .value_index(value)
                                .expect("validated native list registry")
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let mut data = self.inner.data.lock().expect("native list index lock");
        if all {
            data.positions.resize(problem.values().len(), None);
            data.positions.fill(None);
            data.rows.resize_with(assignment.lists().len(), Vec::new);
        } else {
            for &entity in &entities {
                for i in 0..data.rows[entity].len() {
                    let id = data.rows[entity][i];
                    data.positions[id] = None;
                }
            }
        }
        for (entity, ids) in updates {
            for (index, &id) in ids.iter().enumerate() {
                data.positions[id] = Some(ListPosition { entity, index });
            }
            data.rows[entity] = ids;
        }
        data.all_dirty = false;
        data.dirty_rows.clear();
        data.synchronizations = data.synchronizations.saturating_add(1);
        data.refreshed_entities = data
            .refreshed_entities
            .saturating_add(entities.len() as u64);
        self.inner.dirty.store(false, Ordering::Release);
    }
}
/// OnceLock keeps assignments Send/Sync whenever their values are. Links are weak
/// and not cloned, so snapshots cannot keep scoring resources alive or invalidate
/// the live director when a detached snapshot is dropped.
#[derive(Default)]
pub(crate) struct IndexOwner {
    link: OnceLock<Weak<Inner>>,
}
impl IndexOwner {
    pub(crate) fn connect(&self, index: &RetainedListIndex) {
        self.attach(index);
    }
    fn attach(&self, index: &RetainedListIndex) {
        if self.link.get().is_none() {
            let _ = self.link.set(Arc::downgrade(&index.inner));
        }
    }
    pub(crate) fn changed(&self, entities: impl Iterator<Item = usize>) {
        if let Some(index) = self.link.get().and_then(Weak::upgrade) {
            index.dirty.store(true, Ordering::Release);
            let mut data = index.data.lock().expect("native list index lock");
            if !data.all_dirty {
                data.dirty_rows.extend(entities);
            }
        }
    }
    fn invalidate(&self) {
        if let Some(index) = self.link.get().and_then(Weak::upgrade) {
            index.dirty.store(true, Ordering::Release);
            let mut data = index.data.lock().expect("native list index lock");
            data.all_dirty = true;
            data.dirty_rows.clear();
        }
    }
}
impl Clone for IndexOwner {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl PartialEq for IndexOwner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for IndexOwner {}
impl fmt::Debug for IndexOwner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("detached index metadata")
    }
}
impl Drop for IndexOwner {
    fn drop(&mut self) {
        self.invalidate();
    }
}
impl<V: Eq + Clone> ListAssignment<V> {
    pub(crate) fn retained_assigned_mask(&self, problem: &ListProblem<V>) -> Option<Vec<bool>> {
        let index = RetainedListIndex {
            inner: self.index_owner.link.get()?.upgrade()?,
        };
        index.synchronize(problem, self);
        Some(index.assigned_mask())
    }
}
