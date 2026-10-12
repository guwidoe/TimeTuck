//! Snapshot-checked edge reconnections rebased through real value lookups.
use crate::{ListAssignment, ListError, ListKOptError, ListMove, ListProblem, MoveRebaseNamespace};
#[derive(Clone, Debug)]
pub struct ListKOptEdgePlan<V> {
    entity: usize,
    prefix: usize,
    before: Vec<V>,
    removed: Vec<(usize, usize)>,
    added: Vec<(usize, usize)>,
}
#[derive(Debug)]
pub enum ListKOptRebaseError<E> {
    Lookup(E),
    Collision(MoveRebaseNamespace),
    Topology(ListKOptError),
}
impl<E: std::fmt::Debug> std::fmt::Display for ListKOptRebaseError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "k-opt rebase: {self:?}")
    }
}
impl<E: std::fmt::Debug> std::error::Error for ListKOptRebaseError<E> {}
impl<V: Eq + Clone> ListKOptEdgePlan<V> {
    pub fn prepare(
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
        entity: usize,
        removed: &[(usize, usize)],
        added: &[(usize, usize)],
    ) -> Result<Self, ListKOptError> {
        crate::list_k_opt_edge_move(problem, before, entity, removed, added)?;
        Ok(Self {
            entity,
            prefix: problem.entities()[entity].pinned_prefix(),
            before: before.lists()[entity].clone(),
            removed: removed.to_vec(),
            added: added.to_vec(),
        })
    }
    /// Reject stale ordered membership or pin baselines before deriving positions.
    /// Receiving domains and dependency constraints validate through the native
    /// model; the returned composite remains an ordinary snapshot-bound move.
    pub fn movement(
        &self,
        problem: &ListProblem<V>,
        before: &ListAssignment<V>,
    ) -> Result<ListMove<V>, ListKOptError> {
        problem.validate(before)?;
        let meta = problem
            .entities()
            .get(self.entity)
            .ok_or(ListError::EntityIndex {
                entity: self.entity,
                count: problem.entities().len(),
            })?;
        if before.lists()[self.entity] != self.before || meta.pinned_prefix() != self.prefix {
            return Err(ListKOptError::Partition);
        }
        crate::list_k_opt_edge_move(problem, before, self.entity, &self.removed, &self.added)
    }
    /// Map the actual complete source row once, checking identity collisions and
    /// exact destination membership/order before rebuilding destination edges.
    /// Neither model/assignment is changed on lookup, shape or topology errors.
    pub fn rebase<U: Eq + Clone, E>(
        &self,
        problem: &ListProblem<U>,
        before: &ListAssignment<U>,
        mut entity: impl FnMut(usize) -> Result<usize, E>,
        mut value: impl FnMut(&V) -> Result<U, E>,
    ) -> Result<ListKOptEdgePlan<U>, ListKOptRebaseError<E>> {
        let entity = entity(self.entity).map_err(ListKOptRebaseError::Lookup)?;
        let mut row = Vec::with_capacity(self.before.len());
        for source in &self.before {
            let mapped = value(source).map_err(ListKOptRebaseError::Lookup)?;
            if row.contains(&mapped) {
                return Err(ListKOptRebaseError::Collision(
                    MoveRebaseNamespace::ListValue,
                ));
            }
            row.push(mapped);
        }
        problem
            .validate(before)
            .map_err(|e| ListKOptRebaseError::Topology(e.into()))?;
        let meta = problem.entities().get(entity).ok_or_else(|| {
            ListKOptRebaseError::Topology(
                ListError::EntityIndex {
                    entity,
                    count: problem.entities().len(),
                }
                .into(),
            )
        })?;
        if before.lists()[entity] != row || meta.pinned_prefix() != self.prefix {
            return Err(ListKOptRebaseError::Topology(ListKOptError::Partition));
        }
        ListKOptEdgePlan::prepare(problem, before, entity, &self.removed, &self.added)
            .map_err(ListKOptRebaseError::Topology)
    }
}
