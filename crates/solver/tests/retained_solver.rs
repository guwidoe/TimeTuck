//! Original retained-solve orchestration checks; no upstream method credit.
use std::{cell::Cell, time::Duration};
use timetuck::*;
fn normalize<A, S>(mut result: SolverResult<A, S>) -> SolverResult<A, S> {
    result.work.constraint_evaluations = 0;
    for phase in &mut result.phases {
        phase.work.constraint_evaluations = 0;
    }
    result
}
fn configs() -> Vec<SolverConfig<'static>> {
    [0, 1, 2, 5, 25, 100]
        .into_iter()
        .flat_map(|max_evaluations| {
            [None, Some(0), Some(3)].map(|max_move_evaluations| SolverConfig {
                limits: SolverLimits {
                    max_evaluations,
                    max_move_evaluations,
                    ..Default::default()
                },
                local_search: vec![
                    LocalSearchConfig {
                        max_steps: 3,
                        acceptance: Acceptance::HillClimbing,
                        ..Default::default()
                    };
                    2
                ],
                ..Default::default()
            })
        })
        .chain([
            SolverConfig {
                limits: SolverLimits {
                    max_time: Some(Duration::ZERO),
                    ..Default::default()
                },
                ..Default::default()
            },
            SolverConfig {
                limits: SolverLimits {
                    max_unimproved_steps: Some(1),
                    ..Default::default()
                },
                ..Default::default()
            },
            SolverConfig {
                limits: SolverLimits {
                    best_score_limit: Some(SimpleScore::new(0)),
                    ..Default::default()
                },
                ..Default::default()
            },
            SolverConfig {
                limits: SolverLimits {
                    stop_when_feasible: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            SolverConfig {
                limits: SolverLimits {
                    assert_phase_scores: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            SolverConfig {
                construction: ConstructionConfig {
                    max_steps: Some(1),
                    ..Default::default()
                },
                ..Default::default()
            },
        ])
        .collect()
}
fn basic_graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    b.score("value", s.port(), |(_, v)| {
        SimpleScore::new(i64::from(v.unwrap_or(0)))
    })
    .unwrap();
    (b.build(), s)
}
fn list_graph() -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::<i32>::new(&mut b);
    b.score("position", s.values.port(), |r| {
        SimpleScore::new(
            r.position
                .map_or(0, |p| i64::from(r.value) * (p.index + 1) as i64),
        )
    })
    .unwrap();
    (b.build(), s)
}
fn mixed_graph() -> (
    StreamGraph<'static, SimpleScore>,
    MixedStreamSources<i32, i32>,
) {
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<i32, i32>::new(&mut b);
    let join = b
        .join_equal(
            s.basic.port(),
            s.lists.entities.port(),
            |(i, _)| *i,
            |(i, _)| *i,
        )
        .unwrap();
    b.score("joint", &join, |((_, v), (_, list))| {
        SimpleScore::new(i64::from(v.unwrap_or(0)) * list.len() as i64)
    })
    .unwrap();
    (b.build(), s)
}
#[test]
fn basic_shared_controls_and_events_match_fresh_solves_including_frozen_models() {
    for pinned in [false, true] {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![0, 1, 2])
                .with_initial(0)
                .with_pinned(pinned);
            2
        ])
        .unwrap();
        for config in configs() {
            let mut expected_events = vec![];
            let mut actual_events = vec![];
            let expected = solve_stream_with_events(
                &p,
                || Ok(basic_graph()),
                config.clone(),
                |e| expected_events.push(format!("{e:?}")),
            )
            .unwrap();
            let (g, s) = basic_graph();
            let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let actual = d
                .run_solver_with_events(config, |e| actual_events.push(format!("{e:?}")))
                .unwrap();
            assert_eq!(actual_events, expected_events);
            if let Some(score) = actual.score {
                assert_eq!(d.assignment(), &actual.assignment);
                assert_eq!(d.score(), score);
            }
            assert_eq!(normalize(actual), normalize(expected));
        }
    }
}
#[test]
fn list_shared_controls_preserve_prepared_dependencies_and_handoff_events() {
    for dependencies in [false, true] {
        let mut p = ListProblem::new(
            vec![0, 1, 2],
            vec![ListEntity::new(vec![1, 0]), ListEntity::new(vec![])],
        )
        .unwrap();
        if dependencies {
            p = p.with_dependencies(vec![vec![], vec![0], vec![1]]).unwrap();
        }
        for config in configs() {
            let mut expected_events = vec![];
            let mut actual_events = vec![];
            let expected = solve_list_stream_with_events(
                &p,
                || Ok(list_graph()),
                config.clone(),
                |e| expected_events.push(format!("{e:?}")),
            )
            .unwrap();
            let (g, s) = list_graph();
            let prepared = p.prepare_assignment(&p.initial_assignment()).unwrap();
            let mut d = ListStreamScoreDirector::new(&p, g, s, prepared.clone()).unwrap();
            let actual = d
                .run_solver_with_events(config, |e| actual_events.push(format!("{e:?}")))
                .unwrap();
            if dependencies && actual.work.evaluations > 0 {
                // Fresh solve repairs the cyclic model input and suppresses its repair
                // baseline event. A pre-bound director starts from the already valid
                // prepared state and publishes that ordinary initial best.
                assert!(actual_events[0].contains(&format!("assignment: {prepared:?}")));
                assert_eq!(&actual_events[1..], expected_events.as_slice());
            } else {
                assert_eq!(actual_events, expected_events);
            }
            if let Some(score) = actual.score {
                assert_eq!(d.assignment(), &actual.assignment);
                assert_eq!(d.score(), score);
            }
            assert_eq!(normalize(actual), normalize(expected));
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn mixed_shared_controls_preserve_both_construction_orders_and_joint_search() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap(),
        ListProblem::new(
            vec![1, 2],
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        for config in configs() {
            let config = MixedSolverConfig {
                limits: config.limits,
                construction: MixedConstructionConfig {
                    construction: config.construction,
                    order,
                },
                local_search: config
                    .local_search
                    .into_iter()
                    .map(|search| MixedLocalSearchConfig {
                        search,
                        joint_moves: true,
                    })
                    .collect(),
            };
            let mut expected_events = vec![];
            let mut actual_events = vec![];
            let expected = solve_mixed_stream_with_events(
                &p,
                || Ok(mixed_graph()),
                config.clone(),
                |e| expected_events.push(format!("{e:?}")),
            )
            .unwrap();
            let (g, s) = mixed_graph();
            let mut d = MixedStreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
            let actual = d
                .run_solver_with_events(config, |e| actual_events.push(format!("{e:?}")))
                .unwrap();
            assert_eq!(actual_events, expected_events);
            if let Some(score) = actual.score {
                assert_eq!(d.assignment(), &actual.assignment);
                assert_eq!(d.score(), score);
            }
            assert_eq!(normalize(actual), normalize(expected));
        }
    }
}
#[test]
fn lifecycle_cancellation_invalid_plans_and_phase_assertions_preserve_cached_state() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let calls = Cell::new(0);
    let bias = Cell::new(0);
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    b.score("value", s.port(), |(_, v)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(i64::from(v.unwrap_or(0)) + bias.get())
    })
    .unwrap();
    let mut d = StreamScoreDirector::new(&p, b.build(), s, p.initial_assignment()).unwrap();
    let before = calls.get();
    let invalid = SolverConfig {
        local_search: vec![LocalSearchConfig {
            acceptance: Acceptance::LateAcceptance {
                history_size: 0,
                hill_climbing: false,
            },
            ..Default::default()
        }],
        ..Default::default()
    };
    assert!(matches!(
        d.run_solver(invalid),
        Err(SolverError::InvalidAcceptance { phase: 1, .. })
    ));
    assert_eq!(calls.get(), before);
    assert_eq!(d.assignment(), &p.initial_assignment());
    let token = CancellationToken::new();
    let result = d
        .run_solver_with_events(
            SolverConfig {
                limits: SolverLimits {
                    cancellation: Some(&token),
                    ..Default::default()
                },
                ..Default::default()
            },
            |event| {
                if matches!(event, SolverEvent::FirstInitialized { .. }) {
                    token.cancel();
                }
            },
        )
        .unwrap();
    assert_eq!(result.termination, SolverTermination::Cancelled);
    assert_eq!(result.phases.len(), 1);
    assert_eq!(calls.get() - before, result.work.constraint_evaluations);
    assert_eq!(d.assignment(), &result.assignment);
    let mut changed = false;
    let checked = d.run_solver_with_events(
        SolverConfig {
            limits: SolverLimits {
                assert_phase_scores: true,
                ..Default::default()
            },
            ..Default::default()
        },
        |event| {
            if matches!(event, SolverEvent::Best(_)) && !changed {
                bias.set(100);
                changed = true;
            }
        },
    );
    assert!(
        checked.is_err(),
        "phase diagnostics must detect a changed scoring callback"
    );
    bias.set(0);
    d.assert_consistent().unwrap();
}

fn exact_configs() -> Vec<ExactPhaseConfig<'static>> {
    [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
        ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
    ]
    .into_iter()
    .flat_map(|strategy| {
        [0, 1, 3, 100].map(|max_evaluations| ExactPhaseConfig {
            strategy: strategy.clone(),
            max_evaluations,
            ..Default::default()
        })
    })
    .collect()
}
#[test]
fn retained_exact_strategies_match_fresh_results_events_and_best_graphs() {
    let bp = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let lp = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mp = MixedProblem::new(bp.clone(), lp.clone());
    for exact in exact_configs() {
        let config = SolverSequence {
            phases: vec![SolverPhase::Exact(exact.clone())],
            ..Default::default()
        };
        let mut actual_events = vec![];
        let mut expected_events = vec![];
        let expected = solve_stream_with_events(
            &bp,
            || Ok(basic_graph()),
            config.clone(),
            |e| expected_events.push(format!("{e:?}")),
        )
        .unwrap();
        let (g, s) = basic_graph();
        let mut d = StreamScoreDirector::new(&bp, g, s, bp.initial_assignment()).unwrap();
        let initial = d.assignment().clone();
        let actual = d
            .run_solver_with_events(config.clone(), |e| actual_events.push(format!("{e:?}")))
            .unwrap();
        assert_eq!(actual_events, expected_events);
        assert_eq!(normalize(actual.clone()), normalize(expected));
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(score) = actual.score {
            assert_eq!(d.score(), score);
        } else {
            assert_eq!(d.assignment(), &initial);
        }
        d.assert_consistent().unwrap();
        actual_events.clear();
        expected_events.clear();
        let expected = solve_list_stream_with_events(
            &lp,
            || Ok(list_graph()),
            config.clone(),
            |e| expected_events.push(format!("{e:?}")),
        )
        .unwrap();
        let (g, s) = list_graph();
        let mut d = ListStreamScoreDirector::new(&lp, g, s, lp.initial_assignment()).unwrap();
        let actual = d
            .run_solver_with_events(config, |e| actual_events.push(format!("{e:?}")))
            .unwrap();
        assert_eq!(actual_events, expected_events);
        assert_eq!(normalize(actual.clone()), normalize(expected));
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(score) = actual.score {
            assert_eq!(d.score(), score);
        }
        d.assert_consistent().unwrap();
        let config = MixedSolverSequence {
            phases: vec![MixedSolverPhase::Exact(exact)],
            ..Default::default()
        };
        actual_events.clear();
        expected_events.clear();
        let expected = solve_mixed_stream_with_events(
            &mp,
            || Ok(mixed_graph()),
            config.clone(),
            |e| expected_events.push(format!("{e:?}")),
        )
        .unwrap();
        let (g, s) = mixed_graph();
        let mut d = MixedStreamScoreDirector::new(&mp, g, s, mp.initial_assignment()).unwrap();
        let actual = d
            .run_solver_with_events(config, |e| actual_events.push(format!("{e:?}")))
            .unwrap();
        assert_eq!(actual_events, expected_events);
        assert_eq!(normalize(actual.clone()), normalize(expected));
        assert_eq!(d.assignment(), &actual.assignment);
        if let Some(score) = actual.score {
            assert_eq!(d.score(), score);
        }
        d.assert_consistent().unwrap();
    }
}
fn ordered_sequence() -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                max_attempts: 4,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::Complete,
                ..Default::default()
            }),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 2,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig {
                strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
                ..Default::default()
            }),
        ],
    }
}
#[test]
fn custom_repair_exact_and_local_handoffs_share_one_graph_in_every_model() {
    let bp = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    let mut actual_events = vec![];
    let mut expected_events = vec![];
    let command = |index,
                   ctx: &mut CustomPhaseContext<
        '_,
        Assignment<i32>,
        Move<i32>,
        SimpleScore,
        StreamScoringError,
    >| {
        assert_eq!(index, 1);
        ctx.execute(&Move::Change {
            variable: 0,
            value: Some(0),
        })?;
        Ok(())
    };
    let expected = solve_stream_with_commands_and_events(
        &bp,
        || Ok(basic_graph()),
        ordered_sequence(),
        command,
        |e| expected_events.push(format!("{e:?}")),
    )
    .unwrap();
    let (g, s) = basic_graph();
    let mut d = StreamScoreDirector::new(&bp, g, s, bp.initial_assignment()).unwrap();
    let actual = d
        .run_solver_with_commands_and_events(ordered_sequence(), command, |e| {
            actual_events.push(format!("{e:?}"))
        })
        .unwrap();
    assert_eq!(actual_events, expected_events);
    assert_eq!(normalize(actual.clone()), normalize(expected));
    assert_eq!(d.assignment(), &actual.assignment);
    assert_eq!(Some(d.score()), actual.score);
    d.assert_consistent().unwrap();

    let lp = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1]), ListEntity::new(vec![])],
    )
    .unwrap();
    actual_events.clear();
    expected_events.clear();
    let command = |index,
                   ctx: &mut CustomPhaseContext<
        '_,
        ListAssignment<i32>,
        ListMove<i32>,
        SimpleScore,
        ListStreamScoringError,
    >| {
        assert_eq!(index, 1);
        let left = ctx.assignment().location(&1).unwrap();
        let right = ctx.assignment().location(&3).unwrap();
        ctx.execute(&ListMove::Swap { left, right })?;
        Ok(())
    };
    let expected = solve_list_stream_with_commands_and_events(
        &lp,
        || Ok(list_graph()),
        ordered_sequence(),
        command,
        |e| expected_events.push(format!("{e:?}")),
    )
    .unwrap();
    let (g, s) = list_graph();
    let mut d = ListStreamScoreDirector::new(&lp, g, s, lp.initial_assignment()).unwrap();
    let actual = d
        .run_solver_with_commands_and_events(ordered_sequence(), command, |e| {
            actual_events.push(format!("{e:?}"))
        })
        .unwrap();
    assert_eq!(actual_events, expected_events);
    assert_eq!(normalize(actual.clone()), normalize(expected));
    assert_eq!(d.assignment(), &actual.assignment);
    assert_eq!(Some(d.score()), actual.score);
    d.assert_consistent().unwrap();

    let mp = MixedProblem::new(bp, lp);
    let base = ordered_sequence();
    let config = MixedSolverSequence {
        limits: base.limits,
        phases: base
            .phases
            .into_iter()
            .map(|phase| match phase {
                SolverPhase::Construction(construction) => {
                    MixedSolverPhase::Construction(MixedConstructionConfig {
                        construction,
                        order: MixedConstructionOrder::ListsThenBasic,
                    })
                }
                SolverPhase::LocalSearch(search) => {
                    MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                        search,
                        joint_moves: true,
                    })
                }
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::RuinRecreate(search) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search,
                        order: MixedConstructionOrder::ListsThenBasic,
                    })
                }
            })
            .collect(),
    };
    actual_events.clear();
    expected_events.clear();
    let command = |index,
                   ctx: &mut CustomPhaseContext<
        '_,
        MixedAssignment<i32, i32>,
        MixedMove<i32, i32>,
        SimpleScore,
        MixedStreamScoringError,
    >| {
        assert_eq!(index, 1);
        ctx.execute(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(0),
            },
            lists: ListMove::Composite(vec![]),
        })?;
        Ok(())
    };
    let expected = solve_mixed_stream_with_commands_and_events(
        &mp,
        || Ok(mixed_graph()),
        config.clone(),
        command,
        |e| expected_events.push(format!("{e:?}")),
    )
    .unwrap();
    let (g, s) = mixed_graph();
    let mut d = MixedStreamScoreDirector::new(&mp, g, s, mp.initial_assignment()).unwrap();
    let actual = d
        .run_solver_with_commands_and_events(config, command, |e| {
            actual_events.push(format!("{e:?}"))
        })
        .unwrap();
    assert_eq!(actual_events, expected_events);
    assert_eq!(normalize(actual.clone()), normalize(expected));
    assert_eq!(d.assignment(), &actual.assignment);
    assert_eq!(Some(d.score()), actual.score);
    d.assert_consistent().unwrap();
}
#[test]
fn custom_best_restore_unbound_validation_and_caught_observer_panics() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let config = SolverSequence {
        phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
        ..Default::default()
    };
    let (g, s) = basic_graph();
    let mut d = StreamScoreDirector::new(&p, g, s, p.initial_assignment()).unwrap();
    assert!(matches!(
        d.run_solver(config.clone()),
        Err(SolverError::UnboundCustomPhase { phase: 0 })
    ));
    assert!(matches!(
        d.run_solver(SolverSequence {
            phases: vec![],
            ..Default::default()
        }),
        Err(SolverError::EmptyPlan)
    ));
    let result = d
        .run_solver_with_commands(config.clone(), |_, ctx| {
            ctx.execute(&Move::Change {
                variable: 0,
                value: Some(2),
            })?;
            ctx.execute(&Move::Change {
                variable: 0,
                value: Some(0),
            })?;
            Ok(())
        })
        .unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(2)));
    assert_eq!(d.score(), SimpleScore::new(2));
    assert_eq!(d.assignment().values(), &[Some(2)]);
    d.replace_assignment(p.initial_assignment())
        .unwrap()
        .commit();
    let error = d
        .run_solver_with_commands_and_events(
            config,
            |_, ctx| {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| ctx.execute(&Move::Change {
                        variable: 0,
                        value: Some(2)
                    })))
                    .is_err()
                );
                Ok(())
            },
            |event| {
                if let SolverEvent::Best(update) = event
                    && update.score == SimpleScore::new(2)
                {
                    panic!("observer failure");
                }
            },
        )
        .unwrap_err();
    assert!(matches!(
        error,
        SolverError::Custom {
            error: CustomPhaseError::IgnoredOperationError,
            ..
        }
    ));
    d.assert_consistent().unwrap();
}

#[test]
fn repair_only_sequences_preserve_temporary_pins_ranges_dependencies_and_budgets() {
    let bp = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2])
            .with_initial(1)
            .with_pinned(true),
        Variable::new(vec![0, 1, 2]).with_initial(0),
    ])
    .unwrap();
    let lp = ListProblem::new(
        vec![1, 2, 3],
        vec![
            ListEntity::new(vec![1, 3]).with_pinned_prefix(1),
            ListEntity::new(vec![2]).with_value_range(vec![2, 3]),
        ],
    )
    .unwrap();
    for dependencies in [false, true] {
        let lp = if dependencies {
            lp.clone()
                .with_dependencies(vec![vec![], vec![], vec![1]])
                .unwrap()
        } else {
            lp.clone()
        };
        for optional in [false, true] {
            let lp = if optional {
                lp.clone().allowing_unassigned()
            } else {
                lp.clone()
            };
            let mp = MixedProblem::new(bp.clone(), lp.clone());
            for max_evaluations in [0, 1, 7, 50] {
                let repair = RuinRecreateConfig {
                    seed: 37,
                    max_attempts: 10,
                    max_evaluations,
                    minimum_ruined: 1,
                    maximum_ruined: 2,
                    move_weights: RepairMoveWeights {
                        change: 1,
                        swap: 1,
                        repair: 3,
                    },
                    ..Default::default()
                };
                let config = SolverSequence {
                    phases: vec![SolverPhase::RuinRecreate(repair.clone())],
                    ..Default::default()
                };
                let expected = solve_stream(&bp, || Ok(basic_graph()), config.clone()).unwrap();
                let (g, s) = basic_graph();
                let mut d = StreamScoreDirector::new(&bp, g, s, bp.initial_assignment()).unwrap();
                let actual = d.run_solver(config.clone()).unwrap();
                assert_eq!(normalize(actual.clone()), normalize(expected));
                assert_eq!(d.assignment(), &actual.assignment);
                if let Some(score) = actual.score {
                    assert_eq!(d.score(), score);
                }
                d.assert_consistent().unwrap();
                let expected = solve_list_stream(&lp, || Ok(list_graph()), config);
                let (g, s) = list_graph();
                let mut d =
                    ListStreamScoreDirector::new(&lp, g, s, lp.initial_assignment()).unwrap();
                let actual = d.run_solver(SolverSequence {
                    phases: vec![SolverPhase::RuinRecreate(repair.clone())],
                    ..Default::default()
                });
                assert!(expected.is_ok(), "fresh repair failed: {expected:?}");
                assert!(actual.is_ok(), "retained repair failed: {actual:?}");
                assert_eq!(
                    format!("{:?}", actual.as_ref().map(|r| normalize(r.clone()))),
                    format!("{:?}", expected.map(normalize))
                );
                if let Ok(actual) = actual {
                    assert_eq!(d.assignment(), &actual.assignment);
                    if let Some(score) = actual.score {
                        assert_eq!(d.score(), score);
                    }
                }
                d.assert_consistent().unwrap();
                let config = MixedSolverSequence {
                    phases: vec![MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search: repair,
                        order: MixedConstructionOrder::ListsThenBasic,
                    })],
                    ..Default::default()
                };
                let expected = solve_mixed_stream(&mp, || Ok(mixed_graph()), config.clone());
                let (g, s) = mixed_graph();
                let mut d =
                    MixedStreamScoreDirector::new(&mp, g, s, mp.initial_assignment()).unwrap();
                let actual = d.run_solver(config);
                assert!(expected.is_ok(), "fresh repair failed: {expected:?}");
                assert!(actual.is_ok(), "retained repair failed: {actual:?}");
                assert_eq!(
                    format!("{:?}", actual.as_ref().map(|r| normalize(r.clone()))),
                    format!("{:?}", expected.map(normalize))
                );
                if let Ok(actual) = actual {
                    assert_eq!(d.assignment(), &actual.assignment);
                    if let Some(score) = actual.score {
                        assert_eq!(d.score(), score);
                    }
                }
                d.assert_consistent().unwrap();
            }
        }
    }
}

struct RecordedProvider<Q> {
    inner: Q,
    events: std::rc::Rc<std::cell::RefCell<Vec<MoveProviderEvent>>>,
}
impl<P, A, M, Q: MoveProvider<P, A, M>> MoveProvider<P, A, M> for RecordedProvider<Q> {
    fn event(&mut self, event: MoveProviderEvent) {
        self.events.borrow_mut().push(event);
        self.inner.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        self.inner.construction_placement_completed(installed)
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.inner.moves(p, a, step)
    }
}
fn extension_sequence(max_evaluations: u64) -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            max_evaluations,
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::Custom(CustomPhaseConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 6,
                selection: MoveSelection::FirstAccepted,
                acceptance: Acceptance::SimulatedAnnealing {
                    starting_temperature: SimpleScore::new(100),
                    seed: 17,
                    cooling_steps: 50,
                },
                ..Default::default()
            }),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 2,
                ..Default::default()
            }),
        ],
    }
}

#[test]
fn retained_basic_extensions_preserve_providers_commands_events_and_score_checks() {
    let p: FiniteProblem<i32> = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap();
    for max_evaluations in [0, 1, 10, 100] {
        for max_scans in [0, 1, 40] {
            let config = extension_sequence(max_evaluations);
            let mut observations = Vec::new();
            for retained in [false, true] {
                let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
                let mut provider = RecordedProvider {
                    inner: extension_basic_proposals,
                    events: log.clone(),
                };
                let mut events = vec![];
                let mut checks = vec![];
                let mut commands = vec![];
                let mut observe = |e: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
                    events.push(format!("{e:?}"))
                };
                let mut check = |phase, kind, a: &Assignment<i32>| {
                    checks.push((phase, kind));
                    Ok(Some(
                        SimpleScore::new(a.values().iter().flatten().map(|v| i64::from(*v)).sum())
                            .with_structural(
                                -(a.values().iter().filter(|v| v.is_none()).count() as i64),
                            ),
                    ))
                };
                let mut command = |phase,
                                   _: &mut CustomPhaseContext<
                    '_,
                    Assignment<i32>,
                    Move<i32>,
                    SimpleScore,
                    StreamScoringError,
                >| {
                    assert_eq!(phase, 1);
                    commands.push(phase);
                    Ok(())
                };
                let mut ext = SolverExtensions {
                    events: Some(&mut observe),
                    command: Some(&mut command),
                    score_check: Some(&mut check),
                    ..Default::default()
                };
                ext.move_providers
                    .bind_shared([0, 2, 3], &mut provider)
                    .unwrap();
                ext.move_providers.max_construction_scans = max_scans;
                let result = if retained {
                    let (g, source) = basic_graph();
                    let mut d =
                        StreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
                    let result = d.run_solver_with_extensions(config.clone(), ext).unwrap();
                    assert_eq!(d.assignment(), &result.assignment);
                    if let Some(score) = result.score {
                        assert_eq!(d.score(), score);
                    }
                    d.assert_consistent().unwrap();
                    result
                } else {
                    solve_stream_with_extensions(&p, || Ok(basic_graph()), config.clone(), ext)
                        .unwrap()
                };
                let lifecycle = log.borrow().clone();
                assert_eq!(
                    lifecycle
                        .iter()
                        .filter(|e| **e == MoveProviderEvent::SolveStarted)
                        .count(),
                    1
                );
                assert_eq!(
                    lifecycle
                        .iter()
                        .filter(|e| **e == MoveProviderEvent::SolveEnded)
                        .count(),
                    1
                );
                observations.push((normalize(result), events, checks, commands, lifecycle));
            }
            assert_eq!(
                observations[0], observations[1],
                "evaluations={max_evaluations}, scans={max_scans}"
            );
        }
    }
    // Standalone provider phases expose the same retained construction/search path.
    let (g, source) = basic_graph();
    let mut d = StreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
    let mut provider = extension_basic_proposals;
    let mut actual = d
        .run_construction_with_provider(ProviderConstructionConfig::default(), &mut provider)
        .unwrap();
    let (g, source) = basic_graph();
    let mut expected = stream_construction_from_with_provider(
        &p,
        g,
        source,
        p.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut extension_basic_proposals,
    )
    .unwrap();
    actual.constraint_evaluations = 0;
    expected.constraint_evaluations = 0;
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    let initial = actual.assignment;
    let local = LocalSearchConfig {
        max_steps: 4,
        ..Default::default()
    };
    let mut actual = d
        .run_local_search_with_provider(local.clone(), &mut provider)
        .unwrap();
    let (g, source) = basic_graph();
    let mut expected = stream_local_search_from_with_provider(
        &p,
        g,
        source,
        initial,
        local,
        &mut extension_basic_proposals,
    )
    .unwrap();
    actual.constraint_evaluations = 0;
    expected.constraint_evaluations = 0;
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(d.assignment(), &actual.best.unwrap().assignment);
    d.assert_consistent().unwrap();
}

#[test]
fn retained_list_extensions_preserve_providers_commands_events_and_score_checks() {
    let p: ListProblem<i32> =
        ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2]).unwrap();
    for max_evaluations in [0, 1, 10, 100] {
        for max_scans in [0, 1, 40] {
            let config = extension_sequence(max_evaluations);
            let mut observations = Vec::new();
            for retained in [false, true] {
                let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
                let mut provider = RecordedProvider {
                    inner: extension_list_proposals,
                    events: log.clone(),
                };
                let mut events = vec![];
                let mut checks = vec![];
                let mut commands = vec![];
                let mut observe = |e: SolverEvent<'_, ListAssignment<i32>, SimpleScore>| {
                    events.push(format!("{e:?}"))
                };
                let mut check = |phase, kind, a: &ListAssignment<i32>| {
                    checks.push((phase, kind));
                    Ok(Some(
                        SimpleScore::new(
                            a.lists()
                                .iter()
                                .flat_map(|l| {
                                    l.iter()
                                        .enumerate()
                                        .map(|(i, v)| (i as i64 + 1) * i64::from(*v))
                                })
                                .sum(),
                        )
                        .with_structural(
                            -((p.values().len() - a.lists().iter().map(Vec::len).sum::<usize>())
                                as i64),
                        ),
                    ))
                };
                let mut command = |phase,
                                   _: &mut CustomPhaseContext<
                    '_,
                    ListAssignment<i32>,
                    ListMove<i32>,
                    SimpleScore,
                    ListStreamScoringError,
                >| {
                    assert_eq!(phase, 1);
                    commands.push(phase);
                    Ok(())
                };
                let mut ext = SolverExtensions {
                    events: Some(&mut observe),
                    command: Some(&mut command),
                    score_check: Some(&mut check),
                    ..Default::default()
                };
                ext.move_providers
                    .bind_shared([0, 2, 3], &mut provider)
                    .unwrap();
                ext.move_providers.max_construction_scans = max_scans;
                let result = if retained {
                    let (g, source) = list_graph();
                    let mut d = ListStreamScoreDirector::new(&p, g, source, p.initial_assignment())
                        .unwrap();
                    let result = d.run_solver_with_extensions(config.clone(), ext).unwrap();
                    assert_eq!(d.assignment(), &result.assignment);
                    if let Some(score) = result.score {
                        assert_eq!(d.score(), score);
                    }
                    d.assert_consistent().unwrap();
                    result
                } else {
                    solve_list_stream_with_extensions(&p, || Ok(list_graph()), config.clone(), ext)
                        .unwrap()
                };
                let lifecycle = log.borrow().clone();
                assert_eq!(
                    lifecycle
                        .iter()
                        .filter(|e| **e == MoveProviderEvent::SolveStarted)
                        .count(),
                    1
                );
                assert_eq!(
                    lifecycle
                        .iter()
                        .filter(|e| **e == MoveProviderEvent::SolveEnded)
                        .count(),
                    1
                );
                observations.push((normalize(result), events, checks, commands, lifecycle));
            }
            assert_eq!(
                observations[0], observations[1],
                "evaluations={max_evaluations}, scans={max_scans}"
            );
        }
    }
    // Standalone provider phases expose the same retained construction/search path.
    let (g, source) = list_graph();
    let mut d = ListStreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
    let mut provider = extension_list_proposals;
    let mut actual = d
        .run_construction_with_provider(ProviderConstructionConfig::default(), &mut provider)
        .unwrap();
    let (g, source) = list_graph();
    let mut expected = list_stream_construction_from_with_provider(
        &p,
        g,
        source,
        p.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut extension_list_proposals,
    )
    .unwrap();
    actual.constraint_evaluations = 0;
    expected.constraint_evaluations = 0;
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    let initial = actual.assignment;
    let local = LocalSearchConfig {
        max_steps: 4,
        ..Default::default()
    };
    let mut actual = d
        .run_local_search_with_provider(local.clone(), &mut provider)
        .unwrap();
    let (g, source) = list_graph();
    let mut expected = list_stream_local_search_from_with_provider(
        &p,
        g,
        source,
        initial,
        local,
        &mut extension_list_proposals,
    )
    .unwrap();
    actual.constraint_evaluations = 0;
    expected.constraint_evaluations = 0;
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(d.assignment(), &actual.best.unwrap().assignment);
    d.assert_consistent().unwrap();
}

#[test]
fn retained_mixed_extensions_preserve_providers_commands_events_and_score_checks() {
    let p: MixedProblem<i32, i32> = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]); 2]).unwrap(),
        ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![]); 2]).unwrap(),
    );
    for max_evaluations in [0, 1, 10, 100] {
        for max_scans in [0, 1, 40] {
            let config = MixedSolverSequence {
                limits: extension_sequence(max_evaluations).limits,
                phases: extension_sequence(max_evaluations)
                    .phases
                    .into_iter()
                    .map(|phase| match phase {
                        SolverPhase::Construction(c) => {
                            MixedSolverPhase::Construction(MixedConstructionConfig {
                                construction: c,
                                order: MixedConstructionOrder::ListsThenBasic,
                            })
                        }
                        SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
                        SolverPhase::LocalSearch(c) => {
                            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                                search: c,
                                joint_moves: true,
                            })
                        }
                        _ => unreachable!(),
                    })
                    .collect(),
            };
            let mut observations = Vec::new();
            for retained in [false, true] {
                let log = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
                let mut provider = RecordedProvider {
                    inner: extension_mixed_proposals,
                    events: log.clone(),
                };
                let mut events = vec![];
                let mut checks = vec![];
                let mut commands = vec![];
                let mut observe = |e: SolverEvent<'_, MixedAssignment<i32, i32>, SimpleScore>| {
                    events.push(format!("{e:?}"))
                };
                let mut check = |phase, kind, a: &MixedAssignment<i32, i32>| {
                    checks.push((phase, kind));
                    Ok(Some(
                        SimpleScore::new(
                            a.basic()
                                .values()
                                .iter()
                                .zip(a.lists().lists())
                                .map(|(v, l)| i64::from(v.unwrap_or(0)) * l.len() as i64)
                                .sum(),
                        )
                        .with_structural(
                            -((a.basic().values().iter().filter(|v| v.is_none()).count()
                                + p.lists().values().len()
                                - a.lists().lists().iter().map(Vec::len).sum::<usize>())
                                as i64),
                        ),
                    ))
                };
                let mut command = |phase,
                                   _: &mut CustomPhaseContext<
                    '_,
                    MixedAssignment<i32, i32>,
                    MixedMove<i32, i32>,
                    SimpleScore,
                    MixedStreamScoringError,
                >| {
                    assert_eq!(phase, 1);
                    commands.push(phase);
                    Ok(())
                };
                let mut ext = SolverExtensions {
                    events: Some(&mut observe),
                    command: Some(&mut command),
                    score_check: Some(&mut check),
                    ..Default::default()
                };
                ext.move_providers
                    .bind_shared([0, 2, 3], &mut provider)
                    .unwrap();
                ext.move_providers.max_construction_scans = max_scans;
                let result = if retained {
                    let (g, source) = mixed_graph();
                    let mut d =
                        MixedStreamScoreDirector::new(&p, g, source, p.initial_assignment())
                            .unwrap();
                    let result = d.run_solver_with_extensions(config.clone(), ext).unwrap();
                    assert_eq!(d.assignment(), &result.assignment);
                    if let Some(score) = result.score {
                        assert_eq!(d.score(), score);
                    }
                    d.assert_consistent().unwrap();
                    result
                } else {
                    solve_mixed_stream_with_extensions(
                        &p,
                        || Ok(mixed_graph()),
                        config.clone(),
                        ext,
                    )
                    .unwrap()
                };
                let lifecycle = log.borrow().clone();
                assert_eq!(
                    lifecycle
                        .iter()
                        .filter(|e| **e == MoveProviderEvent::SolveStarted)
                        .count(),
                    1
                );
                assert_eq!(
                    lifecycle
                        .iter()
                        .filter(|e| **e == MoveProviderEvent::SolveEnded)
                        .count(),
                    1
                );
                observations.push((normalize(result), events, checks, commands, lifecycle));
            }
            assert_eq!(
                observations[0], observations[1],
                "evaluations={max_evaluations}, scans={max_scans}"
            );
        }
    }
    // Standalone provider phases expose the same retained construction/search path.
    let (g, source) = mixed_graph();
    let mut d = MixedStreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
    let mut provider = extension_mixed_proposals;
    let mut actual = d
        .run_construction_with_provider(ProviderConstructionConfig::default(), &mut provider)
        .unwrap();
    let (g, source) = mixed_graph();
    let mut expected = mixed_stream_construction_from_with_provider(
        &p,
        g,
        source,
        p.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut extension_mixed_proposals,
    )
    .unwrap();
    actual.constraint_evaluations = 0;
    expected.constraint_evaluations = 0;
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    let initial = actual.assignment;
    let local = LocalSearchConfig {
        max_steps: 4,
        ..Default::default()
    };
    let local = MixedLocalSearchConfig {
        search: local,
        joint_moves: true,
    };
    let mut actual = d
        .run_local_search_with_provider(local.clone(), &mut provider)
        .unwrap();
    let (g, source) = mixed_graph();
    let mut expected = mixed_stream_local_search_from_with_provider(
        &p,
        g,
        source,
        initial,
        local,
        &mut extension_mixed_proposals,
    )
    .unwrap();
    actual.constraint_evaluations = 0;
    expected.constraint_evaluations = 0;
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(d.assignment(), &actual.best.unwrap().assignment);
    d.assert_consistent().unwrap();
}

#[test]
fn retained_extension_failures_close_lifecycles_without_rebuilding_facts() {
    use std::{
        cell::RefCell,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };
    for mode in 0..5 {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
        let mut observations = vec![];
        for retained in [false, true] {
            let facts = Cell::new(0);
            let planning = Cell::new(0);
            let graph = || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i32>)>();
                b.score("planning", source.port(), |(_, value)| {
                    planning.set(planning.get() + 1);
                    SimpleScore::new(i64::from(value.unwrap_or(0)))
                })
                .unwrap();
                let f = b.source::<usize, i32>();
                b.score("immutable", f.port(), |_| {
                    facts.set(facts.get() + 1);
                    SimpleScore::new(10)
                })
                .unwrap();
                let mut g = b.build();
                let mut tx = g.transaction();
                let mut batch = StreamBatch::new();
                batch.push(&f, TupleChange::put(0, 1)).unwrap();
                tx.apply_batch(batch).unwrap();
                tx.commit().unwrap();
                (g, source)
            };
            let events = Rc::new(RefCell::new(vec![]));
            let mut provider = RecordedProvider {
                events: events.clone(),
                inner: |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
                    assert_ne!(mode, 4, "provider panic");
                    vec![Move::Change {
                        variable: if mode == 3 { 99 } else { 0 },
                        value: Some(2),
                    }]
                },
            };
            let mut check = |_, _, _: &Assignment<i32>| {
                if mode == 2 {
                    Err("independent check failed".to_owned())
                } else {
                    Ok(Some(SimpleScore::new(999)))
                }
            };
            let mut ext = SolverExtensions::default();
            ext.move_providers
                .bind(if mode == 0 { 9 } else { 0 }, &mut provider);
            if mode == 1 || mode == 2 {
                ext.score_check = Some(&mut check);
            }
            let config = SolverSequence {
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 1,
                    ..Default::default()
                })],
                ..Default::default()
            };
            let outcome = if retained {
                let (g, source) = graph();
                let mut d =
                    StreamScoreDirector::new(&p, g, source, p.initial_assignment()).unwrap();
                let before = planning.get();
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    d.run_solver_with_extensions(config, ext)
                }));
                assert_eq!(facts.get(), 1, "retained call replayed immutable facts");
                if mode == 0 {
                    assert_eq!(planning.get(), before);
                }
                if mode == 1 || mode == 2 {
                    assert_eq!(d.assignment().values(), &[Some(2)]);
                } else {
                    assert_eq!(d.assignment().values(), &[Some(0)]);
                }
                d.assert_consistent().unwrap();
                outcome
            } else {
                catch_unwind(AssertUnwindSafe(|| {
                    solve_stream_with_extensions(&p, || Ok(graph()), config, ext)
                }))
            };
            let outcome = match outcome {
                Ok(Err(error)) => format!("{error:?}"),
                Err(_) => {
                    assert_eq!(mode, 4);
                    "panic".to_owned()
                }
                Ok(Ok(_)) => panic!("expected failure"),
            };
            let log = events.borrow().clone();
            if mode == 0 {
                assert!(log.is_empty());
            } else if mode == 4 {
                assert!(!log.contains(&MoveProviderEvent::SolveEnded));
            } else {
                assert_eq!(log.last(), Some(&MoveProviderEvent::SolveEnded));
            }
            observations.push((outcome, log));
        }
        assert_eq!(observations[0], observations[1], "mode {mode}");
    }
}

fn extension_basic_proposals(
    p: &FiniteProblem<i32>,
    a: &Assignment<i32>,
    _: u64,
) -> Vec<Move<i32>> {
    change_moves(p, a).unwrap().collect()
}
fn extension_list_proposals(
    p: &ListProblem<i32>,
    a: &ListAssignment<i32>,
    _: u64,
) -> Vec<ListMove<i32>> {
    list_assign_moves(p, a)
        .unwrap()
        .chain(list_change_moves(p, a).unwrap())
        .chain(list_swap_moves(p, a).unwrap())
        .collect()
}
fn extension_mixed_proposals(
    p: &MixedProblem<i32, i32>,
    a: &MixedAssignment<i32, i32>,
    step: u64,
) -> Vec<MixedMove<i32, i32>> {
    extension_basic_proposals(p.basic(), a.basic(), step)
        .into_iter()
        .map(|basic| MixedMove {
            basic,
            lists: ListMove::Composite(vec![]),
        })
        .chain(
            extension_list_proposals(p.lists(), a.lists(), step)
                .into_iter()
                .map(|lists| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists,
                }),
        )
        .collect()
}
