//! Seeded ruin-and-recreate configuration, results and basic-variable support.
use crate::*;
use std::{
    error::Error,
    fmt,
    time::{Duration, Instant},
};

/// Relative integer weights for fine changes, fine swaps and reconstruction.
/// Zero disables a family; at least one weight must be positive. For example,
/// 2:2:1 represents 40% changes, 40% swaps and 20% repairs when all are available.
/// An unavailable family is excluded until the next attempt. Within a fine family,
/// one enumerated move is sampled uniformly, retaining duplicate successor states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepairMoveWeights {
    pub change: u32,
    pub swap: u32,
    pub repair: u32,
}
impl Default for RepairMoveWeights {
    fn default() -> Self {
        Self {
            change: 0,
            swap: 0,
            repair: 1,
        }
    }
}
impl RepairMoveWeights {
    pub(crate) fn valid(self) -> bool {
        self.change != 0 || self.swap != 0 || self.repair != 0
    }
}

/// Limits include initialization, reconstruction and enabled fine moves. Each repair samples
/// movable basic slots or list values without replacement, then reconstructs in
/// model order. Basic unsampled values are frozen; list semantics are documented
/// on `ruin_recreate_list_search_from`.
/// The range is clamped to the movable population. No Java random trace is implied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuinRecreateConfig<'a, S = SimpleScore> {
    pub seed: u64,
    pub minimum_ruined: usize,
    pub maximum_ruined: usize,
    /// Total selected attempts: reconstructions plus enabled fine moves.
    pub max_attempts: u64,
    /// Interleave moves within the same current state, acceptor and retained best.
    /// The default preserves repair-only behavior and its seeded trace.
    pub move_weights: RepairMoveWeights,
    /// Includes the initial score, reconstruction initial scores and all trials.
    pub max_evaluations: u64,
    /// Fine-move scores and construction candidate scores; excludes repair initialization.
    /// Zero still permits scoring the supplied complete assignment.
    pub max_move_evaluations: Option<u64>,
    /// Checked against retained complete best at attempt boundaries.
    pub best_score_limit: Option<S>,
    /// Stop at an attempt boundary when retained complete best is hard-feasible.
    pub stop_when_feasible: bool,
    pub max_time: Option<Duration>,
    pub cancellation: Option<&'a CancellationToken>,
    /// Receives changed variables/entities and native move/value identities.
    /// Rejected and unchanged attempts do not advance completed-step history.
    pub acceptance: Acceptance<S>,
    pub placement: ConstructionPlacement,
    pub pick_early: ConstructionPickEarly,
    pub assert_scores: bool,
}
impl<S> Default for RuinRecreateConfig<'_, S> {
    fn default() -> Self {
        Self {
            seed: 0,
            minimum_ruined: 5,
            maximum_ruined: 20,
            max_attempts: 100,
            move_weights: RepairMoveWeights::default(),
            max_evaluations: 10_000,
            max_move_evaluations: None,
            best_score_limit: None,
            stop_when_feasible: false,
            max_time: None,
            cancellation: None,
            acceptance: Acceptance::HillClimbing,
            placement: ConstructionPlacement::Sequential,
            pick_early: ConstructionPickEarly::Never,
            assert_scores: false,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuinRecreateTermination {
    AttemptLimit,
    EvaluationLimit,
    MoveEvaluationLimit,
    BestScoreLimit,
    FeasibleScore,
    UnimprovedTimeLimit,
    UnimprovedStepLimit,
    TimeLimit,
    Cancelled,
    NoMovableVariables,
    NoAvailableMoves,
}
/// Only complete assignments compete for best; interrupted partial repairs are
/// discarded. A complete interrupted repair may still improve the retained best.
/// None is returned when initialization was prevented by cancellation/zero budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuinRecreateOutcome<T> {
    pub best: Option<T>,
    pub evaluations: u64,
    /// Candidate scores inside construction, excluding each fresh initial score.
    pub repair_move_evaluations: u64,
    /// Selected ordinary change/swap attempts, including interrupted scoring.
    pub fine_move_attempts: u64,
    /// Fine-move scores, each charged as one candidate evaluation.
    pub fine_move_evaluations: u64,
    pub constraint_evaluations: u128,
    /// Started reconstruction calls, including incomplete and unchanged results.
    pub attempts: u64,
    pub complete_reconstructions: u64,
    pub accepted_moves: u64,
    pub termination: RuinRecreateTermination,
}
pub type RuinRecreateResult<V, S> = RuinRecreateOutcome<ScoredAssignment<V, S>>;
pub type ListRuinRecreateResult<V, S> = RuinRecreateOutcome<ScoredListAssignment<V, S>>;

#[derive(Debug)]
pub enum RuinRecreateError {
    InvalidRuinRange,
    InvalidMoveWeights,
    IncompleteInitialAssignment,
    Search(LocalSearchError),
    ListSearch(ListLocalSearchError),
    MixedSearch(MixedLocalSearchError),
}
impl fmt::Display for RuinRecreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidMoveWeights => {
                f.write_str("at least one move-family weight must be positive")
            }
            Self::InvalidRuinRange => {
                f.write_str("ruin counts must satisfy 1 <= minimum <= maximum")
            }
            Self::IncompleteInitialAssignment => {
                f.write_str("ruin-and-recreate requires a complete initial assignment")
            }
            Self::Search(e) => e.fmt(f),
            Self::ListSearch(e) => e.fmt(f),
            Self::MixedSearch(e) => e.fmt(f),
        }
    }
}
impl Error for RuinRecreateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Search(e) => Some(e),
            Self::ListSearch(e) => Some(e),
            Self::MixedSearch(e) => Some(e),
            _ => None,
        }
    }
}
impl From<LocalSearchError> for RuinRecreateError {
    fn from(e: LocalSearchError) -> Self {
        Self::Search(e)
    }
}

/// Search by repeatedly clearing a random subset and greedily rebuilding it.
/// Optional weighted change/swap moves share the same current state and acceptor.
/// Fine sampling scans the selected iterator with constant candidate storage;
/// cancellation and deadlines are polled between iterator calls. Each fine score
/// initializes a fresh cache/graph, including full diagnostic work when enabled.
/// Scoring must support partial assignments. Pins and the caller's initial value
/// are never mutated. All native acceptance policies are available. Best retention
/// is strict and independent of acceptance; this method never proves optimality.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap_or(0)))?;
/// let r = ruin_recreate_search_from(&p, &n, p.initial_assignment(),
///     RuinRecreateConfig { seed: 42, max_attempts: 3, ..Default::default() })?;
/// assert_eq!(r.best.unwrap().score, SimpleScore::new(1));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn ruin_recreate_search_from<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_search_from_observed(problem, network, initial, config, None, None)
}

/// Report the initial complete scored solution and each strict retained-best
/// improvement synchronously. No partial repairs, ties or accepted worsening
/// states are reported. Callbacks do not rescore; their time counts toward the
/// search deadline. Cancellation from a callback stops at the next boundary,
/// after retaining that solution. Panics propagate with caller inputs unchanged.
/// Validation failures and skipped initial scoring emit no notifications.
pub fn ruin_recreate_search_from_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
    mut observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_search_from_observed(problem, network, initial, config, Some(&mut observer), None)
}
pub(crate) fn ruin_recreate_search_from_observed<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, Assignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    if network.variable_count() != problem.variables().len() {
        return Err(LocalSearchError::from(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        })
        .into());
    }
    run(problem, initial, config, observer, shared, |p, a, c| {
        greedy_construction_from(p, network, a, c).map_err(LocalSearchError::from)
    })
}

/// A fresh graph is required for initialization and each repair or fine-move score.
/// Factory time counts toward the global time limit but not score evaluations.
/// No factory is invoked with a zero budget or an already cancelled token. Graph
/// preparation is cooperative; errors propagate and panics leave caller data intact.
pub fn ruin_recreate_stream_search_from<'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_stream_search_from_observed(problem, graph, initial, config, None, None)
}

/// Report the initial complete scored solution and each strict retained-best
/// improvement synchronously. No partial repairs, ties or accepted worsening
/// states are reported. Callbacks do not rescore; their time counts toward the
/// search deadline. Cancellation from a callback stops at the next boundary,
/// after retaining that solution. Panics propagate with caller inputs unchanged.
/// Validation failures and skipped initial scoring emit no notifications.
pub fn ruin_recreate_stream_search_from_with_observer<
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &FiniteProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
    mut observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_stream_search_from_observed(
        problem,
        graph,
        initial,
        config,
        Some(&mut observer),
        None,
    )
}
pub(crate) fn ruin_recreate_stream_search_from_observed<
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &FiniteProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, Assignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    run(problem, initial, config, observer, shared, |p, a, mut c| {
        let start = c.max_time.map(|_| Instant::now());
        let (graph, source) =
            graph().map_err(|e| LocalSearchError::from(StreamScoringError::from(e)))?;
        if let (Some(limit), Some(start)) = (c.max_time, start) {
            c.max_time = Some(limit.saturating_sub(start.elapsed()));
        }
        greedy_stream_construction_from(p, graph, source, a, c).map_err(LocalSearchError::from)
    })
}

fn run<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, Assignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    mut rebuild: impl FnMut(
        &FiniteProblem<V>,
        Assignment<V>,
        ConstructionConfig,
    ) -> Result<ConstructionResult<V, S>, LocalSearchError>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    crate::ruin_engine::run(problem, initial, config, observer, shared, |p, a, c| {
        let r = rebuild(p, a, c)?;
        Ok(crate::ruin_engine::Repair {
            assignment: r.assignment,
            score: r.score,
            complete: r.complete,
            evaluations: r.evaluations,
            constraint_evaluations: r.constraint_evaluations,
        })
    })
    .map(|r| r.map(|(assignment, score)| ScoredAssignment { assignment, score }))
}
impl<V: Eq + Clone> crate::ruin_engine::RepairModel for FiniteProblem<V> {
    type Assignment = Assignment<V>;
    type FineMove = Move<V>;
    type Movement = Move<V>;
    type Value = Option<V>;
    fn validate_complete(&self, a: &Assignment<V>) -> Result<(), RuinRecreateError> {
        self.validate(a).map_err(LocalSearchError::from)?;
        if !self.is_complete(a) {
            return Err(RuinRecreateError::IncompleteInitialAssignment);
        }
        Ok(())
    }
    fn initial(&self) -> Assignment<V> {
        self.initial_assignment()
    }
    fn population(&self) -> usize {
        self.variables().len()
    }
    fn movable(&self, _: &Assignment<V>) -> Vec<usize> {
        self.variables()
            .iter()
            .enumerate()
            .filter_map(|(i, v)| (!v.is_pinned()).then_some(i))
            .collect()
    }
    fn repair(&self, a: &Assignment<V>, selected: &[usize]) -> Result<Self, RuinRecreateError> {
        let mut ruined = vec![false; self.variables().len()];
        for &i in selected {
            ruined[i] = true;
        }
        let variables = self
            .variables()
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let mut copy = Variable::new(v.domain().to_vec());
                if v.allows_unassigned() {
                    copy = copy.allowing_unassigned();
                }
                if !ruined[i] {
                    if let Some(value) = a.get(i).expect("validated") {
                        copy = copy.with_initial(value.clone());
                    }
                    copy = copy.with_pinned(true);
                }
                copy
            })
            .collect();
        Self::new(variables).map_err(|e| LocalSearchError::from(e).into())
    }
    fn fine_moves<'a>(
        &'a self,
        a: &'a Assignment<V>,
        swap: bool,
    ) -> Result<Box<dyn Iterator<Item = Move<V>> + 'a>, RuinRecreateError> {
        if swap {
            Ok(Box::new(
                swap_moves(self, a).map_err(LocalSearchError::from)?,
            ))
        } else {
            Ok(Box::new(
                change_moves(self, a).map_err(LocalSearchError::from)?,
            ))
        }
    }
    fn apply_fine(
        &self,
        a: &Assignment<V>,
        m: &Move<V>,
    ) -> Result<Assignment<V>, RuinRecreateError> {
        let mut next = a.clone();
        self.apply(&mut next, m)
            .map_err(LocalSearchError::from)?
            .commit();
        Ok(next)
    }
    fn metadata(
        &self,
        old: &Assignment<V>,
        new: &Assignment<V>,
    ) -> (Vec<usize>, Move<V>, Vec<Option<V>>) {
        let movement = Move::Composite(
            old.values()
                .iter()
                .zip(new.values())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (_, b))| Move::Change {
                    variable: i,
                    value: b.clone(),
                })
                .collect(),
        );
        let variables = movement.touched_variables();
        let values = old.planning_values(&movement).expect("valid move");
        (variables, movement, values)
    }
}
