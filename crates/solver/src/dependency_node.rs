//! Transactional dependency evaluation for keyed stream rows.
use crate::{
    DependencyCache, DependencyError, DependencyTopology, DerivedValue, SnapshotMap, TupleChange,
};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
};

pub type DerivedChange<K, T, V> = TupleChange<K, (T, DerivedValue<V>)>;

type Dependencies<'a, K, T> = dyn Fn(&T) -> Vec<K> + 'a;
type Supplier<'a, K, T, V> = dyn Fn(&K, &T, &[(&K, &DerivedValue<V>)]) -> Option<V> + 'a;
struct State<K, T, V> {
    rows: SnapshotMap<K, T>,
    cache: DependencyCache<K, V>,
}
impl<K, T, V> Clone for State<K, T, V> {
    fn clone(&self) -> Self {
        Self {
            rows: self.rows.clone(),
            cache: self.cache.clone(),
        }
    }
}
/// Derived values for a fixed universe of keyed rows. Missing rows are consistent
/// unavailable values. Cycles and downstream nodes are inconsistent without
/// supplier calls. Dependencies may point to a declared but absent row.
///
/// Use [`crate::StreamGraphBuilder::derive_dependencies`] for graph routing and
/// transaction checkpoints. Output payloads retain the source row alongside its
/// derived value. Updating source metadata emits that row even if its derived
/// result stays equal; dependents emit only when their derived result changes.
/// Keys, rows, values and callback captures must be immutable snapshots.
pub struct DependencyNode<'a, K, T, V> {
    initial: DependencyCache<K, V>,
    state: State<K, T, V>,
    dependencies: Box<Dependencies<'a, K, T>>,
    supplier: Box<Supplier<'a, K, T, V>>,
}
impl<'a, K: Ord + Clone, T: Clone, V: Clone + Eq> DependencyNode<'a, K, T, V> {
    pub fn new(
        keys: impl IntoIterator<Item = K>,
        dependencies: impl Fn(&T) -> Vec<K> + 'a,
        supplier: impl Fn(&K, &T, &[(&K, &DerivedValue<V>)]) -> Option<V> + 'a,
    ) -> Result<Self, DependencyError<K>> {
        let initial = DependencyCache::new(DependencyTopology::new(keys)?, |_, _| None);
        Ok(Self {
            state: State {
                rows: SnapshotMap::new(),
                cache: initial.clone(),
            },
            initial,
            dependencies: Box::new(dependencies),
            supplier: Box::new(supplier),
        })
    }
    pub fn rows(&self) -> impl Iterator<Item = (&K, (&T, &DerivedValue<V>))> {
        self.state
            .rows
            .iter()
            .map(|(k, t)| (k, (t, &self.state.cache.values()[k])))
    }
    pub fn apply(
        &mut self,
        change: TupleChange<K, T>,
    ) -> Result<Vec<DerivedChange<K, T, V>>, DependencyError<K>> {
        let old: Vec<_> = self
            .state
            .cache
            .topology()
            .dependencies(&change.id)?
            .into_iter()
            .cloned()
            .collect();
        if change.value.is_none() && !self.state.rows.contains_key(&change.id) {
            return Ok(vec![]);
        }
        let mut incoming = change
            .value
            .as_ref()
            .map_or_else(Vec::new, |row| (self.dependencies)(row));
        incoming.sort();
        incoming.dedup();
        let removals: Vec<_> = old
            .iter()
            .filter(|k| incoming.binary_search(k).is_err())
            .map(|k| (change.id.clone(), k.clone()))
            .collect();
        let additions: Vec<_> = incoming
            .iter()
            .filter(|k| old.binary_search(k).is_err())
            .map(|k| (change.id.clone(), k.clone()))
            .collect();
        let mut next = self.state.clone();
        if let Some(row) = change.value {
            next.rows.insert(change.id.clone(), row);
        } else {
            next.rows.remove(&change.id);
        }
        let changed = next.cache.update(
            &removals,
            &additions,
            std::slice::from_ref(&change.id),
            |key, inputs| {
                next.rows
                    .get(key)
                    .and_then(|row| (self.supplier)(key, row, inputs))
            },
        )?;
        let mut affected: BTreeSet<_> = changed.into_iter().collect();
        affected.insert(change.id.clone());
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
    /// Settle a complete round of row changes against its final dependency graph.
    /// Repeated keys keep only their last change. Every mentioned key must belong
    /// to the declared universe, including absent retractions; IDs are validated
    /// before invoking dependency callbacks. Dependencies are read once per final
    /// present row, in key order. Superseded rows are never inspected or evaluated.
    /// Removing a row absent before and after the round produces no work/output.
    ///
    /// Suppliers observe all final rows and edges together, so coordinated rewires
    /// cannot expose transient cycles or intermediate derived values. Final cycles
    /// remain inconsistent. Acyclic suppliers run at most once in dependency order;
    /// equal results stop propagation. Each affected present row emits one final
    /// replacement, including explicitly changed metadata with equal derived values.
    /// Retractions emit only for previously present rows. Outputs are sorted by key.
    ///
    /// Metadata errors and ordinary callback/clone/comparison panics preserve the
    /// entire node. External callback effects cannot be rolled back; callbacks must
    /// read immutable inputs. Use separate [`Self::apply`] calls when intermediate
    /// states are intentional, or [`crate::StreamGraphBuilder::derive_dependencies_batched`]
    /// for this behavior inside graph batches and stream score directors.
    pub fn apply_batch(
        &mut self,
        changes: impl IntoIterator<Item = TupleChange<K, T>>,
    ) -> Result<Vec<DerivedChange<K, T, V>>, DependencyError<K>> {
        let mut latest = BTreeMap::new();
        for change in changes {
            self.state.cache.topology().lookup(&change.id)?;
            latest.insert(change.id, change.value);
        }
        let mut next = self.state.clone();
        let mut removals = Vec::new();
        let mut additions = Vec::new();
        let mut dirty = Vec::new();
        for (key, value) in latest {
            if value.is_none() && !self.state.rows.contains_key(&key) {
                continue;
            }
            let old = self.state.cache.topology().dependencies(&key)?;
            let mut incoming = value
                .as_ref()
                .map_or_else(Vec::new, |row| (self.dependencies)(row));
            incoming.sort();
            incoming.dedup();
            removals.extend(
                old.iter()
                    .filter(|k| incoming.binary_search(k).is_err())
                    .map(|k| (key.clone(), (*k).clone())),
            );
            additions.extend(
                incoming
                    .iter()
                    .filter(|k| old.binary_search(k).is_err())
                    .map(|k| (key.clone(), k.clone())),
            );
            if let Some(row) = value {
                next.rows.insert(key.clone(), row);
            } else {
                next.rows.remove(&key);
            }
            dirty.push(key);
        }
        if dirty.is_empty() {
            return Ok(Vec::new());
        }
        let changed = next
            .cache
            .update(&removals, &additions, &dirty, |key, inputs| {
                next.rows
                    .get(key)
                    .and_then(|row| (self.supplier)(key, row, inputs))
            })?;
        let affected: BTreeSet<_> = dirty.into_iter().chain(changed).collect();
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
        self.state = State {
            rows: SnapshotMap::new(),
            cache: self.initial.clone(),
        };
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
        self.state = *saved.downcast().expect("dependency checkpoint type");
    }
}
