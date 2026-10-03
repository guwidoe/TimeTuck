//! Scoped constraints and transactional incremental score recomputation.
//!
//! A constraint receives only the values in its declared scope. A move rescans
//! affected constraints once each; score caches change only when it commits.
use crate::{AdditiveScore, AppliedMove, Assignment, FiniteProblem, ModelError, Move};
use std::{error::Error, fmt};

type Evaluator<'a, V, S> = dyn Fn(&[Option<&V>]) -> S + 'a;
struct Constraint<'a, V, S> {
    name: String,
    scope: Vec<usize>,
    evaluate: Box<Evaluator<'a, V, S>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoringError {
    Model(ModelError),
    VariableCount {
        expected: usize,
        actual: usize,
    },
    InvalidScope {
        variable: usize,
        variable_count: usize,
    },
    DuplicateName(String),
    StructuralContribution {
        constraint: usize,
    },
    CorruptedConstraint {
        constraint: usize,
    },
    CorruptedTotal,
}
impl fmt::Display for ScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::VariableCount { expected, actual } => write!(
                f,
                "constraint network expects {expected} variables, model has {actual}"
            ),
            Self::InvalidScope {
                variable,
                variable_count,
            } => write!(
                f,
                "constraint scope index {variable} is outside {variable_count} variables"
            ),
            Self::DuplicateName(name) => write!(f, "duplicate constraint name: {name}"),
            Self::StructuralContribution { constraint } => write!(
                f,
                "constraint {constraint} returned a nonzero structural component"
            ),
            Self::CorruptedConstraint { constraint } => write!(
                f,
                "constraint {constraint} differs from its cached contribution"
            ),
            Self::CorruptedTotal => {
                f.write_str("cached total differs from full score recomputation")
            }
        }
    }
}
impl Error for ScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Model(e) = self {
            Some(e)
        } else {
            None
        }
    }
}
impl From<ModelError> for ScoringError {
    fn from(value: ModelError) -> Self {
        Self::Model(value)
    }
}

/// A fixed network of named constraints over basic variables.
///
/// Callbacks must be deterministic; captured facts must not change during a
/// scoring session. A callback must return structural zero: the network manages
/// structural incompleteness from required unassigned variables. Ordinary levels
/// add using the score type's arithmetic (wrapping for the built-in integer scores).
/// Scopes may repeat a variable; callback arguments preserve those repetitions.
/// An empty scope represents a constant contribution.
pub struct ConstraintNetwork<'a, V, S> {
    constraints: Vec<Constraint<'a, V, S>>,
    dependents: Vec<Vec<usize>>,
}
impl<'a, V, S: AdditiveScore> ConstraintNetwork<'a, V, S> {
    pub fn new(variable_count: usize) -> Self {
        Self {
            constraints: Vec::new(),
            dependents: vec![Vec::new(); variable_count],
        }
    }
    pub fn variable_count(&self) -> usize {
        self.dependents.len()
    }
    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
    pub fn add_constraint(
        &mut self,
        name: impl Into<String>,
        scope: Vec<usize>,
        evaluate: impl Fn(&[Option<&V>]) -> S + 'a,
    ) -> Result<usize, ScoringError> {
        let name = name.into();
        if self.constraints.iter().any(|c| c.name == name) {
            return Err(ScoringError::DuplicateName(name));
        }
        for &variable in &scope {
            if variable >= self.variable_count() {
                return Err(ScoringError::InvalidScope {
                    variable,
                    variable_count: self.variable_count(),
                });
            }
        }
        let index = self.constraints.len();
        let mut unique = scope.clone();
        unique.sort_unstable();
        unique.dedup();
        for variable in unique {
            self.dependents[variable].push(index);
        }
        self.constraints.push(Constraint {
            name,
            scope,
            evaluate: Box::new(evaluate),
        });
        Ok(index)
    }
    fn contribution(&self, index: usize, assignment: &Assignment<V>) -> Result<S, ScoringError> {
        let constraint = &self.constraints[index];
        let arguments: Vec<_> = constraint
            .scope
            .iter()
            .map(|&i| assignment.values()[i].as_ref())
            .collect();
        let score = (constraint.evaluate)(&arguments);
        if score.structural() != 0 {
            return Err(ScoringError::StructuralContribution { constraint: index });
        }
        Ok(score)
    }
}
impl<V: Eq, S: AdditiveScore> ConstraintNetwork<'_, V, S> {
    pub(crate) fn validate(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
    ) -> Result<(), ScoringError> {
        if self.variable_count() != problem.variables().len() {
            return Err(ScoringError::VariableCount {
                expected: self.variable_count(),
                actual: problem.variables().len(),
            });
        }
        problem.validate(assignment)?;
        Ok(())
    }
    /// Full score recomputation, independent of the incremental contribution cache.
    pub fn calculate(
        &self,
        problem: &FiniteProblem<V>,
        assignment: &Assignment<V>,
    ) -> Result<S, ScoringError> {
        self.validate(problem, assignment)?;
        let mut total = S::ZERO;
        for i in 0..self.constraint_count() {
            total = total + self.contribution(i, assignment)?;
        }
        Ok(with_structure(total, problem, assignment))
    }
}
pub(crate) fn with_structure<V: Eq, S: AdditiveScore>(
    total: S,
    problem: &FiniteProblem<V>,
    assignment: &Assignment<V>,
) -> S {
    let missing = problem
        .variables()
        .iter()
        .zip(assignment.values())
        .filter(|(v, value)| !v.allows_unassigned() && value.is_none())
        .count();
    // A physically realizable Vec cannot contain more than isize::MAX elements.
    total.with_structural(-(missing as i64))
}

/// Owns a working assignment and cached per-constraint contributions.
/// It borrows the model/network, preventing their mutation during the session.
pub struct ScoreDirector<'p, 'c, V, S> {
    problem: &'p FiniteProblem<V>,
    network: &'p ConstraintNetwork<'c, V, S>,
    assignment: Assignment<V>,
    contributions: Vec<S>,
    score: S,
}

struct SelectedCandidate<V, S> {
    accepted: bool,
    movement: Move<V>,
    assignment: Assignment<V>,
    score: S,
    updates: Vec<(usize, S)>,
}

pub(crate) struct Selection<V> {
    pub selected_accepted: bool,
    pub selected: Option<Move<V>>,
    pub evaluations: u64,
    pub constraint_evaluations: u128,
    pub evaluation_limit: bool,
}
impl<'p, 'c, V: Eq + Clone, S: AdditiveScore> ScoreDirector<'p, 'c, V, S> {
    pub fn new(
        problem: &'p FiniteProblem<V>,
        network: &'p ConstraintNetwork<'c, V, S>,
        assignment: Assignment<V>,
    ) -> Result<Self, ScoringError> {
        network.validate(problem, &assignment)?;
        let contributions = (0..network.constraint_count())
            .map(|i| network.contribution(i, &assignment))
            .collect::<Result<Vec<_>, _>>()?;
        let sum = contributions
            .iter()
            .fold(S::ZERO, |sum, score| sum + score.clone());
        let score = with_structure(sum, problem, &assignment);
        Ok(Self {
            problem,
            network,
            assignment,
            contributions,
            score,
        })
    }
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score.clone()
    }
    /// Evaluate against one unchanged base, retaining the first highest-scoring
    /// accepted candidate. The saved assignment and sparse cache updates stay inside this
    /// exclusive director borrow. Installing them does not invoke callbacks again.
    /// Errors/unwinding before installation leave the base and caches unchanged.
    pub(crate) fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        allow_rejected: bool,
        mut accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<Selection<V>, ScoringError> {
        let mut result = Selection {
            selected_accepted: false,
            selected: None,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut best: Option<SelectedCandidate<V, S>> = None;
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
                return Err(ScoringError::CorruptedTotal);
            }
            let accepted = accepts(&movement, candidate.score());
            if !accepted && (!allow_rejected || candidate.score().structural() < 0) {
                continue;
            }
            if best
                .as_ref()
                .is_none_or(|saved| (accepted, &candidate.score()) > (saved.accepted, &saved.score))
            {
                best = Some(SelectedCandidate {
                    accepted,
                    movement: movement.clone(),
                    assignment: candidate.assignment().clone(),
                    score: candidate.score(),
                    updates: candidate.updates.clone(),
                });
            }
            if accepted && quit_early(candidate.score()) {
                break;
            }
        }
        if let Some(selected) = best {
            self.assignment = selected.assignment;
            for (i, contribution) in selected.updates {
                self.contributions[i] = contribution;
            }
            self.score = selected.score;
            result.selected = Some(selected.movement);
            result.selected_accepted = selected.accepted;
        }
        Ok(result)
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.network
            .constraints
            .iter()
            .zip(&self.contributions)
            .map(|(c, s)| (c.name.as_str(), s.clone()))
    }
    /// Compare every cached contribution and the total against fresh calculation.
    /// This is diagnostic work and is not included in search evaluation budgets.
    pub fn assert_consistent(&self) -> Result<(), ScoringError> {
        let mut sum = S::ZERO;
        for (i, cached) in self.contributions.iter().enumerate() {
            let actual = self.network.contribution(i, &self.assignment)?;
            if *cached != actual {
                return Err(ScoringError::CorruptedConstraint { constraint: i });
            }
            sum = sum + actual;
        }
        if self.score != with_structure(sum, self.problem, &self.assignment) {
            return Err(ScoringError::CorruptedTotal);
        }
        Ok(())
    }
    /// Compute a candidate transaction. Errors and ordinary panic unwinding
    /// restore values; caches remain unchanged until `ScoredMove::commit`.
    pub fn apply(&mut self, movement: &Move<V>) -> Result<ScoredMove<'_, V, S>, ScoringError> {
        let transaction = self.problem.apply(&mut self.assignment, movement)?;
        let mut affected: Vec<_> = transaction
            .prior_values()
            .iter()
            .flat_map(|write| self.network.dependents[write.variable].iter().copied())
            .collect();
        affected.sort_unstable();
        affected.dedup();
        let mut score = self.score.clone();
        let mut updates = Vec::with_capacity(affected.len());
        for i in affected {
            let new_score = self.network.contribution(i, &transaction)?;
            score = score - self.contributions[i].clone() + new_score.clone();
            updates.push((i, new_score));
        }
        let candidate_score = with_structure(score, self.problem, &transaction);
        Ok(ScoredMove {
            transaction,
            updates,
            candidate_score,
            cached_score: &mut self.score,
            contributions: &mut self.contributions,
        })
    }
}

/// A scored candidate. Dropping rejects it; committing atomically keeps its
/// variable values and contribution cache. See `AppliedMove` for drop limits.
#[must_use = "dropping a scored move rolls it back; commit to retain it"]
pub struct ScoredMove<'a, V, S> {
    transaction: AppliedMove<'a, V>,
    updates: Vec<(usize, S)>,
    candidate_score: S,
    cached_score: &'a mut S,
    contributions: &'a mut [S],
}
impl<V, S: Clone> ScoredMove<'_, V, S> {
    pub fn assignment(&self) -> &Assignment<V> {
        &self.transaction
    }
    pub fn score(&self) -> S {
        self.candidate_score.clone()
    }
    pub fn constraint_evaluations(&self) -> usize {
        self.updates.len()
    }
    pub fn commit(self) {
        for (i, score) in self.updates {
            self.contributions[i] = score;
        }
        *self.cached_score = self.candidate_score;
        self.transaction.commit();
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::{SimpleScore, Variable};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn selection_errors_and_panics_restore_the_base_after_a_saved_candidate() {
        let problem =
            FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
        for panic in [false, true] {
            let mut network = ConstraintNetwork::new(1);
            network
                .add_constraint("failure on second candidate", vec![0], |v| {
                    let value = *v[0].unwrap();
                    if value == 2 {
                        if panic {
                            panic!("callback failure");
                        }
                        return SimpleScore::new(0).with_structural(-1);
                    }
                    SimpleScore::new(value)
                })
                .unwrap();
            let mut director =
                ScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                director.select_best(
                    [1, 2].map(|v| Move::Change {
                        variable: 0,
                        value: Some(v),
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
                    Err(ScoringError::StructuralContribution { constraint: 0 })
                ));
            }
            assert_eq!(director.assignment().values(), &[Some(0)]);
            assert_eq!(director.score(), SimpleScore::ZERO);
            director.assert_consistent().unwrap();
        }
    }
}
