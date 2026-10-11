//! Scoped constraints and transactional incremental score recomputation.
//!
//! A constraint receives only the values in its declared scope. A move rescans
//! affected constraints once each; score caches change only when it commits.
use crate::{AdditiveScore, AppliedMove, Assignment, FiniteProblem, ModelError, Move};
use std::{error::Error, fmt};

type Evaluator<'a, V, S> = dyn Fn(
        &[Option<&V>],
        &S,
        crate::ScoringArithmetic,
    ) -> Result<S, crate::scoring_arithmetic::WeightArithmeticError>
    + 'a;
struct Constraint<'a, V, S> {
    name: String,
    scope: Vec<usize>,
    weight: Option<S>,
    evaluate: Box<Evaluator<'a, V, S>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoringError {
    Calculator(crate::MatchRegistryError),
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
    InvalidWeight(crate::ConstraintWeightError),
    ScoreOverflow {
        constraint: Option<usize>,
    },
    DecimalWeightArithmetic {
        constraint: usize,
        error: crate::DecimalArithmeticError,
    },
    StructuralContribution {
        constraint: usize,
    },
    ScoreCorruption(Box<crate::ScoreCorruption>),
    CorruptedTotal,
}
impl fmt::Display for ScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(e) => e.fmt(f),
            Self::Calculator(e) => e.fmt(f),
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
            Self::ScoreOverflow { constraint } => {
                write!(f, "score overflow at constraint {constraint:?}")
            }
            Self::InvalidWeight(error) => error.fmt(f),
            Self::DecimalWeightArithmetic { constraint, error } => {
                write!(f, "constraint {constraint} decimal impact failed: {error}")
            }
            Self::DuplicateName(name) => write!(f, "duplicate constraint name: {name}"),
            Self::StructuralContribution { constraint } => write!(
                f,
                "constraint {constraint} returned a nonzero structural component"
            ),
            Self::ScoreCorruption(report) => report.fmt(f),
            Self::CorruptedTotal => {
                f.write_str("cached total differs from full score recomputation")
            }
        }
    }
}
impl Error for ScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ScoreCorruption(report) => Some(report.as_ref()),
            Self::Model(error) => Some(error),
            Self::Calculator(error) => Some(error),
            Self::InvalidWeight(error) => Some(error),
            Self::ScoreOverflow { .. } => Some(&crate::ScoreOverflow),
            Self::DecimalWeightArithmetic { error, .. } => Some(error),
            _ => None,
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
/// use checked built-in integer arithmetic by default; wrapping is configurable.
/// Scopes may repeat a variable; callback arguments preserve those repetitions.
/// An empty scope represents a constant contribution.
pub struct ConstraintNetwork<'a, V, S> {
    arithmetic: crate::ScoringArithmetic,
    constraints: Vec<Constraint<'a, V, S>>,
    dependents: Vec<Vec<usize>>,
}
impl<'a, V, S: AdditiveScore> ConstraintNetwork<'a, V, S> {
    pub fn new(variable_count: usize) -> Self {
        Self {
            arithmetic: crate::ScoringArithmetic::default(),
            constraints: Vec::new(),
            dependents: vec![Vec::new(); variable_count],
        }
    }
    pub fn variable_count(&self) -> usize {
        self.dependents.len()
    }
    /// Choose checked (default) or wrapping scoring before creating a director.
    pub fn with_arithmetic(mut self, arithmetic: crate::ScoringArithmetic) -> Self {
        self.arithmetic = arithmetic;
        self
    }
    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
    /// Number of callbacks used by full scoring (direct callbacks always count).
    pub fn active_constraint_count(&self) -> usize {
        self.constraints
            .iter()
            .filter(|c| c.weight.as_ref().is_none_or(|w| *w != S::ZERO))
            .count()
    }
    pub fn add_constraint(
        &mut self,
        name: impl Into<String>,
        scope: Vec<usize>,
        evaluate: impl Fn(&[Option<&V>]) -> S + 'a,
    ) -> Result<usize, ScoringError> {
        self.register(
            name.into(),
            scope,
            None,
            Box::new(move |facts, _, _| Ok(evaluate(facts))),
        )
    }
    /// Register an integer match factor with a configurable ordinary score weight.
    /// A zero weight skips the callback, including during initialization/diagnostics.
    pub fn add_weighted_constraint(
        &mut self,
        name: impl Into<String>,
        scope: Vec<usize>,
        weight: S,
        factor: impl Fn(&[Option<&V>]) -> i64 + 'a,
    ) -> Result<usize, ScoringError> {
        self.register(
            name.into(),
            scope,
            Some(weight),
            Box::new(move |facts, weight, arithmetic| Ok(arithmetic.scale(weight, factor(facts))?)),
        )
    }
    /// Register an exact decimal match factor. Arithmetic errors roll back moves.
    pub fn add_decimal_weighted_constraint(
        &mut self,
        name: impl Into<String>,
        scope: Vec<usize>,
        weight: S,
        factor: impl Fn(&[Option<&V>]) -> crate::Decimal + 'a,
    ) -> Result<usize, ScoringError>
    where
        S: crate::DecimalWeightedScore,
    {
        self.register(
            name.into(),
            scope,
            Some(weight),
            Box::new(move |facts, weight, _| Ok(weight.multiply_exact(&factor(facts))?)),
        )
    }
    /// Apply a validated batch atomically; omitted names retain their current weights.
    /// Directors borrow the network, so weights cannot change during a scoring session.
    /// Zero disables a weighted constraint; a later nonzero override reactivates it.
    ///
    /// ```
    /// use timetuck::*;
    /// let problem = FiniteProblem::new(vec![Variable::new(vec![2]).with_initial(2)])?;
    /// let mut network = ConstraintNetwork::new(1);
    /// network.add_weighted_constraint("cost", vec![0], SimpleScore::new(-3),
    ///     |values| values[0].copied().unwrap_or(0))?;
    /// let overrides = [("cost".to_owned(), SimpleScore::new(-5))].into_iter().collect();
    /// network.apply_weight_overrides(&overrides)?;
    /// let director = ScoreDirector::new(&problem, &network, problem.initial_assignment())?;
    /// assert_eq!(director.score(), SimpleScore::new(-10));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn apply_weight_overrides(
        &mut self,
        overrides: &crate::ConstraintWeightOverrides<S>,
    ) -> Result<(), crate::ConstraintWeightError> {
        let updates = crate::scoped_weights::resolve_overrides(
            self.constraints
                .iter()
                .map(|c| (c.name.as_str(), c.weight.as_ref())),
            overrides,
        )?;
        for (index, weight) in updates {
            self.constraints[index].weight = Some(weight);
        }
        Ok(())
    }
    /// Configured weighted constraints, in registration order (including zero).
    pub fn constraint_weights(&self) -> impl Iterator<Item = (&str, &S)> {
        self.constraints
            .iter()
            .filter_map(|c| c.weight.as_ref().map(|w| (c.name.as_str(), w)))
    }
    fn register(
        &mut self,
        name: String,
        scope: Vec<usize>,
        weight: Option<S>,
        evaluate: Box<Evaluator<'a, V, S>>,
    ) -> Result<usize, ScoringError> {
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
        if weight.as_ref().is_some_and(|w| w.structural() != 0) {
            return Err(ScoringError::InvalidWeight(
                crate::ConstraintWeightError::StructuralWeight(name),
            ));
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
            weight,
            evaluate,
        });
        Ok(index)
    }
    fn contribution(&self, index: usize, assignment: &Assignment<V>) -> Result<S, ScoringError> {
        let constraint = &self.constraints[index];
        if constraint.weight.as_ref().is_some_and(|w| *w == S::ZERO) {
            return Ok(S::ZERO);
        }
        let arguments: Vec<_> = constraint
            .scope
            .iter()
            .map(|&i| assignment.values()[i].as_ref())
            .collect();
        let score = (constraint.evaluate)(
            &arguments,
            constraint.weight.as_ref().unwrap_or(&S::ZERO),
            self.arithmetic,
        )
        .map_err(|error| match error {
            crate::scoring_arithmetic::WeightArithmeticError::Decimal(error) => {
                ScoringError::DecimalWeightArithmetic {
                    constraint: index,
                    error,
                }
            }
            crate::scoring_arithmetic::WeightArithmeticError::Overflow => {
                ScoringError::ScoreOverflow {
                    constraint: Some(index),
                }
            }
        })?;
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
        let contributions = (0..self.constraint_count())
            .map(|i| self.contribution(i, assignment))
            .collect::<Result<Vec<_>, _>>()?;
        let total = self
            .arithmetic
            .sum(&contributions)
            .map_err(|_| ScoringError::ScoreOverflow { constraint: None })?;
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

// A sparse trial changes completeness only at the first journal entry for each
// required variable. Later entries describe intermediate states, not the base.
// Dense journals are cheaper to handle with the independent full scan; callers
// supply the size of their whole basic/list world when choosing that fallback.
pub(crate) fn basic_structural_delta<V: Eq>(
    problem: &FiniteProblem<V>,
    assignment: &Assignment<V>,
    prior: &[crate::PriorValue<V>],
    world_size: usize,
) -> Option<i64> {
    if world_size <= 32 || (prior.len() > 8 && prior.len() > world_size / 64) {
        return None;
    }
    let contribution = |write: &crate::PriorValue<V>| {
        if problem.variables()[write.variable].allows_unassigned() {
            0
        } else {
            i64::from(write.value.is_none())
                - i64::from(assignment.values()[write.variable].is_none())
        }
    };
    let delta = if prior.len() <= 8 {
        prior
            .iter()
            .enumerate()
            .filter(|(i, write)| !prior[..*i].iter().any(|old| old.variable == write.variable))
            .map(|(_, write)| contribution(write))
            .sum()
    } else {
        let mut seen = std::collections::HashSet::with_capacity(prior.len());
        prior
            .iter()
            .filter(|write| seen.insert(write.variable))
            .map(contribution)
            .sum()
    };
    Some(delta)
}

/// Owns a working assignment and cached per-constraint contributions.
/// It borrows the model/network, preventing their mutation during the session.
pub struct ScoreDirector<'p, 'c, V, S> {
    problem: &'p FiniteProblem<V>,
    network: &'p ConstraintNetwork<'c, V, S>,
    assignment: Assignment<V>,
    contributions: Vec<S>,
    score: S,
    retained_changes: Option<Box<std::cell::RefCell<crate::retained_scoped_phase::ScopedChanges>>>,
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
        let sum = network
            .arithmetic
            .sum(&contributions)
            .map_err(|_| ScoringError::ScoreOverflow { constraint: None })?;
        let score = with_structure(sum, problem, &assignment);
        Ok(Self {
            problem,
            network,
            assignment,
            contributions,
            score,
            retained_changes: None,
        })
    }
    /// Internal transitions between one traversal's fixed value representatives.
    /// The first exact candidate must refresh all scopes: external Eq-equal values
    /// can have different scoring payloads. Not an arbitrary replacement API.
    pub(crate) fn replace_retained_assignment(
        &mut self,
        next: Assignment<V>,
        refresh_all: bool,
    ) -> Result<u128, ScoringError> {
        self.replace_retained_assignment_tracked(next, refresh_all, None)
    }
    pub(crate) fn replace_retained_assignment_tracked(
        &mut self,
        next: Assignment<V>,
        refresh_all: bool,
        changes: Option<&mut crate::retained_scoped_phase::ScopedChanges>,
    ) -> Result<u128, ScoringError> {
        self.network.validate(self.problem, &next)?;
        let mut affected: Vec<usize> = if refresh_all {
            (0..self.network.constraint_count()).collect()
        } else {
            self.assignment
                .values()
                .iter()
                .zip(next.values())
                .enumerate()
                .filter(|(_, (before, after))| before != after)
                .flat_map(|(i, _)| self.network.dependents[i].iter().copied())
                .collect()
        };
        affected.sort_unstable();
        affected.dedup();
        affected.retain(|&i| {
            self.network.constraints[i]
                .weight
                .as_ref()
                .is_none_or(|w| *w != S::ZERO)
        });
        let mut updates = Vec::with_capacity(affected.len());
        for i in affected {
            updates.push((i, self.network.contribution(i, &next)?));
        }
        let score = if refresh_all {
            self.network
                .arithmetic
                .sum(&updates.iter().map(|(_, s)| s.clone()).collect::<Vec<_>>())
        } else {
            self.network.arithmetic.adjust(
                &self.score,
                updates
                    .iter()
                    .map(|(i, new)| (&self.contributions[*i], new)),
            )
        }
        .map_err(|_| ScoringError::ScoreOverflow { constraint: None })?;
        let score = with_structure(score, self.problem, &next);
        let work = updates.len() as u128;
        if let Some(changes) = changes {
            changes.record_batch(updates.iter().map(|(index, _)| *index));
        }
        if let Some(changes) = self.retained_changes.as_deref_mut() {
            changes
                .get_mut()
                .record_batch(updates.iter().map(|(index, _)| *index));
        }
        // Validation, user callbacks and checked arithmetic finish before installation.
        self.assignment = next;
        for (i, value) in updates {
            self.contributions[i] = value;
        }
        self.score = score;
        Ok(work)
    }
    /// Begin a live multi-move scope. Drop restores assignment and cached scores;
    /// commit retains edits. No scoring callbacks run to create or restore it.
    pub fn edit_session(&mut self) -> Result<crate::MoveSession<'_, Self>, ScoringError> {
        let assignment = self.assignment.clone();
        let contributions = self.contributions.clone();
        let score = self.score.clone();
        Ok(crate::MoveSession::new(self, move |director| {
            director.assignment = assignment;
            director.contributions = contributions;
            director.score = score;
        }))
    }
    pub fn assignment(&self) -> &Assignment<V> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score.clone()
    }
    /// Explain each retained scoped contribution without rescoring. The callback
    /// receives name, ordered scope, current facts and the cached weighted score.
    /// Each active constraint has one aggregate match (ID 0); disabled weighted
    /// constraints have no matches and do not invoke the callback. This does not
    /// invent individual matches inside a caller's aggregate scoring function.
    /// The returned full score includes structural incompleteness separately from
    /// ordinary contributions. Snapshots are owned and survive commits/rollback.
    pub fn explain<J, F: Ord + Clone>(
        &self,
        mut explain: impl FnMut(&str, &[usize], &[Option<&V>], &S) -> (J, Vec<F>),
    ) -> crate::ScopedScoreExplanation<S, J, F> {
        let mut matches = crate::ScoreExplanation::default();
        for (constraint, score) in self.network.constraints.iter().zip(&self.contributions) {
            let explanation = if constraint.weight.as_ref().is_some_and(|w| *w == S::ZERO) {
                None
            } else {
                let facts: Vec<_> = constraint
                    .scope
                    .iter()
                    .map(|&i| self.assignment.values()[i].as_ref())
                    .collect();
                Some(explain(&constraint.name, &constraint.scope, &facts, score))
            };
            matches
                .add_constraint(crate::score_explanation::scoped_contribution(
                    &constraint.name,
                    score.clone(),
                    explanation,
                ))
                .expect("network constraint names are unique");
        }
        crate::ScopedScoreExplanation {
            score: self.score.clone(),
            matches,
        }
    }
    /// Keep the ordered fact identities as the default justification while
    /// deduplicating each match's blamed facts. No scoring callbacks are invoked.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)])?;
    /// let mut n = ConstraintNetwork::new(1);
    /// n.add_weighted_constraint("cost", vec![0], SimpleScore::new(-3),
    ///     |values| values[0].copied().unwrap_or(0))?;
    /// let mut director = ScoreDirector::new(&p, &n, p.initial_assignment())?;
    /// let before = director.explain_default(|_, scope, _| scope.to_vec());
    /// director.apply(&Move::Change { variable: 0, value: Some(2) })?.commit();
    /// let after = director.explain_default(|_, scope, _| scope.to_vec());
    /// let diff = after.matches.diff(&before.matches);
    /// assert_eq!(diff.score_delta, SimpleScore::new(-3));
    /// assert_eq!(diff.indictments[&0].score_delta, SimpleScore::new(-3));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn explain_default<F: Ord + Clone>(
        &self,
        mut facts: impl FnMut(&str, &[usize], &[Option<&V>]) -> Vec<F>,
    ) -> crate::ScopedScoreExplanation<S, crate::DefaultJustification<S, F>, F> {
        self.explain(|name, scope, values, score| {
            let facts = facts(name, scope, values);
            (
                crate::DefaultJustification {
                    score: score.clone(),
                    facts: facts.clone(),
                },
                facts,
            )
        })
    }
    /// Evaluate against one unchanged base, retaining the configured best
    /// accepted candidates. The saved assignment and sparse cache updates stay inside this
    /// exclusive director borrow. Installing them does not invoke callbacks again.
    /// Errors/unwinding before installation leave the base and caches unchanged.
    pub(crate) fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        mut accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<Selection<V>, ScoringError> {
        let mut result = Selection {
            selected_accepted: false,
            selected: None,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut forager = crate::MoveForager::new(forager.into());
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
            if assert_scores {
                let mut cached = candidate.contributions.to_vec();
                for (index, score) in &candidate.updates {
                    cached[*index] = score.clone();
                }
                let report = consistency_report(
                    problem,
                    network,
                    candidate.assignment(),
                    &candidate.score(),
                    &cached,
                )?;
                if !report.is_consistent() {
                    return Err(ScoringError::ScoreCorruption(Box::new(report.diagnostic())));
                }
            }
            let accepted = accepts(&movement, candidate.score());
            forager.try_add_with(
                candidate.score(),
                accepted,
                || -> Result<_, ScoringError> {
                    Ok(SelectedCandidate {
                        accepted,
                        movement: movement.clone(),
                        assignment: candidate.assignment().clone(),
                        score: candidate.score(),
                        updates: candidate.updates.clone(),
                    })
                },
            )?;
            let stop =
                forager.is_quit_early() || (accepted && quit_early.should_pick(candidate.score()));
            drop(candidate);
            if assert_scores {
                self.assert_consistent()?;
            }
            if stop {
                break;
            }
        }
        // Every trial has already rolled back. Discarding the saved winner
        // leaves assignment, contribution caches and graph state at the last step.
        if !quit_early.can_commit(result.evaluation_limit) {
            return Ok(result);
        }
        if let Some(selected) = forager.pick() {
            if let Some(changes) = self.retained_changes.as_deref_mut() {
                changes
                    .get_mut()
                    .record_batch(selected.updates.iter().map(|(index, _)| *index));
            }
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
        let report = self.consistency_report()?;
        if report.is_consistent() {
            Ok(())
        } else {
            Err(ScoringError::ScoreCorruption(Box::new(report.diagnostic())))
        }
    }
    /// Recompute every constraint exactly once and retain all changed contributions
    /// plus cached and fresh totals, including structural state. Does not mutate
    /// assignment/caches or search counters. Callback side effects cannot be undone.
    /// Arithmetic/model errors abort reporting and retain their ordinary error type.
    pub fn consistency_report(&self) -> Result<crate::ScoreConsistencyReport<S>, ScoringError> {
        consistency_report(
            self.problem,
            self.network,
            &self.assignment,
            &self.score,
            &self.contributions,
        )
    }

    /// Compute a candidate transaction. Errors and ordinary panic unwinding
    /// restore values; caches remain unchanged until `ScoredMove::commit`.
    pub fn apply(&mut self, movement: &Move<V>) -> Result<ScoredMove<'_, V, S>, ScoringError> {
        let transaction = self
            .problem
            .apply_validated(&mut self.assignment, movement)?;
        let mut affected: Vec<_> = transaction
            .prior_values()
            .iter()
            .flat_map(|write| self.network.dependents[write.variable].iter().copied())
            .collect();
        affected.sort_unstable();
        affected.dedup();
        affected.retain(|&i| {
            self.network.constraints[i]
                .weight
                .as_ref()
                .is_none_or(|w| *w != S::ZERO)
        });
        let mut updates = Vec::with_capacity(affected.len());
        for i in affected {
            let new_score = self.network.contribution(i, &transaction)?;
            updates.push((i, new_score));
        }
        let score = self
            .network
            .arithmetic
            .adjust(
                &self.score,
                updates
                    .iter()
                    .map(|(i, new)| (&self.contributions[*i], new)),
            )
            .map_err(|_| ScoringError::ScoreOverflow { constraint: None })?;
        let candidate_score = match basic_structural_delta(
            self.problem,
            &transaction,
            transaction.prior_values(),
            self.problem.variables().len(),
        ) {
            Some(delta) => score.with_structural(self.score.structural() + delta),
            None => with_structure(score, self.problem, &transaction),
        };
        Ok(ScoredMove {
            transaction,
            updates,
            candidate_score,
            cached_score: &mut self.score,
            contributions: &mut self.contributions,
            retained_changes: self
                .retained_changes
                .as_deref_mut()
                .map(std::cell::RefCell::get_mut),
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
    retained_changes: Option<&'a mut crate::retained_scoped_phase::ScopedChanges>,
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
        if let Some(changes) = self.retained_changes {
            changes.record_batch(self.updates.iter().map(|(index, _)| *index));
        }
        for (i, score) in self.updates {
            self.contributions[i] = score;
        }
        *self.cached_score = self.candidate_score;
        self.transaction.commit();
    }
}

fn consistency_report<V: Eq, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    assignment: &Assignment<V>,
    working: &S,
    cached: &[S],
) -> Result<crate::ScoreConsistencyReport<S>, ScoringError> {
    crate::score_consistency::compare(
        working,
        network
            .constraints
            .iter()
            .zip(cached)
            .map(|(c, score)| (c.name.as_str(), score)),
        |i| network.contribution(i, assignment),
        |fresh| {
            let sum = network
                .arithmetic
                .sum(fresh)
                .map_err(|_| ScoringError::ScoreOverflow { constraint: None })?;
            Ok(with_structure(sum, problem, assignment))
        },
    )
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
                .add_constraint("failure after tied candidates", vec![0], |v| {
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
                    [1, 1, 2].map(|v| Move::Change {
                        variable: 0,
                        value: Some(v),
                    }),
                    3,
                    crate::ForagerConfig {
                        tie_breaking: crate::TieBreaking::Random { seed: 7 },
                        ..Default::default()
                    },
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

crate::retained_scoped_phase::retained_scoped!(ScoreDirector, [V], crate::Assignment<V>, crate::FiniteProblem<V>, crate::Move<V>, crate::ScoringError, crate::LocalSearchError, SolverSequence, basic, construction, crate::ConstructionConfig<'_>, crate::ConstructionResult<V,S>, local_search, crate::LocalSearchConfig<'_,S>, crate::LocalSearchResult<V,S>, basic_director, Some, []);

#[cfg(test)]
mod retained_commit_tests {
    use super::*;
    use crate::{FiniteProblem, ForagerConfig, SimpleScore, Variable};
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    fn change(variable: usize, value: i64) -> Move<i64> {
        Move::Change {
            variable,
            value: Some(value),
        }
    }
    fn pending(d: &ScoreDirector<'_, '_, i64, SimpleScore>) -> usize {
        d.retained_changes
            .as_ref()
            .unwrap()
            .borrow()
            .pending_count()
    }
    #[test]
    fn journal_marks_only_committed_winner_and_rebases_after_best_restore() {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap();
        let mut n = ConstraintNetwork::new(2);
        for i in 0..2 {
            n.add_constraint(i.to_string(), vec![i], |v| {
                SimpleScore::new(v[0].copied().unwrap_or(0))
            })
            .unwrap();
        }
        for i in 0..8 {
            n.add_constraint(format!("constant{i}"), vec![], |_| SimpleScore::ZERO)
                .unwrap();
        }
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        let mut tracked = d.start_retained_tracking();
        let mut best = Some(tracked.capture_retained());
        fn require_send<T: Send>(_: T) {}
        require_send(tracked.apply(&change(0, 1)).unwrap()); // Dropped trial stays Send and untracked.
        assert_eq!(pending(&tracked), 0);
        tracked.apply(&change(0, 1)).unwrap().commit();
        tracked.apply(&change(1, 1)).unwrap().commit();
        assert_eq!(pending(&tracked), 2);
        tracked.save_retained_best(&mut best);
        assert_eq!(pending(&tracked), 0);
        let chosen = tracked
            .select_best(
                [change(1, 2), change(0, 0)],
                10,
                ForagerConfig::default(),
                |_, _| true,
                |_| false,
                false,
            )
            .unwrap();
        assert_eq!(chosen.selected, Some(change(1, 2)));
        assert_eq!(pending(&tracked), 1); // Losing candidate touched the other variable.
        tracked.install_retained_state(best.unwrap());
        assert_eq!(pending(&tracked), 0);
        assert_eq!(tracked.assignment().values(), &[Some(1), Some(1)]);
        tracked.assert_consistent().unwrap();
        drop(tracked);
        assert!(d.retained_changes.is_none());
    }
    #[test]
    fn tracking_guard_cleans_up_after_errors_callback_panics_and_nested_attempts() {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
        let panic = Cell::new(false);
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| {
            assert!(!panic.get(), "injected callback panic");
            SimpleScore::new(v[0].copied().unwrap_or(0))
        })
        .unwrap();
        let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
        {
            let mut tracked = d.start_retained_tracking();
            assert!(tracked.apply(&change(9, 1)).is_err());
        }
        assert!(d.retained_changes.is_none());
        panic.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tracked = d.start_retained_tracking();
                let _ = tracked.apply(&change(0, 1));
            }))
            .is_err()
        );
        assert!(d.retained_changes.is_none());
        panic.set(false);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tracked = d.start_retained_tracking();
                let _nested = tracked.start_retained_tracking();
            }))
            .is_err()
        );
        assert!(d.retained_changes.is_none());
        d.assert_consistent().unwrap();
        let mut tracked = d.start_retained_tracking();
        tracked.apply(&change(0, 1)).unwrap().commit();
        drop(tracked);
        assert_eq!(d.score(), SimpleScore::new(1));
    }
}
