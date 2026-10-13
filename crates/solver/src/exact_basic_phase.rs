//! Real basic moves for an optional exhaustive phase with full entity selection.
use crate::*;
use std::{error::Error, fmt};
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactBasicModelError {
    EntityOrder,
    ValueOrder { variable: usize },
    Model(ModelError),
    Scoring(ScoringError),
}
impl fmt::Display for ExactBasicModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "basic exact phase: {self:?}")
    }
}
impl Error for ExactBasicModelError {}
/// Generate each still-unassigned movable variable in declared entity order, with
/// each domain in its own validated value order. Every expansion considers all
/// remaining entities, so permutations of assignment choices remain distinct paths.
/// Caller supplies an admissible bound from its actual objective/trend; no bound
/// is inferred from callback bodies. Ordinary solver defaults remain unchanged.
pub struct ExactBasicPhaseModel<'p, 'n, 'c, V, S, B> {
    problem: &'p FiniteProblem<V>,
    network: &'n ConstraintNetwork<'c, V, S>,
    entities: Vec<usize>,
    values: Vec<Vec<usize>>,
    bound: B,
    depth: usize,
}
fn permutation(order: &[usize], count: usize) -> bool {
    order.len() == count && order.iter().all(|&i| i < count) && {
        let mut copy = order.to_vec();
        copy.sort_unstable();
        copy.iter().copied().eq(0..count)
    }
}
impl<'p, 'n, 'c, V: Eq + Clone, S: AdditiveScore, B> ExactBasicPhaseModel<'p, 'n, 'c, V, S, B> {
    pub fn new(
        problem: &'p FiniteProblem<V>,
        network: &'n ConstraintNetwork<'c, V, S>,
        entities: Vec<usize>,
        values: Vec<Vec<usize>>,
        bound: B,
    ) -> Result<Self, ExactBasicModelError> {
        if !permutation(&entities, problem.variables().len()) {
            return Err(ExactBasicModelError::EntityOrder);
        }
        if values.len() != problem.variables().len() {
            return Err(ExactBasicModelError::ValueOrder {
                variable: values.len(),
            });
        }
        for (i, order) in values.iter().enumerate() {
            if !permutation(order, problem.variables()[i].domain().len()) {
                return Err(ExactBasicModelError::ValueOrder { variable: i });
            }
        }
        let depth = problem
            .variables()
            .iter()
            .filter(|v| !v.is_pinned() && v.initial().is_none())
            .count();
        Ok(Self {
            problem,
            network,
            entities,
            values,
            bound,
            depth,
        })
    }
}
impl<V: Eq + Clone, S: AdditiveScore, B: FnMut(&Assignment<V>, &S) -> Option<S>> ExactPhaseModel
    for ExactBasicPhaseModel<'_, '_, '_, V, S, B>
{
    type Assignment = Assignment<V>;
    type Move = Move<V>;
    type Score = S;
    type Error = ExactBasicModelError;
    fn initial(&mut self) -> Result<Assignment<V>, Self::Error> {
        Ok(self.problem.initial_assignment())
    }
    fn max_depth(&self) -> usize {
        self.depth
    }
    fn moves(&mut self, _depth: usize, a: &Assignment<V>) -> Result<Vec<Move<V>>, Self::Error> {
        let mut moves = Vec::new();
        for &variable in &self.entities {
            if self.problem.variables()[variable].is_pinned() || a.values()[variable].is_some() {
                continue;
            }
            for &index in &self.values[variable] {
                moves.push(Move::Change {
                    variable,
                    value: Some(self.problem.variables()[variable].domain()[index].clone()),
                });
            }
        }
        Ok(moves)
    }
    fn apply(&mut self, a: &mut Assignment<V>, m: &Move<V>) -> Result<(), Self::Error> {
        self.problem
            .apply(a, m)
            .map_err(ExactBasicModelError::Model)?
            .commit();
        Ok(())
    }
    fn inverse(&self, before: &Assignment<V>, after: &Assignment<V>) -> Move<V> {
        crate::temporary_moves::inverse_basic(before, after)
    }
    fn score(&mut self, a: &Assignment<V>) -> Result<S, Self::Error> {
        self.network
            .calculate(self.problem, a)
            .map_err(ExactBasicModelError::Scoring)
    }
    fn optimistic_bound(
        &mut self,
        _depth: usize,
        a: &Assignment<V>,
        s: &S,
    ) -> Result<Option<S>, Self::Error> {
        Ok((self.bound)(a, s))
    }
}
