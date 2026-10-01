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
    fn validate(
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
fn with_structure<V: Eq, S: AdditiveScore>(
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
            .fold(S::ZERO, |sum, &score| sum + score);
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
        self.score
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.network
            .constraints
            .iter()
            .zip(&self.contributions)
            .map(|(c, &s)| (c.name.as_str(), s))
    }
    /// Compare every cached contribution and the total against fresh calculation.
    /// This is diagnostic work and is not included in search evaluation budgets.
    pub fn assert_consistent(&self) -> Result<(), ScoringError> {
        let mut sum = S::ZERO;
        for (i, &cached) in self.contributions.iter().enumerate() {
            let actual = self.network.contribution(i, &self.assignment)?;
            if cached != actual {
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
        let mut score = self.score;
        let mut updates = Vec::with_capacity(affected.len());
        for i in affected {
            let new_score = self.network.contribution(i, &transaction)?;
            score = score - self.contributions[i] + new_score;
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
impl<V, S: Copy> ScoredMove<'_, V, S> {
    pub fn assignment(&self) -> &Assignment<V> {
        &self.transaction
    }
    pub fn score(&self) -> S {
        self.candidate_score
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
