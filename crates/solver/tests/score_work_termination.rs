//! Original score-work budget checks across scoped, fresh-stream, calculator,
//! and retained-stream solving for basic, list, and mixed planning models.
use timetuck::*;

struct Zero<C>(std::marker::PhantomData<C>);
impl<A, C> IncrementalCalculator<A> for Zero<C> {
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
fn basic_graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    (b.build(), s)
}
fn list_graph() -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::new(&mut b);
    (b.build(), s)
}
fn mixed_graph() -> (
    StreamGraph<'static, SimpleScore>,
    MixedStreamSources<i32, i32>,
) {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::new(&mut b);
    (b.build(), s)
}
fn mixed(c: SolverSequence<'_>) -> MixedSolverSequence<'_> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::Construction(construction) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction,
                        order: MixedConstructionOrder::BasicThenLists,
                    })
                }
                SolverPhase::LocalSearch(search) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search,
                        joint_moves: true,
                    })
                }
                SolverPhase::RuinRecreate(search) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search,
                        order: MixedConstructionOrder::BasicThenLists,
                    })
                }
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
            })
            .collect(),
    }
}
#[derive(Debug, PartialEq)]
struct Outcome {
    work: SolverWork,
    reason: SolverTermination,
    phases: usize,
    score: Option<SimpleScore>,
    complete: bool,
    assignment: String,
}
fn outcome<A: std::fmt::Debug>(r: SolverResult<A, SimpleScore>) -> Outcome {
    Outcome {
        work: r.work,
        reason: r.termination,
        phases: r.phases.len(),
        score: r.score,
        complete: r.complete,
        assignment: format!("{:?}", r.assignment),
    }
}
fn run(mode: usize, initialized: bool, c: SolverSequence<'_>) -> Outcome {
    let v = Variable::new(vec![0, 1, 2]);
    let b = FiniteProblem::new(vec![if initialized { v.with_initial(0) } else { v }; 3]).unwrap();
    let l = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(if initialized { vec![0, 1, 2] } else { vec![] }),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    match mode {
        0 => outcome(solve(&b, &ConstraintNetwork::<_, SimpleScore>::new(3), c).unwrap()),
        1 => outcome(solve_stream(&b, || Ok(basic_graph()), c).unwrap()),
        2 => outcome(solve_calculator(&b, &|| Zero::<Vec<usize>>(Default::default()), c).unwrap()),
        3 => {
            let (g, s) = basic_graph();
            let mut d = StreamScoreDirector::new(&b, g, s, b.initial_assignment()).unwrap();
            outcome(d.run_solver(c).unwrap())
        }
        4 => {
            outcome(solve_list(&l, &ListConstraintNetwork::<_, SimpleScore>::new(2, 3), c).unwrap())
        }
        5 => outcome(solve_list_stream(&l, || Ok(list_graph()), c).unwrap()),
        6 => outcome(
            solve_list_calculator(&l, &|| Zero::<Vec<usize>>(Default::default()), c).unwrap(),
        ),
        7 => {
            let (g, s) = list_graph();
            let mut d = ListStreamScoreDirector::new(&l, g, s, l.initial_assignment()).unwrap();
            outcome(d.run_solver(c).unwrap())
        }
        8..12 => {
            let p = MixedProblem::new(b, l);
            let c = mixed(c);
            match mode {
                8 => outcome(
                    solve_mixed(
                        &p,
                        &MixedConstraintNetwork::<_, _, SimpleScore>::new(3, 2, 3),
                        c,
                    )
                    .unwrap(),
                ),
                9 => outcome(solve_mixed_stream(&p, || Ok(mixed_graph()), c).unwrap()),
                10 => outcome(
                    solve_mixed_calculator(
                        &p,
                        &|| Zero::<MixedCalculatorChange>(Default::default()),
                        c,
                    )
                    .unwrap(),
                ),
                _ => {
                    let (g, s) = mixed_graph();
                    let mut d =
                        MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
                    outcome(d.run_solver(c).unwrap())
                }
            }
        }
        _ => unreachable!(),
    }
}
fn condition(n: u64) -> SearchTermination<'static> {
    SearchTermination::Condition(SearchLimit::ScoreEvaluations(n))
}
fn local(steps: u64) -> SolverPhase<'static, SimpleScore> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: steps,
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::FirstAccepted,
        ..Default::default()
    })
}

#[test]
fn score_conditions_match_hard_budget_work_and_retained_state_across_twelve_backends() {
    let phases = [
        (
            false,
            SolverPhase::Construction(ConstructionConfig::default()),
        ),
        (true, local(100)),
        (
            true,
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 2,
                maximum_ruined: 2,
                max_attempts: 20,
                seed: 7,
                ..Default::default()
            }),
        ),
        (false, SolverPhase::Exact(ExactPhaseConfig::default())),
        (
            false,
            SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
                ..Default::default()
            }),
        ),
    ];
    for mode in 0..12 {
        for (initial, phase) in &phases {
            for cap in 0..=12 {
                let leaf = condition(cap);
                let mut actual = run(
                    mode,
                    *initial,
                    SolverSequence {
                        limits: SolverLimits {
                            termination: Some(&leaf),
                            ..Default::default()
                        },
                        phases: vec![phase.clone()],
                    },
                );
                let mut expected = run(
                    mode,
                    *initial,
                    SolverSequence {
                        limits: SolverLimits {
                            max_evaluations: cap,
                            ..Default::default()
                        },
                        phases: vec![phase.clone()],
                    },
                );
                assert!(
                    actual.work.evaluations <= cap,
                    "mode {mode}, phase {phase:?}, cap {cap}: {actual:?}"
                );
                // A zero expression short-circuits before phase entry; the hard
                // budget's existing unscored entry path still reports a phase.
                if cap == 0 {
                    assert_eq!(actual.phases, 0);
                    expected.phases = 0;
                }
                // Different public stop reasons identify the chosen stopping policy.
                if actual.reason == SolverTermination::Condition {
                    assert_eq!(actual.work.evaluations, cap);
                    // A composed condition also polls at the final phase boundary;
                    // the hard budget may instead finish a fully scanned construction.
                    assert!(matches!(
                        expected.reason,
                        SolverTermination::EvaluationLimit | SolverTermination::PlanCompleted
                    ));
                    actual.reason = expected.reason;
                }
                assert_eq!(actual, expected, "mode {mode}, phase {phase:?}, cap {cap}");
            }
        }
    }
}

#[test]
fn initialization_scores_accumulate_across_zero_step_phases_without_candidates() {
    let leaf = condition(3);
    for mode in 0..12 {
        let r = run(
            mode,
            true,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
                    ..Default::default()
                },
                phases: vec![local(0); 5],
            },
        );
        assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
        assert_eq!(r.work.evaluations, 3);
        assert_eq!(r.work.move_evaluations, 0);
        assert_eq!(r.phases, 3);
        assert!(r.complete);
    }
}

#[test]
fn and_or_score_quotas_compose_with_search_steps_and_cross_phase_initializations() {
    let leaves = [
        condition(3),
        SearchTermination::Condition(SearchLimit::Steps(4)),
    ];
    for mode in 0..12 {
        for (expr, scores, steps, phases) in [
            (SearchTermination::Any(&leaves), 3, 2, 1),
            (SearchTermination::All(&leaves), 6, 4, 2),
        ] {
            let r = run(
                mode,
                true,
                SolverSequence {
                    limits: SolverLimits {
                        termination: Some(&expr),
                        ..Default::default()
                    },
                    phases: vec![local(2); 5],
                },
            );
            assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
            assert_eq!(r.work.evaluations, scores, "mode {mode}");
            assert_eq!(r.work.completed_steps, steps, "mode {mode}");
            assert_eq!(r.phases, phases);
        }
    }
}

#[test]
fn reconstruction_scores_stop_inside_scans_and_do_not_emit_construction_steps() {
    let leaf = condition(4);
    for mode in 0..12 {
        let r = run(
            mode,
            true,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
                    ..Default::default()
                },
                phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                    minimum_ruined: usize::MAX,
                    maximum_ruined: usize::MAX,
                    max_attempts: 30,
                    seed: 8,
                    ..Default::default()
                })],
            },
        );
        assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
        assert_eq!(r.work.evaluations, 4); // Outer initial + rebuild initial + two candidates.
        assert_eq!(r.work.move_evaluations, 2);
        assert_eq!(r.work.repair_attempts, 1);
        assert_eq!(r.work.construction_steps, 0);
        assert_eq!(r.work.completed_steps, 0); // Partial reconstruction is discarded.
        assert!(r.complete);
    }
}

#[test]
fn exact_integer_score_threshold_is_independent_of_rounded_progress() {
    let before = condition(u64::MAX)
        .status(SearchProgress {
            score_evaluations: u64::MAX - 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(before.progress, Some(1.0));
    assert!(!before.reached);
    assert!(
        condition(u64::MAX)
            .status(SearchProgress {
                score_evaluations: u64::MAX,
                ..Default::default()
            })
            .unwrap()
            .reached
    );
}

#[test]
fn diagnostic_rescoring_does_not_consume_score_quota_in_any_backend() {
    let leaf = condition(8);
    for mode in 0..12 {
        let baseline = run(
            mode,
            true,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
                    ..Default::default()
                },
                phases: vec![local(2); 5],
            },
        );
        let mut phases = vec![local(2); 5];
        for phase in &mut phases {
            let SolverPhase::LocalSearch(c) = phase else {
                unreachable!()
            };
            c.assert_scores = true;
            c.assert_step_scores = true;
        }
        let actual = run(
            mode,
            true,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
                    assert_phase_scores: true,
                    ..Default::default()
                },
                phases,
            },
        );
        assert_eq!(actual, baseline, "mode {mode}");
        assert_eq!(actual.work.evaluations, 8);
    }
}

#[test]
fn fine_repair_initializations_count_as_scores_without_reconstruction_moves() {
    let leaf = condition(4);
    for mode in 0..12 {
        let r = run(
            mode,
            true,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
                    ..Default::default()
                },
                phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                    max_attempts: 20,
                    seed: 3,
                    move_weights: RepairMoveWeights {
                        repair: 0,
                        change: 1,
                        swap: 0,
                    },
                    ..Default::default()
                })],
            },
        );
        assert_eq!(r.reason, SolverTermination::Condition, "mode {mode}");
        assert_eq!(r.work.evaluations, 4);
        assert_eq!(r.work.move_evaluations, 3);
        assert_eq!(r.work.fine_move_attempts, 3);
        assert_eq!(r.work.repair_attempts, 0);
        assert_eq!(r.work.construction_steps, 0);
    }
}

#[test]
fn nested_work_conditions_preserve_hard_cap_precedence_and_metrics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let leaves = [
        condition(3),
        SearchTermination::Condition(SearchLimit::MoveEvaluations(4)),
    ];
    let inner = SearchTermination::All(&leaves);
    let outer = [inner, condition(100)];
    let expr = SearchTermination::Any(&outer);
    for hard in [None, Some(4)] {
        for monitored in [false, true] {
            let m = SolverMetrics::default();
            let r = solve_with_extensions(
                &p,
                &n,
                SolverSequence {
                    limits: SolverLimits {
                        termination: Some(&expr),
                        max_evaluations: hard.unwrap_or(100),
                        ..Default::default()
                    },
                    phases: vec![local(30)],
                },
                SolverExtensions {
                    metrics: monitored.then_some(&m),
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(r.work.evaluations, hard.unwrap_or(5));
            assert_eq!(r.work.move_evaluations, hard.unwrap_or(5) - 1);
            assert_eq!(
                r.termination,
                if hard.is_some() {
                    SolverTermination::EvaluationLimit
                } else {
                    SolverTermination::Condition
                }
            );
            if monitored {
                let snapshot = m.snapshot().latest.unwrap();
                assert_eq!(snapshot.work.evaluations, r.work.evaluations);
                assert_eq!(snapshot.move_evaluations.total(), r.work.move_evaluations);
            }
        }
    }
}

#[test]
fn custom_trial_and_execute_share_score_quota_with_initialization() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    for cap in 1..=3 {
        let leaf = condition(cap);
        let calls = std::cell::Cell::new(0);
        let mut command = |_,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<i32>,
            Move<i32>,
            SimpleScore,
            ScoringError,
        >| {
            calls.set(calls.get() + 1);
            let m = Move::Change {
                variable: 0,
                value: Some(1),
            };
            assert!(c.evaluate(&m, |_, _| ())?.is_some());
            c.assert_consistent()?;
            assert_eq!(c.execute(&m)?.is_some(), cap == 3);
            assert!(c.execute(&m)?.is_none());
            assert_eq!(c.termination(), Some(CustomPhaseTermination::Condition));
            Ok(())
        };
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
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
        assert_eq!(r.work.evaluations, cap);
        assert_eq!(r.work.move_evaluations, cap - 1);
        assert_eq!(calls.get(), u64::from(cap > 1));
    }
}

#[test]
fn provider_construction_counts_only_scored_proposals_and_initialization() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let leaf = condition(3);
    let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
        vec![
            Move::Change {
                variable: 99,
                value: Some(0),
            }, // Rejected without scoring.
            Move::Change {
                variable: 0,
                value: Some(0),
            },
            Move::Change {
                variable: 0,
                value: Some(1),
            },
            Move::Change {
                variable: 0,
                value: Some(2),
            },
        ]
    };
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits {
                termination: Some(&leaf),
                ..Default::default()
            },
            phases: vec![SolverPhase::Construction(ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            })],
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.work.construction_steps, 1);
    assert!(!r.complete);
}

#[test]
fn frozen_initial_only_solves_honor_zero_and_one_score_quotas() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let calls = std::cell::Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("score", vec![0], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    for cap in [0, 1, 2] {
        let leaf = condition(cap);
        calls.set(0);
        let r = solve(
            &p,
            &n,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
                    ..Default::default()
                },
                phases: vec![local(10)],
            },
        )
        .unwrap();
        assert!(r.phases.is_empty());
        assert_eq!(r.work.evaluations, cap.min(1));
        assert_eq!(calls.get(), cap.min(1));
        assert_eq!(
            r.termination,
            if cap < 2 {
                SolverTermination::Condition
            } else {
                SolverTermination::PlanCompleted
            }
        );
    }
}

#[test]
fn inactive_score_quota_preserves_complete_multi_phase_execution() {
    let leaf = condition(u64::MAX);
    for mode in 0..12 {
        let phases = vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            local(2),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 2,
                maximum_ruined: 2,
                max_attempts: 2,
                seed: 8,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ];
        let expected = run(
            mode,
            false,
            SolverSequence {
                phases: phases.clone(),
                ..Default::default()
            },
        );
        let actual = run(
            mode,
            false,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&leaf),
                    ..Default::default()
                },
                phases,
            },
        );
        assert_eq!(actual, expected, "mode {mode}");
        assert_eq!(actual.reason, SolverTermination::Optimal);
        assert!(actual.complete);
    }
}

#[test]
fn inactive_shared_significant_window_preserves_work_across_twelve_backends() {
    let policy =
        SignificantImprovementConfig::new(std::time::Duration::MAX, SimpleScore::new(7)).unwrap();
    for mode in 0..12 {
        for phase in [
            local(2),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 1,
                maximum_ruined: 1,
                max_attempts: 2,
                seed: 7,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig::default()),
            SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
                ..Default::default()
            }),
        ] {
            let mut c = SolverSequence {
                limits: SolverLimits {
                    assert_phase_scores: true,
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig::default()),
                    phase,
                ],
            };
            let baseline = run(mode, false, c.clone());
            c.limits.significant_improvement = Some(&policy);
            assert_eq!(run(mode, false, c), baseline, "mode={mode}");
        }
    }
}
