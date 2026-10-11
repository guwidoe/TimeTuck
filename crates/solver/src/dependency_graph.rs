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
    fixed_dependents: Vec<Vec<usize>>,
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
        let mut fixed_dependents = vec![vec![]; keys.len()];
        for (node, dependencies) in fixed.iter().enumerate() {
            for &dependency in dependencies {
                fixed_dependents[dependency].push(node);
            }
        }
        Ok(Self {
            keys,
            index,
            fixed,
            fixed_dependents,
        })
    }
    pub fn keys(&self) -> &[K] {
        &self.keys
    }

    /// One valid fixed-graph order, indexed by `keys()` ordinal. The graph is
    /// already acyclic. Structural clients may use this immutable certificate to
    /// prove that additional edges cannot introduce a cycle; failed proofs must
    /// still use the complete analysis.
    pub(crate) fn fixed_order_ranks(&self) -> Vec<usize> {
        let mut pending: Vec<_> = self.fixed.iter().map(Vec::len).collect();
        let mut ready: Vec<_> = pending
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(i, &count)| (count == 0).then_some(i))
            .collect();
        let mut ranks = vec![0; self.keys.len()];
        let mut next = 0;
        while let Some(i) = ready.pop() {
            ranks[i] = next;
            next += 1;
            for &j in &self.fixed_dependents[i] {
                pending[j] -= 1;
                if pending[j] == 0 {
                    ready.push(j);
                }
            }
        }
        debug_assert_eq!(next, self.keys.len());
        ranks
    }

    /// Cycle members and their downstream dependents in key order, without
    /// evaluating or allocating derived values. Used by structural validation.
    /// Duplicate edges contribute matching counts and releases, including edges
    /// already in the fixed graph; deduplicating them is unnecessary here.
    pub(crate) fn inconsistent_nodes(
        &self,
        dynamic: &[(K, K)],
    ) -> Result<Vec<K>, DependencyError<K>> {
        let mut pending: Vec<_> = self.fixed.iter().map(Vec::len).collect();
        // An adjacency overlay with one allocation for all dynamic edges.
        let mut heads = vec![usize::MAX; self.keys.len()];
        let mut links = Vec::with_capacity(dynamic.len());
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
            pending[i] += 1;
            links.push((i, heads[j]));
            heads[j] = links.len() - 1;
        }
        let mut ready: Vec<_> = pending
            .iter()
            .enumerate()
            .filter_map(|(i, &count)| (count == 0).then_some(i))
            .collect();
        while let Some(i) = ready.pop() {
            let mut release = |j: usize| {
                pending[j] -= 1;
                if pending[j] == 0 {
                    ready.push(j);
                }
            };
            for &j in &self.fixed_dependents[i] {
                release(j);
            }
            let mut edge = heads[i];
            while edge != usize::MAX {
                let (j, next) = links[edge];
                release(j);
                edge = next;
            }
        }
        Ok(self
            .keys
            .iter()
            .zip(pending)
            .filter(|(_, count)| *count != 0)
            .map(|(key, _)| key.clone())
            .collect())
    }
    /// Evaluate explicitly aligned variables as one shared computation.
    ///
    /// Alignment pairs form transitive equivalence classes. Their dependencies
    /// are combined; each dependency appears once under its smallest member key.
    /// The callback receives sorted member keys and sorted canonical dependencies,
    /// runs once per consistent class, and its result is cloned to every member.
    /// Alignment is a caller promise that these variables share a result; this
    /// method does not infer it from equal values or invoke separate suppliers.
    ///
    /// Collapsing a dependency inside its own class creates a cycle. Such classes
    /// and their downstream dependents are inconsistent, without callback calls.
    /// Fixed, dynamic and alignment metadata are all validated before callbacks.
    /// The immutable plan is reusable with different alignments on each call.
    /// Like `evaluate`, this rebuilds the schedule rather than caching updates.
    ///
    /// ```
    /// use timetuck::*;
    /// let graph = DependencyGraph::new([("i", vec![]), ("x", vec!["i"]), ("y", vec![])])?;
    /// let values = graph.evaluate_aligned(&[], &[("x", "y")], |members, inputs| {
    ///     if members == ["i"] { Some(2) } else { inputs[0].1.value.map(|v| v + 1) }
    /// })?;
    /// assert_eq!(values["x"].value, Some(3));
    /// assert_eq!(values["x"], values["y"]);
    /// # Ok::<(), DependencyError<&str>>(())
    /// ```
    pub fn evaluate_aligned<V: Clone>(
        &self,
        dynamic: &[(K, K)],
        alignment: &[(K, K)],
        mut evaluate: impl FnMut(&[K], &[(&K, &DerivedValue<V>)]) -> Option<V>,
    ) -> Result<BTreeMap<K, DerivedValue<V>>, DependencyError<K>> {
        let mut parent: Vec<_> = (0..self.keys.len()).collect();
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        for (a, b) in alignment {
            let a = *self
                .index
                .get(a)
                .ok_or_else(|| DependencyError::UnknownNode(a.clone()))?;
            let b = *self
                .index
                .get(b)
                .ok_or_else(|| DependencyError::UnknownNode(b.clone()))?;
            let (a, b) = (root(&mut parent, a), root(&mut parent, b));
            parent[a.max(b)] = a.min(b);
        }
        let canonical: Vec<_> = (0..self.keys.len()).map(|i| root(&mut parent, i)).collect();
        let mut groups: BTreeMap<K, Vec<K>> = BTreeMap::new();
        for (i, key) in self.keys.iter().enumerate() {
            groups
                .entry(self.keys[canonical[i]].clone())
                .or_default()
                .push(key.clone());
        }
        let mut edges = Vec::new();
        for (i, dependencies) in self.fixed.iter().enumerate() {
            for &j in dependencies {
                edges.push((
                    self.keys[canonical[i]].clone(),
                    self.keys[canonical[j]].clone(),
                ));
            }
        }
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
            edges.push((
                self.keys[canonical[i]].clone(),
                self.keys[canonical[j]].clone(),
            ));
        }
        // All dependencies enter evaluation dynamically, including cycles that
        // only appear when otherwise acyclic fixed nodes are aligned.
        let quotient = DependencyGraph::new(groups.keys().cloned().map(|key| (key, vec![])))?;
        let values = quotient.evaluate(&edges, |key, inputs| evaluate(&groups[key], inputs))?;
        Ok(self
            .keys
            .iter()
            .enumerate()
            .map(|(i, key)| (key.clone(), values[&self.keys[canonical[i]]].clone()))
            .collect())
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
pub(crate) fn strong_components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
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
    let mut components = vec![];
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
        components.push(component);
    }
    components
}

fn cycle_members(edges: &[Vec<usize>]) -> Vec<usize> {
    let mut cycles: Vec<_> = strong_components(edges)
        .into_iter()
        .filter(|c| c.len() > 1 || edges[c[0]].contains(&c[0]))
        .flatten()
        .collect();
    cycles.sort_unstable();
    cycles
}

#[cfg(test)]
mod cycle_check_tests {
    use super::*;

    #[test]
    fn cycle_only_checks_match_reachability_for_all_three_node_graph_pairs() {
        fn edges(mask: usize) -> Vec<(usize, usize)> {
            (0..3)
                .flat_map(|i| {
                    (0..3).filter_map(move |j| (mask & (1 << (3 * i + j)) != 0).then_some((i, j)))
                })
                .collect()
        }
        for fixed_mask in 0..512 {
            let fixed = edges(fixed_mask);
            let Ok(graph) = DependencyGraph::new((0..3).map(|i| {
                (
                    i,
                    fixed
                        .iter()
                        .filter_map(|&(a, b)| (a == i).then_some(b))
                        .collect(),
                )
            })) else {
                continue;
            };
            for dynamic_mask in 0..512 {
                let mut dynamic = edges(dynamic_mask);
                let mut reach = [[false; 3]; 3];
                for &(a, b) in fixed.iter().chain(&dynamic) {
                    reach[a][b] = true;
                }
                for k in 0..3 {
                    for i in 0..3 {
                        for j in 0..3 {
                            reach[i][j] |= reach[i][k] && reach[k][j];
                        }
                    }
                }
                let expected: Vec<_> = (0..3)
                    .filter(|&i| (0..3).any(|c| reach[c][c] && (i == c || reach[i][c])))
                    .collect();
                // Duplicate both dynamic and fixed edges, and reverse their order.
                dynamic.extend(dynamic.clone());
                dynamic.extend(&fixed);
                dynamic.reverse();
                assert_eq!(
                    graph.inconsistent_nodes(&dynamic).unwrap(),
                    expected,
                    "fixed {fixed_mask}, dynamic {dynamic_mask}"
                );
            }
            assert!(graph.inconsistent_nodes(&[]).unwrap().is_empty());
        }
    }

    #[test]
    fn cycle_only_errors_and_sorted_results_preserve_arbitrary_keys() {
        let graph = DependencyGraph::new([
            ("z", vec!["b"]),
            ("b", vec!["a"]),
            ("a", vec![]),
            ("free", vec![]),
        ])
        .unwrap();
        for dynamic in [
            vec![("a", "b"), ("missing", "also missing")],
            vec![("a", "b"), ("b", "missing")],
        ] {
            assert_eq!(
                graph.inconsistent_nodes(&dynamic).unwrap_err(),
                graph.evaluate(&dynamic, |_, _| Some(())).unwrap_err()
            );
        }
        assert_eq!(
            graph.inconsistent_nodes(&[("a", "b")]).unwrap(),
            ["a", "b", "z"]
        );
        assert!(graph.inconsistent_nodes(&[]).unwrap().is_empty());
        assert!(
            DependencyGraph::<usize>::new([])
                .unwrap()
                .inconsistent_nodes(&[])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn cycle_only_checks_handle_long_chains_and_repeated_fixed_edges() {
        let graph = DependencyGraph::new(
            (0..20_000).map(|i| (i, if i == 0 { vec![] } else { vec![i - 1] })),
        )
        .unwrap();
        assert!(
            graph
                .inconsistent_nodes(&[(19_999, 0), (19_999, 0), (1, 0)])
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            graph.inconsistent_nodes(&[(0, 19_999)]).unwrap(),
            (0..20_000).collect::<Vec<_>>()
        );
        assert!(graph.inconsistent_nodes(&[]).unwrap().is_empty());
    }
}
