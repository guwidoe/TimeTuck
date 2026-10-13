//! Explicit exhaustive phase adapter over native reversible ordered-list moves.
use crate::*;

/// A list phase branches over every currently unassigned value and permitted
/// entity, appending at the end. Descendants append only to the same or a later
/// entity in the configured order, giving each completed layout a canonical path.
/// Scheduling depth is the last destination entity position; completion is tested
/// independently against the actual registry.
/// Orders are complete permutations of native entity and value registry indices.
/// The caller supplies scoring and an admissible bound; no score trend is inferred.
pub struct ExactListPhaseModel<'a, V, S, F, B> {
    problem: &'a ListProblem<V>,
    entity_order: Vec<usize>,
    value_order: Vec<usize>,
    score: F,
    bound: B,
    marker: std::marker::PhantomData<S>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactListPhaseError {
    InvalidEntityOrder,
    InvalidValueOrder,
    List(ListError),
}
impl<'a, V: Eq + Clone, S: Score, F, B> ExactListPhaseModel<'a, V, S, F, B>
where
    F: Fn(&ListAssignment<V>) -> S,
    B: Fn(usize, &ListAssignment<V>, &S) -> Option<S>,
{
    pub fn new(
        problem: &'a ListProblem<V>,
        entity_order: Vec<usize>,
        value_order: Vec<usize>,
        score: F,
        bound: B,
    ) -> Result<Self, ExactListPhaseError> {
        fn permutation(order: &[usize], count: usize) -> bool {
            let mut sorted = order.to_vec();
            sorted.sort_unstable();
            sorted == (0..count).collect::<Vec<_>>()
        }
        if !permutation(&entity_order, problem.entities().len()) {
            return Err(ExactListPhaseError::InvalidEntityOrder);
        }
        if !permutation(&value_order, problem.values().len()) {
            return Err(ExactListPhaseError::InvalidValueOrder);
        }
        Ok(Self {
            problem,
            entity_order,
            value_order,
            score,
            bound,
            marker: std::marker::PhantomData,
        })
    }
}
impl<V: Eq + Clone, S: Score, F, B> ExactPhaseModel for ExactListPhaseModel<'_, V, S, F, B>
where
    F: Fn(&ListAssignment<V>) -> S,
    B: Fn(usize, &ListAssignment<V>, &S) -> Option<S>,
{
    type Assignment = ListAssignment<V>;
    type Move = ListMove<V>;
    type Score = S;
    type Error = ExactListPhaseError;
    fn initial(&mut self) -> Result<Self::Assignment, Self::Error> {
        Ok(self.problem.initial_assignment())
    }
    fn max_depth(&self) -> usize {
        self.problem.entities().len()
    }
    fn moves(
        &mut self,
        depth: usize,
        assignment: &Self::Assignment,
    ) -> Result<Vec<Self::Move>, Self::Error> {
        let mut result = Vec::new();
        for &entity in self.entity_order.iter().skip(depth.saturating_sub(1)) {
            if self.problem.entities()[entity].is_pinned() {
                continue;
            }
            for &index in &self.value_order {
                let value = &self.problem.values()[index];
                if assignment.location(value).is_some() {
                    continue;
                }
                let movement = ListMove::Assign {
                    value: value.clone(),
                    destination: ListPosition {
                        entity,
                        index: assignment.lists()[entity].len(),
                    },
                };
                let mut trial = assignment.clone();
                if self.problem.apply(&mut trial, &movement).is_ok() {
                    result.push(movement);
                }
            }
        }
        Ok(result)
    }
    fn apply(
        &mut self,
        assignment: &mut Self::Assignment,
        movement: &Self::Move,
    ) -> Result<(), Self::Error> {
        self.problem
            .apply(assignment, movement)
            .map_err(ExactListPhaseError::List)?
            .commit();
        Ok(())
    }
    fn inverse(&self, before: &Self::Assignment, after: &Self::Assignment) -> Self::Move {
        let entity = before
            .lists()
            .iter()
            .zip(after.lists())
            .position(|(a, b)| a != b)
            .expect("an append changes one list");
        let start = before.lists()[entity].len();
        ListMove::Unassign {
            entity,
            start,
            end: start + 1,
        }
    }
    fn child_depth(&self, _: usize, movement: &Self::Move, _: &Self::Assignment) -> usize {
        match movement {
            ListMove::Assign { destination, .. } => {
                self.entity_order
                    .iter()
                    .position(|&e| e == destination.entity)
                    .expect("generated entity")
                    + 1
            }
            _ => unreachable!("phase only generates assignments"),
        }
    }
    fn is_terminal(&self, _: usize, assignment: &Self::Assignment) -> bool {
        self.problem
            .values()
            .iter()
            .all(|v| assignment.location(v).is_some())
    }
    fn score(&mut self, assignment: &Self::Assignment) -> Result<S, Self::Error> {
        Ok((self.score)(assignment))
    }
    fn optimistic_bound(
        &mut self,
        depth: usize,
        assignment: &Self::Assignment,
        score: &S,
    ) -> Result<Option<S>, Self::Error> {
        Ok((self.bound)(depth, assignment, score))
    }
    fn restore_mode(&self) -> BranchRestoreMode {
        BranchRestoreMode::RootReplay
    }
}
