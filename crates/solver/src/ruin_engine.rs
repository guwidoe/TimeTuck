//! Shared bounded search loop; model-specific removal and metadata stay separate.
use crate::*;

pub(crate) type RepairObserver<'a, A, S> = Option<&'a mut dyn FnMut(SolutionUpdate<'_, A, S>)>;

pub(crate) enum FineMoves<'a, M> {
    // Keep the existing iterator allocation for models without dependency checks.
    All(Box<dyn Iterator<Item = M> + 'a>),
    Checked(Box<dyn Iterator<Item = Option<M>> + 'a>),
}
impl<M> Iterator for FineMoves<'_, M> {
    type Item = Option<M>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::All(moves) => moves.next().map(Some),
            Self::Checked(moves) => moves.next(),
        }
    }
}

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
    type FineMove;
    type Movement: Clone + Eq;
    type Value: Clone + Eq;
    fn validate_complete(&self, a: &Self::Assignment) -> Result<(), RuinRecreateError>;
    fn initial(&self) -> Self::Assignment;
    fn population(&self) -> usize;
    fn candidate_count(&self) -> usize;
    fn movable(&self, a: &Self::Assignment) -> Vec<usize>;
    fn repair(&self, a: &Self::Assignment, selected: &[usize]) -> Result<Self, RuinRecreateError>;
    fn validate_neighborhood(
        &self,
        neighborhood: RepairNeighborhood<'_>,
    ) -> Result<(), RuinRecreateError>;
    // Yield one slot per raw candidate. None is a rejected structural cycle;
    // the outer sampler still polls cancellation/time between rejected slots.
    fn fine_moves<'a>(
        &'a self,
        a: &'a Self::Assignment,
        neighborhood: RepairNeighborhood<'a>,
    ) -> Result<FineMoves<'a, Self::FineMove>, RuinRecreateError>;
    fn apply_fine(
        &self,
        a: &Self::Assignment,
        m: &Self::FineMove,
    ) -> Result<Self::Assignment, RuinRecreateError>;
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
            fine_move_attempts: self.fine_move_attempts,
            fine_move_evaluations: self.fine_move_evaluations,
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
    mut observer: crate::ruin_engine::RepairObserver<'_, P::Assignment, S>,
    mut shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    mut rebuild: impl FnMut(
        &P,
        P::Assignment,
        ConstructionConfig,
        Option<&mut dyn crate::time_limit::SearchControl<S>>,
    ) -> Result<Repair<P::Assignment, S>, RuinRecreateError>,
) -> Result<RuinRecreateOutcome<(P::Assignment, S)>, RuinRecreateError> {
    let move_counter = crate::move_metrics::from_control(&shared);
    let elapsed = crate::solver_clock::monitored_clock(config.max_time.is_some(), &shared);
    problem.validate_complete(&initial)?;
    config.validate_candidates(problem.candidate_count())?;
    if config.minimum_ruined == 0 || config.minimum_ruined > config.maximum_ruined {
        return Err(RuinRecreateError::InvalidRuinRange);
    }
    if !config.has_moves() {
        return Err(RuinRecreateError::InvalidMoveWeights);
    }
    for &(neighborhood, weight) in config.additional_neighborhoods {
        if weight > 0 {
            problem.validate_neighborhood(neighborhood)?;
        }
    }
    let policy = config.acceptance.for_population(problem.population());
    policy.validate().map_err(LocalSearchError::from)?;
    let cancelled = || {
        config
            .cancellation
            .is_some_and(CancellationToken::is_cancelled)
    };
    let remaining = || config.max_time.map(|limit| limit.saturating_sub(elapsed()));
    let mut result = RuinRecreateOutcome {
        best: None,
        evaluations: 0,
        repair_move_evaluations: 0,
        fine_move_attempts: 0,
        fine_move_evaluations: 0,
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
        assert_step_scores: config.assert_step_scores,
        ..Default::default()
    };
    let initial = crate::score_evaluations::rebuilding(&mut shared, |monitor| {
        rebuild(problem, initial, construction, monitor)
    })?;
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
    if let Some(shared) = &mut shared {
        shared.start_search();
        shared.observe_complete_best_score(&current.1);
    }
    if let Some(observer) = &mut observer {
        observer(SolutionUpdate {
            assignment: &current.0,
            score: current.1.clone(),
            complete: true,
        });
    }
    // Keep the previous permutation when the eligible set is unchanged, retaining
    // the established basic-variable seeded trace. Lists can change eligibility.
    let eligible_candidates = |assignment: &P::Assignment| {
        let mut indices = problem.movable(assignment);
        if let Some(selected) = config.ruin_candidates {
            indices.retain(|i| selected.contains(i));
        }
        indices
    };
    let mut movable = eligible_candidates(&current.0);
    let mut eligible = movable.clone();
    let mut random = crate::random::SplitMix64::new(config.seed);
    'search: loop {
        let now_eligible = eligible_candidates(&current.0);
        if now_eligible != eligible {
            movable = now_eligible.clone();
            eligible = now_eligible;
        }
        let shared_reason = shared.as_mut().and_then(|s| s.stop());
        let reason = if cancelled() {
            Some(RuinRecreateTermination::Cancelled)
        } else if remaining().is_some_and(|d| d.is_zero()) {
            Some(RuinRecreateTermination::TimeLimit)
        } else if let Some(reason) = shared_reason {
            Some(match reason {
                LocalSearchTermination::SignificantImprovementTimeLimit => {
                    RuinRecreateTermination::SignificantImprovementTimeLimit
                }
                LocalSearchTermination::UnimprovedTimeLimit => {
                    RuinRecreateTermination::UnimprovedTimeLimit
                }
                LocalSearchTermination::StepCondition => RuinRecreateTermination::Condition,
                LocalSearchTermination::UnimprovedStepLimit => {
                    RuinRecreateTermination::UnimprovedStepLimit
                }
                _ => unreachable!("solve-wide monitor returns only stagnation stops"),
            })
        } else if result.evaluations >= config.max_evaluations {
            Some(RuinRecreateTermination::EvaluationLimit)
        } else if config.max_move_evaluations.is_some_and(|limit| {
            result.repair_move_evaluations + result.fine_move_evaluations >= limit
        }) {
            Some(RuinRecreateTermination::MoveEvaluationLimit)
        } else if result.attempts + result.fine_move_attempts >= config.max_attempts {
            Some(RuinRecreateTermination::AttemptLimit)
        } else if config
            .best_score_limit
            .as_ref()
            .is_some_and(|limit| result.best.as_ref().expect("initial best").1 >= *limit)
        {
            Some(RuinRecreateTermination::BestScoreLimit)
        } else if config.stop_when_feasible
            && result.best.as_ref().expect("initial best").1.is_feasible()
        {
            Some(RuinRecreateTermination::FeasibleScore)
        } else if movable.is_empty()
            && config.move_weights.change == 0
            && config.move_weights.swap == 0
            && !config.additional_neighborhoods.iter().any(|(_, w)| *w > 0)
        {
            Some(RuinRecreateTermination::NoMovableVariables)
        } else {
            None
        };
        if let Some(reason) = reason {
            result.termination = reason;
            return Ok(result);
        }
        // Select a family by its fixed weight, then a uniform fine candidate.
        // Empty families are removed and the remaining weights renormalized.
        // Repair-only calls consume exactly the historical random sequence.
        let mut weights = vec![
            config.move_weights.change as u128,
            config.move_weights.swap as u128,
            if movable.is_empty() || config.repair_pools.is_some() {
                0
            } else {
                config.move_weights.repair as u128
            },
        ];
        weights.extend(
            config
                .additional_neighborhoods
                .iter()
                .map(|(_, w)| u128::from(*w)),
        );
        let pool_start = weights.len();
        let pools = config.repair_pools.unwrap_or_default();
        weights.extend(pools.iter().map(|pool| {
            if movable.iter().any(|i| pool.candidates.contains(i)) {
                u128::from(pool.weight) * u128::from(config.move_weights.repair)
            } else {
                0
            }
        }));
        let (family, movement) = loop {
            let total: u128 = weights.iter().sum();
            if total == 0 {
                result.termination = RuinRecreateTermination::NoAvailableMoves;
                return Ok(result);
            }
            let family = if weights.iter().enumerate().all(|(i, w)| i == 2 || *w == 0) {
                2
            } else {
                let mut ticket = if let Ok(total) = u64::try_from(total) {
                    u128::from(random.below(total))
                } else {
                    random.below_wide(total)
                };
                let mut index = 0;
                while ticket >= weights[index] {
                    ticket -= weights[index];
                    index += 1;
                }
                index
            };
            if family == 2 || family >= pool_start {
                break (family, None);
            }
            let neighborhood = match family {
                0 => RepairNeighborhood::Standard(Neighborhood::Change),
                1 => RepairNeighborhood::Standard(Neighborhood::Swap),
                _ => config.additional_neighborhoods[family - 3].0,
            }
            .at_step(result.attempts + result.fine_move_attempts);
            let mut moves = problem.fine_moves(&current.0, neighborhood)?;
            let mut selected = None;
            let mut seen = 0_u64;
            loop {
                if cancelled()
                    || remaining().is_some_and(|d| d.is_zero())
                    || shared.as_mut().is_some_and(|s| s.stop().is_some())
                {
                    continue 'search;
                }
                let Some(movement) = moves.next() else {
                    break;
                };
                let Some(movement) = movement else { continue };
                seen += 1;
                if random.below(seen) == 0 {
                    selected = Some(movement);
                }
            }
            if selected.is_some() {
                break (family, selected);
            }
            weights[family] = 0;
        };
        if let Some(shared) = &mut shared {
            shared.step_started();
        }
        let mut completed = false;
        let candidate = if family == 2 || family >= pool_start {
            let mut pool_candidates;
            let movable = if family == 2 {
                &mut movable
            } else {
                pool_candidates = movable
                    .iter()
                    .copied()
                    .filter(|i| pools[family - pool_start].candidates.contains(i))
                    .collect::<Vec<_>>();
                &mut pool_candidates
            };
            let low = config.minimum_ruined.min(movable.len());
            let high = config.maximum_ruined.min(movable.len());
            let count = low + random.below((high - low + 1) as u64) as usize;
            random.shuffle(movable);
            let repair = problem.repair(&current.0, &movable[..count])?;
            result.attempts += 1;
            let candidate = crate::score_evaluations::reconstructing(&mut shared, |monitor| {
                rebuild(
                    &repair,
                    repair.initial(),
                    ConstructionConfig {
                        max_evaluations: config.max_evaluations - result.evaluations,
                        max_move_evaluations: config.max_move_evaluations.map(|limit| {
                            limit - result.repair_move_evaluations - result.fine_move_evaluations
                        }),
                        max_steps: None,
                        max_time: remaining(),
                        ..construction
                    },
                    monitor,
                )
            })?;
            result.repair_move_evaluations += candidate.evaluations.saturating_sub(1);
            if let Some(counter) = &move_counter {
                counter.add(
                    crate::MoveEvaluationKind::Reconstruction,
                    candidate.evaluations.saturating_sub(1),
                );
            }
            if candidate.complete && candidate.score.is_some() {
                result.complete_reconstructions += 1;
            }
            candidate
        } else {
            let next = problem.apply_fine(&current.0, &movement.expect("selected fine move"))?;
            result.fine_move_attempts += 1;
            let candidate = crate::score_evaluations::rebuilding(&mut shared, |monitor| {
                rebuild(
                    problem,
                    next,
                    ConstructionConfig {
                        max_evaluations: 1,
                        max_move_evaluations: Some(0),
                        max_steps: Some(0),
                        max_time: remaining(),
                        ..construction
                    },
                    monitor,
                )
            })?;
            result.fine_move_evaluations += candidate.evaluations;
            if let Some(counter) = &move_counter {
                counter.add(crate::MoveEvaluationKind::FineRepair, candidate.evaluations);
            }
            candidate
        };
        result.evaluations += candidate.evaluations;
        result.constraint_evaluations += candidate.constraint_evaluations;
        if let Some(candidate) = candidate.solution() {
            let improved = candidate.1 > result.best.as_ref().expect("initial best").1;
            if improved {
                result.best = Some(candidate.clone());
                if let Some(shared) = &mut shared {
                    shared.record_improvement();
                    shared.observe_complete_best_score(&candidate.1);
                }
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
                    completed = true;
                    if let Some(shared) = &mut shared {
                        shared.complete_step(improved);
                    }
                }
            }
            if improved && let Some(observer) = &mut observer {
                let (assignment, score) = result.best.as_ref().expect("retained best");
                observer(SolutionUpdate {
                    assignment,
                    score: score.clone(),
                    complete: true,
                });
            }
        }
        if let Some(shared) = &mut shared {
            crate::time_limit::finish_step(*shared, completed, || current.1.clone(), || true);
        }
    }
}

#[cfg(test)]
mod fine_dependency_tests {
    use super::*;

    #[test]
    fn cyclic_proposals_leave_slots_for_control_polling_and_do_not_enter_the_reservoir() {
        let lists = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
            .unwrap()
            .with_dependencies(vec![vec![], vec![0], vec![1]])
            .unwrap();
        let initial = lists.initial_assignment();
        let raw: Vec<_> = list_moves(&lists, &initial, Neighborhood::Swap)
            .unwrap()
            .collect();
        assert!(!raw.is_empty());
        assert!(raw.iter().all(|m| matches!(
            lists.apply(&mut initial.clone(), m),
            Err(ListError::DependencyCycle { .. })
        )));
        let slots: Vec<_> = lists
            .fine_moves(&initial, RepairNeighborhood::Standard(Neighborhood::Swap))
            .unwrap()
            .collect();
        assert_eq!(slots.len(), raw.len());
        assert!(slots.iter().all(Option::is_none));
        let network = ListConstraintNetwork::<_, SimpleScore>::new(1, 3);
        let result = ruin_recreate_list_search_from(
            &lists,
            &network,
            initial,
            RuinRecreateConfig {
                move_weights: RepairMoveWeights {
                    change: 0,
                    swap: 1,
                    repair: 0,
                },
                max_attempts: 100,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            result.termination,
            RuinRecreateTermination::NoAvailableMoves
        );
        assert_eq!(result.evaluations, 1);
        assert_eq!(result.fine_move_attempts, 0);
        assert_eq!(result.fine_move_evaluations, 0);
    }

    #[test]
    fn configured_mixed_candidates_preserve_basic_singles_and_reject_cyclic_joint_moves() {
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
            ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![0, 1, 2])])
                .unwrap()
                .with_dependencies(vec![vec![], vec![0], vec![1]])
                .unwrap(),
        );
        let a = p.initial_assignment();
        let config = MixedNeighborhood {
            basic: Some(Neighborhood::Change),
            lists: Some(Neighborhood::Swap),
            joint_moves: true,
            ..Default::default()
        };
        let raw: Vec<_> = mixed_moves_configured(&p, &a, config).unwrap().collect();
        let expected: Vec<_> = raw
            .iter()
            .map(|m| p.apply(&mut a.clone(), m).is_ok().then(|| m.clone()))
            .collect();
        let actual: Vec<_> = p
            .fine_moves(&a, RepairNeighborhood::Mixed(config))
            .unwrap()
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.iter().flatten().count(), 1);
        assert!(actual.len() > 1);
        assert_eq!(
            actual[0].as_ref().unwrap().basic,
            Move::Change {
                variable: 0,
                value: Some(1)
            }
        );
    }
}
