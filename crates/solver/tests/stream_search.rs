// Original search-backend integration regressions; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn score(values: &[Option<usize>]) -> HardSoftScore {
    let mut loads = [0; 3];
    let mut conflicts = 0;
    let mut preference = 0;
    for (i, &value) in values.iter().enumerate() {
        if let Some(v) = value {
            loads[v] += (i % 3 + 1) as i64;
            preference -= i64::from(v != i % 3);
            for (j, &other) in values.iter().enumerate().skip(i + 1) {
                if (i + j) % 3 != 0 && other == Some(v) {
                    conflicts -= 1;
                }
            }
        }
    }
    HardSoftScore::new(
        conflicts
            - loads
                .into_iter()
                .enumerate()
                .map(|(m, load)| (load - (m as i64 + 3)).max(0))
                .sum::<i64>(),
        preference,
    )
}
fn graph(
    calls: Rc<Cell<u128>>,
) -> (
    StreamGraph<'static, HardSoftScore>,
    PlanningStreamSource<usize>,
) {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let present = b.filter(source.port(), |(_, v)| v.is_some()).unwrap();
    let assigned = b.map(&present, |&(id, v)| (id, v.unwrap())).unwrap();
    let joined = b
        .join_equal(&assigned, &assigned, |v| v.1, |v| v.1)
        .unwrap();
    let conflicts = b
        .filter(&joined, |(a, b)| a.0 < b.0 && (a.0 + b.0) % 3 != 0)
        .unwrap();
    let c = calls.clone();
    b.score("conflicts", &conflicts, move |_| {
        c.set(c.get() + 1);
        HardSoftScore::new(-1, 0)
    })
    .unwrap();
    let groups = b
        .group_collect(&assigned, |&(id, machine)| (machine, (id % 3 + 1) as i64))
        .unwrap();
    let capacities = b.source::<usize, (usize, i64)>();
    let loads = b
        .join_equal(&groups, capacities.port(), |row| row.0, |row| row.0)
        .unwrap();
    let c = calls.clone();
    b.score("capacity", &loads, move |((_, s), (_, cap))| {
        c.set(c.get() + 1);
        HardSoftScore::new(-(s.sum - cap).max(0), 0)
    })
    .unwrap();
    b.score("preference", &assigned, move |&(id, value)| {
        calls.set(calls.get() + 1);
        HardSoftScore::new(0, -i64::from(value != id % 3))
    })
    .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for m in 0..3 {
        tx.apply(&capacities, TupleChange::put(m, (m, m as i64 + 3)))
            .unwrap();
    }
    tx.commit().unwrap();
    (graph, source)
}
fn network(n: usize) -> ConstraintNetwork<'static, usize, HardSoftScore> {
    let mut network = ConstraintNetwork::new(n);
    network
        .add_constraint("full", (0..n).collect(), |values| {
            score(&values.iter().map(|v| v.copied()).collect::<Vec<_>>())
        })
        .unwrap();
    network
}
fn problem(initial: bool) -> FiniteProblem<usize> {
    FiniteProblem::new(
        (0..5)
            .map(|i| {
                let mut v = Variable::new(if i == 2 { vec![0, 2] } else { vec![0, 1, 2] });
                if initial || i == 0 {
                    v = v.with_initial(0);
                }
                if i == 0 {
                    v = v.with_pinned(true);
                }
                if i == 4 {
                    v = v.allowing_unassigned();
                }
                v
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn construction_policies_budgets_and_phase_handoff_match_full_scoring() {
    let p = problem(false);
    let network = network(5);
    for policy in [
        ConstructionPickEarly::Never,
        ConstructionPickEarly::FirstNonDeterioratingScore,
        ConstructionPickEarly::FirstFeasibleScore,
        ConstructionPickEarly::FirstFeasibleScoreOrNonDeterioratingHard,
    ] {
        for budget in [0, 1, 2, 4, 8, 100] {
            let config = ConstructionConfig {
                max_steps: None,
                optional: timetuck::OptionalConstruction::Preserve,
                cancellation: None,
                max_move_evaluations: None,
                max_time: None,
                max_evaluations: budget,
                pick_early: policy,
                assert_scores: true,
            };
            let expected = greedy_construction(&p, &network, config).unwrap();
            let (g, s) = graph(Rc::new(Cell::new(0)));
            let actual = greedy_stream_construction(&p, g, s, config).unwrap();
            assert_eq!(actual.assignment, expected.assignment);
            assert_eq!(actual.score, expected.score);
            assert_eq!(actual.complete, expected.complete);
            assert_eq!(actual.termination, expected.termination);
            assert_eq!(actual.evaluations, expected.evaluations);
            assert_eq!(actual.assigned_variables, expected.assigned_variables);
            if let Some(solution) = actual.solution() {
                let config = LocalSearchConfig {
                    max_evaluations: 80,
                    assert_scores: true,
                    ..Default::default()
                };
                let expected =
                    local_search_from(&p, &network, solution.assignment.clone(), config).unwrap();
                let (g, s) = graph(Rc::new(Cell::new(0)));
                let actual =
                    stream_local_search_from(&p, g, s, solution.assignment, config).unwrap();
                compare(actual, expected);
            }
        }
    }
}
fn compare(
    mut actual: LocalSearchResult<usize, HardSoftScore>,
    mut expected: LocalSearchResult<usize, HardSoftScore>,
) {
    // Different scoring backends do different callback work; all solver behavior
    // and budgets must still agree, including seeded policies and fallback.
    actual.constraint_evaluations = 0;
    expected.constraint_evaluations = 0;
    assert_eq!(actual, expected);
    if let Some(best) = actual.best {
        assert_eq!(best.score, score(best.assignment.values()));
    }
}
#[test]
fn local_search_policies_seeds_selection_and_fallback_match_full_scoring() {
    let p = problem(true);
    let n = network(5);
    let extras = [
        Acceptance::LateAcceptance {
            history_size: 2,
            hill_climbing: true,
        },
        Acceptance::VariableTabu {
            tenure: 1,
            aspiration: true,
        },
    ];
    for (acceptance, additional_acceptance) in [
        Acceptance::StrictImprovement,
        Acceptance::HillClimbing,
        Acceptance::LateAcceptance {
            history_size: 3,
            hill_climbing: false,
        },
        Acceptance::VariableTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::MoveTabu {
            tenure: 2,
            aspiration: true,
        },
        Acceptance::FadingMoveTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 77,
        },
        Acceptance::FadingVariableTabu {
            hard_tenure: 1,
            fading_tenure: 4,
            aspiration: true,
            seed: 20261229,
        },
        Acceptance::SimulatedAnnealing {
            starting_temperature: HardSoftScore::new(2, 4),
            cooling_steps: 12,
            seed: 20261014,
        },
    ]
    .into_iter()
    .flat_map(|a| [(a, &[][..]), (a, &extras[..])])
    {
        for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
            for rejected_moves in [RejectedMovePolicy::Stop, RejectedMovePolicy::BestScore] {
                for move_order in [
                    MoveOrder::Original,
                    MoveOrder::ShuffledVariables { seed: 20261015 },
                ] {
                    for budget in [0, 1, 2, 17, 80] {
                        let config = LocalSearchConfig {
                            max_evaluations: budget,
                            max_steps: 15,
                            max_accepted_moves: 10,
                            acceptance,
                            additional_acceptance,
                            selection,
                            rejected_moves,
                            move_order,
                            assert_scores: true,
                            ..Default::default()
                        };
                        let expected = local_search(&p, &n, config).unwrap();
                        let (g, s) = graph(Rc::new(Cell::new(0)));
                        let actual = stream_local_search(&p, g, s, config).unwrap();
                        compare(actual, expected);
                    }
                }
            }
        }
    }
}
#[test]
fn scoring_counts_include_trials_but_not_winner_replay_or_diagnostic_rebuilds() {
    let p = problem(false);
    for diagnostics in [false, true] {
        let calls = Rc::new(Cell::new(0));
        let (g, s) = graph(calls.clone());
        let config = ConstructionConfig {
            assert_scores: diagnostics,
            ..Default::default()
        };
        let result = greedy_stream_construction(&p, g, s, config).unwrap();
        if diagnostics {
            assert!(calls.get() > result.constraint_evaluations);
        } else {
            assert_eq!(calls.get(), result.constraint_evaluations);
        }
        let calls = Rc::new(Cell::new(0));
        let (g, s) = graph(calls.clone());
        let config = LocalSearchConfig {
            max_evaluations: 80,
            max_steps: 8,
            selection: MoveSelection::BestAccepted,
            acceptance: Acceptance::VariableTabu {
                tenure: 2,
                aspiration: true,
            },
            assert_scores: diagnostics,
            ..Default::default()
        };
        let actual = stream_local_search_from(&p, g, s, result.assignment, config).unwrap();
        if diagnostics {
            assert!(calls.get() > actual.constraint_evaluations);
        } else {
            assert_eq!(calls.get(), actual.constraint_evaluations);
        }
    }
    // A single source/terminal has exactly one callback per trial plus initial
    // loading. Selecting an earlier winner must not call it again.
    let p = FiniteProblem::new(vec![Variable::new(vec![2_i64, 1, 0])]).unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i64>)>();
    b.score("value", s.port(), |(_, v)| {
        calls.set(calls.get() + 1);
        SimpleScore::new(v.unwrap_or(0))
    })
    .unwrap();
    let result =
        greedy_stream_construction(&p, b.build(), s, ConstructionConfig::default()).unwrap();
    assert_eq!(result.assignment.values(), &[Some(2)]);
    assert_eq!(result.evaluations, 4);
    assert_eq!(calls.get(), 4);
    assert_eq!(result.constraint_evaluations, 4);
}
#[test]
fn zero_budgets_impossible_models_empty_models_and_invalid_inputs() {
    let never = || {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        b.score("must not run", s.port(), |_| panic!("unexpected scoring"))
            .unwrap();
        (b.build(), s)
    };
    let p = FiniteProblem::new(vec![Variable::new(vec![0])]).unwrap();
    let (g, s) = never();
    let c = greedy_stream_construction(
        &p,
        g,
        s,
        ConstructionConfig {
            max_evaluations: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(c.evaluations, 0);
    assert_eq!(c.score, None);
    let (g, s) = never();
    let c = stream_local_search(
        &p,
        g,
        s,
        LocalSearchConfig {
            max_evaluations: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(c.best, None);
    assert_eq!(c.evaluations, 0);
    let impossible = FiniteProblem::new(vec![Variable::new(Vec::<usize>::new())]).unwrap();
    let (g, s) = never();
    assert_eq!(
        greedy_stream_construction(&impossible, g, s, ConstructionConfig::default())
            .unwrap()
            .termination,
        ConstructionTermination::NoCompleteAssignment
    );
    let (g, s) = never();
    assert_eq!(
        stream_local_search(&impossible, g, s, LocalSearchConfig::default())
            .unwrap()
            .termination,
        LocalSearchTermination::NoCompleteAssignment
    );
    let empty = FiniteProblem::<usize>::new(vec![]).unwrap();
    let (g, s) = never();
    let c = greedy_stream_construction(&empty, g, s, ConstructionConfig::default()).unwrap();
    assert!(c.complete);
    assert_eq!(c.score, Some(SimpleScore::ZERO));
    assert_eq!(c.evaluations, 1);
    let (g, s) = never();
    assert!(matches!(
        stream_local_search_from(
            &p,
            g,
            s,
            Assignment::new(vec![None]),
            LocalSearchConfig {
                max_evaluations: 0,
                ..Default::default()
            }
        ),
        Err(LocalSearchError::IncompleteInitialAssignment)
    ));
    let (g, s) = never();
    assert!(matches!(
        stream_local_search_from(
            &p,
            g,
            s,
            Assignment::new(vec![Some(9)]),
            LocalSearchConfig {
                max_evaluations: 0,
                ..Default::default()
            }
        ),
        Err(LocalSearchError::Scoring(_))
    ));
    let (g, s) = never();
    assert!(matches!(
        stream_local_search(
            &p,
            g,
            s,
            LocalSearchConfig {
                max_evaluations: 0,
                acceptance: Acceptance::VariableTabu {
                    tenure: 0,
                    aspiration: false
                },
                ..Default::default()
            }
        ),
        Err(LocalSearchError::Acceptance(_))
    ));
    let (g, _) = never();
    let (_, foreign) = never();
    assert!(matches!(
        greedy_stream_construction(
            &p,
            g,
            foreign,
            ConstructionConfig {
                max_evaluations: 0,
                ..Default::default()
            }
        ),
        Err(StreamScoringError::Graph(GraphError::ForeignPort))
    ));
}
#[test]
fn fresh_graph_assertions_detect_cancelling_match_changes_and_restore_on_failure() {
    let altered = Cell::new(false);
    let panic = Cell::new(false);
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, i64>();
    let c = b
        .score("value", s.port(), |&v| {
            assert!(!panic.get(), "injected callback panic");
            SimpleScore::new(if altered.get() { -v } else { v })
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    tx.apply(&s, TupleChange::put(0, 1)).unwrap();
    tx.apply(&s, TupleChange::put(1, -1)).unwrap();
    tx.commit().unwrap();
    graph.assert_consistent().unwrap();
    let before = graph.constraint_matches(&c).unwrap().clone();
    altered.set(true);
    assert_eq!(graph.assert_consistent(), Err(GraphError::CorruptedScore));
    assert_eq!(graph.score(), SimpleScore::ZERO);
    assert_eq!(graph.constraint_matches(&c).unwrap(), &before);
    altered.set(false);
    panic.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| graph.assert_consistent())).is_err());
    panic.set(false);
    graph.assert_consistent().unwrap();
    assert_eq!(graph.constraint_matches(&c).unwrap(), &before);
    let mut tx = graph.transaction();
    tx.apply(&s, TupleChange::put(0, 2)).unwrap();
    let calls = tx.score_evaluations();
    tx.assert_consistent().unwrap();
    assert_eq!(tx.score_evaluations(), calls);
    altered.set(true);
    assert_eq!(tx.assert_consistent(), Err(GraphError::CorruptedScore));
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    assert_eq!(graph.constraint_matches(&c).unwrap(), &before);
}

#[test]
fn preloaded_fact_scores_and_zero_step_limits_preserve_phase_accounting() {
    let p = FiniteProblem::<usize>::new(vec![]).unwrap();
    let calls = Cell::new(0);
    let build = || {
        let mut b = StreamGraphBuilder::new();
        let planning = b.source::<usize, (usize, Option<usize>)>();
        let facts = b.source::<usize, i64>();
        b.score("constant facts", facts.port(), |&v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(v)
        })
        .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&facts, TupleChange::put(0, 3)).unwrap();
        tx.apply(&facts, TupleChange::put(1, 4)).unwrap();
        tx.commit().unwrap();
        (g, planning)
    };
    let (g, s) = build();
    let before = calls.get();
    let result = greedy_stream_construction(&p, g, s, ConstructionConfig::default()).unwrap();
    assert_eq!(result.score, Some(SimpleScore::new(7)));
    assert!(result.complete);
    assert_eq!(result.constraint_evaluations, 0);
    assert_eq!(calls.get(), before);
    for (accepted, steps, termination) in [
        (0, 0, LocalSearchTermination::AcceptedMoveLimit),
        (1, 0, LocalSearchTermination::StepLimit),
        (1, 1, LocalSearchTermination::NoAcceptedMove),
    ] {
        let (g, s) = build();
        let before = calls.get();
        let config = LocalSearchConfig {
            max_accepted_moves: accepted,
            max_steps: steps,
            ..Default::default()
        };
        let result = stream_local_search(&p, g, s, config).unwrap();
        assert_eq!(result.best.unwrap().score, SimpleScore::new(7));
        assert_eq!(result.evaluations, 1);
        assert_eq!(result.constraint_evaluations, 0);
        assert_eq!(result.completed_steps, 0);
        assert_eq!(result.termination, termination);
        assert_eq!(calls.get(), before);
    }
}
