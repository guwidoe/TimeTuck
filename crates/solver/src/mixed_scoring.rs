//! Scoped incremental contributions spanning basic variables and ordered lists.
//!
//! ```
//! use timetuck::{FiniteProblem, ListEntity, ListMove, ListProblem, MixedConstraintNetwork,
//!     MixedFact, MixedMove, MixedProblem, MixedScope, MixedScoreDirector, Move,
//!     SimpleScore, Variable};
//! let problem = MixedProblem::new(
//!     FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)])?,
//!     ListProblem::new(vec![7], vec![ListEntity::new(vec![7])])?,
//! );
//! let mut network = MixedConstraintNetwork::new(1, 1, 1);
//! network.add_constraint("weighted load", vec![MixedScope::Basic(0), MixedScope::ListEntity(0)], |facts| {
//!     let (MixedFact::Basic(value), MixedFact::ListEntity(list)) = (&facts[0], &facts[1]) else { unreachable!() };
//!     SimpleScore::new(value.copied().unwrap_or(0) * list.len() as i64)
//! })?;
//! let mut director = MixedScoreDirector::new(&problem, &network, problem.initial_assignment())?;
//! director.apply(&MixedMove { basic: Move::Change { variable: 0, value: Some(2) },
//!     lists: ListMove::Composite(vec![]) })?.commit();
//! assert_eq!(director.score(), SimpleScore::new(2));
//! director.assert_consistent()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::{
    AdditiveScore, AppliedMixedMove, ListElement, MixedAssignment, MixedError, MixedMove,
    MixedProblem,
};
use std::{error::Error, fmt};

/// Indices into basic variables, list entities, or the list value registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum MixedScope {
    Basic(usize),
    ListEntity(usize),
    ListValue(usize),
}

/// Read-only scoped facts. Unassigned list values retain their identity but have
/// no owner/index/neighbors. Repeated scopes preserve callback argument order.
#[derive(Debug)]
pub enum MixedFact<'a, B, L> {
    Basic(Option<&'a B>),
    ListEntity(&'a [L]),
    ListValue {
        value: &'a L,
        element: Option<ListElement<'a, L>>,
    },
}

type Evaluator<'a, B, L, S> = dyn Fn(
        &[MixedFact<'_, B, L>],
        &S,
        crate::ScoringArithmetic,
    ) -> Result<S, crate::scoring_arithmetic::WeightArithmeticError>
    + 'a;
struct Constraint<'a, B, L, S> {
    name: String,
    scope: Vec<MixedScope>,
    weight: Option<S>,
    evaluate: Box<Evaluator<'a, B, L, S>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MixedScoringError {
    Calculator(crate::MatchRegistryError),
    Model(MixedError),
    /// Counts in basic-variable, list-entity, list-value order.
    ModelShape {
        expected: [usize; 3],
        actual: [usize; 3],
    },
    InvalidScope(MixedScope),
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
    StructuralCountOverflow,
    ScoreCorruption(Box<crate::ScoreCorruption>),
    CorruptedTotal,
}
impl fmt::Display for MixedScoringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Model(error) if error.range_report().is_some() => error.fmt(f),
            Self::ScoreCorruption(report) => report.fmt(f),
            Self::Calculator(error) => error.fmt(f),
            _ => write!(f, "mixed scoring error: {self:?}"),
        }
    }
}
impl Error for MixedScoringError {
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
impl From<MixedError> for MixedScoringError {
    fn from(error: MixedError) -> Self {
        Self::Model(error)
    }
}

/// Named additive constraints over both component types. Callbacks must be
/// deterministic and return structural zero. Captured facts and value identities
/// must stay immutable. Empty scopes are constant contributions. Ordinary score
/// levels use checked built-in integer arithmetic by default; wrapping is configurable.
/// The network supplies minus the combined count of missing required assignments.
pub struct MixedConstraintNetwork<'a, B, L, S> {
    arithmetic: crate::ScoringArithmetic,
    constraints: Vec<Constraint<'a, B, L, S>>,
    basic_dependents: Vec<Vec<usize>>,
    entity_dependents: Vec<Vec<usize>>,
    value_dependents: Vec<Vec<usize>>,
}
impl<'a, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore> MixedConstraintNetwork<'a, B, L, S> {
    pub fn new(variable_count: usize, entity_count: usize, value_count: usize) -> Self {
        Self {
            arithmetic: crate::ScoringArithmetic::default(),
            constraints: vec![],
            basic_dependents: vec![vec![]; variable_count],
            entity_dependents: vec![vec![]; entity_count],
            value_dependents: vec![vec![]; value_count],
        }
    }
    pub fn variable_count(&self) -> usize {
        self.basic_dependents.len()
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
        scope: Vec<MixedScope>,
        evaluate: impl Fn(&[MixedFact<'_, B, L>]) -> S + 'a,
    ) -> Result<usize, MixedScoringError> {
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
        scope: Vec<MixedScope>,
        weight: S,
        factor: impl Fn(&[MixedFact<'_, B, L>]) -> i64 + 'a,
    ) -> Result<usize, MixedScoringError> {
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
        scope: Vec<MixedScope>,
        weight: S,
        factor: impl Fn(&[MixedFact<'_, B, L>]) -> crate::Decimal + 'a,
    ) -> Result<usize, MixedScoringError>
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
        scope: Vec<MixedScope>,
        weight: Option<S>,
        evaluate: Box<Evaluator<'a, B, L, S>>,
    ) -> Result<usize, MixedScoringError> {
        if self.constraints.iter().any(|c| c.name == name) {
            return Err(MixedScoringError::DuplicateName(name));
        }
        for &item in &scope {
            let valid = match item {
                MixedScope::Basic(i) => i < self.variable_count(),
                MixedScope::ListEntity(i) => i < self.entity_count(),
                MixedScope::ListValue(i) => i < self.value_count(),
            };
            if !valid {
                return Err(MixedScoringError::InvalidScope(item));
            }
        }
        if weight.as_ref().is_some_and(|w| w.structural() != 0) {
            return Err(MixedScoringError::InvalidWeight(
                crate::ConstraintWeightError::StructuralWeight(name),
            ));
        }
        let index = self.constraints.len();
        for &item in &scope {
            let dependents = match item {
                MixedScope::Basic(i) => &mut self.basic_dependents[i],
                MixedScope::ListEntity(i) => &mut self.entity_dependents[i],
                MixedScope::ListValue(i) => &mut self.value_dependents[i],
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
        problem: &MixedProblem<B, L>,
        assignment: &MixedAssignment<B, L>,
    ) -> Result<(), MixedScoringError> {
        let expected = [
            self.variable_count(),
            self.entity_count(),
            self.value_count(),
        ];
        let actual = [
            problem.basic().variables().len(),
            problem.lists().entities().len(),
            problem.lists().values().len(),
        ];
        if expected != actual {
            return Err(MixedScoringError::ModelShape { expected, actual });
        }
        problem.validate(assignment)?;
        Ok(())
    }
    fn contribution(
        &self,
        index: usize,
        problem: &MixedProblem<B, L>,
        assignment: &MixedAssignment<B, L>,
    ) -> Result<S, MixedScoringError> {
        let constraint = &self.constraints[index];
        if constraint.weight.as_ref().is_some_and(|w| *w == S::ZERO) {
            return Ok(S::ZERO);
        }
        let facts: Vec<_> = constraint
            .scope
            .iter()
            .map(|scope| match *scope {
                MixedScope::Basic(i) => MixedFact::Basic(assignment.basic().values()[i].as_ref()),
                MixedScope::ListEntity(i) => MixedFact::ListEntity(&assignment.lists().lists()[i]),
                MixedScope::ListValue(i) => {
                    let value = &problem.lists().values()[i];
                    MixedFact::ListValue {
                        value,
                        element: assignment.lists().element(value),
                    }
                }
            })
            .collect();
        let score = (constraint.evaluate)(
            &facts,
            constraint.weight.as_ref().unwrap_or(&S::ZERO),
            self.arithmetic,
        )
        .map_err(|error| match error {
            crate::scoring_arithmetic::WeightArithmeticError::Decimal(error) => {
                MixedScoringError::DecimalWeightArithmetic {
                    constraint: index,
                    error,
                }
            }
            crate::scoring_arithmetic::WeightArithmeticError::Overflow => {
                MixedScoringError::ScoreOverflow {
                    constraint: Some(index),
                }
            }
        })?;
        if score.structural() != 0 {
            return Err(MixedScoringError::StructuralContribution { constraint: index });
        }
        Ok(score)
    }
    /// Recompute all contributions, independent of any director cache.
    pub fn calculate(
        &self,
        problem: &MixedProblem<B, L>,
        assignment: &MixedAssignment<B, L>,
    ) -> Result<S, MixedScoringError> {
        self.validate(problem, assignment)?;
        let contributions = (0..self.constraint_count())
            .map(|i| self.contribution(i, problem, assignment))
            .collect::<Result<Vec<_>, _>>()?;
        let total = self
            .arithmetic
            .sum(&contributions)
            .map_err(|_| MixedScoringError::ScoreOverflow { constraint: None })?;
        with_structure(total, problem, assignment)
    }
}
pub(crate) fn with_structure<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    score: S,
    problem: &MixedProblem<B, L>,
    assignment: &MixedAssignment<B, L>,
) -> Result<S, MixedScoringError> {
    let basic_missing = problem
        .basic()
        .variables()
        .iter()
        .zip(assignment.basic().values())
        .filter(|(variable, value)| !variable.allows_unassigned() && value.is_none())
        .count();
    let list_missing = if problem.lists().allows_unassigned() {
        0
    } else {
        problem.lists().values().len()
            - assignment
                .lists()
                .lists()
                .iter()
                .map(Vec::len)
                .sum::<usize>()
    };
    let missing = i64::try_from(basic_missing as u128 + list_missing as u128)
        .map_err(|_| MixedScoringError::StructuralCountOverflow)?;
    Ok(score.with_structural(-missing))
}

/// Owns the combined assignment and cached contributions. Only affected callbacks
/// are incremental: validation, snapshots, structural counting and dependency
/// discovery still scan/copy model state. No constant-time update claim is made.
pub struct MixedScoreDirector<'p, 'c, B, L, S> {
    problem: &'p MixedProblem<B, L>,
    network: &'p MixedConstraintNetwork<'c, B, L, S>,
    assignment: MixedAssignment<B, L>,
    contributions: Vec<S>,
    score: S,
}
pub(crate) struct MixedSelection<B, L> {
    pub selected: Option<MixedMove<B, L>>,
    pub selected_accepted: bool,
    pub evaluations: u64,
    pub constraint_evaluations: u128,
    pub evaluation_limit: bool,
}
struct Selected<B, L, S> {
    movement: MixedMove<B, L>,
    assignment: MixedAssignment<B, L>,
    score: S,
    updates: Vec<(usize, S)>,
    accepted: bool,
}
impl<'p, 'c, B: Eq + Clone, L: Eq + Clone, S: AdditiveScore> MixedScoreDirector<'p, 'c, B, L, S> {
    pub fn new(
        problem: &'p MixedProblem<B, L>,
        network: &'p MixedConstraintNetwork<'c, B, L, S>,
        assignment: MixedAssignment<B, L>,
    ) -> Result<Self, MixedScoringError> {
        network.validate(problem, &assignment)?;
        let contributions = (0..network.constraint_count())
            .map(|i| network.contribution(i, problem, &assignment))
            .collect::<Result<Vec<_>, _>>()?;
        let total = network
            .arithmetic
            .sum(&contributions)
            .map_err(|_| MixedScoringError::ScoreOverflow { constraint: None })?;
        let score = with_structure(total, problem, &assignment)?;
        Ok(Self {
            problem,
            network,
            assignment,
            contributions,
            score,
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
        mut explain: impl FnMut(&str, &[MixedScope], &[MixedFact<'_, B, L>], &S) -> (J, Vec<F>),
    ) -> crate::ScopedScoreExplanation<S, J, F> {
        let mut matches = crate::ScoreExplanation::default();
        for (constraint, score) in self.network.constraints.iter().zip(&self.contributions) {
            let explanation = if constraint.weight.as_ref().is_some_and(|w| *w == S::ZERO) {
                None
            } else {
                let facts: Vec<_> = constraint
                    .scope
                    .iter()
                    .map(|scope| match *scope {
                        MixedScope::Basic(i) => {
                            MixedFact::Basic(self.assignment.basic().values()[i].as_ref())
                        }
                        MixedScope::ListEntity(i) => {
                            MixedFact::ListEntity(&self.assignment.lists().lists()[i])
                        }
                        MixedScope::ListValue(i) => {
                            let value = &self.problem.lists().values()[i];
                            MixedFact::ListValue {
                                value,
                                element: self.assignment.lists().element(value),
                            }
                        }
                    })
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
    pub fn explain_default<F: Ord + Clone>(
        &self,
        mut facts: impl FnMut(&str, &[MixedScope], &[MixedFact<'_, B, L>]) -> Vec<F>,
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
    // Save assignments/cache updates inside this exclusive borrow. All scans
    // use one unchanged base; selection failure cannot install an earlier winner.
    pub(crate) fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = MixedMove<B, L>>,
        max_evaluations: u64,
        forager: impl Into<crate::ForagerConfig<S>>,
        mut accepts: impl FnMut(&MixedMove<B, L>, S) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<S>,
        assert_scores: bool,
    ) -> Result<MixedSelection<B, L>, MixedScoringError> {
        let mut result = MixedSelection {
            selected: None,
            selected_accepted: false,
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
            let candidate = match self.apply(&movement) {
                Ok(candidate) => candidate,
                Err(MixedScoringError::Model(crate::MixedError::Lists(
                    crate::ListError::DependencyCycle { .. },
                ))) => continue,
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
                )?;
                if !report.is_consistent() {
                    return Err(MixedScoringError::ScoreCorruption(Box::new(
                        report.diagnostic(),
                    )));
                }
            }
            let accepted = accepts(&movement, candidate.score());
            forager.try_add_with(
                candidate.score(),
                accepted,
                || -> Result<_, MixedScoringError> {
                    Ok(Selected {
                        movement: movement.clone(),
                        assignment: candidate.assignment().clone(),
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
            self.assignment = best.assignment;
            for (i, score) in best.updates {
                self.contributions[i] = score;
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
        movement: &MixedMove<B, L>,
    ) -> Result<crate::MoveInspection<MixedAssignment<B, L>>, MixedScoringError> {
        Ok(crate::inspect_mixed_move(
            self.problem,
            &self.assignment,
            movement,
        )?)
    }
    /// Begin a live multi-move scope. Drop restores assignment and cached scores;
    /// commit retains edits. No scoring callbacks run to create or restore it.
    pub fn edit_session(&mut self) -> Result<crate::MoveSession<'_, Self>, MixedScoringError> {
        let assignment = self.assignment.clone();
        let contributions = self.contributions.clone();
        let score = self.score.clone();
        Ok(crate::MoveSession::new(self, move |director| {
            director.assignment = assignment;
            director.contributions = contributions;
            director.score = score;
        }))
    }
    pub fn assignment(&self) -> &MixedAssignment<B, L> {
        &self.assignment
    }
    pub fn score(&self) -> S {
        self.score.clone()
    }
    pub fn constraint_scores(&self) -> impl Iterator<Item = (&str, S)> {
        self.network
            .constraints
            .iter()
            .zip(&self.contributions)
            .map(|(c, score)| (c.name.as_str(), score.clone()))
    }
    /// Recompute every contribution to detect even mutually cancelling corruption.
    /// Diagnostics do not change cached contributions or move evaluation counts.
    pub fn assert_consistent(&self) -> Result<(), MixedScoringError> {
        let report = self.consistency_report()?;
        if report.is_consistent() {
            Ok(())
        } else {
            Err(MixedScoringError::ScoreCorruption(Box::new(
                report.diagnostic(),
            )))
        }
    }
    /// Recompute every constraint exactly once and retain all changed contributions
    /// plus cached and fresh totals, including structural state. Does not mutate
    /// assignment/caches or search counters. Callback side effects cannot be undone.
    /// Arithmetic/model errors abort reporting and retain their ordinary error type.
    pub fn consistency_report(
        &self,
    ) -> Result<crate::ScoreConsistencyReport<S>, MixedScoringError> {
        consistency_report(
            self.problem,
            self.network,
            &self.assignment,
            &self.score,
            &self.contributions,
        )
    }

    /// Reevaluate the union of constraints depending on touched basic variables,
    /// touched lists, and values in those lists before/after the move. This includes
    /// neighbors and no-op writes, and evaluates repeated dependencies once.
    /// Cache updates stay deferred; errors and unwinding restore both components.
    pub fn apply(
        &mut self,
        movement: &MixedMove<B, L>,
    ) -> Result<ScoredMixedMove<'_, B, L, S>, MixedScoringError> {
        let transaction = self
            .problem
            .apply_validated(&mut self.assignment, movement)?;
        let mut affected: Vec<_> = movement
            .basic
            .touched_variables()
            .iter()
            .flat_map(|&i| self.network.basic_dependents[i].iter().copied())
            .collect();
        let touched = movement.lists.touched_entities();
        for &entity in &touched {
            affected.extend(&self.network.entity_dependents[entity]);
        }
        for (i, value) in self.problem.lists().values().iter().enumerate() {
            if touched.iter().any(|&entity| {
                transaction.original().lists().lists()[entity].contains(value)
                    || transaction.lists().lists()[entity].contains(value)
            }) {
                affected.extend(&self.network.value_dependents[i]);
            }
        }
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
            let contribution = self.network.contribution(i, self.problem, &transaction)?;
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
            .map_err(|_| MixedScoringError::ScoreOverflow { constraint: None })?;
        let candidate_score = with_structure(score, self.problem, &transaction)?;
        Ok(ScoredMixedMove {
            transaction,
            updates,
            candidate_score,
            cached_score: &mut self.score,
            contributions: &mut self.contributions,
        })
    }
}

/// Read-only combined candidate. Drop restores both assignment components; commit
/// installs assignment and cached contributions without replaying callbacks.
/// Ordinary transaction drop limitations also apply here.
#[must_use = "dropping rejects the mixed candidate; call commit to retain it"]
pub struct ScoredMixedMove<'a, B, L, S> {
    transaction: AppliedMixedMove<'a, B, L>,
    updates: Vec<(usize, S)>,
    candidate_score: S,
    cached_score: &'a mut S,
    contributions: &'a mut [S],
}
impl<B, L, S: Clone> ScoredMixedMove<'_, B, L, S> {
    pub fn assignment(&self) -> &MixedAssignment<B, L> {
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

fn consistency_report<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    assignment: &MixedAssignment<B, L>,
    working: &S,
    cached: &[S],
) -> Result<crate::ScoreConsistencyReport<S>, MixedScoringError> {
    crate::score_consistency::compare(
        working,
        network
            .constraints
            .iter()
            .zip(cached)
            .map(|(c, score)| (c.name.as_str(), score)),
        |i| network.contribution(i, problem, assignment),
        |fresh| {
            let sum = network
                .arithmetic
                .sum(fresh)
                .map_err(|_| MixedScoringError::ScoreOverflow { constraint: None })?;
            with_structure(sum, problem, assignment)
        },
    )
}

#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::{
        FiniteProblem, ListEntity, ListMove, ListPosition, ListProblem, Move, SimpleScore, Variable,
    };
    use std::panic::{AssertUnwindSafe, catch_unwind};
    #[test]
    fn failed_scan_after_saved_joint_candidate_restores_base_and_caches() {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap(),
            ListProblem::new(
                vec![7],
                vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
            )
            .unwrap(),
        );
        for panic in [false, true] {
            let mut n = MixedConstraintNetwork::new(1, 2, 1);
            n.add_constraint(
                "fail later",
                vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
                |f| {
                    let MixedFact::Basic(v) = f[0] else { panic!() };
                    let v = *v.unwrap();
                    if v == 2 {
                        if panic {
                            panic!("later candidate")
                        };
                        return SimpleScore::ZERO.with_structural(-1);
                    }
                    SimpleScore::new(v)
                },
            )
            .unwrap();
            let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                d.select_best(
                    [1, 1, 2].map(|v| MixedMove {
                        basic: Move::Change {
                            variable: 0,
                            value: Some(v),
                        },
                        lists: ListMove::Change {
                            source: ListPosition {
                                entity: 0,
                                index: 0,
                            },
                            destination: ListPosition {
                                entity: 1,
                                index: 0,
                            },
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
                    Err(MixedScoringError::StructuralContribution { constraint: 0 })
                ));
            }
            assert_eq!(d.assignment(), &p.initial_assignment());
            assert_eq!(d.score(), SimpleScore::ZERO);
            d.assert_consistent().unwrap();
        }
    }
}
