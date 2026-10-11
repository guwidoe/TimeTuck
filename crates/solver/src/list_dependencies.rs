//! Structural dependencies between list values, independent of score penalties.
use crate::{DependencyGraph, ListAssignment, ListError, ListProblem};

// Keys are exactly the registry indices 0..len. Ranks belong to the immutable
// fixed graph, not an assignment: cloning is safe, and with_dependencies always
// builds a new certificate. Small graphs retain the original direct analysis.
#[derive(Clone, Debug)]
pub(crate) struct ListDependencies {
    graph: DependencyGraph<usize>,
    ranks: Option<Vec<usize>>,
}
impl std::ops::Deref for ListDependencies {
    type Target = DependencyGraph<usize>;
    fn deref(&self) -> &Self::Target {
        &self.graph
    }
}
impl ListDependencies {
    fn new(graph: DependencyGraph<usize>) -> Self {
        let ranks = (graph.keys().len() >= 64).then(|| graph.fixed_order_ranks());
        Self { graph, ranks }
    }

    pub(crate) fn inconsistent_nodes(
        &self,
        dynamic: &[(usize, usize)],
    ) -> Result<Vec<usize>, crate::DependencyError<usize>> {
        if let Some(ranks) = &self.ranks
            && dynamic.iter().all(|&(node, parent)| {
                matches!((ranks.get(node), ranks.get(parent)), (Some(n), Some(p)) if p < n)
            })
        {
            return Ok(vec![]);
        }
        // A backward edge need not create a cycle. Unknown identities also fall
        // through, retaining the complete analyzer's diagnostic precedence.
        self.graph.inconsistent_nodes(dynamic)
    }
}

impl<V: Eq + Clone> ListProblem<V> {
    /// Declare each value's fixed dependencies by registry index. Every value also
    /// depends on its previous element while assigned. Fixed cycles and unknown
    /// indices are configuration errors. Missing inputs are not themselves cycles.
    ///
    /// Complete list/mixed solvers clear inconsistent movable initial values before
    /// scoring, and reject pinned cycles. The repaired initial score is a baseline,
    /// not a best-solution event; subsequent improvements are published normally.
    /// Trial selection skips cyclic candidates;
    /// explicit moves (including custom commands) return DependencyCycle and roll
    /// back. Standalone phase/director callers can use prepare_assignment first.
    /// Models without this opt-in retain their existing behavior.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = ListProblem::new(vec!["a", "b"], vec![ListEntity::new(vec!["b", "a"])])?
    ///     .with_dependencies(vec![vec![], vec![0]])?;
    /// let clean = p.prepare_assignment(&p.initial_assignment())?;
    /// assert!(clean.lists()[0].is_empty());
    /// # Ok::<(), ListError>(())
    /// ```
    pub fn with_dependencies(mut self, dependencies: Vec<Vec<usize>>) -> Result<Self, ListError> {
        if dependencies.len() != self.values().len() {
            return Err(ListError::DependencyCount {
                expected: self.values().len(),
                actual: dependencies.len(),
            });
        }
        self.dependencies = Some(ListDependencies::new(
            DependencyGraph::new(dependencies.into_iter().enumerate())
                .map_err(ListError::Dependency)?,
        ));
        Ok(self)
    }

    /// Registry indices of assigned cycle members and assigned downstream values.
    /// Pin, range, identity and list shape validation precede dependency traversal.
    pub fn inconsistent_values(
        &self,
        assignment: &ListAssignment<V>,
    ) -> Result<Vec<usize>, ListError> {
        self.validate_layout(assignment)?;
        self.inconsistent_values_unchecked(assignment)
    }

    pub(crate) fn inconsistent_values_unchecked(
        &self,
        assignment: &ListAssignment<V>,
    ) -> Result<Vec<usize>, ListError> {
        let Some(graph) = &self.dependencies else {
            return Ok(vec![]);
        };
        let index = |value: &V| self.value_index(value).expect("validated registry");
        let edge_count = assignment
            .lists()
            .iter()
            .map(|list| list.len().saturating_sub(1))
            .sum();
        // Fixed dependencies were checked for cycles when the model was built.
        // Without list predecessor edges there can be no new cycle.
        if edge_count == 0 {
            return Ok(vec![]);
        }
        let mut dynamic = Vec::with_capacity(edge_count);
        let mut assigned = vec![false; self.values().len()];
        for list in assignment.lists().iter().filter(|list| list.len() > 1) {
            let mut previous = None;
            for value in list {
                let id = index(value);
                assigned[id] = true;
                if let Some(parent) = previous {
                    dynamic.push((id, parent));
                }
                previous = Some(id);
            }
        }
        let inconsistent = graph
            .inconsistent_nodes(&dynamic)
            .map_err(ListError::Dependency)?;
        // Singletons add no edges, but can be downstream of a cycle through
        // fixed dependencies (including paths through unassigned values).
        if !inconsistent.is_empty() {
            for list in assignment.lists().iter().filter(|list| list.len() == 1) {
                assigned[index(&list[0])] = true;
            }
        }
        Ok(inconsistent.into_iter().filter(|&i| assigned[i]).collect())
    }

    /// Remove all assigned cycle members and their downstream dependents together.
    /// Unrelated values retain their order. Any affected full/prefix pin aborts
    /// before editing. The input, model metadata and pin baselines stay unchanged.
    /// Removing values can reveal new predecessor edges; repeat until consistent.
    pub fn prepare_assignment(
        &self,
        assignment: &ListAssignment<V>,
    ) -> Result<ListAssignment<V>, ListError> {
        self.validate_layout(assignment)?;
        let mut clean = assignment.clone();
        loop {
            let bad = self.inconsistent_values_unchecked(&clean)?;
            if bad.is_empty() {
                return Ok(clean);
            }
            if bad.len() >= 64 && self.has_value_index() {
                self.remove_inconsistent_indexed(&mut clean, &bad)?;
                continue;
            }
            for &value in &bad {
                let position = clean
                    .location(&self.values()[value])
                    .expect("assigned inconsistent value");
                let entity = &self.entities()[position.entity];
                if entity.is_pinned() || position.index < entity.pinned_prefix() {
                    return Err(ListError::PinnedDependencyCycle { value, position });
                }
            }
            for list in &mut clean.lists {
                list.retain(|v| !bad.iter().any(|&i| self.values()[i] == *v));
            }
        }
    }

    // Plan all removals before mutating rows. Registry ranks preserve the exact
    // original bad-value diagnostic order independently of row encounter order.
    // Sparse domain registries allocate only for bad and assigned values.
    fn remove_inconsistent_indexed(
        &self,
        assignment: &mut ListAssignment<V>,
        bad: &[usize],
    ) -> Result<(), ListError> {
        let ranks: std::collections::HashMap<_, _> = bad
            .iter()
            .enumerate()
            .map(|(rank, &id)| (id, rank))
            .collect();
        let mut keep = Vec::new();
        let mut pinned = None;
        for (entity, row) in assignment.lists().iter().enumerate() {
            let meta = &self.entities()[entity];
            for (index, value) in row.iter().enumerate() {
                let id = self.value_index(value).expect("validated registry");
                let rank = ranks.get(&id).copied();
                keep.push(rank.is_none());
                if let Some(rank) = rank
                    && (meta.is_pinned() || index < meta.pinned_prefix())
                    && pinned.as_ref().is_none_or(|&(first, _, _)| rank < first)
                {
                    pinned = Some((rank, id, crate::ListPosition { entity, index }));
                }
            }
        }
        if let Some((_, value, position)) = pinned {
            return Err(ListError::PinnedDependencyCycle { value, position });
        }
        let mut keep = keep.into_iter();
        for row in &mut assignment.lists {
            row.retain(|_| keep.next().expect("planned assigned value"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod indexing_tests {
    use super::*;
    use crate::ListEntity;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn order_certificate_matches_complete_analysis_and_unknown_error_order() {
        for n in [8, 64] {
            for fixed_mask in 0..8 {
                let nodes = (0..n).map(|i| {
                    let deps = if (1..=3).contains(&i) && fixed_mask & (1 << (i - 1)) != 0 {
                        vec![i - 1]
                    } else {
                        vec![]
                    };
                    (i, deps)
                });
                let graph = DependencyGraph::new(nodes).unwrap();
                let certified = ListDependencies::new(graph.clone());
                assert_eq!(certified.ranks.is_some(), n >= 64);
                let edges = [
                    (1, 0),
                    (0, 1),
                    (2, 1),
                    (1, 2),
                    (3, 2),
                    (2, 3),
                    (3, 0),
                    (0, 3),
                ];
                for mask in 0..256 {
                    let dynamic: Vec<_> = edges
                        .iter()
                        .enumerate()
                        .filter_map(|(i, &edge)| (mask & (1 << i) != 0).then_some(edge))
                        .collect();
                    assert_eq!(
                        certified.inconsistent_nodes(&dynamic),
                        graph.inconsistent_nodes(&dynamic)
                    );
                }
                for dynamic in [
                    vec![(0, n)],
                    vec![(n, 0)],
                    vec![(0, n), (n, 0)],
                    vec![(1, 0), (0, 1), (n, n)],
                ] {
                    assert_eq!(
                        certified.inconsistent_nodes(&dynamic),
                        graph.inconsistent_nodes(&dynamic)
                    );
                }
            }
        }
    }

    #[derive(Clone)]
    struct Counted(usize, Rc<Cell<usize>>);
    impl PartialEq for Counted {
        fn eq(&self, other: &Self) -> bool {
            self.1.set(self.1.get() + 1);
            self.0 == other.0
        }
    }
    impl Eq for Counted {}

    #[test]
    fn candidate_dependencies_do_not_repeat_registry_searches_for_adjacent_edges() {
        let comparisons = Rc::new(Cell::new(0));
        let values: Vec<_> = (0..96).map(|i| Counted(i, comparisons.clone())).collect();
        let p = ListProblem::new(values.clone(), vec![ListEntity::new(values)])
            .unwrap()
            .with_dependencies(vec![vec![]; 96])
            .unwrap();
        let a = p.initial_assignment();
        comparisons.set(0);
        assert!(p.inconsistent_values_unchecked(&a).unwrap().is_empty());
        // At most one full registry lookup per assigned value. Allows future
        // indexing implementations to do better; the former edge-pair scan fails.
        assert!(comparisons.get() <= (1..=96).sum::<usize>());
    }

    #[test]
    fn singleton_downstream_values_are_reported_through_unassigned_dependencies() {
        let p = ListProblem::new(
            (0..7).collect(),
            vec![
                ListEntity::new(vec![1, 0]),
                ListEntity::new(vec![3]),
                ListEntity::new(vec![6]),
            ],
        )
        .unwrap()
        .with_dependencies(vec![
            vec![],
            vec![0],
            vec![1],
            vec![2],
            vec![3],
            vec![4],
            vec![],
        ])
        .unwrap();
        let a = p.initial_assignment();
        assert_eq!(p.inconsistent_values(&a).unwrap(), vec![0, 1, 3]);
        assert_eq!(
            p.prepare_assignment(&a).unwrap().lists(),
            &[vec![], vec![], vec![6]]
        );
        assert_eq!(a.lists(), &[vec![1, 0], vec![3], vec![6]]);
    }

    #[test]
    fn empty_and_singleton_rows_need_no_value_searches() {
        let comparisons = Rc::new(Cell::new(0));
        let values: Vec<_> = (0..32).map(|i| Counted(i, comparisons.clone())).collect();
        let entities = values
            .iter()
            .cloned()
            .map(|v| ListEntity::new(vec![v]))
            .chain([ListEntity::new(vec![])])
            .collect();
        let p = ListProblem::new(values, entities)
            .unwrap()
            .with_dependencies(
                (0..32)
                    .map(|i| if i == 0 { vec![] } else { vec![i - 1] })
                    .collect(),
            )
            .unwrap();
        let a = p.initial_assignment();
        comparisons.set(0);
        assert!(p.inconsistent_values_unchecked(&a).unwrap().is_empty());
        assert_eq!(comparisons.get(), 0);
    }
}
