//! Atomic dependency rows whose active key universe is discovered at runtime.
use crate::{
    DependencyCache, DependencyError, DependencyTopology, DerivedChange, DerivedValue, SnapshotMap,
    TupleChange,
};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
};

/// One declared dependency, in the caller's original order, including duplicates.
/// `row == None` identifies a referenced but absent source. A present source may
/// independently have a consistent absent derived value. Inconsistent inputs
/// suppress the consuming supplier as for fixed-universe dependency nodes.
pub struct DynamicDependencyInput<'a, K, T, V> {
    pub key: &'a K,
    pub row: Option<&'a T>,
    pub derived: &'a DerivedValue<V>,
}
type Dependencies<'a, K, T> = dyn Fn(&T) -> Vec<K> + 'a;
type Supplier<'a, K, T, V> =
    dyn Fn(&K, &T, &[DynamicDependencyInput<'_, K, T, V>]) -> Option<V> + 'a;
struct State<K, T, V> {
    rows: SnapshotMap<K, T>,
    dependencies: SnapshotMap<K, Vec<K>>,
    references: SnapshotMap<K, SnapshotMap<K, ()>>,
    cache: DependencyCache<K, V>,
}
impl<K, T, V> Clone for State<K, T, V> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            dependencies: self.dependencies.clone(),
            references: self.references.clone(),
            cache: self.cache.clone(),
        }
    }
}
/// Incremental derived rows with runtime key arrival, removal and reclamation.
/// The active universe contains every present row and every declared source key.
/// Referenced absent sources have consistent `None` values and no supplier calls.
/// Keys absent and unreferenced after a complete batch are reclaimed; reinsertion
/// starts from an absent cached value. Cycles and their descendants are marked
/// inconsistent until the final graph recovers, without invoking those suppliers.
///
/// Batches retain the last update per key, inspect final present rows in key order,
/// and evaluate the final graph once. Source lists retain order and duplicates in
/// supplier inputs; structural edges are deduplicated. A changed row always dirties
/// its direct consumers because suppliers may inspect source presence/metadata,
/// even when its derived value stays equal. Further propagation stops at equal
/// derived values. Output changes are unique and sorted by key; changed source row
/// metadata is emitted even when its derived value is equal.
///
/// Existing-universe updates use incremental dependency scheduling. Membership
/// changes rebuild topology over the complete active universe (with ordered-map
/// lookup costs), retaining surviving cached values and skipping unrelated
/// suppliers. This is not constant-cost insertion/removal. Keys, rows and values
/// must be immutable. Callback, comparison and clone panics preserve the node;
/// captured callback effects and panicking destructors cannot be rolled back.
/// Use [`crate::StreamGraphBuilder::derive_dynamic_dependencies`] for transaction
/// checkpoints, downstream scoring and diagnostic rebuilds.
pub struct DynamicDependencyNode<'a, K, T, V> {
    state: State<K, T, V>,
    dependencies: Box<Dependencies<'a, K, T>>,
    supplier: Box<Supplier<'a, K, T, V>>,
}
impl<'a, K: Ord + Clone, T: Clone, V: Clone + Eq> DynamicDependencyNode<'a, K, T, V> {
    pub fn new(
        dependencies: impl Fn(&T) -> Vec<K> + 'a,
        supplier: impl Fn(&K, &T, &[DynamicDependencyInput<'_, K, T, V>]) -> Option<V> + 'a,
    ) -> Self {
        Self {
            state: Self::empty_state(),
            dependencies: Box::new(dependencies),
            supplier: Box::new(supplier),
        }
    }
    fn empty_state() -> State<K, T, V> {
        State {
            rows: SnapshotMap::new(),
            dependencies: SnapshotMap::new(),
            references: SnapshotMap::new(),
            cache: DependencyCache::new(
                DependencyTopology::new([]).unwrap_or_else(|_| unreachable!("empty key universe")),
                |_, _| None,
            ),
        }
    }
    /// Sorted present and referenced keys; absent/unreferenced keys are reclaimed.
    pub fn keys(&self) -> &[K] {
        self.state.cache.topology().keys()
    }
    pub fn rows(&self) -> impl Iterator<Item = (&K, (&T, &DerivedValue<V>))> {
        self.state
            .rows
            .iter()
            .map(|(key, row)| (key, (row, &self.state.cache.values()[key])))
    }
    pub fn apply(
        &mut self,
        change: TupleChange<K, T>,
    ) -> Result<Vec<DerivedChange<K, T, V>>, DependencyError<K>> {
        self.apply_batch([change])
    }
    /// Apply one atomic final-state batch. Retractions of absent keys do nothing;
    /// a superseded insertion followed by retraction neither registers its keys
    /// nor invokes its callbacks. No intermediate graph or output is published.
    pub fn apply_batch(
        &mut self,
        changes: impl IntoIterator<Item = TupleChange<K, T>>,
    ) -> Result<Vec<DerivedChange<K, T, V>>, DependencyError<K>> {
        let mut latest = BTreeMap::new();
        for change in changes {
            latest.insert(change.id, change.value);
        }
        let mut next = self.state.clone();
        let mut touched = BTreeSet::new();
        let mut edited = BTreeSet::new();
        let mut removals = Vec::new();
        let mut additions = Vec::new();
        for (key, row) in latest {
            if row.is_none() && !next.rows.contains_key(&key) {
                continue;
            }
            let previous: BTreeSet<_> = self
                .state
                .dependencies
                .get(&key)
                .into_iter()
                .flatten()
                .cloned()
                .collect();
            let declared = row
                .as_ref()
                .map_or_else(Vec::new, |row| (self.dependencies)(row));
            let incoming: BTreeSet<_> = declared.iter().cloned().collect();
            for source in previous.difference(&incoming) {
                let mut consumers = next
                    .references
                    .get(source)
                    .expect("declared source reference")
                    .clone();
                consumers.remove(&key);
                if consumers.is_empty() {
                    next.references.remove(source);
                } else {
                    next.references.insert(source.clone(), consumers);
                }
                removals.push((key.clone(), source.clone()));
            }
            for source in incoming.difference(&previous) {
                let mut consumers = next.references.get(source).cloned().unwrap_or_default();
                consumers.insert(key.clone(), ());
                next.references.insert(source.clone(), consumers);
                additions.push((key.clone(), source.clone()));
            }
            touched.extend(previous);
            touched.extend(incoming);
            touched.insert(key.clone());
            if declared.is_empty() {
                next.dependencies.remove(&key);
            } else {
                next.dependencies.insert(key.clone(), declared);
            }
            if let Some(row) = row {
                next.rows.insert(key.clone(), row);
            } else {
                next.rows.remove(&key);
            }
            edited.insert(key);
        }
        let mut inserted = BTreeSet::new();
        let mut removed = BTreeSet::new();
        for key in touched {
            let wanted = next.rows.contains_key(&key) || next.references.contains_key(&key);
            let known = self.state.cache.values().contains_key(&key);
            if wanted && !known {
                inserted.insert(key);
            } else if !wanted && known {
                removed.insert(key);
            }
        }
        let mut dirty = edited.clone();
        for key in &edited {
            if let Some(consumers) = next.references.get(key) {
                dirty.extend(consumers.keys().cloned());
            }
        }
        dirty.retain(|key| !removed.contains(key));
        let dirty: Vec<_> = dirty.into_iter().collect();
        let supplier = &self.supplier;
        let rows = &next.rows;
        let declarations = &next.dependencies;
        let evaluate = |key: &K, inputs: &[(&K, &DerivedValue<V>)]| {
            rows.get(key).and_then(|row| {
                let ordered: Vec<_> = declarations
                    .get(key)
                    .into_iter()
                    .flatten()
                    .map(|source| {
                        let at = inputs
                            .binary_search_by(|(key, _)| (*key).cmp(source))
                            .expect("declared dependency in final topology");
                        DynamicDependencyInput {
                            key: inputs[at].0,
                            row: rows.get(source),
                            derived: inputs[at].1,
                        }
                    })
                    .collect();
                supplier(key, row, &ordered)
            })
        };
        let changed = if inserted.is_empty() && removed.is_empty() {
            next.cache.update(&removals, &additions, &dirty, evaluate)?
        } else {
            let keys = self
                .keys()
                .iter()
                .filter(|key| !removed.contains(*key))
                .cloned()
                .chain(inserted);
            let mut topology = DependencyTopology::new(keys)?;
            let edges: Vec<_> = next
                .dependencies
                .iter()
                .flat_map(|(target, sources)| {
                    sources
                        .iter()
                        .map(move |source| (target.clone(), source.clone()))
                })
                .collect();
            topology.update(&[], &edges)?;
            next.cache.reconcile_topology(topology, &dirty, evaluate)?
        };
        let affected: BTreeSet<_> = edited.into_iter().chain(changed).collect();
        let output = affected
            .into_iter()
            .filter_map(|key| {
                if let Some(row) = next.rows.get(&key) {
                    Some(TupleChange::put(
                        key.clone(),
                        (row.clone(), next.cache.values()[&key].clone()),
                    ))
                } else if self.state.rows.contains_key(&key) {
                    Some(TupleChange::retract(key))
                } else {
                    None
                }
            })
            .collect();
        self.state = next;
        Ok(output)
    }
    pub(crate) fn clear(&mut self) {
        self.state = Self::empty_state();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        K: 'static,
        T: 'static,
        V: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, saved: Box<dyn Any>)
    where
        K: 'static,
        T: 'static,
        V: 'static,
    {
        self.state = *saved.downcast().expect("dynamic dependency checkpoint");
    }
}
