//! Deterministic exact search for finite basic-variable problems.
//!
//! This is an exhaustive correctness baseline for small problems, not a scalable
//! heuristic. A result is proven optimal only after the entire assignment space
//! is exhausted. Finishing a fixed evaluation budget alone proves no optimality.

use crate::{Score, model::FiniteProblem, moves::Assignment};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Termination {
    /// Every complete assignment was evaluated.
    Exhausted,
    /// Unvisited assignments remain, but no score-evaluation budget remains.
    EvaluationLimit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredAssignment<V, S> {
    pub assignment: Assignment<V>,
    pub score: S,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResult<V, S> {
    /// Best structurally sound complete assignment seen, even if hard-infeasible.
    pub best: Option<ScoredAssignment<V, S>>,
    pub evaluations: u64,
    pub termination: Termination,
}
impl<V, S> SearchResult<V, S> {
    pub fn is_optimal(&self) -> bool {
        self.termination == Termination::Exhausted && self.best.is_some()
    }
}

/// Maximize a deterministic score over all complete assignments until exhausted
/// or `max_evaluations` scores have been calculated. `u64::MAX` is the largest
/// supported budget. A zero budget is valid and does not invoke the scorer.
///
/// Domain order is preserved, with `None` last for optional variables. Pinned
/// values stay fixed. Initial values of movable variables do not restrict search.
/// Ties retain the first complete assignment encountered. Negative structural
/// scores are rejected; negative hard scores are retained as infeasible candidates.
/// The callback must be deterministic and may not mutate problem facts through
/// interior mutability. Optimality claims depend on that scoring contract.
///
/// The working assignment and best solution are owned here; the caller's model
/// is never mutated. Enumeration is iterative, using linear auxiliary memory.
pub fn exhaustive_search<V, S, F>(
    problem: &FiniteProblem<V>,
    mut evaluate: F,
    max_evaluations: u64,
) -> SearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&Assignment<V>) -> S,
{
    let mut result = SearchResult {
        best: None,
        evaluations: 0,
        termination: Termination::Exhausted,
    };
    if problem.search_space_size() == Some(0) {
        return result;
    }
    let variables = problem.variables();
    let mut working = problem.initial_assignment();
    let mut choices = vec![0_usize; variables.len()];
    let movable: Vec<_> = variables
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (!v.is_pinned()).then_some(i))
        .collect();
    for &index in &movable {
        working.values[index] = variables[index].domain().first().cloned();
    }
    loop {
        if result.evaluations == max_evaluations {
            result.termination = Termination::EvaluationLimit;
            return result;
        }
        let score = evaluate(&working);
        result.evaluations += 1;
        if score.structural() >= 0 && result.best.as_ref().is_none_or(|best| score > best.score) {
            result.best = Some(ScoredAssignment {
                assignment: working.clone(),
                score,
            });
        }
        let mut advanced = false;
        for &index in movable.iter().rev() {
            let variable = &variables[index];
            let last = if variable.allows_unassigned() {
                variable.domain().len()
            } else {
                variable.domain().len() - 1
            };
            if choices[index] < last {
                choices[index] += 1;
                working.values[index] = variable.domain().get(choices[index]).cloned();
                advanced = true;
                break;
            }
            choices[index] = 0;
            working.values[index] = variable.domain().first().cloned();
        }
        if !advanced {
            return result;
        }
    }
}
