//! Seeded ruin-and-recreate search for complete basic-variable assignments.
use crate::*;
use std::{
    error::Error,
    fmt,
    time::{Duration, Instant},
};

/// Limits include initialization and every reconstruction. Each attempt samples
/// unpinned variables without replacement, clears them, then reconstructs them in
/// model order. Unselected values are frozen, including optional None values.
/// The range is clamped to the movable population. No Java random trace is implied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuinRecreateConfig<'a, S = SimpleScore> {
    pub seed: u64,
    pub minimum_ruined: usize,
    pub maximum_ruined: usize,
    pub max_attempts: u64,
    /// Includes the initial score, reconstruction initial scores and all trials.
    pub max_evaluations: u64,
    pub max_time: Option<Duration>,
    pub cancellation: Option<&'a CancellationToken>,
    /// Receives only changed variables and final composite move/value identities.
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
            max_evaluations: 10_000,
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
    TimeLimit,
    Cancelled,
    NoMovableVariables,
}
/// Only complete assignments compete for best; interrupted partial repairs are
/// discarded. A complete interrupted repair may still improve the retained best.
/// None is returned when initialization was prevented by cancellation/zero budget.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuinRecreateResult<V, S> {
    pub best: Option<ScoredAssignment<V, S>>,
    pub evaluations: u64,
    /// Candidate scores inside construction, excluding each fresh initial score.
    pub repair_move_evaluations: u64,
    pub constraint_evaluations: u128,
    /// Started reconstruction calls, including incomplete and unchanged results.
    pub attempts: u64,
    pub complete_reconstructions: u64,
    pub accepted_moves: u64,
    pub termination: RuinRecreateTermination,
}
#[derive(Debug)]
pub enum RuinRecreateError {
    InvalidRuinRange,
    IncompleteInitialAssignment,
    Search(LocalSearchError),
}
impl fmt::Display for RuinRecreateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRuinRange => {
                f.write_str("ruin counts must satisfy 1 <= minimum <= maximum")
            }
            Self::IncompleteInitialAssignment => {
                f.write_str("ruin-and-recreate requires a complete initial assignment")
            }
            Self::Search(e) => e.fmt(f),
        }
    }
}
impl Error for RuinRecreateError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Search(e) => Some(e),
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
    if network.variable_count() != problem.variables().len() {
        return Err(LocalSearchError::from(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        })
        .into());
    }
    run(problem, initial, config, |p, a, c| {
        greedy_construction_from(p, network, a, c).map_err(LocalSearchError::from)
    })
}

/// A fresh graph is required for initialization and each attempted reconstruction.
/// Factory time counts toward the global time limit but not score evaluations.
/// No factory is invoked with a zero budget or an already cancelled token. Graph
/// preparation is cooperative; errors propagate and panics leave caller data intact.
pub fn ruin_recreate_stream_search_from<'g, V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, PlanningStreamSource<V>), GraphError>,
    initial: Assignment<V>,
    config: RuinRecreateConfig<S>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    run(problem, initial, config, |p, a, mut c| {
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
    mut rebuild: impl FnMut(
        &FiniteProblem<V>,
        Assignment<V>,
        ConstructionConfig,
    ) -> Result<ConstructionResult<V, S>, LocalSearchError>,
) -> Result<RuinRecreateResult<V, S>, RuinRecreateError> {
    let start = config.max_time.map(|_| Instant::now());
    problem.validate(&initial).map_err(LocalSearchError::from)?;
    if !problem.is_complete(&initial) {
        return Err(RuinRecreateError::IncompleteInitialAssignment);
    }
    if config.minimum_ruined == 0 || config.minimum_ruined > config.maximum_ruined {
        return Err(RuinRecreateError::InvalidRuinRange);
    }
    let policy = config.acceptance.for_population(problem.variables().len());
    policy.validate().map_err(LocalSearchError::from)?;
    let cancelled = || {
        config
            .cancellation
            .is_some_and(CancellationToken::is_cancelled)
    };
    let remaining = || {
        config
            .max_time
            .map(|limit| limit.saturating_sub(start.expect("timed").elapsed()))
    };
    let mut result = RuinRecreateResult {
        best: None,
        evaluations: 0,
        repair_move_evaluations: 0,
        constraint_evaluations: 0,
        attempts: 0,
        complete_reconstructions: 0,
        accepted_moves: 0,
        termination: RuinRecreateTermination::EvaluationLimit,
    };
    if config.max_evaluations == 0 {
        return Ok(result);
    }
    if cancelled() {
        result.termination = RuinRecreateTermination::Cancelled;
        return Ok(result);
    }
    let construction = ConstructionConfig {
        placement: config.placement,
        cancellation: config.cancellation,
        max_evaluations: config.max_evaluations,
        max_steps: Some(0),
        optional: OptionalConstruction::NonWorsening,
        max_time: remaining(),
        pick_early: config.pick_early,
        assert_scores: config.assert_scores,
        ..Default::default()
    };
    let initial = rebuild(problem, initial, construction)?;
    result.evaluations = initial.evaluations;
    result.constraint_evaluations = initial.constraint_evaluations;
    let Some(mut current) = initial.solution() else {
        result.termination = if cancelled() {
            RuinRecreateTermination::Cancelled
        } else {
            RuinRecreateTermination::EvaluationLimit
        };
        return Ok(result);
    };
    result.best = Some(current.clone());
    let mut acceptor = policy
        .start_with_values(current.score.clone())
        .map_err(LocalSearchError::from)?;
    let mut movable: Vec<_> = problem
        .variables()
        .iter()
        .enumerate()
        .filter_map(|(i, v)| (!v.is_pinned()).then_some(i))
        .collect();
    let mut random = crate::random::SplitMix64::new(config.seed);
    loop {
        let reason = if cancelled() {
            Some(RuinRecreateTermination::Cancelled)
        } else if remaining().is_some_and(|d| d.is_zero()) {
            Some(RuinRecreateTermination::TimeLimit)
        } else if result.evaluations >= config.max_evaluations {
            Some(RuinRecreateTermination::EvaluationLimit)
        } else if result.attempts >= config.max_attempts {
            Some(RuinRecreateTermination::AttemptLimit)
        } else if movable.is_empty() {
            Some(RuinRecreateTermination::NoMovableVariables)
        } else {
            None
        };
        if let Some(reason) = reason {
            result.termination = reason;
            return Ok(result);
        }
        let low = config.minimum_ruined.min(movable.len());
        let high = config.maximum_ruined.min(movable.len());
        let count = low + random.below((high - low + 1) as u64) as usize;
        random.shuffle(&mut movable);
        let mut ruined = vec![false; problem.variables().len()];
        for &i in &movable[..count] {
            ruined[i] = true;
        }
        let variables = problem
            .variables()
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let mut copy = Variable::new(v.domain().to_vec());
                if v.allows_unassigned() {
                    copy = copy.allowing_unassigned();
                }
                if !ruined[i] {
                    if let Some(value) = current.assignment.get(i).expect("validated") {
                        copy = copy.with_initial(value.clone());
                    }
                    copy = copy.with_pinned(true);
                }
                copy
            })
            .collect();
        let repair = FiniteProblem::new(variables).map_err(LocalSearchError::from)?;
        result.attempts += 1;
        let candidate = rebuild(
            &repair,
            repair.initial_assignment(),
            ConstructionConfig {
                max_evaluations: config.max_evaluations - result.evaluations,
                max_steps: None,
                max_time: remaining(),
                ..construction
            },
        )?;
        result.evaluations += candidate.evaluations;
        result.repair_move_evaluations += candidate.move_evaluations();
        result.constraint_evaluations += candidate.constraint_evaluations;
        if let Some(candidate) = candidate.solution() {
            result.complete_reconstructions += 1;
            if candidate.score > result.best.as_ref().expect("initial best").score {
                result.best = Some(candidate.clone());
            }
            let changes: Vec<_> = current
                .assignment
                .values()
                .iter()
                .zip(candidate.assignment.values())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (_, b))| Move::Change {
                    variable: i,
                    value: b.clone(),
                })
                .collect();
            if !changes.is_empty() {
                let movement = Move::Composite(changes);
                let variables = movement.touched_variables();
                let values = current
                    .assignment
                    .planning_values(&movement)
                    .expect("valid move");
                if acceptor.accepts(
                    current.score.clone(),
                    candidate.score.clone(),
                    &variables,
                    &movement,
                    &values,
                ) {
                    acceptor.step_ended(candidate.score.clone(), &variables, &movement, &values);
                    current = candidate;
                    result.accepted_moves += 1;
                }
            }
        }
    }
}
