//! Original weighted repair/fine-move phase and control checks.
#![allow(dead_code)]
include!("solver_support/fixture.rs");
fn sequence<'a>(weights: RepairMoveWeights, attempts: u64) -> SolverSequence<'a> {
    SolverSequence {
        phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
            minimum_ruined: usize::MAX,
            maximum_ruined: usize::MAX,
            move_weights: weights,
            max_attempts: attempts,
            assert_scores: true,
            ..Default::default()
        })],
        ..Default::default()
    }
}
const CHANGE: RepairMoveWeights = RepairMoveWeights {
    change: 1,
    swap: 0,
    repair: 0,
};
#[test]
fn fine_moves_obey_every_shared_budget_prefix_and_phase_work_sum() {
    for mode in 0..6 {
        for scores in 0..=12 {
            for moves in 0..=12 {
                let mut c = sequence(CHANGE, 7);
                c.limits.max_evaluations = scores;
                c.limits.max_move_evaluations = Some(moves);
                let factories = Cell::new(0);
                let r = run_scored(
                    (mode, true, MixedConstructionOrder::BasicThenLists),
                    c,
                    &factories,
                    None,
                    || {},
                    |_| SimpleScore::ZERO,
                )
                .unwrap();
                let attempted = 7.min(scores.saturating_sub(1)).min(moves);
                assert_eq!(r.work.fine_move_attempts, attempted);
                assert_eq!(r.work.evaluations, u64::from(scores > 0) + attempted);
                assert_eq!(r.work.move_evaluations, attempted);
                assert_eq!(
                    (r.work.accepted_moves, r.work.completed_steps),
                    (attempted, attempted)
                );
                assert_eq!(
                    (r.work.repair_attempts, r.work.complete_reconstructions),
                    (0, 0)
                );
                assert_eq!(
                    factories.get(),
                    if mode % 2 == 1 {
                        (u64::from(scores > 0) + attempted) as usize
                    } else {
                        0
                    }
                );
            }
        }
        let mut c = sequence(CHANGE, 7);
        c.phases.push(c.phases[0].clone());
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        assert_eq!(
            (
                r.work.fine_move_attempts,
                r.work.move_evaluations,
                r.work.evaluations
            ),
            (14, 14, 16)
        );
        assert_eq!(
            r.work.fine_move_attempts,
            r.phases.iter().map(|p| p.work.fine_move_attempts).sum()
        );
    }
}
#[test]
fn empty_families_terminate_and_invalid_weights_fail_before_any_phase() {
    for mode in 0..6 {
        let factories = Cell::new(0);
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            sequence(
                RepairMoveWeights {
                    change: 0,
                    swap: 1,
                    repair: 0,
                },
                10,
            ),
            &factories,
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.work.fine_move_attempts, 0);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::RuinRecreate(RuinRecreateTermination::NoAvailableMoves)
        );
        factories.set(0);
        let mut c = sequence(
            RepairMoveWeights {
                change: 0,
                swap: 0,
                repair: 0,
            },
            10,
        );
        c.phases
            .insert(0, SolverPhase::Construction(ConstructionConfig::default()));
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &factories,
            None,
            || panic!("invalid sequence scored"),
            SimpleScore::new,
        );
        assert_eq!(r, Err(Failure::Repair(1)));
        assert_eq!(factories.get(), 0);
        // Empty swap families renormalize even at the largest supported weights.
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            sequence(
                RepairMoveWeights {
                    change: 1,
                    swap: u32::MAX,
                    repair: 0,
                },
                5,
            ),
            &factories,
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        assert_eq!(r.work.fine_move_attempts, 5);
    }
}
#[test]
fn fine_and_repair_families_share_tabu_memory_and_current_state() {
    for mode in 0..6 {
        for seed in 0..8 {
            let mut c = sequence(
                RepairMoveWeights {
                    change: 2,
                    swap: 0,
                    repair: 1,
                },
                30,
            );
            let SolverPhase::RuinRecreate(ref mut config) = c.phases[0] else {
                unreachable!()
            };
            config.seed = seed;
            config.acceptance = Acceptance::VariableTabu {
                tenure: 3,
                aspiration: true,
            };
            let r = run_scored_events_initial(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &Cell::new(0),
                None,
                || {},
                |_| SimpleScore::ZERO,
                None,
                None,
                2,
            )
            .unwrap();
            assert!(r.work.repair_attempts > 0 && r.work.fine_move_attempts > 0);
            assert_eq!(r.work.repair_attempts + r.work.fine_move_attempts, 30);
            assert!(
                r.work.accepted_moves > 0 && r.work.accepted_moves <= if mode < 4 { 1 } else { 2 }
            );
            assert_eq!(r.work.accepted_moves, r.work.completed_steps);
        }
    }
}
#[test]
fn fine_factory_cancellation_keeps_the_previous_incumbent_and_attempt_count() {
    for mode in [1, 3, 5] {
        let token = CancellationToken::new();
        let mut c = sequence(CHANGE, 10);
        c.limits.cancellation = Some(&token);
        let factories = Cell::new(0);
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &factories,
            None,
            || {
                if factories.get() == 2 {
                    token.cancel();
                }
            },
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(r.work.fine_move_attempts, 1);
        assert!((1..=2).contains(&r.work.evaluations));
        assert_eq!(r.work.move_evaluations, r.work.evaluations - 1);
    }
}
#[test]
fn sampling_polls_cancellation_without_scoring_an_unselected_move() {
    use std::rc::Rc;
    #[derive(Clone)]
    struct V {
        value: usize,
        armed: Rc<Cell<bool>>,
        calls: Rc<Cell<usize>>,
        token: CancellationToken,
    }
    impl PartialEq for V {
        fn eq(&self, other: &Self) -> bool {
            if self.armed.get() {
                self.calls.set(self.calls.get() + 1);
                if self.calls.get() == 30 {
                    self.token.cancel();
                }
            }
            self.value == other.value
        }
    }
    impl Eq for V {}
    let token = CancellationToken::new();
    let armed = Rc::new(Cell::new(false));
    let calls = Rc::new(Cell::new(0));
    let value = |v| V {
        value: v,
        armed: armed.clone(),
        calls: calls.clone(),
        token: token.clone(),
    };
    let p = FiniteProblem::new(vec![
        Variable::new((0..200).map(value).collect()).with_initial(value(0)),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("score", vec![0], |_| SimpleScore::ZERO)
        .unwrap();
    let r = ruin_recreate_search_from_with_observer(
        &p,
        &n,
        p.initial_assignment(),
        RuinRecreateConfig {
            move_weights: CHANGE,
            cancellation: Some(&token),
            ..Default::default()
        },
        |_| armed.set(true),
    )
    .unwrap();
    assert_eq!(r.termination, RuinRecreateTermination::Cancelled);
    assert_eq!(
        (r.evaluations, r.fine_move_attempts, r.fine_move_evaluations),
        (1, 0, 0)
    );
    assert!(calls.get() >= 30 && calls.get() < 200);
}

#[test]
fn accepted_fine_moves_share_stagnation_across_phase_boundaries() {
    for mode in 0..6 {
        let mut c = sequence(CHANGE, 2);
        c.phases.push(sequence(CHANGE, 10).phases.remove(0));
        c.limits.max_unimproved_steps = Some(3);
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::UnimprovedStepLimit);
        assert_eq!(r.work.completed_steps, 3);
        assert_eq!(r.work.fine_move_attempts, 3);
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.phases[0].work.completed_steps, 2);
        assert_eq!(r.phases[1].work.completed_steps, 1);
    }
}

#[test]
fn managed_jobs_publish_actual_fine_work_and_preserve_weighted_configuration() {
    let config = sequence(CHANGE, 7);
    let factory = SolverFactory::new(config.clone(), |p: FiniteProblem<i64>, c, progress| {
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("flat", vec![0], |_| SimpleScore::ZERO)
            .unwrap();
        let r = solve(&p, &n, c).map_err(|e| e.to_string())?;
        assert!(progress.record_work(r.work));
        Ok::<_, String>(r)
    })
    .unwrap();
    let mut manager = factory.build_manager(2).unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let limited = manager
        .job_builder(1)
        .with_problem(p.clone())
        .with_limits(SolverLimits {
            max_move_evaluations: Some(3),
            ..Default::default()
        })
        .run()
        .unwrap();
    let full = manager.job_builder(2).with_problem(p).run().unwrap();
    for (job, attempts) in [(&limited, 3), (&full, 7)] {
        let result = job
            .wait_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        let r = result.as_ref().as_ref().unwrap();
        assert_eq!(r.work.fine_move_attempts, attempts);
        assert_eq!(r.work.move_evaluations, attempts);
        assert_eq!(r.work.evaluations, attempts + 1);
        assert_eq!(r.work.repair_attempts, 0);
        assert_eq!(job.statistics().work, Some(r.work));
        assert_eq!(job.configuration().phases, config.phases);
    }
    manager.shutdown().unwrap();
}
