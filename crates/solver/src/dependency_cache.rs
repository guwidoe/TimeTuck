//! Incremental derived values over explicit mutable dependencies.
use crate::{DependencyError, DependencyTopology, DerivedValue, SnapshotMap};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

struct Plan<K> {
    topology: DependencyTopology<K>,
    consumers: BTreeMap<K, Vec<K>>,
}
impl<K: Ord + Clone> Plan<K> {
    fn new(topology: DependencyTopology<K>) -> Self {
        let mut consumers: BTreeMap<_, Vec<_>> = topology
            .keys()
            .iter()
            .cloned()
            .map(|k| (k, vec![]))
            .collect();
        for key in topology.keys() {
            for dependency in topology
                .dependencies(key)
                .unwrap_or_else(|_| unreachable!("declared node"))
            {
                consumers
                    .get_mut(dependency)
                    .expect("declared dependency")
                    .push(key.clone());
            }
        }
        Self {
            topology,
            consumers,
        }
    }
    fn derive<V>(
        &self,
        key: &K,
        values: &SnapshotMap<K, DerivedValue<V>>,
        evaluate: &mut impl FnMut(&K, &[(&K, &DerivedValue<V>)]) -> Option<V>,
    ) -> DerivedValue<V> {
        if self
            .topology
            .is_cyclic(key)
            .unwrap_or_else(|_| unreachable!("declared node"))
        {
            return DerivedValue {
                value: None,
                inconsistent: true,
            };
        }
        let inputs: Vec<_> = self
            .topology
            .dependencies(key)
            .unwrap_or_else(|_| unreachable!("declared node"))
            .into_iter()
            .map(|k| (k, &values[k]))
            .collect();
        if inputs.iter().any(|(_, v)| v.inconsistent) {
            DerivedValue {
                value: None,
                inconsistent: true,
            }
        } else {
            DerivedValue {
                value: evaluate(key, &inputs),
                inconsistent: false,
            }
        }
    }
    fn rank(&self, key: &K) -> usize {
        self.topology
            .rank(key)
            .unwrap_or_else(|_| unreachable!("declared node"))
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
/// supporting trial snapshots even for non-Clone values. Changed edges rebuild
/// topology/reverse indexes; value-only updates do not scan all nodes.
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
        let mut ordered: Vec<_> = plan.topology.keys().iter().collect();
        ordered.sort_by_key(|k| plan.rank(k));
        let mut values = SnapshotMap::new();
        for key in ordered {
            let value = plan.derive(key, &values, &mut evaluate);
            values.insert(key.clone(), value);
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
            self.plan.topology.rank(key)?;
            roots.insert(key.clone());
        }
        let mut plan = self.plan.clone();
        if !removals.is_empty() || !additions.is_empty() {
            let mut topology = self.plan.topology.clone();
            let toggled = topology.update(removals, additions)?;
            if topology != self.plan.topology {
                roots.extend(toggled);
                for (key, _) in removals.iter().chain(additions) {
                    if self.plan.topology.dependencies(key)? != topology.dependencies(key)? {
                        roots.insert(key.clone());
                    }
                }
                plan = Arc::new(Plan::new(topology));
            }
        }
        let mut ready: BTreeSet<_> = roots.into_iter().map(|k| (plan.rank(&k), k)).collect();
        let mut values = self.values.clone();
        let mut changed = BTreeSet::new();
        while let Some((_, key)) = ready.pop_first() {
            let value = plan.derive(&key, &values, &mut evaluate);
            if values[&key] != value {
                values.insert(key.clone(), value);
                changed.insert(key.clone());
                for consumer in &plan.consumers[&key] {
                    ready.insert((plan.rank(consumer), consumer.clone()));
                }
            }
        }
        // All callbacks, comparisons and allocations finish before installation.
        let changed = changed.into_iter().collect();
        self.plan = plan;
        self.values = values;
        Ok(changed)
    }
}
