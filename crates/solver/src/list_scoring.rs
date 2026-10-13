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
use crate::list::ValueLocations;
use crate::{
    AdditiveScore, AppliedListMove, ListAssignment, ListElement, ListError, ListMove, ListPosition,
    ListProblem,
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

type Evaluator<'a, V, S> = dyn Fn(
        &[ListFact<'_, V>],
        &S,
        crate::ScoringArithmetic,
    ) -> Result<S, crate::scoring_arithmetic::WeightArithmeticError>
    + 'a;
struct Constraint<'a, V, S> {
    name: String,
    scope: Vec<ListScope>,
    weight: Option<S>,
    evaluate: Box<Evaluator<'a, V, S>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListScoringError {
    Provider(crate::MoveProviderError),
    Calculator(crate::MatchRegistryError),
    Model(ListError),
    ModelShape {
        expected_entities: usize,
        actual_entities: usize,
        expected_values: usize,
        actual_values: usize,
    },
    InvalidScope(ListScope),
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
    UndoCorruption(Box<crate::CalculatorUndoCorruption>),
    CorruptedTotal,
}
impl fmt::Display for ListScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(e) => e.fmt(f),
            Self::Model(error) if error.range_report().is_some() => error.fmt(f),
            Self::ScoreCorruption(report) => report.fmt(f),
            Self::UndoCorruption(report) => report.fmt(f),
            Self::Calculator(error) => error.fmt(f),
            _ => write!(f, "list scoring error: {self:?}"),
        }
    }
}
impl Error for ListScoringError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Provider(e) => Some(e),
            Self::ScoreCorruption(report) => Some(report.as_ref()),
            Self::UndoCorruption(report) => Some(report.as_ref()),
            Self::Model(error) => Some(error),
            Self::Calculator(error) => Some(error),
            Self::InvalidWeight(error) => Some(error),
            Self::ScoreOverflow { .. } => Some(&crate::ScoreOverflow),
            Self::DecimalWeightArithmetic { error, .. } => Some(error),
            _ => None,
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
    arithmetic: crate::ScoringArithmetic,
    constraints: Vec<Constraint<'a, V, S>>,
    entity_dependents: Vec<Vec<usize>>,
    value_dependents: Vec<Vec<usize>>,
    scoped_values: Vec<usize>,
}
impl<'a, V: Eq + Clone, S: AdditiveScore> ListConstraintNetwork<'a, V, S> {
    pub fn new(entity_count: usize, value_count: usize) -> Self {
        Self {
            arithmetic: crate::ScoringArithmetic::default(),
            constraints: vec![],
            entity_dependents: vec![vec![]; entity_count],
            value_dependents: vec![vec![]; value_count],
            scoped_values: vec![],
        }
    }
    pub fn entity_count(&self) -> usize {
        self.entity_dependents.len()
    }
    pub fn value_count(&self) -> usize {
        self.value_dependents.len()
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
        scope: Vec<ListScope>,
        evaluate: impl Fn(&[ListFact<'_, V>]) -> S + 'a,
    ) -> Result<usize, ListScoringError> {
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
        scope: Vec<ListScope>,
        weight: S,
        factor: impl Fn(&[ListFact<'_, V>]) -> i64 + 'a,
    ) -> Result<usize, ListScoringError> {
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
        scope: Vec<ListScope>,
        weight: S,
        factor: impl Fn(&[ListFact<'_, V>]) -> crate::Decimal + 'a,
    ) -> Result<usize, ListScoringError>
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
    /// Existing directors borrow this configuration; their local overrides are independent.
    /// Zero disables a weighted constraint; a later nonzero override reactivates it.
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
        scope: Vec<ListScope>,
        weight: Option<S>,
        evaluate: Box<Evaluator<'a, V, S>>,
    ) -> Result<usize, ListScoringError> {
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
        if weight.as_ref().is_some_and(|w| w.structural() != 0) {
            return Err(ListScoringError::InvalidWeight(
                crate::ConstraintWeightError::StructuralWeight(name),
            ));
        }
        let index = self.constraints.len();
        for &item in &scope {
            let dependents = match item {
                ListScope::Entity(i) => &mut self.entity_dependents[i],
                ListScope::Value(i) => {
                    if self.value_dependents[i].is_empty() {
                        self.scoped_values.push(i);
                    }
                    &mut self.value_dependents[i]
                }
            };
            if dependents.last() != Some(&index) {
                dependents.push(index);
            }
        }
        self.constraints.push(Constraint {
            name,
            scope,
            weight,
            evaluate,
        });
        Ok(index)
    }
    pub(crate) fn validate(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<(), ListScoringError> {
        self.validate_shape(problem)?;
        problem.validate(assignment)?;
        Ok(())
    }
    fn validate_shape(&self, problem: &ListProblem<V>) -> Result<(), ListScoringError> {
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
        Ok(())
    }
    fn validate_for_scoring(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Result<Option<ValueLocations>, ListScoringError> {
        self.validate_shape(problem)?;
        // Only static scope shape is inspected before model validation. Weight
        // comparisons and callbacks retain their position after model errors.
        // Capture conservatively without traversing scopes twice. Dense position
        // slots also perform duplicate tracking; sparse registries stay sparse.
        let potential = self.scoped_values.len() >= 64 && problem.has_value_index();
        let positions = problem.validate_using(assignment, potential)?;
        if self.needs_value_locations(
            problem,
            0..self.constraint_count(),
            None,
            &Default::default(),
        ) {
            Ok(positions
                .or_else(|| problem.indexed_value_locations(assignment))
                .map(ValueLocations::Full))
        } else {
            Ok(None)
        }
    }
    // Snapshot only dense queries. Sparse scopes retain their allocation-free
    // direct lookup, and Eq-only models need no new trait bounds. The snapshot
    // belongs to this immutable scoring pass, never to a retained director cache.
    fn value_locations(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        constraints: impl Iterator<Item = usize>,
        changed: Option<Vec<(usize, Option<ListPosition>)>>,
        weights: &crate::scoped_weights::LocalWeights<S>,
    ) -> Option<ValueLocations> {
        // Complete changed evidence already covers the registry, including known
        // unassignment. Turn it into direct slots without rescanning the world.
        if changed
            .as_ref()
            .is_some_and(|p| p.len() == problem.values().len())
        {
            return changed.map(|p| ValueLocations::Full(p.into_iter().map(|(_, p)| p).collect()));
        }
        let changed = changed.map(ValueLocations::Changed);
        if self.needs_value_locations(problem, constraints, changed.as_ref(), weights) {
            problem
                .indexed_value_locations(assignment)
                .map(ValueLocations::Full)
        } else {
            changed
        }
    }
    fn needs_value_locations(
        &self,
        problem: &ListProblem<V>,
        constraints: impl Iterator<Item = usize>,
        changed: Option<&ValueLocations>,
        weights: &crate::scoped_weights::LocalWeights<S>,
    ) -> bool {
        if self.scoped_values.len() < 64 || !problem.has_value_index() {
            return false;
        }
        let threshold = 64.max(problem.values().len() / 8);
        let queries = constraints
            .filter(|&i| {
                weights
                    .get(i, self.constraints[i].weight.as_ref())
                    .is_none_or(|w| *w != S::ZERO)
            })
            .flat_map(|i| &self.constraints[i].scope)
            .filter(|scope| match scope {
                ListScope::Value(id) => changed.is_none_or(|p| p.get(*id).is_none()),
                _ => false,
            })
            .take(threshold)
            .count();
        queries == threshold
    }

    fn facts<'f>(
        constraint: &Constraint<'_, V, S>,
        problem: &'f ListProblem<V>,
        assignment: &'f ListAssignment<V>,
        locations: Option<&ValueLocations>,
    ) -> Vec<ListFact<'f, V>> {
        let from_position = |position: ListPosition| {
            let row = &assignment.lists()[position.entity];
            ListElement {
                position,
                previous: position.index.checked_sub(1).and_then(|i| row.get(i)),
                next: row.get(position.index + 1),
            }
        };
        // Dispatch once per constraint. Full snapshots retain direct indexing
        // inside the fact loop; sparse evidence alone needs binary lookup.
        match locations {
            Some(ValueLocations::Full(positions)) => {
                Self::facts_using(constraint, problem, assignment, |i| {
                    positions[i].map(from_position)
                })
            }
            Some(changed @ ValueLocations::Changed(_)) => {
                Self::facts_using(constraint, problem, assignment, |i| match changed.get(i) {
                    Some(position) => position.map(from_position),
                    None => assignment.element(&problem.values()[i]),
                })
            }
            None => Self::facts_using(constraint, problem, assignment, |i| {
                assignment.element(&problem.values()[i])
            }),
        }
    }

    fn facts_using<'f>(
        constraint: &Constraint<'_, V, S>,
        problem: &'f ListProblem<V>,
        assignment: &'f ListAssignment<V>,
        mut locate: impl FnMut(usize) -> Option<ListElement<'f, V>>,
    ) -> Vec<ListFact<'f, V>> {
        constraint
            .scope
            .iter()
            .map(|scope| match *scope {
                ListScope::Entity(i) => ListFact::Entity(&assignment.lists()[i]),
                ListScope::Value(i) => ListFact::Value {
                    value: &problem.values()[i],
                    element: locate(i),
                },
            })
            .collect()
    }

    fn contribution(
        &self,
        index: usize,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        locations: Option<&ValueLocations>,
    ) -> Result<S, ListScoringError> {
        self.contribution_weighted(
            index,
            problem,
            assignment,
            locations,
            self.constraints[index].weight.as_ref(),
        )
    }
    fn contribution_weighted(
        &self,
        index: usize,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        locations: Option<&ValueLocations>,
        weight: Option<&S>,
    ) -> Result<S, ListScoringError> {
        let constraint = &self.constraints[index];
        if weight.is_some_and(|w| *w == S::ZERO) {
            return Ok(S::ZERO);
        }
        let facts = Self::facts(constraint, problem, assignment, locations);
        let score = (constraint.evaluate)(&facts, weight.unwrap_or(&S::ZERO), self.arithmetic)
            .map_err(|error| match error {
                crate::scoring_arithmetic::WeightArithmeticError::Decimal(error) => {
                    ListScoringError::DecimalWeightArithmetic {
                        constraint: index,
                        error,
                    }
                }
                crate::scoring_arithmetic::WeightArithmeticError::Overflow => {
                    ListScoringError::ScoreOverflow {
                        constraint: Some(index),
                    }
                }
            })?;
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
        self.calculate_with_metrics(problem, assignment, None)
    }
    pub(crate) fn calculate_with_metrics(
        &self,
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
        capture: Option<&crate::solver_metrics::ConstraintMetricCapture<S>>,
    ) -> Result<S, ListScoringError> {
        let locations = self.validate_for_scoring(problem, assignment)?;
        let contributions = (0..self.constraint_count())
            .map(|i| self.contribution(i, problem, assignment, locations.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let total = self
            .arithmetic
            .sum(&contributions)
            .map_err(|_| ListScoringError::ScoreOverflow { constraint: None })?;
        let score = with_structure(total, problem, assignment);
        if let Some(capture) = capture {
            capture.capture_totals(
                self.constraints
                    .iter()
                    .zip(contributions)
                    .map(|(c, score)| {
                        (
                            c.name.as_str(),
                            score,
                            usize::from(c.weight.as_ref().is_none_or(|w| *w != S::ZERO)),
                        )
                    }),
            );
        }
        Ok(score)
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
/// snapshot the touched lists. Entry validation is complete; subsequent edits
/// preserve global membership/uniqueness and check touched pins/ranges plus global
/// dependencies. Value dependency discovery inspects the touched lists before
/// and after the edit. Callback evaluation is incremental.
pub struct ListScoreDirector<'p, 'c, V, S> {
    problem: &'p ListProblem<V>,
    network: &'p ListConstraintNetwork<'c, V, S>,
    assignment: ListAssignment<V>,
    retained_index: crate::RetainedListIndex,
    weights: crate::scoped_weights::LocalWeights<S>,
    contributions: Vec<S>,
    score: S,
    retained_changes: Option<Box<std::cell::RefCell<crate::retained_scoped_phase::ScopedChanges>>>,
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
    assignment: crate::candidate_snapshot::ListSnapshot<V>,
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
        let locations = network.validate_for_scoring(problem, &assignment)?;
        let contributions = (0..network.constraint_count())
            .map(|i| network.contribution(i, problem, &assignment, locations.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        let total = network
            .arithmetic
            .sum(&contributions)
            .map_err(|_| ListScoringError::ScoreOverflow { constraint: None })?;
        let score = with_structure(total, problem, &assignment);
        let retained_index = crate::RetainedListIndex::new();
        assignment.index_owner.connect(&retained_index);
        Ok(Self {
            retained_index,
            problem,
            network,
            assignment,
            weights: crate::scoped_weights::LocalWeights::default(),
            contributions,
            score,
            retained_changes: None,
        })
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
        mut explain: impl FnMut(&str, &[ListScope], &[ListFact<'_, V>], &S) -> (J, Vec<F>),
    ) -> crate::ScopedScoreExplanation<S, J, F> {
        let locations = self.network.value_locations(
            self.problem,
            &self.assignment,
            0..self.network.constraint_count(),
            None,
            &self.weights,
        );
        let mut matches = crate::ScoreExplanation::default();
        for (i, (constraint, score)) in self
            .network
            .constraints
            .iter()
            .zip(&self.contributions)
            .enumerate()
        {
            let explanation = if self
                .weights
                .get(i, constraint.weight.as_ref())
                .is_some_and(|w| *w == S::ZERO)
            {
                None
            } else {
                let facts = ListConstraintNetwork::facts(
                    constraint,
                    self.problem,
                    &self.assignment,
                    locations.as_ref(),
                );
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
    pub fn explain_default<F: Ord + Clone>(
        &self,
        mut facts: impl FnMut(&str, &[ListScope], &[ListFact<'_, V>]) -> Vec<F>,
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
    /// Keep candidates and sparse updates within this exclusive session borrow.
    /// Any error or panic before installation leaves the base and cache intact.
    pub(crate) fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        mut accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, ListScoringError> {
        let mut result = ListSelection {
            selected: None,
            selected_accepted: false,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut forager = crate::MoveForager::new(forager.into());
        let network = self.network;
        let weights = self.weights.clone();
        let problem = self.problem;
        for movement in moves {
            if result.evaluations == max_evaluations {
                result.evaluation_limit = true;
                break;
            }
            let candidate = match self.apply(&movement) {
                Ok(candidate) => candidate,
                Err(ListScoringError::Model(crate::ListError::DependencyCycle { .. })) => continue,
                Err(error) => return Err(error),
            };
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
                    &weights,
                )?;
                if !report.is_consistent() {
                    return Err(ListScoringError::ScoreCorruption(Box::new(
                        report.diagnostic(),
                    )));
                }
            }
            let accepted = accepts(&movement, candidate.score());
            forager.try_add_with(
                candidate.score(),
                accepted,
                || -> Result<_, ListScoringError> {
                    Ok(Selected {
                        movement: movement.clone(),
                        assignment: candidate.transaction.snapshot(),
                        score: candidate.score(),
                        updates: candidate.updates.clone(),
                        accepted,
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
        if let Some(best) = forager.pick() {
            if let Some(changes) = self.retained_changes.as_deref_mut() {
                let changes = changes.get_mut();
                changes.record_batch(best.updates.iter().map(|(index, _)| *index));
                best.assignment.record_retained(&mut changes.assignment);
            }
            best.assignment.install(&mut self.assignment);
            for (index, score) in best.updates {
                self.contributions[index] = score;
            }
            self.score = best.score;
            result.selected = Some(best.movement);
            result.selected_accepted = best.accepted;
        }
        Ok(result)
    }
    /// Inspect a layout-valid candidate without changing this director or scoring it.
    pub fn inspect_move(
        &self,
        movement: &ListMove<V>,
    ) -> Result<crate::MoveInspection<ListAssignment<V>>, ListScoringError> {
        Ok(crate::inspect_list_move(
            self.problem,
            &self.assignment,
            movement,
        )?)
    }
    /// Internal transitions between one traversal's fixed value representatives.
    /// The first exact candidate must refresh all scopes: external Eq-equal values
    /// can have different scoring payloads. Not an arbitrary replacement API.
    pub(crate) fn replace_retained_assignment(
        &mut self,
        next: ListAssignment<V>,
        refresh_all: bool,
    ) -> Result<u128, ListScoringError> {
        self.replace_retained_assignment_tracked(next, refresh_all, None)
    }
    pub(crate) fn replace_retained_assignment_tracked(
        &mut self,
        next: ListAssignment<V>,
        refresh_all: bool,
        changes: Option<&mut crate::retained_scoped_phase::ScopedChanges>,
    ) -> Result<u128, ListScoringError> {
        self.network.validate(self.problem, &next)?;
        let mut affected: Vec<usize> = if refresh_all {
            (0..self.network.constraint_count()).collect()
        } else {
            let mut affected = Vec::new();
            let touched: Vec<usize> = self
                .assignment
                .lists()
                .iter()
                .zip(next.lists())
                .enumerate()
                .filter_map(|(i, (before, after))| (before != after).then_some(i))
                .collect();
            for &entity in &touched {
                affected.extend(&self.network.entity_dependents[entity]);
            }
            let changed = (self.network.scoped_values.len() >= 64)
                .then(|| {
                    self.problem
                        .indexed_changed_value_positions(&next, &touched, |entity| {
                            &self.assignment.lists()[entity]
                        })
                })
                .flatten();
            if let Some(changed) = changed {
                for (i, _) in changed {
                    affected.extend(&self.network.value_dependents[i]);
                }
            } else {
                // All values in changed lists may have changed owner/index/neighbors.
                // Conservative invalidation includes unchanged positions with new neighbors.
                for &i in &self.network.scoped_values {
                    let value = &self.problem.values()[i];
                    if touched.iter().any(|&e| {
                        self.assignment.lists()[e].contains(value)
                            || next.lists()[e].contains(value)
                    }) {
                        affected.extend(&self.network.value_dependents[i]);
                    }
                }
            }
            affected
        };
        affected.sort_unstable();
        affected.dedup();
        affected.retain(|&i| {
            self.weights
                .get(i, self.network.constraints[i].weight.as_ref())
                .is_none_or(|w| *w != S::ZERO)
        });
        let locations = self.network.value_locations(
            self.problem,
            &next,
            affected.iter().copied(),
            None,
            &self.weights,
        );
        let mut updates = Vec::with_capacity(affected.len());
        for i in affected {
            updates.push((
                i,
                self.network.contribution_weighted(
                    i,
                    self.problem,
                    &next,
                    locations.as_ref(),
                    self.weights
                        .get(i, self.network.constraints[i].weight.as_ref()),
                )?,
            ));
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
        .map_err(|_| ListScoringError::ScoreOverflow { constraint: None })?;
        let score = with_structure(score, self.problem, &next);
        let work = updates.len() as u128;
        if let Some(changes) = changes {
            changes.record_batch(updates.iter().map(|(index, _)| *index));
            changes.assignment.mark_all();
        }
        if let Some(changes) = self.retained_changes.as_deref_mut() {
            let changes = changes.get_mut();
            changes.record_batch(updates.iter().map(|(index, _)| *index));
            changes.assignment.mark_all();
        }
        // Validation, user callbacks and checked arithmetic finish before installation.
        self.assignment = next;
        for (i, value) in updates {
            self.contributions[i] = value;
        }
        self.score = score;
        Ok(work)
    }
    /// Effective configurable weights in registration order, including disabled ones.
    pub fn constraint_weights(&self) -> impl Iterator<Item = (&str, &S)> {
        self.network
            .constraints
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                self.weights
                    .get(i, c.weight.as_ref())
                    .map(|w| (c.name.as_str(), w))
            })
    }
    /// Atomically override weights on this director's current assignment.
    /// Omitted names retain their effective weights; the borrowed network and
    /// other directors are unchanged. Unchanged weights and zero disables run no
    /// scoring callbacks. Changed nonzero weights recalculate in registration order.
    /// Returns the number of scoring callbacks, separate from search work budgets.
    /// Validation, cloning, callbacks and checked arithmetic finish before publication;
    /// errors or ordinary panic unwinding preserve weights, contributions and score.
    /// Callback side effects and panicking destructors cannot be rolled back.
    /// Edit-session rollback restores weights together with assignment and score.
    pub fn apply_weight_overrides(
        &mut self,
        overrides: &crate::ConstraintWeightOverrides<S>,
    ) -> Result<u128, ListScoringError> {
        let mut changes = crate::scoped_weights::resolve_overrides(
            self.network
                .constraints
                .iter()
                .map(|c| (c.name.as_str(), c.weight.as_ref())),
            overrides,
        )
        .map_err(ListScoringError::InvalidWeight)?;
        changes.retain(|(i, weight)| {
            self.weights
                .get(*i, self.network.constraints[*i].weight.as_ref())
                != Some(weight)
        });
        if changes.is_empty() {
            return Ok(0);
        }
        changes.sort_unstable_by_key(|(i, _)| *i);
        let weights = self.weights.updated(
            self.network.constraints.iter().map(|c| c.weight.as_ref()),
            &changes,
        );
        let locations = self.network.value_locations(
            self.problem,
            &self.assignment,
            changes.iter().map(|(i, _)| *i),
            None,
            &weights,
        );
        let mut work = 0;
        let mut updates = Vec::with_capacity(changes.len());
        for (i, weight) in &changes {
            let i = *i;
            let score = if *weight == S::ZERO {
                S::ZERO
            } else {
                work += 1;
                self.network.contribution_weighted(
                    i,
                    self.problem,
                    &self.assignment,
                    locations.as_ref(),
                    Some(weight),
                )?
            };
            updates.push((i, score));
        }
        let score = self
            .network
            .arithmetic
            .adjust(
                &self.score,
                updates
                    .iter()
                    .map(|(i, score)| (&self.contributions[*i], score)),
            )
            .map_err(|_| ListScoringError::ScoreOverflow { constraint: None })?;
        let score = score.with_structural(self.score.structural());
        if let Some(changes) = self.retained_changes.as_deref_mut() {
            changes
                .get_mut()
                .record_batch(updates.iter().map(|(i, _)| *i));
        }
        self.weights = weights;
        for (i, score) in updates {
            self.contributions[i] = score;
        }
        self.score = score;
        Ok(work)
    }
    /// Begin a live multi-move scope. Drop restores assignment and cached scores;
    /// commit retains edits. No scoring callbacks run to create or restore it.
    pub fn edit_session(&mut self) -> Result<crate::MoveSession<'_, Self>, ListScoringError> {
        let assignment = self.assignment.clone();
        let weights = self.weights.clone();
        let contributions = self.contributions.clone();
        let score = self.score.clone();
        Ok(crate::MoveSession::new(self, move |director| {
            director.assignment = assignment;
            director.weights = weights;
            director.contributions = contributions;
            director.score = score;
        }))
    }
    pub fn assignment(&self) -> &ListAssignment<V> {
        self.assignment.index_owner.connect(&self.retained_index);
        &self.assignment
    }
    /// Synchronize and borrow this director's persistent committed-layout index.
    pub fn retained_list_index(&self) -> crate::RetainedListIndex {
        self.retained_index
            .synchronize(self.problem, &self.assignment);
        self.retained_index.clone()
    }
    pub fn score(&self) -> S {
        self.score.clone()
    }
    pub(crate) fn capture_constraint_metrics(
        &self,
        capture: &crate::solver_metrics::ConstraintMetricCapture<S>,
    ) {
        capture.capture_totals(
            self.network
                .constraints
                .iter()
                .zip(&self.contributions)
                .enumerate()
                .map(|(index, (c, score))| {
                    (
                        c.name.as_str(),
                        score.clone(),
                        usize::from(
                            self.weights
                                .get(index, c.weight.as_ref())
                                .is_none_or(|w| *w != S::ZERO),
                        ),
                    )
                }),
        );
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.network
            .constraints
            .iter()
            .zip(&self.contributions)
            .map(|(c, s)| (c.name.as_str(), s.clone()))
    }
    /// Check individual contributions, detecting cancelling errors as well as
    /// total corruption. Diagnostic callback evaluations are not move statistics.
    pub fn assert_consistent(&self) -> Result<(), ListScoringError> {
        let report = self.consistency_report()?;
        if report.is_consistent() {
            Ok(())
        } else {
            Err(ListScoringError::ScoreCorruption(Box::new(
                report.diagnostic(),
            )))
        }
    }
    /// Recompute every constraint exactly once and retain all changed contributions
    /// plus cached and fresh totals, including structural state. Does not mutate
    /// assignment/caches or search counters. Callback side effects cannot be undone.
    /// Arithmetic/model errors abort reporting and retain their ordinary error type.
    pub fn consistency_report(&self) -> Result<crate::ScoreConsistencyReport<S>, ListScoringError> {
        consistency_report(
            self.problem,
            self.network,
            &self.assignment,
            &self.score,
            &self.contributions,
            &self.weights,
        )
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
        let transaction = self
            .problem
            .apply_validated(&mut self.assignment, movement)?;
        let touched = movement.touched_entities();
        let mut affected =
            crate::affected_constraints::AffectedConstraints::new(self.network.constraints.len());
        for &entity in &touched {
            affected.extend(&self.network.entity_dependents[entity]);
        }
        // Entity-only constraints need no value identity lookup. Registration
        // retains each scoped index once, including currently disabled weights.
        let changed = (self.network.scoped_values.len() >= 64)
            .then(|| {
                self.problem
                    .indexed_changed_value_positions(&transaction, &touched, |entity| {
                        transaction.original_list(entity).unwrap()
                    })
            })
            .flatten();
        if let Some(changed) = &changed {
            for &(i, _) in changed {
                affected.extend(&self.network.value_dependents[i]);
            }
        } else {
            for &i in &self.network.scoped_values {
                let value = &self.problem.values()[i];
                if touched.iter().any(|&entity| {
                    transaction.original_list(entity).unwrap().contains(value)
                        || transaction.lists()[entity].contains(value)
                }) {
                    affected.extend(&self.network.value_dependents[i]);
                }
            }
        }
        let mut affected = affected.into_sorted();
        affected.retain(|&i| {
            self.weights
                .get(i, self.network.constraints[i].weight.as_ref())
                .is_none_or(|w| *w != S::ZERO)
        });
        let locations = self.network.value_locations(
            self.problem,
            &transaction,
            affected.iter().copied(),
            changed,
            &self.weights,
        );
        let mut updates = Vec::with_capacity(affected.len());
        for i in affected {
            let contribution = self.network.contribution_weighted(
                i,
                self.problem,
                &transaction,
                locations.as_ref(),
                self.weights
                    .get(i, self.network.constraints[i].weight.as_ref()),
            )?;
            updates.push((i, contribution));
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
            .map_err(|_| ListScoringError::ScoreOverflow { constraint: None })?;
        let candidate_score = if self.problem.allows_unassigned() {
            score.with_structural(0)
        } else if let Some(delta) = transaction.assigned_count_delta() {
            score.with_structural(self.score.structural() + delta)
        } else {
            with_structure(score, self.problem, &transaction)
        };
        Ok(ScoredListMove {
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

/// Read-only scored candidate. Dropping rolls back all lists; commit installs
/// both the assignment and its cache updates without invoking callbacks again.
#[must_use = "dropping a scored list move rejects it; call commit to retain it"]
pub struct ScoredListMove<'a, V, S> {
    transaction: AppliedListMove<'a, V>,
    updates: Vec<(usize, S)>,
    candidate_score: S,
    cached_score: &'a mut S,
    contributions: &'a mut [S],
    retained_changes: Option<&'a mut crate::retained_scoped_phase::ScopedChanges>,
}
impl<V, S: Clone> ScoredListMove<'_, V, S> {
    pub fn assignment(&self) -> &ListAssignment<V> {
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
            self.transaction.record_retained(&mut changes.assignment);
        }
        for (index, score) in self.updates {
            self.contributions[index] = score;
        }
        *self.cached_score = self.candidate_score;
        self.transaction.commit();
    }
}

fn consistency_report<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    assignment: &ListAssignment<V>,
    working: &S,
    cached: &[S],
    weights: &crate::scoped_weights::LocalWeights<S>,
) -> Result<crate::ScoreConsistencyReport<S>, ListScoringError> {
    crate::score_consistency::compare(
        working,
        network
            .constraints
            .iter()
            .zip(cached)
            .map(|(c, score)| (c.name.as_str(), score)),
        // Diagnostics deliberately rediscover positions without the scoring snapshot.
        |i| {
            network.contribution_weighted(
                i,
                problem,
                assignment,
                None,
                weights.get(i, network.constraints[i].weight.as_ref()),
            )
        },
        |fresh| {
            let sum = network
                .arithmetic
                .sum(fresh)
                .map_err(|_| ListScoringError::ScoreOverflow { constraint: None })?;
            Ok(with_structure(sum, problem, assignment))
        },
    )
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
                    [1, 1, 2].map(|value| ListMove::Assign {
                        value,
                        destination: ListPosition {
                            entity: 0,
                            index: 0,
                        },
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
                    Err(ListScoringError::StructuralContribution { constraint: 0 })
                ));
            }
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.score(), SimpleScore::new(1));
            d.assert_consistent().unwrap();
        }
    }
}

crate::retained_scoped_phase::retained_scoped!(ListScoreDirector, [V], crate::ListAssignment<V>, crate::ListProblem<V>, crate::ListMove<V>, crate::ListScoringError, crate::ListLocalSearchError, SolverSequence, list, list_construction, crate::ConstructionConfig<'_>, crate::ListConstructionResult<V,S>, list_local_search, crate::LocalSearchConfig<'_,S>, crate::ListLocalSearchResult<V,S>, list_director, |a| a, []);

impl From<crate::MoveProviderError> for ListScoringError {
    fn from(error: crate::MoveProviderError) -> Self {
        Self::Provider(error)
    }
}
