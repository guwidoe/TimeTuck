//! Original multi-phase orchestration checks; no upstream method credit.
include!("solver_support/fixture.rs");

#[test]
fn construction_and_two_phases_share_budgets_and_count_each_initialization() {
    for mode in 0..6 {
        let full = execute(mode, plan());
        assert_eq!(full.reason, SolverTermination::PlanCompleted);
        assert_eq!(full.score, Some(SimpleScore::new(2)));
        assert_eq!(full.work.evaluations, 6);
        assert_eq!(full.work.move_evaluations, 3);
        assert_eq!(full.work.completed_steps, 2);
        assert_eq!(full.work.accepted_moves, 2);
        assert_eq!(
            full.phases
                .iter()
                .map(|p| p.work.evaluations)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        for (cap, score, phases) in [
            (0, None, 1),
            (1, Some(0), 1),
            (2, Some(0), 2),
            (3, Some(1), 2),
            (4, Some(1), 3),
            (5, Some(1), 3),
            (6, Some(2), 3),
        ] {
            let r = execute(
                mode,
                SolverConfig {
                    limits: SolverLimits {
                        max_evaluations: cap,
                        ..Default::default()
                    },
                    ..plan()
                },
            );
            assert_eq!(r.work.evaluations, cap);
            assert_eq!(r.score, score.map(SimpleScore::new));
            assert_eq!(r.phases.len(), phases);
            assert_eq!(
                r.reason,
                if cap == 6 {
                    SolverTermination::PlanCompleted
                } else {
                    SolverTermination::EvaluationLimit
                }
            );
        }
        for (cap, evals, score, phases) in [(0, 1, 0, 1), (1, 3, 1, 2), (2, 5, 1, 3), (3, 6, 2, 3)]
        {
            let r = execute(
                mode,
                SolverConfig {
                    limits: SolverLimits {
                        max_move_evaluations: Some(cap),
                        ..Default::default()
                    },
                    ..plan()
                },
            );
            assert_eq!(r.work.move_evaluations, cap);
            assert_eq!(r.work.evaluations, evals);
            assert_eq!(r.score, Some(SimpleScore::new(score)));
            assert_eq!(r.phases.len(), phases);
            assert_eq!(
                r.reason,
                if cap == 3 {
                    SolverTermination::PlanCompleted
                } else {
                    SolverTermination::MoveEvaluationLimit
                }
            );
        }
    }
}
#[test]
fn paired_backends_agree_for_competing_global_and_phase_budgets() {
    for score_cap in 0..=8 {
        for move_cap in 0..=5 {
            for phase_cap in 0..=4 {
                let c = SolverConfig {
                    limits: SolverLimits {
                        max_evaluations: score_cap,
                        max_move_evaluations: Some(move_cap),
                        ..Default::default()
                    },
                    local_search: vec![
                        LocalSearchConfig {
                            max_evaluations: phase_cap,
                            max_steps: 1,
                            ..Default::default()
                        };
                        2
                    ],
                    ..Default::default()
                };
                for pair in [0, 2, 4] {
                    let scoped = execute(pair, c.clone());
                    let stream = execute(pair + 1, c.clone());
                    assert_eq!(scoped, stream);
                    assert!(scoped.work.evaluations <= score_cap);
                    assert!(scoped.work.move_evaluations <= move_cap);
                    let scores: Vec<_> = scoped.phases.iter().filter_map(|p| p.score).collect();
                    assert!(scores.windows(2).all(|w| w[0] <= w[1]));
                }
            }
        }
    }
}
#[test]
fn whole_solve_targets_stop_later_phases_and_phase_targets_do_not() {
    for mode in 0..6 {
        for (target, expected_score, phase_count) in [(0, 0, 1), (1, 1, 2), (2, 2, 3)] {
            let r = execute(
                mode,
                SolverConfig {
                    limits: SolverLimits {
                        best_score_limit: Some(SimpleScore::new(target)),
                        ..Default::default()
                    },
                    ..plan()
                },
            );
            assert_eq!(r.reason, SolverTermination::BestScoreLimit);
            assert_eq!(r.score, Some(SimpleScore::new(expected_score)));
            assert_eq!(r.phases.len(), phase_count);
        }
        let r = execute(
            mode,
            SolverConfig {
                limits: SolverLimits {
                    stop_when_feasible: true,
                    ..Default::default()
                },
                ..plan()
            },
        );
        assert_eq!(r.reason, SolverTermination::FeasibleScore);
        assert_eq!(r.phases.len(), 1);
        let mut c = plan();
        c.local_search[0].best_score_limit = Some(SimpleScore::ZERO);
        let r = execute(mode, c);
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        assert_eq!(r.phases.len(), 3);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        let mut c = plan();
        c.local_search.insert(
            0,
            LocalSearchConfig {
                max_evaluations: 0,
                ..Default::default()
            },
        );
        let r = execute(mode, c);
        assert_eq!(r.phases.len(), 4);
        assert_eq!(r.work.evaluations, 6);
        assert_eq!(r.score, Some(SimpleScore::new(2)));
    }
}
#[test]
fn incomplete_construction_is_retained_and_never_passed_to_local_search() {
    for mode in 0..6 {
        for order in [
            MixedConstructionOrder::BasicThenLists,
            MixedConstructionOrder::ListsThenBasic,
        ] {
            let factories = Cell::new(0);
            let c = SolverConfig {
                construction: ConstructionConfig {
                    max_evaluations: 2,
                    ..Default::default()
                },
                ..plan()
            };
            let r = run(mode, false, c, order, &factories, None, || {}).unwrap();
            if mode >= 4 {
                assert_eq!(r.reason, SolverTermination::IncompleteConstruction);
                assert_eq!(r.phases.len(), 1);
                assert!(!r.complete);
                assert_eq!(r.values.iter().filter(|v| v.is_some()).count(), 1);
            } else {
                assert_eq!(r.reason, SolverTermination::PlanCompleted);
                assert!(r.complete);
                assert_eq!(r.phases.len(), 3);
            }
            let r = run(
                mode,
                false,
                SolverConfig {
                    limits: SolverLimits {
                        max_evaluations: 2,
                        ..Default::default()
                    },
                    ..plan()
                },
                order,
                &Cell::new(0),
                None,
                || {},
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::EvaluationLimit);
            assert_eq!(r.phases.len(), 1);
            assert_eq!(r.work.evaluations, 2);
        }
    }
}
#[test]
fn cancellation_reaches_all_phases_and_preserves_the_last_scored_best() {
    for mode in 0..6 {
        for at in 0..=6 {
            let token = CancellationToken::new();
            let clone = token.clone();
            if at == 0 {
                token.cancel();
            }
            let calls = Cell::new(0);
            let factories = Cell::new(0);
            let mut c = plan();
            c.limits.cancellation = Some(&token);
            c.local_search[0].cancellation = Some(&clone);
            let r = run(
                mode,
                true,
                c,
                MixedConstructionOrder::BasicThenLists,
                &factories,
                None,
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == at {
                        token.cancel();
                    }
                },
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::Cancelled);
            assert_eq!(r.work.evaluations, at as u64);
            assert_eq!(calls.get(), at);
            assert_eq!(
                r.score,
                match at {
                    0 => None,
                    1 | 2 => Some(SimpleScore::ZERO),
                    3..=5 => Some(SimpleScore::new(1)),
                    _ => Some(SimpleScore::new(2)),
                }
            );
            assert_eq!(
                r.phases.len(),
                if at <= 1 {
                    1
                } else if at <= 3 {
                    2
                } else {
                    3
                }
            );
        }
    }
}
#[test]
fn invalid_future_settings_fail_before_any_scoring_or_factory_work() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        let other = CancellationToken::new();
        token.cancel();
        let mut c = plan();
        c.limits.max_evaluations = 0;
        c.limits.cancellation = Some(&token);
        c.local_search[1].cancellation = Some(&other);
        let factories = Cell::new(0);
        assert_eq!(
            run(
                mode,
                true,
                c,
                MixedConstructionOrder::BasicThenLists,
                &factories,
                None,
                || panic!("invalid plan scored")
            ),
            Err(Failure::Token(2))
        );
        let mut c = plan();
        c.local_search[1].acceptance = Acceptance::LateAcceptance {
            history_size: 0,
            hill_climbing: false,
        };
        assert_eq!(
            run(
                mode,
                true,
                c,
                MixedConstructionOrder::BasicThenLists,
                &factories,
                None,
                || panic!("invalid plan scored")
            ),
            Err(Failure::Acceptance(2))
        );
        assert_eq!(factories.get(), 0);
    }
}
#[test]
fn graph_factory_errors_identify_the_phase_and_skip_later_work() {
    for mode in [1, 3, 5] {
        for fail in 1..=3 {
            let factories = Cell::new(0);
            let callbacks = Cell::new(0);
            assert_eq!(
                run(
                    mode,
                    true,
                    plan(),
                    MixedConstructionOrder::BasicThenLists,
                    &factories,
                    Some(fail),
                    || callbacks.set(callbacks.get() + 1)
                ),
                Err(Failure::Phase(fail - 1))
            );
            assert_eq!(factories.get(), fail);
            assert_eq!(callbacks.get(), [0, 1, 3][fail - 1]);
        }
    }
}

#[test]
fn empty_plans_and_phase_local_limits_preserve_results_without_stopping_the_plan() {
    for mode in 0..6 {
        let mut c = plan();
        c.local_search.clear();
        let r = execute(mode, c.clone());
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(r.phases.len(), 1);
        c.construction.max_evaluations = 0;
        let r = execute(mode, c);
        assert!(r.complete);
        assert_eq!(r.score, None);
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        let mut c = plan();
        c.construction.max_evaluations = 0;
        let r = execute(mode, c);
        assert_eq!(r.work.evaluations, 5);
        assert_eq!(r.score, Some(SimpleScore::new(2)));
        let mut c = plan();
        c.local_search[0].max_move_evaluations = Some(0);
        let r = execute(mode, c);
        assert_eq!(
            r.phases[1].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::MoveEvaluationLimit)
        );
        assert_eq!(r.work.evaluations, 4);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        let c = SolverConfig {
            limits: SolverLimits {
                max_evaluations: 5,
                max_move_evaluations: Some(2),
                ..Default::default()
            },
            ..plan()
        };
        assert_eq!(execute(mode, c).reason, SolverTermination::EvaluationLimit);
    }
}
#[test]
fn impossible_models_and_invalid_networks_keep_their_reasons_at_zero_budget() {
    let basic = FiniteProblem::<usize>::new(vec![Variable::new(vec![])]).unwrap();
    let lists = ListProblem::<usize>::new(vec![7], vec![]).unwrap();
    let c = SolverConfig::<SimpleScore> {
        limits: SolverLimits {
            max_evaluations: 0,
            ..Default::default()
        },
        ..Default::default()
    };
    let b = solve(&basic, &ConstraintNetwork::new(1), c.clone()).unwrap();
    let l = solve_list(&lists, &ListConstraintNetwork::new(0, 1), c.clone()).unwrap();
    let p = MixedProblem::new(basic, lists);
    let m = solve_mixed(
        &p,
        &MixedConstraintNetwork::new(1, 0, 1),
        MixedSolverConfig {
            limits: c.limits,
            ..Default::default()
        },
    )
    .unwrap();
    for (reason, work, count) in [
        (b.termination, b.work, b.phases.len()),
        (l.termination, l.work, l.phases.len()),
        (m.termination, m.work, m.phases.len()),
    ] {
        assert_eq!(reason, SolverTermination::NoCompleteAssignment);
        assert_eq!(work, SolverWork::default());
        assert_eq!(count, 1);
    }
    assert!(matches!(
        solve(p.basic(), &ConstraintNetwork::new(0), c.clone()),
        Err(SolverError::Phase { phase: 0, .. })
    ));
    assert!(matches!(
        solve_list(p.lists(), &ListConstraintNetwork::new(1, 1), c.clone()),
        Err(SolverError::Phase { phase: 0, .. })
    ));
    assert!(matches!(
        solve_mixed(
            &p,
            &MixedConstraintNetwork::new(0, 0, 1),
            MixedSolverConfig {
                limits: c.limits,
                ..Default::default()
            }
        ),
        Err(SolverError::Phase { phase: 0, .. })
    ));
}
#[test]
fn mixed_phases_keep_individual_joint_move_settings_and_escape_a_coupled_trap() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0usize, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(
            vec![7usize],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let value = |basic: Option<usize>, owner: Option<usize>| {
        SimpleScore::new(match (basic, owner) {
            (Some(1), Some(1)) => 10,
            (Some(0), Some(0)) => 0,
            _ => -1,
        })
    };
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |v| {
            let (MixedFact::Basic(b), MixedFact::ListValue { element, .. }) = (&v[0], &v[1]) else {
                panic!()
            };
            value(b.copied(), element.as_ref().map(|v| v.position.entity))
        },
    )
    .unwrap();
    let c = MixedSolverConfig {
        local_search: vec![
            MixedLocalSearchConfig {
                joint_moves: false,
                ..Default::default()
            },
            MixedLocalSearchConfig {
                joint_moves: true,
                search: LocalSearchConfig {
                    max_steps: 1,
                    ..Default::default()
                },
            },
        ],
        ..Default::default()
    };
    let scoped = solve_mixed(&p, &n, c.clone()).unwrap();
    let stream = solve_mixed_stream(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::<usize, usize>::new(&mut b);
            let j = b.join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())?;
            b.score("coupled", &j, |((_, b), v)| {
                value(*b, v.position.map(|p| p.entity))
            })?;
            Ok((b.build(), s))
        },
        c,
    )
    .unwrap();
    // The joint move changes the basic row, then the list-value row. The stream
    // join scores both updates, whereas the scoped constraint scores once.
    let mut expected_stream = scoped.clone();
    expected_stream.work.constraint_evaluations += 1;
    expected_stream.phases[2].work.constraint_evaluations += 1;
    assert_eq!(stream, expected_stream);
    assert_eq!(scoped.work.constraint_evaluations, 8);
    assert_eq!(stream.work.constraint_evaluations, 9);
    assert_eq!(
        scoped.phases[1].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::NoAcceptedMove)
    );
    assert_eq!(scoped.phases[1].score, Some(SimpleScore::ZERO));
    assert_eq!(scoped.score, Some(SimpleScore::new(10)));
    assert_eq!(scoped.assignment.basic().values(), &[Some(1)]);
    assert_eq!(scoped.assignment.lists().location(&7).unwrap().entity, 1);
    let exact = exhaustive_mixed_search(
        &p,
        |a| {
            value(
                a.basic().values()[0],
                a.lists().location(&7).map(|p| p.entity),
            )
        },
        4,
    );
    assert!(exact.is_optimal());
    assert_eq!(exact.best.unwrap().score, scoped.score.unwrap());
}
#[test]
fn a_controller_thread_can_cancel_the_second_improvement_phase() {
    use std::{sync::mpsc, time::Duration};
    for mode in 0..6 {
        let token = CancellationToken::new();
        let worker_token = token.clone();
        let (entered_tx, entered_rx) = mpsc::sync_channel(0);
        let (release_tx, release_rx) = mpsc::sync_channel(0);
        let worker = std::thread::spawn(move || {
            let mut c = plan();
            c.limits.cancellation = Some(&worker_token);
            let calls = Cell::new(0);
            run(
                mode,
                true,
                c,
                MixedConstructionOrder::BasicThenLists,
                &Cell::new(0),
                None,
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == 5 {
                        entered_tx.send(()).unwrap();
                        release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                    }
                },
            )
            .unwrap()
        });
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        token.cancel();
        release_tx.send(()).unwrap();
        let r = worker.join().unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(r.phases.len(), 3);
        assert_eq!(r.work.evaluations, 5);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
    }
}

#[test]
fn zero_shared_time_scores_initial_state_only_in_all_six_entry_points() {
    use std::time::Duration;
    for mode in 0..6 {
        for initialized in [false, true] {
            let factories = Cell::new(0);
            let calls = Cell::new(0);
            let r = run(
                mode,
                initialized,
                SolverConfig {
                    limits: SolverLimits {
                        max_time: Some(Duration::ZERO),
                        ..Default::default()
                    },
                    ..plan()
                },
                MixedConstructionOrder::BasicThenLists,
                &factories,
                None,
                || calls.set(calls.get() + 1),
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::TimeLimit);
            assert_eq!(r.complete, initialized);
            assert_eq!(r.work.evaluations, 1);
            assert_eq!(r.work.move_evaluations, 0);
            assert_eq!(calls.get(), 1);
            assert_eq!(factories.get(), usize::from(mode % 2 == 1));
            assert_eq!(r.phases.len(), 1);
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::Construction(ConstructionTermination::TimeLimit)
            );
        }
    }
}

#[test]
fn phase_local_zero_time_allows_later_improvement_when_construction_is_complete() {
    use std::time::Duration;
    for mode in 0..6 {
        for initialized in [false, true] {
            let mut c = plan();
            c.construction.max_time = Some(Duration::ZERO);
            c.local_search[0].max_time = Some(Duration::ZERO);
            let r = run(
                mode,
                initialized,
                c,
                MixedConstructionOrder::BasicThenLists,
                &Cell::new(0),
                None,
                || {},
            )
            .unwrap();
            if initialized {
                assert_eq!(r.reason, SolverTermination::PlanCompleted);
                assert_eq!(r.phases.len(), 3);
                assert_eq!(r.score, Some(SimpleScore::new(1)));
                assert_eq!(r.work.evaluations, 4);
                assert_eq!(r.work.move_evaluations, 1);
                assert_eq!(
                    r.phases[1].termination,
                    PhaseTermination::LocalSearch(LocalSearchTermination::TimeLimit)
                );
            } else {
                assert_eq!(r.reason, SolverTermination::IncompleteConstruction);
                assert_eq!(r.phases.len(), 1);
                assert_eq!(r.work.evaluations, 1);
            }
        }
    }
}

#[test]
fn zero_shared_time_preserves_validation_budget_and_cancellation_precedence() {
    use std::time::Duration;
    for mode in 0..6 {
        let token = CancellationToken::new();
        token.cancel();
        for budget in [0, 100] {
            let r = run(
                mode,
                false,
                SolverConfig {
                    limits: SolverLimits {
                        cancellation: Some(&token),
                        max_time: Some(Duration::ZERO),
                        max_evaluations: budget,
                        ..Default::default()
                    },
                    ..plan()
                },
                MixedConstructionOrder::BasicThenLists,
                &Cell::new(0),
                None,
                || panic!("no callbacks"),
            )
            .unwrap();
            assert_eq!(r.work.evaluations, 0);
            assert_eq!(
                r.reason,
                if budget == 0 {
                    SolverTermination::EvaluationLimit
                } else {
                    SolverTermination::Cancelled
                }
            );
        }
        let mut c = plan();
        c.limits.max_time = Some(Duration::ZERO);
        c.local_search[1].acceptance = Acceptance::LateAcceptance {
            history_size: 0,
            hill_climbing: false,
        };
        let factories = Cell::new(0);
        assert_eq!(
            run(
                mode,
                false,
                c,
                MixedConstructionOrder::BasicThenLists,
                &factories,
                None,
                || panic!("invalid plan callback")
            ),
            Err(Failure::Acceptance(2))
        );
        assert_eq!(factories.get(), 0);
    }
}

#[test]
fn shared_zero_stagnation_excludes_construction_and_stops_all_six_search_paths() {
    use std::time::Duration;
    for mode in 0..6 {
        for initialized in [false, true] {
            for order in [
                MixedConstructionOrder::BasicThenLists,
                MixedConstructionOrder::ListsThenBasic,
            ] {
                let factories = Cell::new(0);
                let mut c = plan();
                c.limits.max_unimproved_time = Some(Duration::ZERO);
                let r = run(mode, initialized, c, order, &factories, None, || {}).unwrap();
                assert!(r.complete);
                assert_eq!(r.reason, SolverTermination::UnimprovedTimeLimit);
                assert_eq!(r.phases.len(), 2);
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Construction(ConstructionTermination::Completed)
                );
                assert_eq!(
                    r.phases[1].termination,
                    PhaseTermination::LocalSearch(LocalSearchTermination::UnimprovedTimeLimit)
                );
                assert_eq!(r.phases[1].work.evaluations, 1);
                assert_eq!(r.phases[1].work.move_evaluations, 0);
                assert_eq!(r.phases[0].score, r.score);
                assert_eq!(factories.get(), if mode % 2 == 1 { 2 } else { 0 });
            }
        }
    }
}

#[test]
fn shared_stagnation_stays_inactive_without_started_local_search() {
    use std::time::Duration;
    for mode in 0..6 {
        for construction_only in [false, true] {
            let mut c = plan();
            c.limits.max_unimproved_time = Some(Duration::ZERO);
            if construction_only {
                c.local_search.clear();
            } else {
                c.construction.max_evaluations = 1;
            }
            let r = run(
                mode,
                false,
                c,
                MixedConstructionOrder::BasicThenLists,
                &Cell::new(0),
                None,
                || {},
            )
            .unwrap();
            assert_eq!(r.phases.len(), 1);
            assert_eq!(
                r.reason,
                if construction_only {
                    SolverTermination::PlanCompleted
                } else {
                    SolverTermination::IncompleteConstruction
                }
            );
        }
        let mut c = plan();
        c.limits.max_unimproved_time = Some(Duration::ZERO);
        c.limits.best_score_limit = Some(SimpleScore::ZERO);
        let r = run(
            mode,
            true,
            c,
            MixedConstructionOrder::BasicThenLists,
            &Cell::new(0),
            None,
            || {},
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::BestScoreLimit);
        assert_eq!(r.phases.len(), 1);
    }
}

#[test]
fn shared_stagnation_preserves_validation_and_zero_budget_precedence() {
    use std::time::Duration;
    for mode in 0..6 {
        let mut c = plan();
        c.limits.max_unimproved_time = Some(Duration::ZERO);
        c.local_search[1].acceptance = Acceptance::LateAcceptance {
            history_size: 0,
            hill_climbing: false,
        };
        assert_eq!(
            run(
                mode,
                true,
                c,
                MixedConstructionOrder::BasicThenLists,
                &Cell::new(0),
                None,
                || panic!("invalid plan")
            ),
            Err(Failure::Acceptance(2))
        );
        let mut c = plan();
        c.limits.max_unimproved_time = Some(Duration::ZERO);
        c.limits.max_evaluations = 0;
        let r = run(
            mode,
            true,
            c,
            MixedConstructionOrder::BasicThenLists,
            &Cell::new(0),
            None,
            || panic!("zero work"),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::EvaluationLimit);
        assert_eq!(r.work.evaluations, 0);
    }
}

#[test]
fn shared_unimproved_steps_count_plateaus_across_all_six_backends() {
    for mode in 0..6 {
        for cap in 0..=5 {
            for phase_steps in [1, 2, 5] {
                let mut c = plan();
                c.limits.max_unimproved_steps = Some(cap);
                c.local_search = vec![
                    LocalSearchConfig {
                        max_steps: phase_steps,
                        acceptance: Acceptance::HillClimbing,
                        ..Default::default()
                    };
                    8
                ];
                let r = run_scored(
                    (mode, true, MixedConstructionOrder::BasicThenLists),
                    c,
                    &Cell::new(0),
                    None,
                    || {},
                    |n| SimpleScore::new(n.min(1)),
                )
                .unwrap();
                assert_eq!(
                    r.reason,
                    SolverTermination::UnimprovedStepLimit,
                    "mode {mode}, cap {cap}, phase steps {phase_steps}"
                );
                assert_eq!(r.work.completed_steps, if cap == 0 { 0 } else { cap + 1 });
                assert_eq!(r.work.accepted_moves, r.work.completed_steps);
                assert_eq!(r.score, Some(SimpleScore::new(i64::from(cap > 0))));
                assert_eq!(
                    r.phases.iter().map(|p| p.work.completed_steps).sum::<u64>(),
                    r.work.completed_steps
                );
                assert!(r.complete);
            }
        }
    }
}

#[test]
fn shared_steps_count_rejected_fallback_and_recovery_without_reset() {
    for mode in 0..6 {
        let mut c = plan();
        c.limits.max_unimproved_steps = Some(2);
        c.local_search = vec![
            LocalSearchConfig {
                max_steps: 20,
                rejected_moves: RejectedMovePolicy::BestScore,
                ..Default::default()
            };
            3
        ];
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &Cell::new(0),
            None,
            || {},
            |n| SimpleScore::new(-n),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::UnimprovedStepLimit);
        assert_eq!(r.work.completed_steps, 2);
        assert_eq!(r.work.accepted_moves, 1);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert!(r.values.iter().all(|v| *v == Some(0)));
        assert_eq!(r.phases.len(), 2);
    }
}

#[test]
fn phases_without_selected_moves_do_not_consume_shared_steps() {
    for mode in 0..6 {
        for limit in [1, u64::MAX] {
            let mut c = plan();
            c.limits.max_unimproved_steps = Some(limit);
            c.local_search = vec![LocalSearchConfig::default(); 6];
            c.local_search[0].max_steps = 0;
            let r = run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &Cell::new(0),
                None,
                || {},
                |_| SimpleScore::ZERO,
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::PlanCompleted);
            assert_eq!(r.work.completed_steps, 0);
            assert_eq!(r.phases.len(), 7);
            assert_eq!(
                r.phases[1].termination,
                PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit)
            );
            assert!(r.phases[2..].iter().all(|p| p.termination
                == PhaseTermination::LocalSearch(LocalSearchTermination::NoAcceptedMove)));
        }
    }
}

#[test]
fn local_unimproved_step_caps_leave_shared_progress_for_later_phases() {
    for mode in 0..6 {
        let mut c = plan();
        c.limits.max_unimproved_steps = Some(3);
        c.local_search = vec![
            LocalSearchConfig {
                max_steps: 100,
                max_unimproved_steps: Some(1),
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            };
            5
        ];
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
        assert_eq!(r.phases.len(), 4);
        assert!(r.phases[1..].iter().all(|p| p.work.completed_steps == 1));
    }
}

#[test]
fn zero_shared_step_limit_ignores_construction_and_pre_search_goals() {
    for mode in 0..6 {
        for initialized in [false, true] {
            for construction_only in [false, true] {
                let mut c = plan();
                c.limits.max_unimproved_steps = Some(0);
                if construction_only {
                    c.local_search.clear();
                }
                let r = run(
                    mode,
                    initialized,
                    c,
                    MixedConstructionOrder::BasicThenLists,
                    &Cell::new(0),
                    None,
                    || {},
                )
                .unwrap();
                assert!(r.complete);
                assert_eq!(r.work.completed_steps, 0);
                assert_eq!(r.phases.len(), if construction_only { 1 } else { 2 });
                assert_eq!(
                    r.reason,
                    if construction_only {
                        SolverTermination::PlanCompleted
                    } else {
                        SolverTermination::UnimprovedStepLimit
                    }
                );
            }
        }
        let mut c = plan();
        c.limits.max_unimproved_steps = Some(0);
        c.limits.best_score_limit = Some(SimpleScore::ZERO);
        let r = run(
            mode,
            true,
            c,
            MixedConstructionOrder::ListsThenBasic,
            &Cell::new(0),
            None,
            || {},
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::BestScoreLimit);
        assert_eq!(r.phases.len(), 1);
    }
}

#[test]
fn shared_steps_keep_cancellation_validation_and_time_precedence() {
    use std::time::Duration;
    for mode in 0..6 {
        let token = CancellationToken::new();
        let calls = Cell::new(0);
        let mut c = plan();
        c.limits.max_unimproved_steps = Some(0);
        c.limits.cancellation = Some(&token);
        let r = run(
            mode,
            true,
            c,
            MixedConstructionOrder::BasicThenLists,
            &Cell::new(0),
            None,
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 2 {
                    token.cancel();
                }
            },
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        let mut c = plan();
        c.limits.max_unimproved_steps = Some(0);
        c.limits.max_unimproved_time = Some(Duration::ZERO);
        let r = run(
            mode,
            true,
            c,
            MixedConstructionOrder::BasicThenLists,
            &Cell::new(0),
            None,
            || {},
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::UnimprovedTimeLimit);
        let mut c = plan();
        c.limits.max_unimproved_steps = Some(0);
        c.local_search[1].acceptance = Acceptance::LateAcceptance {
            history_size: 0,
            hill_climbing: false,
        };
        assert_eq!(
            run(
                mode,
                true,
                c,
                MixedConstructionOrder::BasicThenLists,
                &Cell::new(0),
                None,
                || panic!("invalid plan")
            ),
            Err(Failure::Acceptance(2))
        );
    }
}

#[path = "support/solver_exact_cases.rs"]
mod exact_cases;

#[path = "support/solver_step_counting_cases.rs"]
mod step_counting_cases;

#[path = "support/solver_great_deluge_cases.rs"]
mod great_deluge_cases;

#[path = "support/solver_manager_integration.rs"]
mod manager_integration;

#[path = "support/solver_observer_cases.rs"]
mod observer_cases;

#[path = "support/solver_construction_cases.rs"]
mod construction_cases;

#[path = "support/solver_initialization_cases.rs"]
mod initialization_cases;

#[path = "support/solver_sequence_cases.rs"]
mod sequence_cases;

#[path = "support/solver_incomplete_handoff_cases.rs"]
mod incomplete_handoff_cases;

#[path = "support/solver_bounded_cases.rs"]
mod bounded_cases;

#[test]
fn borrowed_step_conditions_restart_in_each_local_phase_in_all_backends() {
    let children = [
        StepTermination::Steps(2),
        StepTermination::UnimprovedSteps(1),
    ];
    let condition = StepTermination::All(&children);
    for mode in 0..6 {
        let factories = Cell::new(0);
        let c = SolverConfig {
            local_search: vec![
                LocalSearchConfig {
                    step_termination: Some(&condition),
                    acceptance: Acceptance::HillClimbing,
                    assert_scores: true,
                    ..Default::default()
                };
                2
            ],
            ..Default::default()
        };
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &factories,
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        assert_eq!(r.phases.len(), 3);
        for phase in &r.phases[1..] {
            assert_eq!(
                phase.termination,
                PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
            );
            assert_eq!(phase.work.completed_steps, 2);
            assert_eq!(phase.work.accepted_moves, 2);
        }
        assert_eq!(r.work.completed_steps, 4);
    }
}
