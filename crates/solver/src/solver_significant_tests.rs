//! Solve-wide score-threshold lifecycle and shared-engine regression coverage.
use super::*;
use crate::time_limit::SearchControl;

fn threshold(window: u64, minimum: i64) -> SignificantImprovementConfig<SimpleScore> {
    SignificantImprovementConfig::new(ms(window), SimpleScore::new(minimum)).unwrap()
}

// Adapted from Timefold Community's
// UnimprovedTimeMillisSpentScoreDifferenceThresholdTerminationTest.withConstructionHeuristic.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/licenses/timefold-Apache-2.0.txt.
#[test]
fn significant_with_construction_heuristic() {
    let now = Cell::new(0);
    let mut clock = || ms(now.get());
    let mut shared = step_control(u64::MAX, &mut clock);
    shared.significant = Some(ScopedSignificantImprovement::new(threshold(1000, 7)));
    shared
        .significant
        .as_mut()
        .unwrap()
        .phase_started(SolverPhaseKind::Construction, Duration::ZERO);
    shared.lifecycle.start();
    shared
        .lifecycle
        .start_phase(0, SolverPhaseKind::Construction);
    shared.complete_construction_step();
    for elapsed in [500, 1001] {
        now.set(elapsed);
        // The phase's cooperative construction check and the solver's check are
        // both inactive; their shared policy reports zero progress throughout.
        assert!(!shared.construction_stopped());
        assert_eq!(
            shared
                .significant
                .as_mut()
                .unwrap()
                .progress(TerminationProgressScope::Solve, ms(elapsed)),
            0.0
        );
        assert!(shared.stop().is_none());
        assert_eq!(
            shared
                .significant
                .as_mut()
                .unwrap()
                .progress(TerminationProgressScope::Solve, ms(elapsed)),
            0.0
        );
        if elapsed == 500 {
            shared.complete_construction_step();
        }
    }
    shared.lifecycle.end_phase();
    drop(shared);
}

#[test]
fn significant_accumulates_small_gains_and_keeps_one_anchor_across_phases() {
    let now = Cell::new(0);
    let mut clock = || ms(now.get());
    let mut shared = step_control(u64::MAX, &mut clock);
    shared.significant = Some(ScopedSignificantImprovement::new(threshold(1000, 7)));
    shared
        .significant
        .as_mut()
        .unwrap()
        .phase_started(SolverPhaseKind::LocalSearch, Duration::ZERO);
    shared.start_search();
    shared.observe_best_score(&SimpleScore::ZERO);
    now.set(500);
    shared.record_improvement();
    shared.observe_best_score(&SimpleScore::new(5));
    assert_eq!(
        shared
            .significant
            .as_mut()
            .unwrap()
            .progress(TerminationProgressScope::Solve, ms(500)),
        0.5
    );
    shared.start_search(); // Phase handoff does not restart the anchor.
    shared.observe_best_score(&SimpleScore::new(5));
    now.set(750);
    shared.record_improvement();
    shared.observe_best_score(&SimpleScore::new(7)); // Accumulated 5 + 2 qualifies.
    assert_eq!(
        shared
            .significant
            .as_mut()
            .unwrap()
            .progress(TerminationProgressScope::Solve, ms(750)),
        0.0
    );
    now.set(1750);
    assert!(shared.stop().is_none()); // Exact equality is deliberately allowed.
    now.set(1751);
    shared.record_improvement();
    shared.observe_best_score(&SimpleScore::new(100)); // Too late to reset.
    assert_eq!(
        shared.stop(),
        Some(LocalSearchTermination::SignificantImprovementTimeLimit)
    );
    now.set(1); // Clock reversal and a new phase cannot revive a latched timer.
    shared.start_search();
    shared.observe_best_score(&SimpleScore::new(200));
    assert_eq!(
        shared.stop(),
        Some(LocalSearchTermination::SignificantImprovementTimeLimit)
    );
}

#[test]
fn significant_initial_construction_and_first_factory_do_not_consume_the_window() {
    let policy = threshold(1, 7);
    // Initialization is deliberately longer than the entire shared window.
    let construction = SolverSequence {
        limits: SolverLimits {
            significant_improvement: Some(&policy),
            ..Default::default()
        },
        phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
    };
    let r = exercise_variable(construction, 50_000, |_| 0, false);
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
    assert!(r.complete);
    let sequence = SolverSequence {
        limits: SolverLimits {
            significant_improvement: Some(&policy),
            ..Default::default()
        },
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig::default())],
    };
    let cheap = exercise_scored(sequence.clone(), 0, |_| 0);
    let expensive = exercise_scored(sequence, 50_000, |_| 0);
    assert_eq!(cheap.work, expensive.work);
    assert_eq!(
        cheap.termination,
        SolverTermination::SignificantImprovementTimeLimit
    );
    assert_eq!(
        cheap.phases[0].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::SignificantImprovementTimeLimit)
    );
}

#[test]
fn significant_custom_phases_preserve_the_shared_window_and_late_best() {
    for window in 0..=8 {
        let policy = threshold(window, 7);
        let r = exercise_scored(
            SolverSequence {
                limits: SolverLimits {
                    significant_improvement: Some(&policy),
                    ..Default::default()
                },
                phases: vec![SolverPhase::Custom(CustomPhaseConfig::default()); 2],
            },
            0,
            |_| 0,
        );
        // t=1 initializes. Equality continues, each candidate costs one tick,
        // and the second initialization costs a tick without resetting anything.
        assert_eq!(r.work.evaluations, (window + 2).min(8), "window={window}");
        assert_eq!(
            r.termination,
            if window < 7 {
                SolverTermination::SignificantImprovementTimeLimit
            } else {
                SolverTermination::PlanCompleted
            }
        );
    }
    let policy = threshold(0, 1);
    let r = exercise_scored(
        SolverSequence {
            limits: SolverLimits {
                significant_improvement: Some(&policy),
                ..Default::default()
            },
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
        },
        0,
        |v| v as i64,
    );
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(
        r.termination,
        SolverTermination::SignificantImprovementTimeLimit
    );
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Custom(CustomPhaseTermination::SignificantImprovementTimeLimit)
    );
}

#[test]
fn significant_exact_and_bounded_search_share_the_local_search_anchor() {
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
    ] {
        let policy = threshold(1, 7);
        let r = exercise_scored(
            SolverSequence {
                limits: SolverLimits {
                    significant_improvement: Some(&policy),
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 0,
                        ..Default::default()
                    }),
                    SolverPhase::Exact(ExactPhaseConfig {
                        strategy: strategy.clone(),
                        ..Default::default()
                    }),
                ],
            },
            0,
            |_| 0,
        );
        assert_eq!(r.phases.len(), 2);
        assert_eq!(
            r.termination,
            SolverTermination::SignificantImprovementTimeLimit
        );
        assert_eq!(
            r.phases[1].termination,
            PhaseTermination::Exact(ExactPhaseTermination::SignificantImprovementTimeLimit)
        );
        assert!(r.work.evaluations > 1);
        // A new factory belongs to the existing window and may expire it before
        // exact search evaluates anything.
        let r = exercise_scored(
            SolverSequence {
                limits: SolverLimits {
                    significant_improvement: Some(&policy),
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 0,
                        ..Default::default()
                    }),
                    SolverPhase::Exact(ExactPhaseConfig {
                        strategy: strategy.clone(),
                        ..Default::default()
                    }),
                ],
            },
            100,
            |_| 0,
        );
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(
            r.termination,
            SolverTermination::SignificantImprovementTimeLimit
        );
    }
}

#[test]
fn significant_repair_and_fine_search_keep_late_improvement_but_stop() {
    for fine in [false, true] {
        let now = Cell::new(0);
        let mut clock = || ms(now.get());
        let mut shared = step_control(u64::MAX, &mut clock);
        shared.significant = Some(ScopedSignificantImprovement::new(threshold(0, 1)));
        shared
            .significant
            .as_mut()
            .unwrap()
            .phase_started(SolverPhaseKind::LocalSearch, Duration::ZERO);
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
        let result = crate::ruin_engine::run(
            &p,
            p.initial_assignment(),
            RuinRecreateConfig {
                minimum_ruined: 1,
                maximum_ruined: 1,
                max_attempts: 20,
                seed: 7,
                move_weights: if fine {
                    RepairMoveWeights {
                        change: 1,
                        swap: 0,
                        repair: 0,
                    }
                } else {
                    RepairMoveWeights::default()
                },
                ..Default::default()
            },
            None,
            Some(&mut shared),
            |_, mut a, _, _| {
                // A scored reconstruction kernel: one tick per complete result.
                // The second result strictly improves after the zero window.
                let value = now.get().min(2);
                now.set(now.get() + 1);
                a.values[0] = Some(value as i32);
                Ok(crate::ruin_engine::Repair {
                    assignment: a,
                    score: Some(SimpleScore::new(value as i64)),
                    complete: true,
                    evaluations: 1,
                    constraint_evaluations: 1,
                })
            },
        )
        .unwrap();
        assert_eq!(result.evaluations, 2);
        assert_eq!(result.best.unwrap().1, SimpleScore::new(1));
        assert_eq!(
            result.termination,
            RuinRecreateTermination::SignificantImprovementTimeLimit
        );
        assert_eq!(result.fine_move_attempts, u64::from(fine));
    }
}

#[test]
fn significant_threshold_and_ordinary_stagnation_remain_independent() {
    let policy = threshold(10, 100);
    let r = exercise_scored(
        SolverSequence {
            limits: SolverLimits {
                significant_improvement: Some(&policy),
                max_unimproved_time: Some(ms(0)),
                ..Default::default()
            },
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
        },
        0,
        |v| v as i64,
    );
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    assert_eq!(r.work.evaluations, 1);
}

#[test]
fn significant_shared_timer_interrupts_list_and_mixed_candidate_scans() {
    for mixed in [false, true] {
        let now = Cell::new(0);
        let mut clock = || ms(now.get());
        let mut shared = step_control(u64::MAX, &mut clock);
        shared.significant = Some(ScopedSignificantImprovement::new(threshold(1, 7)));
        shared
            .significant
            .as_mut()
            .unwrap()
            .phase_started(SolverPhaseKind::LocalSearch, Duration::ZERO);
        let lists = ListProblem::new(
            vec![0, 1, 2],
            vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![])],
        )
        .unwrap();
        let config = LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        };
        let (reason, evaluations) = if mixed {
            let p = MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap(),
                lists,
            );
            let mut n = MixedConstraintNetwork::new(1, 2, 3);
            n.add_constraint(
                "clock",
                vec![
                    MixedScope::Basic(0),
                    MixedScope::ListEntity(0),
                    MixedScope::ListEntity(1),
                ],
                |_| {
                    now.set(now.get() + 1);
                    SimpleScore::ZERO
                },
            )
            .unwrap();
            let r = crate::mixed_local_search::mixed_local_search_from_monitored(
                Some(&mut shared),
                &p,
                &n,
                p.initial_assignment(),
                MixedLocalSearchConfig {
                    search: config,
                    joint_moves: true,
                },
            )
            .unwrap();
            (r.termination, r.evaluations)
        } else {
            let mut n = ListConstraintNetwork::new(2, 3);
            n.add_constraint(
                "clock",
                vec![ListScope::Entity(0), ListScope::Entity(1)],
                |_| {
                    now.set(now.get() + 1);
                    SimpleScore::ZERO
                },
            )
            .unwrap();
            let r = crate::list_local_search::list_local_search_from_monitored(
                Some(&mut shared),
                &lists,
                &n,
                lists.initial_assignment(),
                config,
            )
            .unwrap();
            (r.termination, r.evaluations)
        };
        assert_eq!(
            reason,
            LocalSearchTermination::SignificantImprovementTimeLimit
        );
        assert_eq!(evaluations, 3);
    }
}

#[test]
fn significant_score_notifications_are_inert_without_the_policy() {
    let mut clock = || panic!("disabled significant policy sampled time");
    let mut shared = step_control(u64::MAX, &mut clock);
    shared.start_search();
    shared.observe_best_score(&SimpleScore::ZERO);
    shared.record_improvement();
    shared.observe_best_score(&SimpleScore::new(100));
    assert!(shared.stop().is_none());
    assert!(shared.significant.is_none());
}
