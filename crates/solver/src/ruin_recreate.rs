//! Seeded ruin-and-recreate configuration, results and basic-variable support.
use crate::*;
use std::{error::Error, fmt, time::Duration};

/// A fine-move family used alongside reconstruction. Standard families use the
/// existing per-model interpretation; Mixed permits independent components and
/// slot filters and is rejected for basic-only or list-only models. Basic scopes
/// only basic variables and works with basic or mixed models; lists reject it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairNeighborhood<'a> {
    Standard(Neighborhood),
    Basic(BasicNeighborhood<'a>),
    Mixed(MixedNeighborhood<'a>),
}
impl From<Neighborhood> for RepairNeighborhood<'_> {
    fn from(n: Neighborhood) -> Self {
        Self::Standard(n)
    }
}
impl RepairNeighborhood<'_> {
    pub(crate) fn at_step(self, step: u64) -> Self {
        match self {
            Self::Standard(n) => Self::Standard(n.at_step(step)),
            Self::Basic(n) => Self::Basic(n.at_step(step)),
            Self::Mixed(n) => Self::Mixed(n.at_step(step)),
        }
    }
    pub(crate) fn standard(self) -> Result<Neighborhood, RuinRecreateError> {
        match self {
            Self::Standard(n) => Ok(n),
            Self::Basic(_) => Err(RuinRecreateError::BasicFamilyRequiresBasicVariables),
            Self::Mixed(_) => Err(RuinRecreateError::MixedFamilyRequiresMixedModel),
        }
    }
}
/// A separate weighted reconstruction pool. Candidate indices follow
/// RuinRecreateConfig::ruin_candidates. Only eligible members can be sampled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepairPool<'a> {
    pub candidates: &'a [usize],
    pub weight: u32,
}

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
    /// Optional allowed repair identities. Basic models use variable indices;
    /// list models use registry indices; mixed models use basic indices followed
    /// by list registry indices offset by the number of basic variables.
    /// None allows every eligible identity; an empty slice disables repair.
    /// Pins and range eligibility still apply. Order is canonical model order,
    /// independent of this slice. Duplicate/out-of-range indices are errors.
    /// Fine neighborhoods are unaffected by this restriction.
    pub ruin_candidates: Option<&'a [usize]>,
    /// When present, replaces the single reconstruction pool with independently
    /// weighted pools. Weights multiply move_weights.repair; empty/zero-weight
    /// pools are unavailable. Global ruin_candidates also restricts every pool.
    /// Each chosen attempt samples only one pool. None preserves the legacy trace.
    pub repair_pools: Option<&'a [RepairPool<'a>]>,
    /// Total selected attempts: reconstructions plus enabled fine moves.
    pub max_attempts: u64,
    /// Interleave moves within the same current state, acceptor and retained best.
    /// The default preserves repair-only behavior and its seeded trace.
    pub move_weights: RepairMoveWeights,
    /// Additional independently weighted move families, sharing repair's current
    /// state, acceptor and budgets. Zero weights disable enumeration and validation.
    /// Each attempt samples a family, then uniformly samples one of its moves.
    /// Empty families are removed for that attempt. Duplicate families remain
    /// separate; sampled neighborhoods derive their seed from the attempt index.
    /// An empty slice preserves the original change/swap/repair seeded trace.
    pub additional_neighborhoods: &'a [(RepairNeighborhood<'a>, u32)],
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
    /// Check initialization and completed scan/placement boundaries without
    /// candidate-state or rollback rescoring. Full assert_scores takes precedence.
    /// Diagnostic work is excluded from evaluation counters but consumes time.
    pub assert_step_scores: bool,
}
impl<S> Default for RuinRecreateConfig<'_, S> {
    fn default() -> Self {
        Self {
            seed: 0,
            minimum_ruined: 5,
            maximum_ruined: 20,
            ruin_candidates: None,
            repair_pools: None,
            max_attempts: 100,
            move_weights: RepairMoveWeights::default(),
            additional_neighborhoods: &[],
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
            assert_step_scores: false,
        }
    }
}
impl<S> RuinRecreateConfig<'_, S> {
    pub(crate) fn validate_candidates(&self, count: usize) -> Result<(), RuinRecreateError> {
        let validate = |indices: &[usize]| {
            for (position, &index) in indices.iter().enumerate() {
                if index >= count {
                    return Err(RuinRecreateError::UnknownRuinCandidate { index, count });
                }
                if indices[..position].contains(&index) {
                    return Err(RuinRecreateError::DuplicateRuinCandidate { index });
                }
            }
            Ok(())
        };
        if let Some(indices) = self.ruin_candidates {
            validate(indices)?;
        }
        for pool in self.repair_pools.into_iter().flatten() {
            if pool.weight > 0 && self.move_weights.repair > 0 {
                validate(pool.candidates)?;
            }
        }
        Ok(())
    }
    pub(crate) fn has_moves(&self) -> bool {
        self.move_weights.valid() || self.additional_neighborhoods.iter().any(|(_, w)| *w > 0)
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
    SignificantImprovementTimeLimit,
    UnimprovedStepLimit,
    Condition,
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
    /// Selected fine-neighborhood attempts, including interrupted scoring.
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
    BasicFamilyRequiresBasicVariables,
    BasicNeighborhood(BasicNeighborhoodError),
    MixedFamilyRequiresMixedModel,
    MixedNeighborhood(MixedNeighborhoodError),
    UnknownRuinCandidate { index: usize, count: usize },
    DuplicateRuinCandidate { index: usize },
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
            Self::BasicFamilyRequiresBasicVariables => {
                f.write_str("basic repair family requires basic variables")
            }
            Self::BasicNeighborhood(e) => e.fmt(f),
            Self::MixedFamilyRequiresMixedModel => {
                f.write_str("mixed repair family requires a mixed model")
            }
            Self::MixedNeighborhood(e) => e.fmt(f),
            Self::UnknownRuinCandidate { index, count } => {
                write!(f, "repair candidate {index} outside {count} identities")
            }
            Self::DuplicateRuinCandidate { index } => {
                write!(f, "repair candidate {index} occurs more than once")
            }
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
            Self::BasicNeighborhood(e) => Some(e),
            Self::MixedNeighborhood(e) => Some(e),
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
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    if network.variable_count() != problem.variables().len() {
        return Err(LocalSearchError::from(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        })
        .into());
    }
    run(
        problem,
        initial,
        config,
        observer,
        shared,
        |p, a, c, monitor| {
            crate::construction::greedy_construction_from_monitored(monitor, p, network, a, c)
                .map_err(LocalSearchError::from)
        },
    )
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
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    run(
        problem,
        initial,
        config,
        observer,
        shared,
        |p, a, mut c, monitor| {
            let elapsed = crate::solver_clock::monitored_clock(c.max_time.is_some(), &monitor);
            let (graph, source) =
                graph().map_err(|e| LocalSearchError::from(StreamScoringError::from(e)))?;
            if let Some(limit) = c.max_time {
                c.max_time = Some(limit.saturating_sub(elapsed()));
            }
            crate::stream_scoring::greedy_stream_construction_from_monitored(
                monitor, p, graph, source, a, c,
            )
            .map_err(LocalSearchError::from)
        },
    )
}

fn run<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, Assignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    mut rebuild: impl FnMut(
        &FiniteProblem<V>,
        Assignment<V>,
        ConstructionConfig,
        Option<&mut dyn crate::time_limit::SearchControl<S>>,
    ) -> Result<ConstructionResult<V, S>, LocalSearchError>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    crate::ruin_engine::run(
        problem,
        initial,
        config,
        observer,
        shared,
        |p, a, c, monitor| {
            let r = rebuild(p, a, c, monitor)?;
            Ok(crate::ruin_engine::Repair {
                assignment: r.assignment,
                score: r.score,
                complete: r.complete,
                evaluations: r.evaluations,
                constraint_evaluations: r.constraint_evaluations,
            })
        },
    )
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
    fn candidate_count(&self) -> usize {
        self.variables().len()
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
        Ok(self.with_ruined_variables(a, selected))
    }
    fn validate_neighborhood(
        &self,
        neighborhood: RepairNeighborhood<'_>,
    ) -> Result<(), RuinRecreateError> {
        if let RepairNeighborhood::Basic(c) = neighborhood {
            return c
                .validate(self)
                .map_err(RuinRecreateError::BasicNeighborhood);
        }
        neighborhood
            .standard()?
            .validate_basic(self)
            .map_err(|e| LocalSearchError::from(e).into())
    }
    fn fine_moves<'a>(
        &'a self,
        a: &'a Assignment<V>,
        neighborhood: RepairNeighborhood<'a>,
    ) -> Result<crate::ruin_engine::FineMoves<'a, Move<V>>, RuinRecreateError> {
        self.validate(a).map_err(LocalSearchError::from)?;
        self.validate_neighborhood(neighborhood)?;
        let moves = if let RepairNeighborhood::Basic(c) = neighborhood {
            crate::basic_neighborhood::configured(self, a, c)
        } else {
            crate::neighborhood::ordered(
                self,
                a,
                neighborhood.standard()?,
                0..self.variables().len(),
            )
        };
        Ok(crate::ruin_engine::FineMoves::All(moves))
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
