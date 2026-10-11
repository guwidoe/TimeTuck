//! Original live work-count termination regressions; no upstream method credit.
#![allow(dead_code)]
include!("solver_support/fixture.rs");

fn candidate_limit(n: u64) -> SearchTermination<'static> {
    SearchTermination::Condition(SearchLimit::MoveEvaluations(n))
}
fn family_limit(kind: MoveEvaluationKind, limit: u64) -> SearchTermination<'static> {
    SearchTermination::Condition(SearchLimit::MoveEvaluationsOf { kind, limit })
}

#[test]
fn exact_progress_and_saturating_offline_counts() {
    let counts: MoveEvaluationCounts = [
        (MoveEvaluationKind::Change, u64::MAX - 2),
        (MoveEvaluationKind::Swap, 2),
    ]
    .into_iter()
    .collect();
    let progress = SearchProgress {
        move_evaluations: counts,
        ..Default::default()
    };
    assert!(candidate_limit(u64::MAX).status(progress).unwrap().reached);
    let state = family_limit(MoveEvaluationKind::Change, u64::MAX)
        .status(progress)
        .unwrap();
    assert_eq!(state.progress, Some(1.0));
    assert!(!state.reached); // Float rounding must not decide termination.
    let saturated: MoveEvaluationCounts = counts
        .iter()
        .chain([
            (MoveEvaluationKind::Change, 10),
            (MoveEvaluationKind::Swap, u64::MAX),
        ])
        .collect();
    assert_eq!(saturated.total(), u64::MAX);
    assert_eq!(saturated.get(MoveEvaluationKind::Change), u64::MAX);
    assert_eq!(saturated.get(MoveEvaluationKind::Swap), u64::MAX);
    assert_eq!(saturated.get(MoveEvaluationKind::Custom), 0);
    assert!(
        candidate_limit(0)
            .status(SearchProgress::default())
            .unwrap()
            .reached
    );
    assert!(
        family_limit(MoveEvaluationKind::Custom, 0)
            .status(SearchProgress::default())
            .unwrap()
            .reached
    );
}

#[test]
fn zero_work_conditions_stop_before_factories_and_initial_scoring_in_all_six_paths() {
    for condition in [
        candidate_limit(0),
        family_limit(MoveEvaluationKind::ExactAssignment, 0),
    ] {
        for mode in 0..6 {
            let factories = Cell::new(0);
            let r = run_scored(
                (mode, false, MixedConstructionOrder::BasicThenLists),
                SolverSequence {
                    limits: SolverLimits {
                        termination: Some(&condition),
                        ..Default::default()
                    },
                    phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
                },
                &factories,
                None,
                || panic!("must not score"),
                SimpleScore::new,
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
            assert_eq!(r.work, SolverWork::default());
            assert!(r.phases.is_empty());
            assert_eq!(factories.get(), 0);
        }
    }
}

#[test]
fn any_and_all_candidate_quotas_span_local_phases_without_metrics_in_all_six_paths() {
    for mode in 0..6 {
        let leaves = [candidate_limit(3), candidate_limit(5)];
        for (condition, wanted, phases) in [
            (SearchTermination::Any(&leaves), 3, 2),
            (SearchTermination::All(&leaves), 5, 3),
        ] {
            let r = run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                SolverSequence {
                    limits: SolverLimits {
                        termination: Some(&condition),
                        ..Default::default()
                    },
                    phases: vec![
                        SolverPhase::LocalSearch(LocalSearchConfig {
                            max_steps: 2,
                            acceptance: Acceptance::HillClimbing,
                            selection: MoveSelection::FirstAccepted,
                            ..Default::default()
                        });
                        4
                    ],
                },
                &Cell::new(0),
                None,
                || {},
                |_| SimpleScore::ZERO,
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
            assert_eq!(r.work.move_evaluations, wanted, "mode {mode}");
            assert_eq!(r.phases.len(), phases, "mode {mode}");
            assert_eq!(r.work.evaluations, wanted + phases as u64);
            assert!(r.complete);
        }
    }
}

#[test]
fn construction_keeps_or_discards_interrupted_prefix_under_work_condition() {
    for mode in 0..6 {
        for interruption in [
            ConstructionInterruption::KeepBestPrefix,
            ConstructionInterruption::DiscardIncompleteStep,
        ] {
            let condition = candidate_limit(1);
            let r = run_scored(
                (mode, false, MixedConstructionOrder::BasicThenLists),
                SolverSequence {
                    limits: SolverLimits {
                        termination: Some(&condition),
                        ..Default::default()
                    },
                    phases: vec![
                        SolverPhase::Construction(ConstructionConfig {
                            interruption,
                            ..Default::default()
                        }),
                        SolverPhase::Exact(ExactPhaseConfig::default()),
                    ],
                },
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
            assert_eq!(r.work.move_evaluations, 1);
            assert_eq!(r.work.evaluations, 2);
            assert_eq!(r.phases.len(), 1);
            assert_eq!(
                r.work.construction_steps,
                u64::from(interruption == ConstructionInterruption::KeepBestPrefix)
            );
        }
    }
}

#[test]
fn exact_candidate_quota_stops_enumeration_but_final_exhaustion_wins() {
    for mode in 0..6 {
        let condition = family_limit(MoveEvaluationKind::ExactAssignment, 1);
        let r = run_scored(
            (mode, false, MixedConstructionOrder::BasicThenLists),
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            },
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
        assert_eq!(r.work.move_evaluations, 1);
        assert_eq!(r.work.evaluations, 1);
        assert!(r.complete);
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let condition = candidate_limit(2);
    let r = solve(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits {
                termination: Some(&condition),
                ..Default::default()
            },
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
        },
    )
    .unwrap();
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.termination, SolverTermination::Optimal);
}

#[test]
fn nested_family_quotas_count_rejected_trials_without_double_counting_metrics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let leaves = [
        family_limit(MoveEvaluationKind::Swap, 1),
        family_limit(MoveEvaluationKind::Composite, 1),
    ];
    let inner = SearchTermination::All(&leaves);
    let outer = [inner, candidate_limit(100)];
    let condition = SearchTermination::Any(&outer);
    for metrics_enabled in [false, true] {
        let metrics = SolverMetrics::default();
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            vec![
                Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                Move::Swap { left: 0, right: 1 },
                Move::Composite(vec![
                    Move::Change {
                        variable: 0,
                        value: Some(1),
                    },
                    Move::Change {
                        variable: 1,
                        value: Some(1),
                    },
                ]),
                // Never reached, even though all earlier candidates are rejected.
                Move::Change {
                    variable: 99,
                    value: Some(1),
                },
            ]
        };
        let mut ext = SolverExtensions {
            metrics: metrics_enabled.then_some(&metrics),
            ..Default::default()
        };
        ext.move_providers.bind(0, &mut provider);
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    assert_phase_scores: true,
                    ..Default::default()
                },
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    assert_scores: true,
                    ..Default::default()
                })],
            },
            ext,
        )
        .unwrap();
        assert_eq!(r.termination, SolverTermination::Condition);
        assert_eq!(r.work.move_evaluations, 4);
        assert_eq!(r.work.evaluations, 5);
        assert_eq!(r.work.accepted_moves, 0);
        assert_eq!(r.assignment, p.initial_assignment());
        if metrics_enabled {
            let counts = metrics.snapshot().latest.unwrap().move_evaluations;
            assert_eq!(counts.total(), 4);
            assert_eq!(counts.get(MoveEvaluationKind::Change), 2);
            assert_eq!(counts.get(MoveEvaluationKind::Swap), 1);
            assert_eq!(counts.get(MoveEvaluationKind::Composite), 1);
        }
    }
}

#[test]
fn and_with_unreached_step_leaf_does_not_turn_candidate_quota_into_hard_cap() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let leaves = [
        candidate_limit(1),
        SearchTermination::Condition(SearchLimit::Steps(3)),
    ];
    let condition = SearchTermination::All(&leaves);
    for hard_cap in [None, Some(2)] {
        let r = solve(
            &p,
            &n,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    max_move_evaluations: hard_cap,
                    ..Default::default()
                },
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 10,
                    selection: MoveSelection::FirstAccepted,
                    acceptance: Acceptance::HillClimbing,
                    ..Default::default()
                })],
            },
        )
        .unwrap();
        assert_eq!(r.work.move_evaluations, hard_cap.unwrap_or(3));
        assert_eq!(
            r.termination,
            if hard_cap.is_some() {
                SolverTermination::MoveEvaluationLimit
            } else {
                SolverTermination::Condition
            }
        );
    }
}

#[test]
fn custom_operations_poll_work_condition_before_next_command_and_ignore_diagnostics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let condition = family_limit(MoveEvaluationKind::Custom, 2);
    let mut command = |_,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i32>,
        Move<i32>,
        SimpleScore,
        ScoringError,
    >| {
        let movement = Move::Change {
            variable: 0,
            value: Some(1),
        };
        assert!(c.evaluate(&movement, |_, _| ())?.is_some());
        c.assert_consistent()?;
        assert!(c.execute(&movement)?.is_some());
        assert!(c.execute(&movement)?.is_none());
        assert_eq!(c.termination(), Some(CustomPhaseTermination::Condition));
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits {
                termination: Some(&condition),
                assert_phase_scores: true,
                ..Default::default()
            },
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
        },
        SolverExtensions {
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.work.evaluations, 3);
}

#[test]
fn repair_family_quotas_are_cooperative_at_rebuild_boundaries() {
    for mode in 0..6 {
        for (kind, weights) in [
            (
                MoveEvaluationKind::Reconstruction,
                RepairMoveWeights {
                    repair: 1,
                    change: 0,
                    swap: 0,
                },
            ),
            (
                MoveEvaluationKind::FineRepair,
                RepairMoveWeights {
                    repair: 0,
                    change: 1,
                    swap: 0,
                },
            ),
        ] {
            let condition = family_limit(kind, 1);
            let r = run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                SolverSequence {
                    limits: SolverLimits {
                        termination: Some(&condition),
                        ..Default::default()
                    },
                    phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                        move_weights: weights,
                        minimum_ruined: usize::MAX,
                        maximum_ruined: usize::MAX,
                        max_attempts: 20,
                        seed: 71,
                        ..Default::default()
                    })],
                },
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
            )
            .unwrap();
            assert_eq!(
                r.reason,
                SolverTermination::Condition,
                "mode {mode}, {kind:?}"
            );
            assert_eq!(r.work.repair_attempts + r.work.fine_move_attempts, 1);
            assert!(r.work.move_evaluations >= 1);
            if kind == MoveEvaluationKind::FineRepair {
                assert_eq!(r.work.move_evaluations, 1);
            }
        }
    }
}

#[test]
fn bounded_tree_counts_nodes_and_stops_before_claiming_optimality() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 3]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let condition = family_limit(MoveEvaluationKind::ExactNode, 3);
    let r = solve(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits {
                termination: Some(&condition),
                ..Default::default()
            },
            phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
                ..Default::default()
            })],
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.work.move_evaluations, 3);
    assert_eq!(r.work.evaluations, 3);
}

struct ZeroCalculator<C>(std::marker::PhantomData<C>);
impl<A, C> IncrementalCalculator<A> for ZeroCalculator<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        _: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}

#[test]
fn calculator_backends_observe_same_work_condition_in_construction_local_and_exact() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap();
    let lists = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![0, 1, 2]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    let bn = ConstraintNetwork::<_, SimpleScore>::new(2);
    let ln = ListConstraintNetwork::<_, SimpleScore>::new(2, 3);
    let mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(2, 2, 3);
    let condition = candidate_limit(2);
    let phases = [
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 20,
            acceptance: Acceptance::HillClimbing,
            selection: MoveSelection::FirstAccepted,
            ..Default::default()
        }),
        SolverPhase::Exact(ExactPhaseConfig::default()),
        SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            ..Default::default()
        }),
    ];
    fn same<A: Eq + std::fmt::Debug>(
        a: SolverResult<A, SimpleScore>,
        b: SolverResult<A, SimpleScore>,
    ) {
        assert_eq!(a.termination, SolverTermination::Condition);
        assert_eq!(a.termination, b.termination);
        assert_eq!(a.work.move_evaluations, 2);
        assert_eq!(a.work.move_evaluations, b.work.move_evaluations);
        assert_eq!(a.work.evaluations, b.work.evaluations);
        assert_eq!(a.assignment, b.assignment);
        assert_eq!(a.score, b.score);
        assert_eq!(a.complete, b.complete);
    }
    for phase in phases {
        let c = SolverSequence {
            limits: SolverLimits {
                termination: Some(&condition),
                ..Default::default()
            },
            phases: vec![phase.clone()],
        };
        same(
            solve_calculator(
                &basic,
                &|| ZeroCalculator::<Vec<usize>>(Default::default()),
                c.clone(),
            )
            .unwrap(),
            solve(&basic, &bn, c.clone()).unwrap(),
        );
        same(
            solve_list_calculator(
                &lists,
                &|| ZeroCalculator::<Vec<usize>>(Default::default()),
                c.clone(),
            )
            .unwrap(),
            solve_list(&lists, &ln, c).unwrap(),
        );
        let phase = match phase {
            SolverPhase::LocalSearch(search) => {
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search,
                    joint_moves: true,
                })
            }
            SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
            _ => unreachable!(),
        };
        let c = MixedSolverSequence {
            limits: SolverLimits {
                termination: Some(&condition),
                ..Default::default()
            },
            phases: vec![phase],
        };
        same(
            solve_mixed_calculator(
                &mixed,
                &|| ZeroCalculator::<MixedCalculatorChange>(Default::default()),
                c.clone(),
            )
            .unwrap(),
            solve_mixed(&mixed, &mn, c).unwrap(),
        );
    }
    let basic = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let lists = ListProblem::new(vec![0, 1, 2], vec![ListEntity::new(vec![]); 2]).unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    let c = SolverSequence {
        limits: SolverLimits {
            termination: Some(&condition),
            ..Default::default()
        },
        phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
    };
    same(
        solve_calculator(
            &basic,
            &|| ZeroCalculator::<Vec<usize>>(Default::default()),
            c.clone(),
        )
        .unwrap(),
        solve(&basic, &bn, c.clone()).unwrap(),
    );
    same(
        solve_list_calculator(
            &lists,
            &|| ZeroCalculator::<Vec<usize>>(Default::default()),
            c.clone(),
        )
        .unwrap(),
        solve_list(&lists, &ln, c).unwrap(),
    );
    let c = MixedSolverSequence {
        limits: SolverLimits {
            termination: Some(&condition),
            ..Default::default()
        },
        phases: vec![MixedSolverPhase::Construction(
            MixedConstructionConfig::default(),
        )],
    };
    same(
        solve_mixed_calculator(
            &mixed,
            &|| ZeroCalculator::<MixedCalculatorChange>(Default::default()),
            c.clone(),
        )
        .unwrap(),
        solve_mixed(&mixed, &mn, c).unwrap(),
    );
}
