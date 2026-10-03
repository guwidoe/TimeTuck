//! Explicit fixed and per-evaluation dependencies for derived domain values.
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

/// `None` may be a legitimate unavailable value. Inconsistency separately marks
/// a dynamic cycle or a value transitively depending on such a cycle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedValue<V> {
    pub value: Option<V>,
    pub inconsistent: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DependencyError<K> {
    DuplicateNode(K),
    UnknownDependency { node: K, dependency: K },
    UnknownNode(K),
    FixedCycle(Vec<K>),
}
impl<K: fmt::Debug> fmt::Display for DependencyError<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNode(k) => write!(f, "duplicate derived variable: {k:?}"),
            Self::UnknownDependency { node, dependency } => write!(
                f,
                "variable {node:?} depends on unknown variable {dependency:?}"
            ),
            Self::UnknownNode(k) => write!(f, "unknown derived variable: {k:?}"),
            // Diagnostic prefix specified by the pinned Community test; see provenance.
            Self::FixedCycle(nodes) => write!(
                f,
                "There are fixed dependency loops in the graph for variables {nodes:?}"
            ),
        }
    }
}
impl<K: fmt::Debug> Error for DependencyError<K> {}

/// Immutable derived-value plan. Keys declare variable identities; dependencies
/// point from a variable to the variables it reads. Fixed cycles are rejected
/// with exactly the cycle members (not their downstream dependents).
///
/// Evaluation adds dynamic dependencies such as the previous element in a list.
/// Available nodes run in deterministic key order among ready nodes. Dynamic
/// cycles and their downstream dependents produce inconsistent, absent values;
/// their callbacks are not invoked. Removing the dynamic cycle on a later
/// evaluation restores consistency. No recursive fixed-point evaluation occurs.
///
/// This evaluator rebuilds the schedule each call. It is a pure projection for
/// use inside scoring callbacks, not an incremental dependency cache. Solver
/// score transactions cache/restore the callback's result as usual. Callbacks
/// must only read their supplied dependencies and immutable captured facts.
///
/// ```
/// use timetuck::*;
/// let graph = DependencyGraph::new(vec![("start", vec![]), ("end", vec!["start"])])?;
/// let values = graph.evaluate(&[], |key, inputs| {
///     Some(if *key == "start" { 10 } else { inputs[0].1.value.unwrap() + 5 })
/// })?;
/// assert_eq!(values["end"].value, Some(15));
/// let looped = graph.evaluate(&[("start", "end")], |_, _| Some(0))?;
/// assert!(looped.values().all(|v| v.inconsistent));
/// # Ok::<(), DependencyError<&str>>(())
/// ```
#[derive(Clone, Debug)]
pub struct DependencyGraph<K> {
    keys: Vec<K>,
    index: BTreeMap<K, usize>,
    fixed: Vec<Vec<usize>>,
}
impl<K: Ord + Clone> DependencyGraph<K> {
    pub fn new(nodes: impl IntoIterator<Item = (K, Vec<K>)>) -> Result<Self, DependencyError<K>> {
        let mut declarations = BTreeMap::new();
        for (key, dependencies) in nodes {
            if declarations.contains_key(&key) {
                return Err(DependencyError::DuplicateNode(key));
            }
            declarations.insert(key, dependencies);
        }
        let keys: Vec<_> = declarations.keys().cloned().collect();
        let index: BTreeMap<_, _> = keys
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, k)| (k, i))
            .collect();
        let mut fixed = Vec::with_capacity(keys.len());
        for (key, dependencies) in &declarations {
            let mut edges = Vec::with_capacity(dependencies.len());
            for dependency in dependencies {
                edges.push(*index.get(dependency).ok_or_else(|| {
                    DependencyError::UnknownDependency {
                        node: key.clone(),
                        dependency: dependency.clone(),
                    }
                })?);
            }
            edges.sort_unstable();
            edges.dedup();
            fixed.push(edges);
        }
        let cycles = cycle_members(&fixed);
        if !cycles.is_empty() {
            return Err(DependencyError::FixedCycle(
                cycles.into_iter().map(|i| keys[i].clone()).collect(),
            ));
        }
        Ok(Self { keys, index, fixed })
    }
    pub fn keys(&self) -> &[K] {
        &self.keys
    }
    pub fn evaluate<V>(
        &self,
        dynamic: &[(K, K)],
        mut evaluate: impl FnMut(&K, &[(&K, &DerivedValue<V>)]) -> Option<V>,
    ) -> Result<BTreeMap<K, DerivedValue<V>>, DependencyError<K>> {
        // Validate the entire dynamic graph before invoking any callback.
        let mut dependencies = self.fixed.clone();
        for (node, dependency) in dynamic {
            let &i = self
                .index
                .get(node)
                .ok_or_else(|| DependencyError::UnknownNode(node.clone()))?;
            let &j =
                self.index
                    .get(dependency)
                    .ok_or_else(|| DependencyError::UnknownDependency {
                        node: node.clone(),
                        dependency: dependency.clone(),
                    })?;
            dependencies[i].push(j);
        }
        for edges in &mut dependencies {
            edges.sort_unstable();
            edges.dedup();
        }
        let mut pending: Vec<_> = dependencies.iter().map(Vec::len).collect();
        let mut dependents = vec![vec![]; self.keys.len()];
        for (i, edges) in dependencies.iter().enumerate() {
            for &j in edges {
                dependents[j].push(i);
            }
        }
        let mut ready: BTreeSet<_> = pending
            .iter()
            .enumerate()
            .filter_map(|(i, &n)| (n == 0).then_some(i))
            .collect();
        let mut values: Vec<Option<DerivedValue<V>>> = (0..self.keys.len()).map(|_| None).collect();
        while let Some(i) = ready.pop_first() {
            let inputs: Vec<_> = dependencies[i]
                .iter()
                .map(|&j| (&self.keys[j], values[j].as_ref().expect("ready dependency")))
                .collect();
            let value = evaluate(&self.keys[i], &inputs);
            values[i] = Some(DerivedValue {
                value,
                inconsistent: false,
            });
            for &j in &dependents[i] {
                pending[j] -= 1;
                if pending[j] == 0 {
                    ready.insert(j);
                }
            }
        }
        Ok(self
            .keys
            .iter()
            .cloned()
            .zip(values.into_iter().map(|v| {
                v.unwrap_or(DerivedValue {
                    value: None,
                    inconsistent: true,
                })
            }))
            .collect())
    }
}

// Iterative two-pass SCC discovery avoids call-stack limits on large chains.
fn cycle_members(edges: &[Vec<usize>]) -> Vec<usize> {
    let mut visited = vec![false; edges.len()];
    let mut finish = Vec::with_capacity(edges.len());
    for start in 0..edges.len() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start, 0)];
        while let Some((node, next)) = stack.last_mut() {
            if *next < edges[*node].len() {
                let target = edges[*node][*next];
                *next += 1;
                if !visited[target] {
                    visited[target] = true;
                    stack.push((target, 0));
                }
            } else {
                finish.push(*node);
                stack.pop();
            }
        }
    }
    let mut reverse = vec![vec![]; edges.len()];
    for (node, targets) in edges.iter().enumerate() {
        for &target in targets {
            reverse[target].push(node);
        }
    }
    visited.fill(false);
    let mut cycles = vec![];
    for start in finish.into_iter().rev() {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![start];
        let mut component = vec![];
        while let Some(node) = stack.pop() {
            component.push(node);
            for &target in &reverse[node] {
                if !visited[target] {
                    visited[target] = true;
                    stack.push(target);
                }
            }
        }
        if component.len() > 1 || edges[start].contains(&start) {
            cycles.extend(component);
        }
    }
    cycles.sort_unstable();
    cycles
}
