//! Incremental derived values over explicit mutable dependencies.
use crate::{DependencyError, DependencyTopology, DerivedValue, NodeOrderQueue, SnapshotMap};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

struct Plan<K> {
    topology: DependencyTopology<K>,
    consumers: SnapshotMap<usize, BTreeSet<usize>>,
    order: Arc<[usize]>,
}
impl<K: Ord + Clone> Plan<K> {
    fn new(topology: DependencyTopology<K>) -> Self {
        let mut indexed: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for index in 0..topology.keys().len() {
            for &dependency in topology.dependency_indices(index) {
                indexed.entry(dependency).or_default().insert(index);
            }
        }
        let mut consumers = SnapshotMap::new();
        for (dependency, targets) in indexed {
            consumers.insert(dependency, targets);
        }
        let order = (0..topology.keys().len())
            .map(|i| topology.rank_at(i))
            .collect();
        Self {
            topology,
            consumers,
            order,
        }
    }
    fn updated(
        &self,
        topology: DependencyTopology<K>,
        changed_nodes: &[usize],
        reordered: bool,
    ) -> Self {
        let mut touched = BTreeMap::new();
        for &index in changed_nodes {
            let old = self.topology.dependency_indices(index);
            let new = topology.dependency_indices(index);
            for &dependency in old.difference(new) {
                touched
                    .entry(dependency)
                    .or_insert_with(|| self.consumers.get(&dependency).cloned().unwrap_or_default())
                    .remove(&index);
            }
            for &dependency in new.difference(old) {
                touched
                    .entry(dependency)
                    .or_insert_with(|| self.consumers.get(&dependency).cloned().unwrap_or_default())
                    .insert(index);
            }
        }
        let mut consumers = self.consumers.clone();
        for (dependency, targets) in touched {
            if targets.is_empty() {
                consumers.remove(&dependency);
            } else {
                consumers.insert(dependency, targets);
            }
        }
        let order = if reordered {
            (0..topology.keys().len())
                .map(|i| topology.rank_at(i))
                .collect()
        } else {
            self.order.clone()
        };
        Self {
            topology,
            consumers,
            order,
        }
    }
    fn derive<V>(
        &self,
        index: usize,
        values: &SnapshotMap<K, DerivedValue<V>>,
        evaluate: &mut impl FnMut(&K, &[(&K, &DerivedValue<V>)]) -> Option<V>,
    ) -> DerivedValue<V> {
        if self.topology.cyclic_at(index) {
            return DerivedValue {
                value: None,
                inconsistent: true,
            };
        }
        let inputs: Vec<_> = self
            .topology
            .dependency_indices(index)
            .iter()
            .map(|&i| {
                let key = &self.topology.keys()[i];
                (key, &values[key])
            })
            .collect();
        if inputs.iter().any(|(_, v)| v.inconsistent) {
            DerivedValue {
                value: None,
                inconsistent: true,
            }
        } else {
            DerivedValue {
                value: evaluate(&self.topology.keys()[index], &inputs),
                inconsistent: false,
            }
        }
    }
    fn rank(&self, index: usize) -> usize {
        self.order[index]
    }
}

/// Cached derived values, with explicit invalidation and dynamic edges.
///
/// Initialize every node once, skipping cycles and their downstream dependents.
/// On update, dirty keys and consumers whose incoming edges changed are scheduled
/// in dependency order. A changed value or inconsistency flag schedules its direct
/// consumers; equal results stop propagation. `None` alone is a consistent result.
///
/// Suppliers must depend only on supplied inputs and immutable captured facts.
/// Mark every key that reads a changed external fact (or uses a changed formula)
/// dirty. Changing a supplier without invalidation leaves cached values unchanged.
///
/// Metadata errors invoke no suppliers and leave the cache unchanged. Ordinary
/// callback unwinding also preserves the cache; external callback side effects
/// cannot be undone. Clone shares immutable topology and persistent value roots,
/// supporting trial snapshots even for non-Clone values. Sparse edge edits update
/// only affected reverse links; edits that preserve component ranks also reuse
/// the evaluation order. Value-only updates do not scan all nodes.
/// Scheduling and reverse links use stable internal indices. Suppliers receive
/// the declared topology keys; changed-key results remain sorted by key.
///
/// ```
/// use timetuck::*;
/// let mut topology = DependencyTopology::new([0, 1, 2])?;
/// topology.update(&[], &[(1, 0), (2, 1)])?;
/// let mut cache = DependencyCache::new(topology, |_, inputs| {
///     Some(inputs.first().map_or(1, |(_, v)| v.value.unwrap() + 1))
/// });
/// let saved = cache.clone();
/// let changed = cache.update(&[], &[], &[0], |_, inputs| {
///     Some(inputs.first().map_or(10, |(_, v)| v.value.unwrap() + 1))
/// })?;
/// assert_eq!(changed, [0, 1, 2]);
/// assert_eq!(cache.values()[&2].value, Some(12));
/// assert_eq!(saved.values()[&2].value, Some(3));
/// # Ok::<(), DependencyError<usize>>(())
/// ```
pub struct DependencyCache<K, V> {
    plan: Arc<Plan<K>>,
    values: SnapshotMap<K, DerivedValue<V>>,
}
impl<K, V> Clone for DependencyCache<K, V> {
    fn clone(&self) -> Self {
        Self {
            plan: self.plan.clone(),
            values: self.values.clone(),
        }
    }
}
impl<K: Ord + Clone, V: Eq> DependencyCache<K, V> {
    pub fn new(
        topology: DependencyTopology<K>,
        mut evaluate: impl FnMut(&K, &[(&K, &DerivedValue<V>)]) -> Option<V>,
    ) -> Self {
        let plan = Arc::new(Plan::new(topology));
        let mut ordered: Vec<_> = (0..plan.topology.keys().len()).collect();
        ordered.sort_by_key(|&i| plan.rank(i));
        let mut values = SnapshotMap::new();
        for index in ordered {
            let value = plan.derive(index, &values, &mut evaluate);
            values.insert(plan.topology.keys()[index].clone(), value);
        }
        Self { plan, values }
    }
    pub fn values(&self) -> &SnapshotMap<K, DerivedValue<V>> {
        &self.values
    }
    pub fn topology(&self) -> &DependencyTopology<K> {
        &self.plan.topology
    }
    /// Apply removals, then additions, and reevaluate dirty/affected values.
    /// Returns exactly the final changed value keys, sorted by key. No-op edge
    /// batches alone invoke no suppliers. Unknown dirty keys fail atomically.
    pub fn update(
        &mut self,
        removals: &[(K, K)],
        additions: &[(K, K)],
        dirty: &[K],
        mut evaluate: impl FnMut(&K, &[(&K, &DerivedValue<V>)]) -> Option<V>,
    ) -> Result<Vec<K>, DependencyError<K>> {
        let mut roots = BTreeSet::new();
        for key in dirty {
            roots.insert(self.plan.topology.lookup(key)?);
        }
        let mut plan = self.plan.clone();
        if !removals.is_empty() || !additions.is_empty() {
            let mut topology = self.plan.topology.clone();
            let update = topology.update_indexed(removals, additions)?;
            if !update.changed_nodes.is_empty() {
                for key in update.cycle_toggled {
                    roots.insert(topology.lookup(&key)?);
                }
                roots.extend(update.changed_nodes.iter().copied());
                plan = Arc::new(self.plan.updated(
                    topology,
                    &update.changed_nodes,
                    update.reordered,
                ));
            }
        }
        let mut ready = NodeOrderQueue::new(plan.order.clone());
        for index in roots {
            ready.offer(index).expect("validated dependency node");
        }
        let mut values = self.values.clone();
        let mut changed = BTreeSet::new();
        while let Some(index) = ready.pop() {
            let key = &plan.topology.keys()[index];
            let value = plan.derive(index, &values, &mut evaluate);
            if values[key] != value {
                values.insert(key.clone(), value);
                changed.insert(index);
                if let Some(consumers) = plan.consumers.get(&index) {
                    for &consumer in consumers {
                        ready.offer(consumer).expect("validated consumer node");
                    }
                }
            }
        }
        // All callbacks, comparisons and allocations finish before installation.
        let changed = changed
            .into_iter()
            .map(|i| plan.topology.keys()[i].clone())
            .collect();
        self.plan = plan;
        self.values = values;
        Ok(changed)
    }
}
