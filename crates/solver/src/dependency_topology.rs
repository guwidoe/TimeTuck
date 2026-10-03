//! Persistent dependency edges with atomic topology updates.
use crate::{DependencyError, dependency_graph::strong_components};
use std::collections::{BTreeMap, BTreeSet};

/// A mutable topology on a fixed set of keys. Edges point from a node to a
/// dependency, as in [`crate::DependencyGraph`]. Components identify mutual
/// reachability; component ranks strictly increase from dependency to dependent.
/// Nodes in a component share a rank. Unrelated ranks have no semantic meaning.
///
/// An update validates the entire batch before changing anything, removes edges,
/// then adds edges. Duplicate additions and absent removals are harmless. Its
/// result lists exactly the keys whose cycle membership toggled, in key order.
/// This excludes downstream nodes that merely depend on cycles. Snapshots can be
/// cloned for trial/undo. Nodes cannot be inserted or removed after construction.
///
/// Edges persist between updates; components and ranks are rebuilt on each actual
/// change. This is not an incremental value cache and invokes no suppliers.
///
/// ```
/// use timetuck::*;
/// let mut graph = DependencyTopology::new(["a", "b", "c"])?;
/// assert!(graph.update(&[], &[("b", "a"), ("c", "b")])?.is_empty());
/// assert!(graph.rank(&"a")? < graph.rank(&"c")?);
/// assert_eq!(graph.update(&[], &[("a", "b")])?, ["a", "b"]);
/// assert_eq!(graph.component(&"a")?, ["a", "b"]);
/// assert!(!graph.is_cyclic(&"c")?);
/// assert_eq!(graph.update(&[("a", "b")], &[])?, ["a", "b"]);
/// # Ok::<(), DependencyError<&str>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyTopology<K> {
    keys: Vec<K>,
    index: BTreeMap<K, usize>,
    edges: Vec<BTreeSet<usize>>,
    components: Vec<Vec<K>>,
    component_of: Vec<usize>,
    ranks: Vec<usize>,
    cyclic: Vec<bool>,
}
impl<K: Ord + Clone> DependencyTopology<K> {
    pub fn new(keys: impl IntoIterator<Item = K>) -> Result<Self, DependencyError<K>> {
        let mut sorted = BTreeSet::new();
        for key in keys {
            if !sorted.insert(key.clone()) {
                return Err(DependencyError::DuplicateNode(key));
            }
        }
        let keys: Vec<_> = sorted.into_iter().collect();
        let n = keys.len();
        Ok(Self {
            index: keys
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, k)| (k, i))
                .collect(),
            components: keys.iter().cloned().map(|k| vec![k]).collect(),
            keys,
            edges: vec![BTreeSet::new(); n],
            component_of: (0..n).collect(),
            ranks: (0..n).collect(),
            cyclic: vec![false; n],
        })
    }
    pub fn keys(&self) -> &[K] {
        &self.keys
    }
    fn lookup(&self, key: &K) -> Result<usize, DependencyError<K>> {
        self.index
            .get(key)
            .copied()
            .ok_or_else(|| DependencyError::UnknownNode(key.clone()))
    }
    pub fn component(&self, key: &K) -> Result<&[K], DependencyError<K>> {
        Ok(&self.components[self.component_of[self.lookup(key)?]])
    }
    pub fn rank(&self, key: &K) -> Result<usize, DependencyError<K>> {
        Ok(self.ranks[self.component_of[self.lookup(key)?]])
    }
    pub fn is_cyclic(&self, key: &K) -> Result<bool, DependencyError<K>> {
        Ok(self.cyclic[self.lookup(key)?])
    }
    pub fn dependencies(&self, key: &K) -> Result<Vec<&K>, DependencyError<K>> {
        Ok(self.edges[self.lookup(key)?]
            .iter()
            .map(|&i| &self.keys[i])
            .collect())
    }
    pub fn update(
        &mut self,
        removals: &[(K, K)],
        additions: &[(K, K)],
    ) -> Result<Vec<K>, DependencyError<K>> {
        let validate = |edges: &[(K, K)]| -> Result<Vec<(usize, usize)>, DependencyError<K>> {
            edges
                .iter()
                .map(|(node, dependency)| {
                    let i = self.lookup(node)?;
                    let j = self.index.get(dependency).copied().ok_or_else(|| {
                        DependencyError::UnknownDependency {
                            node: node.clone(),
                            dependency: dependency.clone(),
                        }
                    })?;
                    Ok((i, j))
                })
                .collect()
        };
        let removals = validate(removals)?;
        let additions = validate(additions)?;
        let mut edges = self.edges.clone();
        for (i, j) in removals {
            edges[i].remove(&j);
        }
        for (i, j) in additions {
            edges[i].insert(j);
        }
        if edges == self.edges {
            return Ok(vec![]);
        }
        let adjacency: Vec<Vec<usize>> = edges
            .iter()
            .map(|targets| targets.iter().copied().collect())
            .collect();
        let mut components = strong_components(&adjacency);
        for component in &mut components {
            component.sort_unstable();
        }
        components.sort_unstable_by_key(|component| component[0]);
        let mut component_of = vec![0; self.keys.len()];
        let mut cyclic = vec![false; self.keys.len()];
        for (c, members) in components.iter().enumerate() {
            let cycle = members.len() > 1 || edges[members[0]].contains(&members[0]);
            for &i in members {
                component_of[i] = c;
                cyclic[i] = cycle;
            }
        }
        let mut dependencies = vec![BTreeSet::new(); components.len()];
        for (i, targets) in edges.iter().enumerate() {
            for &j in targets {
                if component_of[i] != component_of[j] {
                    dependencies[component_of[i]].insert(component_of[j]);
                }
            }
        }
        let mut pending: Vec<_> = dependencies.iter().map(BTreeSet::len).collect();
        let mut consumers = vec![vec![]; components.len()];
        for (i, inputs) in dependencies.iter().enumerate() {
            for &j in inputs {
                consumers[j].push(i);
            }
        }
        let mut ready: BTreeSet<_> = pending
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n == 0).then_some(i))
            .collect();
        let mut ranks = vec![0; components.len()];
        let mut rank = 0;
        while let Some(i) = ready.pop_first() {
            ranks[i] = rank;
            rank += 1;
            for &j in &consumers[i] {
                pending[j] -= 1;
                if pending[j] == 0 {
                    ready.insert(j);
                }
            }
        }
        debug_assert_eq!(rank, components.len());
        let changed = self
            .keys
            .iter()
            .enumerate()
            .filter_map(|(i, k)| (self.cyclic[i] != cyclic[i]).then_some(k.clone()))
            .collect();
        self.components = components
            .into_iter()
            .map(|members| members.into_iter().map(|i| self.keys[i].clone()).collect())
            .collect();
        self.edges = edges;
        self.component_of = component_of;
        self.cyclic = cyclic;
        self.ranks = ranks;
        Ok(changed)
    }
}
