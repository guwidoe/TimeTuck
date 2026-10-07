//! Identity-based warm starts for changed list planning problems.
use crate::{ListAssignment, ListError, ListProblem};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListReconciliationError {
    IdentityCount { expected: usize, actual: usize },
    DuplicateIdentity { index: usize },
    DuplicateValue,
    Model(ListError),
}
impl fmt::Display for ListReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list reconciliation: {self:?}")
    }
}
impl Error for ListReconciliationError {}
fn validate_ids<I: Ord>(ids: &[I], count: usize) -> Result<(), ListReconciliationError> {
    if ids.len() != count {
        return Err(ListReconciliationError::IdentityCount {
            expected: count,
            actual: ids.len(),
        });
    }
    let mut seen = BTreeSet::new();
    for (index, id) in ids.iter().enumerate() {
        if !seen.insert(id) {
            return Err(ListReconciliationError::DuplicateIdentity { index });
        }
    }
    Ok(())
}
/// Owned disjoint lists paired with stable entity IDs. Empty lists retain their
/// identity. Values use Eq planning identity; IDs may include a field identifier.
#[derive(Clone, Debug)]
pub struct ListAssignmentSnapshot<I, V> {
    ids: Vec<I>,
    assignment: ListAssignment<V>,
}
impl<I: Ord, V: Eq> ListAssignmentSnapshot<I, V> {
    pub fn new(
        ids: Vec<I>,
        assignment: ListAssignment<V>,
    ) -> Result<Self, ListReconciliationError> {
        validate_ids(&ids, assignment.lists().len())?;
        let mut seen = Vec::new();
        for value in assignment.lists().iter().flatten() {
            if seen.contains(&value) {
                return Err(ListReconciliationError::DuplicateValue);
            }
            seen.push(value);
        }
        Ok(Self { ids, assignment })
    }
    pub fn ids(&self) -> &[I] {
        &self.ids
    }
    pub fn assignment(&self) -> &ListAssignment<V> {
        &self.assignment
    }
}
/// Reconcile changed list entities without confusing ownership with row position.
/// Reserve ALL current full/prefix pins first. Existing entities then retain their
/// previous order, excluding removed values, values outside the new entity range,
/// and values now reserved by pins. New entities take their current initial suffix
/// after retained ownership has been reserved. Existing empty lists stay empty;
/// current unpinned defaults do not override a previous entity's list.
///
/// Removed entities' values and new values remain unassigned unless claimed by a
/// new entity's initial state or a pin. Ordinary construction may insert required
/// missing values afterward. With no previous snapshot, use the new initial state.
/// All assigned objects are cloned from the new registry, preserving updated facts
/// behind equal value identities. Existing snapshots and models are never mutated.
///
/// Optionality, ranges, pins and dependency metadata remain from the new model.
/// Dependency cycles introduced by reconciliation are repaired using the model's
/// existing prepare_assignment policy; a cycle touching any pin returns an error.
/// This handles basic list layout only: callers rebuild scores and external shadows.
/// Compose with reconcile_problem and MixedProblem::new for mixed models.
///
/// ```
/// use timetuck::*;
/// let old = ListAssignmentSnapshot::new(vec!["a", "b"],
///     ListAssignment::new(vec![vec![1,2],vec![3]]))?;
/// let next = ListProblem::new(vec![1,3,4],vec![ListEntity::new(vec![]);2]).unwrap();
/// let resumed = reconcile_list_problem(&next, &["b","a"], Some(&old))?;
/// assert_eq!(resumed.initial_assignment().lists(), &[vec![3],vec![1]]);
/// # Ok::<(), ListReconciliationError>(())
/// ```
pub fn reconcile_list_problem<I: Ord, V: Clone + Eq>(
    problem: &ListProblem<V>,
    ids: &[I],
    previous: Option<&ListAssignmentSnapshot<I, V>>,
) -> Result<ListProblem<V>, ListReconciliationError> {
    validate_ids(ids, problem.entities().len())?;
    let registry = problem.values();
    let canonical = |value: &V| registry.iter().position(|v| v == value);
    let mut claimed = vec![false; registry.len()];
    let mut rows = vec![vec![]; ids.len()];
    // Valid model metadata guarantees disjoint pins in the current registry.
    for (i, entity) in problem.entities().iter().enumerate() {
        let length = if entity.is_pinned() {
            entity.initial().len()
        } else {
            entity.pinned_prefix()
        };
        for value in &entity.initial()[..length] {
            let index = canonical(value).expect("validated model pin");
            claimed[index] = true;
            rows[i].push(registry[index].clone());
        }
    }
    let old: BTreeMap<_, _> = previous
        .into_iter()
        .flat_map(|p| p.ids.iter().zip(p.assignment.lists()))
        .collect();
    // Two passes make retained ownership win over new-entity unpinned defaults,
    // independently of the new entity ordering.
    for retained in [true, false] {
        for (i, (id, entity)) in ids.iter().zip(problem.entities()).enumerate() {
            if entity.is_pinned() {
                continue;
            }
            let prior = old.get(id);
            if prior.is_some() != retained {
                continue;
            }
            let candidates = prior.map_or(entity.initial(), |row| row.as_slice());
            for value in candidates {
                let Some(index) = canonical(value) else {
                    continue;
                };
                if claimed[index]
                    || entity
                        .value_range()
                        .is_some_and(|r| !r.contains(&registry[index]))
                {
                    continue;
                }
                claimed[index] = true;
                rows[i].push(registry[index].clone());
            }
        }
    }
    let clean = problem
        .prepare_assignment(&ListAssignment::new(rows))
        .map_err(ListReconciliationError::Model)?;
    problem
        .with_initial_assignment(&clean)
        .map_err(ListReconciliationError::Model)
}
