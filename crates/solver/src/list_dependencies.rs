//! Structural dependencies between list values, independent of score penalties.
use crate::{DependencyGraph, ListAssignment, ListError, ListProblem};

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
        self.dependencies = Some(
            DependencyGraph::new(dependencies.into_iter().enumerate())
                .map_err(ListError::Dependency)?,
        );
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
        let index = |value: &V| {
            self.values()
                .iter()
                .position(|v| v == value)
                .expect("validated registry")
        };
        let dynamic: Vec<_> = assignment
            .lists()
            .iter()
            .flat_map(|list| {
                list.windows(2)
                    .map(|pair| (index(&pair[1]), index(&pair[0])))
            })
            .collect();
        let state = graph
            .evaluate(&dynamic, |_, _| Some(()))
            .map_err(ListError::Dependency)?;
        Ok(state
            .into_iter()
            .filter_map(|(i, derived)| {
                (derived.inconsistent && assignment.location(&self.values()[i]).is_some())
                    .then_some(i)
            })
            .collect())
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
}
