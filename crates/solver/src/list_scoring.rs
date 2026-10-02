//! Static list/entity and element scopes with transactional contribution caches.
//!
//! ```
//! use timetuck::{ListConstraintNetwork, ListEntity, ListFact, ListMove,
//!     ListPosition, ListProblem, ListScope, ListScoreDirector, SimpleScore};
//! let problem = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1])])?;
//! let mut network = ListConstraintNetwork::new(1, 2);
//! network.add_constraint("load", vec![ListScope::Entity(0)], |facts| {
//!     let ListFact::Entity(values) = &facts[0] else { unreachable!() };
//!     SimpleScore::new(-(values.len() as i64))
//! })?;
//! let mut director = ListScoreDirector::new(&problem, &network,
//!     problem.initial_assignment())?;
//! assert_eq!(director.score().structural(), -1);
//! director.apply(&ListMove::Assign { value: 2,
//!     destination: ListPosition { entity: 0, index: 1 } })?.commit();
//! assert_eq!(director.score(), SimpleScore::new(-2));
//! director.assert_consistent()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::{
    AdditiveScore, AppliedListMove, ListAssignment, ListElement, ListError, ListMove, ListProblem,
};
use std::{error::Error, fmt};

/// Entity lists or stable value-range indices, in callback argument order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ListScope {
    Entity(usize),
    Value(usize),
}

/// A scoped read-only fact. An unassigned value retains its identity but has no
/// owner/index/neighbors. Callbacks cannot inspect other lists through this API.
#[derive(Debug)]
pub enum ListFact<'a, V> {
    Entity(&'a [V]),
    Value {
        value: &'a V,
        element: Option<ListElement<'a, V>>,
    },
}

type Evaluator<'a, V, S> = dyn Fn(&[ListFact<'_, V>]) -> S + 'a;
struct Constraint<'a, V, S> {
    name: String,
    scope: Vec<ListScope>,
    evaluate: Box<Evaluator<'a, V, S>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListScoringError {
    Model(ListError),
    ModelShape {
        expected_entities: usize,
        actual_entities: usize,
        expected_values: usize,
        actual_values: usize,
    },
    InvalidScope(ListScope),
    DuplicateName(String),
    StructuralContribution {
        constraint: usize,
    },
    CorruptedConstraint {
        constraint: usize,
    },
    CorruptedTotal,
}
impl fmt::Display for ListScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "list scoring error: {self:?}")
    }
}
impl Error for ListScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Model(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
impl From<ListError> for ListScoringError {
    fn from(e: ListError) -> Self {
        Self::Model(e)
    }
}

/// A deterministic additive network. Captured facts must remain unchanged during
/// a scoring session. Contributions must have structural zero; the network owns
/// the missing-required-value component. Repeated scopes preserve argument order.
/// Empty scopes are constant constraints and are never reevaluated by a move.
pub struct ListConstraintNetwork<'a, V, S> {
    constraints: Vec<Constraint<'a, V, S>>,
    entity_dependents: Vec<Vec<usize>>,
    value_dependents: Vec<Vec<usize>>,
}
impl<'a, V: Eq + Clone, S: AdditiveScore> ListConstraintNetwork<'a, V, S> {
    pub fn new(entity_count: usize, value_count: usize) -> Self {
        Self {
            constraints: vec![],
            entity_dependents: vec![vec![]; entity_count],
            value_dependents: vec![vec![]; value_count],
        }
    }
    pub fn entity_count(&self) -> usize {
        self.entity_dependents.len()
    }
    pub fn value_count(&self) -> usize {
        self.value_dependents.len()
    }
    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
    pub fn add_constraint(
        &mut self,
        name: impl Into<String>,
        scope: Vec<ListScope>,
        evaluate: impl Fn(&[ListFact<'_, V>]) -> S + 'a,
    ) -> Result<usize, ListScoringError> {
        let name = name.into();
        if self.constraints.iter().any(|c| c.name == name) {
            return Err(ListScoringError::DuplicateName(name));
        }
        for &item in &scope {
            let valid = match item {
                ListScope::Entity(i) => i < self.entity_count(),
                ListScope::Value(i) => i < self.value_count(),
            };
            if !valid {
                return Err(ListScoringError::InvalidScope(item));
            }
        }
        let index = self.constraints.len();
        for &item in &scope {
            let dependents = match item {
                ListScope::Entity(i) => &mut self.entity_dependents[i],
                ListScope::Value(i) => &mut self.value_dependents[i],
            };
            if dependents.last() != Some(&index) {
                dependents.push(index);
            }
        }
        self.constraints.push(Constraint {
            name,
            scope,
            evaluate: Box::new(evaluate),
        });
        Ok(index)
    }
    pub(crate) fn validate(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<(), ListScoringError> {
        if self.entity_count() != problem.entities().len()
            || self.value_count() != problem.values().len()
        {
            return Err(ListScoringError::ModelShape {
                expected_entities: self.entity_count(),
                actual_entities: problem.entities().len(),
                expected_values: self.value_count(),
                actual_values: problem.values().len(),
            });
        }
        problem.validate(assignment)?;
        Ok(())
    }
    fn contribution(
        &self,
        index: usize,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<S, ListScoringError> {
        let constraint = &self.constraints[index];
        let facts: Vec<_> = constraint
            .scope
            .iter()
            .map(|scope| match *scope {
                ListScope::Entity(i) => ListFact::Entity(&assignment.lists()[i]),
                ListScope::Value(i) => {
                    let value = &problem.values()[i];
                    ListFact::Value {
                        value,
                        element: assignment.element(value),
                    }
                }
            })
            .collect();
        let score = (constraint.evaluate)(&facts);
        if score.structural() != 0 {
            return Err(ListScoringError::StructuralContribution { constraint: index });
        }
        Ok(score)
    }
    /// Recompute every contribution, independently of any director cache.
    pub fn calculate(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<S, ListScoringError> {
        self.validate(problem, assignment)?;
        let mut score = S::ZERO;
        for i in 0..self.constraint_count() {
            score = score + self.contribution(i, problem, assignment)?;
        }
        Ok(with_structure(score, problem, assignment))
    }
}
pub(crate) fn with_structure<V: Eq + Clone, S: AdditiveScore>(
    score: S,
    problem: &ListProblem<V>,
    assignment: &ListAssignment<V>,
) -> S {
    let missing = if problem.allows_unassigned() {
        0
    } else {
        problem.values().len() - assignment.lists().iter().map(Vec::len).sum::<usize>()
    };
    score.with_structural(-(missing as i64))
}

/// Owns a validated working assignment and cached contributions. List edits
/// retain the existing full-state snapshot transaction; callback evaluation is
/// incremental, while model validation and dependency discovery still scan lists.
pub struct ListScoreDirector<'p, 'c, V, S> {
    problem: &'p ListProblem<V>,
    network: &'p ListConstraintNetwork<'c, V, S>,
    assignment: ListAssignment<V>,
    contributions: Vec<S>,
    score: S,
}
pub(crate) struct ListSelection<V> {
    pub selected: Option<ListMove<V>>,
    pub selected_accepted: bool,
    pub evaluations: u64,
    pub constraint_evaluations: u128,
    pub evaluation_limit: bool,
}
struct Selected<V, S> {
    movement: ListMove<V>,
    assignment: ListAssignment<V>,
    score: S,
    updates: Vec<(usize, S)>,
    accepted: bool,
}
impl<'p, 'c, V: Eq + Clone, S: AdditiveScore> ListScoreDirector<'p, 'c, V, S> {
    pub fn new(
        problem: &'p ListProblem<V>,
        network: &'p ListConstraintNetwork<'c, V, S>,
        assignment: ListAssignment<V>,
    ) -> Result<Self, ListScoringError> {
        network.validate(problem, &assignment)?;
        let contributions = (0..network.constraint_count())
            .map(|i| network.contribution(i, problem, &assignment))
            .collect::<Result<Vec<_>, _>>()?;
        let total = contributions.iter().fold(S::ZERO, |total, &s| total + s);
        let score = with_structure(total, problem, &assignment);
        Ok(Self {
            problem,
            network,
            assignment,
            contributions,
            score,
        })
    }
    /// Keep candidates and sparse updates within this exclusive session borrow.
    /// Any error or panic before installation leaves the base and cache intact.
    pub(crate) fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        allow_rejected: bool,
        mut accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, ListScoringError> {
        let mut result = ListSelection {
            selected: None,
            selected_accepted: false,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut best: Option<Selected<V, S>> = None;
        let network = self.network;
        let problem = self.problem;
        for movement in moves {
            if result.evaluations == max_evaluations {
                result.evaluation_limit = true;
                break;
            }
            let candidate = self.apply(&movement)?;
            result.evaluations += 1;
            result.constraint_evaluations += candidate.constraint_evaluations() as u128;
            if assert_scores
                && network.calculate(problem, candidate.assignment())? != candidate.score()
            {
                return Err(ListScoringError::CorruptedTotal);
            }
            let accepted = accepts(&movement, candidate.score());
            if !accepted && (!allow_rejected || candidate.score().structural() < 0) {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|saved| (accepted, candidate.score()) > (saved.accepted, saved.score))
            {
                best = Some(Selected {
                    movement: movement.clone(),
                    assignment: candidate.assignment().clone(),
                    score: candidate.score(),
                    updates: candidate.updates.clone(),
                    accepted,
                });
            }
            if accepted && quit_early(candidate.score()) {
                break;
            }
        }
        if let Some(best) = best {
            self.assignment = best.assignment;
            for (index, score) in best.updates {
                self.contributions[index] = score;
            }
            self.score = best.score;
            result.selected = Some(best.movement);
            result.selected_accepted = best.accepted;
        }
        Ok(result)
    }
    pub fn assignment(&self) -> &ListAssignment<V> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.network
            .constraints
            .iter()
            .zip(&self.contributions)
            .map(|(c, &s)| (c.name.as_str(), s))
    }
    /// Check individual contributions, detecting cancelling errors as well as
    /// total corruption. Diagnostic callback evaluations are not move statistics.
    pub fn assert_consistent(&self) -> Result<(), ListScoringError> {
        let mut total = S::ZERO;
        for (i, &cached) in self.contributions.iter().enumerate() {
            let actual = self
                .network
                .contribution(i, self.problem, &self.assignment)?;
            if actual != cached {
                return Err(ListScoringError::CorruptedConstraint { constraint: i });
            }
            total = total + actual;
        }
        if with_structure(total, self.problem, &self.assignment) != self.score {
            return Err(ListScoringError::CorruptedTotal);
        }
        Ok(())
    }
    /// Reevaluate each dependent constraint once. Entity scopes depend on touched
    /// lists; value scopes depend on any value present in those lists before or
    /// after the composite, including neighbors whose index/adjacency changed.
    /// This is conservative invalidation, including no-op edits. Cache updates
    /// are deferred; errors and ordinary panic unwinding leave the base unchanged.
    pub fn apply(
        &mut self,
        movement: &ListMove<V>,
    ) -> Result<ScoredListMove<'_, V, S>, ListScoringError> {
        let transaction = self.problem.apply(&mut self.assignment, movement)?;
        let touched = movement.touched_entities();
        let mut affected: Vec<_> = touched
            .iter()
            .flat_map(|&e| self.network.entity_dependents[e].iter().copied())
            .collect();
        for (i, value) in self.problem.values().iter().enumerate() {
            let belongs = |a: &ListAssignment<V>| {
                a.location(value)
                    .is_some_and(|p| touched.binary_search(&p.entity).is_ok())
            };
            if belongs(transaction.original()) || belongs(&transaction) {
                affected.extend(&self.network.value_dependents[i]);
            }
        }
        affected.sort_unstable();
        affected.dedup();
        let mut score = self.score;
        let mut updates = Vec::with_capacity(affected.len());
        for i in affected {
            let contribution = self.network.contribution(i, self.problem, &transaction)?;
            score = score - self.contributions[i] + contribution;
            updates.push((i, contribution));
        }
        let candidate_score = with_structure(score, self.problem, &transaction);
        Ok(ScoredListMove {
            transaction,
            updates,
            candidate_score,
            cached_score: &mut self.score,
            contributions: &mut self.contributions,
        })
    }
}

/// Read-only scored candidate. Dropping rolls back all lists; commit installs
/// both the assignment and its cache updates without invoking callbacks again.
#[must_use = "dropping a scored list move rejects it; call commit to retain it"]
pub struct ScoredListMove<'a, V, S> {
    transaction: AppliedListMove<'a, V>,
    updates: Vec<(usize, S)>,
    candidate_score: S,
    cached_score: &'a mut S,
    contributions: &'a mut [S],
}
impl<V, S: Copy> ScoredListMove<'_, V, S> {
    pub fn assignment(&self) -> &ListAssignment<V> {
        &self.transaction
    }
    pub fn score(&self) -> S {
        self.candidate_score
    }
    pub fn constraint_evaluations(&self) -> usize {
        self.updates.len()
    }
    pub fn commit(self) {
        for (index, score) in self.updates {
            self.contributions[index] = score;
        }
        *self.cached_score = self.candidate_score;
        self.transaction.commit();
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::{ListEntity, ListPosition, SimpleScore};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn selection_failure_after_saved_candidate_restores_assignment_and_cache() {
        let p = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0])])
            .unwrap()
            .allowing_unassigned();
        for panic in [false, true] {
            let mut network = ListConstraintNetwork::new(1, 3);
            network
                .add_constraint(
                    "fail on second candidate",
                    vec![ListScope::Entity(0)],
                    |f| {
                        let ListFact::Entity(list) = &f[0] else {
                            unreachable!()
                        };
                        if list.contains(&2) {
                            if panic {
                                panic!("callback failure");
                            }
                            return SimpleScore::ZERO.with_structural(-1);
                        }
                        SimpleScore::new(list.len() as i64)
                    },
                )
                .unwrap();
            let mut d = ListScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    [1, 2].map(|value| ListMove::Assign {
                        value,
                        destination: ListPosition {
                            entity: 0,
                            index: 0,
                        },
                    }),
                    2,
                    false,
                    |_, _| true,
                    |_| false,
                    false,
                )
            }));
            if panic {
                assert!(outcome.is_err());
            } else {
                assert!(matches!(
                    outcome.unwrap(),
                    Err(ListScoringError::StructuralContribution { constraint: 0 })
                ));
            }
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.score(), SimpleScore::new(1));
            d.assert_consistent().unwrap();
        }
    }
}
