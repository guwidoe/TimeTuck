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
