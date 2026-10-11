//! Original score-aware composition regressions; no upstream-method credit.
use timetuck::*;

#[derive(Clone)]
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
        12 => {
            let n = ConstraintNetwork::<_, SimpleScore>::new(3);
            outcome(
                ScoreDirector::new(&b, &n, b.initial_assignment())
                    .unwrap()
                    .run_solver(c)
                    .unwrap(),
            )
        }
        13 => {
            let n = ListConstraintNetwork::<_, SimpleScore>::new(2, 3);
            outcome(
                ListScoreDirector::new(&l, &n, l.initial_assignment())
                    .unwrap()
                    .run_solver(c)
                    .unwrap(),
            )
        }
        14 => {
            let p = MixedProblem::new(b, l);
            let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(3, 2, 3);
            outcome(
                MixedScoreDirector::new(&p, &n, p.initial_assignment())
                    .unwrap()
                    .run_solver(mixed(c))
                    .unwrap(),
            )
        }
        15 => {
            let mut d = CalculatorScoreDirector::new(
                &b,
                b.initial_assignment(),
                || Zero::<Vec<usize>>(Default::default()),
                false,
            )
            .unwrap();
            outcome(d.run_solver(c).unwrap())
        }
        16 => {
            let mut d = ListCalculatorScoreDirector::new(
                &l,
                l.initial_assignment(),
                || Zero::<Vec<usize>>(Default::default()),
                false,
            )
            .unwrap();
            outcome(d.run_solver(c).unwrap())
        }
        17 => {
            let p = MixedProblem::new(b, l);
            let mut d = MixedCalculatorScoreDirector::new(
                &p,
                p.initial_assignment(),
                || Zero::<MixedCalculatorChange>(Default::default()),
                false,
            )
            .unwrap();
            outcome(d.run_solver(mixed(c)).unwrap())
        }
        _ => unreachable!(),
    }
}

fn local(steps: u64) -> SolverPhase<'static> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: steps,
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::FirstAccepted,
        ..Default::default()
    })
}
fn phases() -> Vec<(bool, SolverPhase<'static>)> {
    vec![
        (false, SolverPhase::Construction(Default::default())),
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
        (false, SolverPhase::Exact(Default::default())),
        (
            false,
            SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::BranchAndBound(Default::default()),
                ..Default::default()
            }),
        ),
    ]
}
#[test]
fn score_goals_wait_for_complete_states_and_minimum_work_in_eighteen_backends() {
    for mode in 0..18 {
        for (initialized, phase) in phases() {
            for minimum in [1, 4] {
                let children = [
                    ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(
                        SimpleScore::ZERO,
                    )),
                    ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
                        SearchLimit::ScoreEvaluations(minimum),
                    )),
                ];
                let condition = ScoredSearchTermination::All(&children);
                let r = run(
                    mode,
                    initialized,
                    SolverSequence {
                        phases: vec![phase.clone()],
                        limits: SolverLimits {
                            scored_termination: Some(&condition),
                            ..Default::default()
                        },
                    },
                );
                assert_eq!(
                    r.reason,
                    SolverTermination::Condition,
                    "mode={mode} phase={phase:?} minimum={minimum}: {r:?}"
                );
                assert!(r.complete, "mode={mode} phase={phase:?}");
                assert_eq!(r.score, Some(SimpleScore::ZERO));
                assert!(r.work.evaluations >= minimum);
            }
        }
    }
}
#[test]
fn score_or_budget_matches_existing_count_expression_without_extra_work() {
    for mode in 0..18 {
        for (initialized, phase) in phases() {
            for cap in [0, 1, 4, 9] {
                let count = SearchTermination::Condition(SearchLimit::ScoreEvaluations(cap));
                let children = [
                    ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(
                        SimpleScore::new(1),
                    )),
                    ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
                        SearchLimit::ScoreEvaluations(cap),
                    )),
                ];
                let scored = ScoredSearchTermination::Any(&children);
                let c = SolverSequence {
                    phases: vec![phase.clone()],
                    limits: SolverLimits {
                        termination: Some(&count),
                        ..Default::default()
                    },
                };
                let expected = run(mode, initialized, c.clone());
                let actual = run(
                    mode,
                    initialized,
                    SolverSequence {
                        limits: SolverLimits {
                            termination: None,
                            scored_termination: Some(&scored),
                            ..c.limits
                        },
                        ..c
                    },
                );
                assert_eq!(actual, expected, "mode={mode} phase={phase:?} cap={cap}");
            }
        }
    }
}
#[test]
fn counters_and_best_goals_span_phase_handoffs() {
    for mode in 0..18 {
        let children = [
            ScoredSearchTermination::Condition(ScoredSearchLimit::Feasible),
            ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(3))),
        ];
        let stop = ScoredSearchTermination::All(&children);
        let r = run(
            mode,
            true,
            SolverSequence {
                phases: vec![local(1); 8],
                limits: SolverLimits {
                    scored_termination: Some(&stop),
                    ..Default::default()
                },
            },
        );
        assert_eq!(r.reason, SolverTermination::Condition, "mode={mode}");
        assert_eq!(r.work.completed_steps, 3);
        assert_eq!(r.phases, 3);
        assert_eq!(r.work.evaluations, 6); // Each phase initialization still counts.
    }
}
#[test]
fn accepted_worsening_custom_moves_do_not_forget_complete_best() {
    let p = FiniteProblem::new(vec![Variable::new(vec![-1, 0, 10]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let leaves = [
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::new(10))),
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
            SearchLimit::ScoreEvaluations(4),
        )),
    ];
    let stop = ScoredSearchTermination::All(&leaves);
    for retained in [false, true] {
        let c = SolverSequence {
            phases: vec![SolverPhase::Custom(Default::default()), local(20)],
            limits: SolverLimits {
                scored_termination: Some(&stop),
                ..Default::default()
            },
        };
        let mut command = |_: usize,
                           ctx: &mut CustomPhaseContext<
            '_,
            Assignment<i64>,
            Move<i64>,
            SimpleScore,
            ScoringError,
        >| {
            for value in [10, 0, -1] {
                assert!(
                    ctx.execute(&Move::Change {
                        variable: 0,
                        value: Some(value)
                    })?
                    .is_some()
                );
            }
            assert_eq!(ctx.score(), SimpleScore::new(-1));
            assert_eq!(ctx.best_score(), SimpleScore::new(10));
            assert!(ctx.is_terminated());
            assert!(
                ctx.execute(&Move::Change {
                    variable: 0,
                    value: Some(10)
                })?
                .is_none()
            );
            Ok(())
        };
        let extensions = SolverExtensions {
            command: Some(&mut command),
            ..Default::default()
        };
        let r = if retained {
            ScoreDirector::new(&p, &n, p.initial_assignment())
                .unwrap()
                .run_solver_with_extensions(c, extensions)
                .unwrap()
        } else {
            solve_with_extensions(&p, &n, c, extensions).unwrap()
        };
        assert_eq!(r.termination, SolverTermination::Condition);
        assert_eq!(r.score, Some(SimpleScore::new(10)));
        assert_eq!(r.work.evaluations, 4);
        assert_eq!(r.work.move_evaluations, 3);
        assert_eq!(r.phases.len(), 1);
    }
}
#[test]
fn nested_multilevel_and_decimal_goals_use_exact_score_order() {
    let progress = SearchProgress::default();
    let target = HardSoftScore::new(0, 10);
    let goal = ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(target));
    assert!(!goal.status(progress, None).unwrap().reached);
    assert!(
        !goal
            .status(progress, Some(&HardSoftScore::new(-1, 999)))
            .unwrap()
            .reached
    );
    assert!(
        !goal
            .status(progress, Some(&target.with_structural(-1)))
            .unwrap()
            .reached
    );
    assert!(goal.status(progress, Some(&target)).unwrap().reached);
    assert!(
        goal.status(progress, Some(&HardSoftScore::new(1, -999)))
            .unwrap()
            .reached
    );
    let feasible = ScoredSearchTermination::Condition(ScoredSearchLimit::Feasible);
    assert!(
        feasible
            .status(progress, Some(&HardSoftScore::new(0, -999)))
            .unwrap()
            .reached
    );
    assert!(
        !feasible
            .status(progress, Some(&HardSoftScore::new(-1, 999)))
            .unwrap()
            .reached
    );
    let target: SimpleDecimalScore = "0.0000000000000000001".parse().unwrap();
    let children = [
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(target.clone())),
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(2))),
    ];
    let all = ScoredSearchTermination::All(&children);
    assert!(!all.status(progress, Some(&target)).unwrap().reached);
    let progressed = SearchProgress {
        completed_steps: 2,
        ..progress
    };
    assert!(all.status(progressed, Some(&target)).unwrap().reached);
    assert!(
        !all.status(progressed, Some(&SimpleDecimalScore::ZERO))
            .unwrap()
            .reached
    );
}

#[test]
fn zero_steps_never_turns_missing_complete_best_into_a_goal() {
    let children = [
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::new(-1000))),
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(0))),
    ];
    let stop = ScoredSearchTermination::All(&children);
    assert!(
        !stop
            .status(SearchProgress::default(), None)
            .unwrap()
            .reached
    );
    for mode in 0..18 {
        let r = run(
            mode,
            false,
            SolverSequence {
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig {
                        max_steps: Some(1),
                        ..Default::default()
                    }),
                    SolverPhase::Construction(Default::default()),
                    local(10),
                ],
                limits: SolverLimits {
                    scored_termination: Some(&stop),
                    ..Default::default()
                },
            },
        );
        assert_eq!(r.reason, SolverTermination::Condition, "mode={mode}: {r:?}");
        assert_eq!(
            r.phases, 2,
            "partial first phase must hand off, mode={mode}"
        );
        assert!(r.complete);
    }
}

#[test]
fn optional_construction_uses_current_score_until_actual_handoff_is_retained() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]).allowing_unassigned(); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("assigned penalty", vec![0, 1], |v| {
        SimpleScore::new(-(v.iter().flatten().count() as i64))
    })
    .unwrap();
    // All assignments are complete. Initial score 0 is lost when optional
    // construction commits -1; it must not remain an eligible retained best.
    for work in [
        SearchLimit::ConstructionSteps(1),
        SearchLimit::ScoreEvaluations(2),
    ] {
        let leaves = [
            ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::ZERO)),
            ScoredSearchTermination::Condition(ScoredSearchLimit::Search(work)),
        ];
        let stop = ScoredSearchTermination::All(&leaves);
        for (retained, supplied) in [(false, false), (true, false), (false, true), (true, true)] {
            let c = SolverSequence {
                phases: vec![SolverPhase::Construction(ConstructionConfig {
                    optional: OptionalConstruction::EvaluateUnassigned,
                    pick_early: ConstructionPickEarly::FirstFeasibleScore,
                    ..Default::default()
                })],
                limits: SolverLimits {
                    scored_termination: Some(&stop),
                    ..Default::default()
                },
            };
            let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
                vec![
                    Move::Change {
                        variable: 0,
                        value: Some(1),
                    },
                    Move::Change {
                        variable: 1,
                        value: Some(1),
                    },
                ]
            };
            let mut bindings = PhaseMoveProviders::default();
            if supplied {
                bindings.bind(0, &mut provider);
            }
            let extensions = SolverExtensions {
                move_providers: bindings,
                ..Default::default()
            };
            let r = if retained {
                ScoreDirector::new(&p, &n, p.initial_assignment())
                    .unwrap()
                    .run_solver_with_extensions(c, extensions)
                    .unwrap()
            } else {
                solve_with_extensions(&p, &n, c, extensions).unwrap()
            };
            assert_eq!(r.termination, SolverTermination::PlanCompleted, "{work:?}");
            assert_eq!(r.score, Some(SimpleScore::new(-2)));
            assert_eq!(r.work.construction_steps, 2);
        }
    }
    // Without a delayed work gate, the initial complete state can stop construction.
    let stop = ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::ZERO));
    let r = solve(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig {
                optional: OptionalConstruction::EvaluateUnassigned,
                pick_early: ConstructionPickEarly::FirstFeasibleScore,
                ..Default::default()
            })],
            limits: SolverLimits {
                scored_termination: Some(&stop),
                ..Default::default()
            },
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(r.work.construction_steps, 0);
}

#[test]
fn hard_limits_and_unconditional_goals_remain_independent() {
    let stop =
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::new(100)));
    let legacy = SearchTermination::Condition(SearchLimit::Steps(2));
    for mode in 0..18 {
        for (limits, wanted) in [
            (
                SolverLimits {
                    scored_termination: Some(&stop),
                    max_evaluations: 2,
                    ..Default::default()
                },
                SolverTermination::EvaluationLimit,
            ),
            (
                SolverLimits {
                    scored_termination: Some(&stop),
                    termination: Some(&legacy),
                    ..Default::default()
                },
                SolverTermination::Condition,
            ),
            (
                SolverLimits {
                    scored_termination: Some(&stop),
                    best_score_limit: Some(SimpleScore::ZERO),
                    ..Default::default()
                },
                SolverTermination::BestScoreLimit,
            ),
        ] {
            assert_eq!(
                run(
                    mode,
                    true,
                    SolverSequence {
                        phases: vec![local(20)],
                        limits
                    }
                )
                .reason,
                wanted,
                "mode={mode}"
            );
        }
    }
}

#[path = "support/snapshot_score.rs"]
mod owned;
#[test]
fn unused_and_counter_only_expressions_do_not_clone_best_scores_or_rescore() {
    use std::cell::Cell;
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("score", vec![0], |v| {
        calls.set(calls.get() + 1);
        owned::OwnedScore::new(0, v[0].copied().unwrap())
    })
    .unwrap();
    let count = ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
        SearchLimit::ScoreEvaluations(u64::MAX),
    ));
    let mut outcomes = vec![];
    for condition in [None, Some(&count)] {
        calls.set(0);
        owned::reset_clones();
        let r = solve(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 3,
                    ..Default::default()
                })],
                limits: SolverLimits {
                    scored_termination: condition,
                    ..Default::default()
                },
            },
        )
        .unwrap();
        outcomes.push((r, calls.get(), owned::clones()));
    }
    assert_eq!(outcomes[0], outcomes[1]);
}

#[test]
fn decimal_and_multilevel_targets_stop_actual_solver_plans() {
    fn check<S: AdditiveScore>(values: [S; 3]) {
        let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2])]).unwrap();
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| {
            values[v[0].copied().unwrap_or(0)].clone()
        })
        .unwrap();
        let stop =
            ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(values[1].clone()));
        let r = solve(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Exact(Default::default())],
                limits: SolverLimits {
                    scored_termination: Some(&stop),
                    ..Default::default()
                },
            },
        )
        .unwrap();
        assert_eq!(r.termination, SolverTermination::Condition);
        assert_eq!(r.score, Some(values[1].clone()));
        assert_eq!(r.assignment.values(), &[Some(1)]);
        assert_eq!(r.work.evaluations, 2);
    }
    check([
        HardSoftScore::new(-1, 999),
        HardSoftScore::new(0, -999),
        HardSoftScore::new(1, -999),
    ]);
    check(
        ["0", "0.0000000000000000001", "0.0000000000000000002"]
            .map(|s| s.parse::<SimpleDecimalScore>().unwrap()),
    );
}

#[test]
fn scored_construction_step_gate_observes_no_change_placements() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]).allowing_unassigned(); 3]).unwrap();
    let mut n = ConstraintNetwork::new(3);
    n.add_constraint("penalty", vec![0, 1, 2], |v| {
        SimpleScore::new(-(v.iter().flatten().count() as i64))
    })
    .unwrap();
    let leaves = [
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::ZERO)),
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
            SearchLimit::ConstructionSteps(1),
        )),
    ];
    let stop = ScoredSearchTermination::All(&leaves);
    let r = solve(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Construction(ConstructionConfig {
                optional: OptionalConstruction::EvaluateUnassigned,
                ..Default::default()
            })],
            limits: SolverLimits {
                scored_termination: Some(&stop),
                ..Default::default()
            },
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
    assert_eq!(r.work.construction_steps, 1);
    assert_eq!(r.work.evaluations, 3); // Initial, assignment trial, winning no-change.
    assert_eq!(r.assignment.values(), &[None, None, None]);
}
