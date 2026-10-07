//! Reconcile owned basic-variable assignments across changed problem revisions.
use crate::{Assignment, FiniteProblem, ModelError};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReconciliationError {
    IdentityCount { expected: usize, actual: usize },
    DuplicateIdentity { index: usize },
    Model(ModelError),
}
impl fmt::Display for ReconciliationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "problem reconciliation: {self:?}")
    }
}
impl Error for ReconciliationError {}
fn validate_ids<I: Ord>(ids: &[I], count: usize) -> Result<(), ReconciliationError> {
    if ids.len() != count {
        return Err(ReconciliationError::IdentityCount {
            expected: count,
            actual: ids.len(),
        });
    }
    let mut seen = BTreeSet::new();
    for (index, id) in ids.iter().enumerate() {
        if !seen.insert(id) {
            return Err(ReconciliationError::DuplicateIdentity { index });
        }
    }
    Ok(())
}
/// Assignment values paired with stable variable identities. For multiple fields
/// per entity, use a compound (entity, field) identity. No score or search history
/// is retained. Equality on values must represent stable planning identity.
#[derive(Clone, Debug)]
pub struct AssignmentSnapshot<I, V> {
    ids: Vec<I>,
    assignment: Assignment<V>,
}
impl<I: Ord, V> AssignmentSnapshot<I, V> {
    pub fn new(ids: Vec<I>, assignment: Assignment<V>) -> Result<Self, ReconciliationError> {
        validate_ids(&ids, assignment.values().len())?;
        Ok(Self { ids, assignment })
    }
    pub fn ids(&self) -> &[I] {
        &self.ids
    }
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
}
/// Build a fresh initial model for a changed problem, matching by identity rather
/// than position. Valid previous values win for existing unpinned variables; an
/// existing unassigned value stays unassigned. Missing entities or removed values
/// fall back to the new model's initial state. New pins always win, including null
/// pins. To explicitly reset assignments, omit the previous snapshot.
///
/// Retained values are cloned from the NEW domain, so changed facts on an equal
/// planning identity are not replaced with stale snapshot objects. Domains, pins,
/// optionality and pillar metadata come from the new model. Inputs are untouched
/// on errors or panics. The caller rebuilds scoring against the returned model;
/// previous scores are never reused. LiveSolverManager can supply the previous
/// snapshot from a completed run, including runs interrupted by queued changes.
///
/// ```
/// use timetuck::*;
/// let old = AssignmentSnapshot::new(vec!["a", "b"], Assignment::new(vec![Some(1), Some(2)]))?;
/// let next = FiniteProblem::new(vec![Variable::new(vec![1,2]); 2]).unwrap();
/// let resumed = reconcile_problem(&next, &["b", "a"], Some(&old))?;
/// assert_eq!(resumed.initial_assignment().values(), &[Some(2), Some(1)]);
/// # Ok::<(), ReconciliationError>(())
/// ```
pub fn reconcile_problem<I: Ord, V: Clone + Eq>(
    problem: &FiniteProblem<V>,
    ids: &[I],
    previous: Option<&AssignmentSnapshot<I, V>>,
) -> Result<FiniteProblem<V>, ReconciliationError> {
    validate_ids(ids, problem.variables().len())?;
    let old: BTreeMap<_, _> = previous
        .into_iter()
        .flat_map(|p| p.ids.iter().zip(p.assignment.values()))
        .collect();
    let values = ids
        .iter()
        .zip(problem.variables())
        .map(|(id, variable)| {
            if variable.is_pinned() {
                return variable.initial().cloned();
            }
            match old.get(id) {
                Some(Some(value)) => variable
                    .domain()
                    .iter()
                    .find(|candidate| *candidate == value)
                    .or_else(|| variable.initial())
                    .cloned(),
                Some(None) => None,
                None => variable.initial().cloned(),
            }
        })
        .collect();
    problem
        .with_initial_assignment(&Assignment::new(values))
        .map_err(ReconciliationError::Model)
}
