//! Shared bounded search loop; model-specific removal and metadata stay separate.
use crate::*;
use std::time::Instant;

pub(crate) struct Repair<A, S> {
    pub assignment: A,
    pub score: Option<S>,
    pub complete: bool,
    pub evaluations: u64,
    pub constraint_evaluations: u128,
}
impl<A, S> Repair<A, S> {
    fn solution(self) -> Option<(A, S)> {
        self.complete
            .then_some(self.score)
            .flatten()
            .map(|s| (self.assignment, s))
    }
}
pub(crate) trait RepairModel: Sized {
    type Assignment: Clone + Eq;
    type Movement: Clone + Eq;
    type Value: Clone + Eq;
    fn validate_complete(&self, a: &Self::Assignment) -> Result<(), RuinRecreateError>;
    fn initial(&self) -> Self::Assignment;
    fn population(&self) -> usize;
    fn movable(&self, a: &Self::Assignment) -> Vec<usize>;
    fn repair(&self, a: &Self::Assignment, selected: &[usize]) -> Result<Self, RuinRecreateError>;
    fn metadata(
        &self,
        old: &Self::Assignment,
        new: &Self::Assignment,
    ) -> (Vec<usize>, Self::Movement, Vec<Self::Value>);
}
impl<T> RuinRecreateOutcome<T> {
    pub(crate) fn map<U>(self, f: impl FnOnce(T) -> U) -> RuinRecreateOutcome<U> {
        RuinRecreateOutcome {
            best: self.best.map(f),
            evaluations: self.evaluations,
            repair_move_evaluations: self.repair_move_evaluations,
            constraint_evaluations: self.constraint_evaluations,
            attempts: self.attempts,
            complete_reconstructions: self.complete_reconstructions,
            accepted_moves: self.accepted_moves,
            termination: self.termination,
        }
    }
}
pub(crate) fn run<P: RepairModel, S: AdditiveScore>(
    problem: &P,
    initial: P::Assignment,
    config: RuinRecreateConfig<S>,
    mut rebuild: impl FnMut(
        &P,
        P::Assignment,
        ConstructionConfig,
    ) -> Result<Repair<P::Assignment, S>, RuinRecreateError>,
) -> Result<RuinRecreateOutcome<(P::Assignment, S)>, RuinRecreateError> {
    let start = config.max_time.map(|_| Instant::now());
    problem.validate_complete(&initial)?;
    if config.minimum_ruined == 0 || config.minimum_ruined > config.maximum_ruined {
        return Err(RuinRecreateError::InvalidRuinRange);
    }
    let policy = config.acceptance.for_population(problem.population());
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
    let mut result = RuinRecreateOutcome {
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
        .start_with_values(current.1.clone())
        .map_err(LocalSearchError::from)?;
    // Keep the previous permutation when the eligible set is unchanged, retaining
    // the established basic-variable seeded trace. Lists can change eligibility.
    let mut movable = problem.movable(&current.0);
    let mut eligible = movable.clone();
    let mut random = crate::random::SplitMix64::new(config.seed);
    loop {
        let now_eligible = problem.movable(&current.0);
        if now_eligible != eligible {
            movable = now_eligible.clone();
            eligible = now_eligible;
        }
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
        let repair = problem.repair(&current.0, &movable[..count])?;
        result.attempts += 1;
        let candidate = rebuild(
            &repair,
            repair.initial(),
            ConstructionConfig {
                max_evaluations: config.max_evaluations - result.evaluations,
                max_steps: None,
                max_time: remaining(),
                ..construction
            },
        )?;
        result.evaluations += candidate.evaluations;
        result.repair_move_evaluations += candidate.evaluations.saturating_sub(1);
        result.constraint_evaluations += candidate.constraint_evaluations;
        if let Some(candidate) = candidate.solution() {
            result.complete_reconstructions += 1;
            if candidate.1 > result.best.as_ref().expect("initial best").1 {
                result.best = Some(candidate.clone());
            }
            if current.0 != candidate.0 {
                let (variables, movement, values) = problem.metadata(&current.0, &candidate.0);
                if acceptor.accepts(
                    current.1.clone(),
                    candidate.1.clone(),
                    &variables,
                    &movement,
                    &values,
                ) {
                    acceptor.step_ended(candidate.1.clone(), &variables, &movement, &values);
                    current = candidate;
                    result.accepted_moves += 1;
                }
            }
        }
    }
}
