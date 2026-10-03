//! Original controlled-clock construction checks; no upstream method credit.
use crate::*;
use std::{cell::Cell, time::Duration};

#[derive(Debug, PartialEq)]
struct Outcome {
    values: Vec<Option<usize>>,
    score: Option<SimpleScore>,
    complete: bool,
    evaluations: u64,
    callbacks: u128,
    assigned: usize,
    reason: ConstructionTermination,
}
fn objective(v: &[Option<usize>]) -> SimpleScore {
    SimpleScore::new(v.iter().flatten().map(|&v| v as i64).sum())
}
fn run(
    mode: usize,
    order: MixedConstructionOrder,
    config: ConstructionConfig<'_>,
    hook: impl Fn(),
    clock: impl FnMut() -> Duration,
) -> Outcome {
    let score = |v: &[Option<usize>]| {
        hook();
        objective(v)
    };
    macro_rules! outcome {
        ($r:expr, $p:expr, $values:expr, $assigned:expr) => {{
            let r = $r.unwrap();
            assert_eq!(r.move_evaluations(), r.evaluations.saturating_sub(1));
            $p.validate(&r.assignment).unwrap();
            let values: Vec<Option<usize>> = $values(&r.assignment);
            assert_eq!(r.complete, values.iter().all(Option::is_some));
            assert_eq!(r.solution().is_some(), r.complete && r.score.is_some());
            if let Some(s) = r.score {
                assert_eq!(
                    s,
                    objective(&values)
                        .with_structural(-(values.iter().filter(|v| v.is_none()).count() as i64))
                );
            }
            Outcome {
                values,
                score: r.score,
                complete: r.complete,
                evaluations: r.evaluations,
                callbacks: r.constraint_evaluations,
                assigned: $assigned(&r),
                reason: r.termination,
            }
        }};
    }
    if mode < 2 {
        let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]); 2]).unwrap();
        let values = |a: &Assignment<usize>| a.values().to_vec();
        let assigned = |r: &ConstructionResult<usize, SimpleScore>| r.assigned_variables;
        if mode == 0 {
            let mut n = ConstraintNetwork::new(2);
            n.add_constraint("value", vec![0, 1], |v| {
                score(&v.iter().map(|v| v.copied()).collect::<Vec<_>>())
            })
            .unwrap();
            return outcome!(
                construction::construction_impl_with_clock(
                    &p,
                    config,
                    |a| ScoreDirector::new(&p, &n, a),
                    clock
                ),
                p,
                values,
                assigned
            );
        }
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        let a = b.filter(s.port(), |(i, _)| *i == 0).unwrap();
        let z = b.filter(s.port(), |(i, _)| *i == 1).unwrap();
        let j = b.join_equal(&a, &z, |_| (), |_| ()).unwrap();
        b.score("value", &j, |((_, a), (_, z))| score(&[*a, *z]))
            .unwrap();
        return outcome!(
            construction::construction_impl_with_clock(
                &p,
                config,
                |a| StreamScoreDirector::new(&p, b.build(), s, a),
                clock
            ),
            p,
            values,
            assigned
        );
    }
    let lists = |values| {
        ListProblem::new(
            values,
            vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
        )
        .unwrap()
    };
    if mode < 4 {
        let p = lists(vec![7, 8]);
        let values = |a: &ListAssignment<usize>| {
            [7, 8]
                .iter()
                .map(|v| a.location(v).map(|pos| pos.entity))
                .collect::<Vec<_>>()
        };
        let assigned = |r: &ListConstructionResult<usize, SimpleScore>| r.assigned_values;
        let evaluate_lists = |a: &[usize], z: &[usize]| {
            score(
                &[7, 8]
                    .iter()
                    .map(|v| {
                        if a.contains(v) {
                            Some(0)
                        } else if z.contains(v) {
                            Some(1)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        };
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(2, 2);
            n.add_constraint(
                "value",
                vec![ListScope::Entity(0), ListScope::Entity(1)],
                |v| {
                    let (ListFact::Entity(a), ListFact::Entity(z)) = (&v[0], &v[1]) else {
                        panic!()
                    };
                    evaluate_lists(a, z)
                },
            )
            .unwrap();
            return outcome!(
                list_construction::construction_impl_with_clock(
                    &p,
                    p.initial_assignment(),
                    config,
                    |a| ListScoreDirector::new(&p, &n, a),
                    clock
                ),
                p,
                values,
                assigned
            );
        }
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::<usize>::new(&mut b);
        let a = b.filter(s.entities.port(), |(i, _)| *i == 0).unwrap();
        let z = b.filter(s.entities.port(), |(i, _)| *i == 1).unwrap();
        let j = b.join_equal(&a, &z, |_| (), |_| ()).unwrap();
        b.score("value", &j, |((_, a), (_, z))| evaluate_lists(a, z))
            .unwrap();
        return outcome!(
            list_construction::construction_impl_with_clock(
                &p,
                p.initial_assignment(),
                config,
                |a| ListStreamScoreDirector::new(&p, b.build(), s, a),
                clock
            ),
            p,
            values,
            assigned
        );
    }
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0usize, 1])]).unwrap(),
        lists(vec![7]),
    );
    let values = |a: &MixedAssignment<usize, usize>| {
        vec![
            a.basic().values()[0],
            a.lists().location(&7).map(|p| p.entity),
        ]
    };
    let assigned = |r: &MixedConstructionResult<usize, usize, SimpleScore>| {
        r.assigned_basic_variables + r.assigned_list_values
    };
    let config = MixedConstructionConfig {
        construction: config,
        order,
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(1, 2, 1);
        n.add_constraint(
            "value",
            vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
            |v| {
                let (MixedFact::Basic(a), MixedFact::ListValue { element, .. }) = (&v[0], &v[1])
                else {
                    panic!()
                };
                score(&[a.copied(), element.as_ref().map(|v| v.position.entity)])
            },
        )
        .unwrap();
        return outcome!(
            mixed_construction::construction_impl_with_clock(
                &p,
                p.initial_assignment(),
                config,
                |a| MixedScoreDirector::new(&p, &n, a),
                clock
            ),
            p,
            values,
            assigned
        );
    }
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<usize, usize>::new(&mut b);
    let j = b
        .join_equal(s.basic.port(), s.lists.values.port(), |_| (), |_| ())
        .unwrap();
    b.score("value", &j, |((_, a), v)| {
        score(&[*a, v.position.map(|p| p.entity)])
    })
    .unwrap();
    outcome!(
        mixed_construction::construction_impl_with_clock(
            &p,
            p.initial_assignment(),
            config,
            |a| MixedStreamScoreDirector::new(&p, b.build(), s, a),
            clock
        ),
        p,
        values,
        assigned
    )
}
const ORDERS: [MixedConstructionOrder; 2] = [
    MixedConstructionOrder::BasicThenLists,
    MixedConstructionOrder::ListsThenBasic,
];

#[test]
fn elapsed_limits_keep_the_same_scored_prefix_as_evaluation_caps() {
    for mode in 0..6 {
        for order in ORDERS {
            for pick_early in [
                ConstructionPickEarly::Never,
                ConstructionPickEarly::FirstFeasibleScore,
            ] {
                let base = ConstructionConfig {
                    pick_early,
                    ..Default::default()
                };
                let full = run(mode, order, base, || {}, || panic!("disabled clock"));
                for limit in 0..=full.evaluations + 1 {
                    let calls = Cell::new(0);
                    let actual = run(
                        mode,
                        order,
                        ConstructionConfig {
                            max_time: Some(Duration::from_millis(limit)),
                            ..base
                        },
                        || calls.set(calls.get() + 1),
                        || Duration::from_millis(calls.get()),
                    );
                    let mut expected = run(
                        mode,
                        order,
                        ConstructionConfig {
                            max_evaluations: limit.max(1),
                            ..base
                        },
                        || {},
                        || panic!("disabled clock"),
                    );
                    if limit <= full.evaluations {
                        expected.reason = ConstructionTermination::TimeLimit;
                    }
                    assert_eq!(actual, expected, "mode {mode}, {order:?}, limit {limit}");
                }
            }
        }
    }
}

#[test]
fn initialization_and_diagnostics_count_as_time_but_not_move_work() {
    for mode in 0..6 {
        for order in ORDERS {
            let calls = Cell::new(0);
            let r = run(
                mode,
                order,
                ConstructionConfig {
                    max_time: Some(Duration::from_millis(2)),
                    assert_scores: true,
                    ..Default::default()
                },
                || calls.set(calls.get() + 1),
                || Duration::from_millis(calls.get()),
            );
            assert_eq!(r.evaluations, 1);
            assert_eq!(r.callbacks, 1);
            assert_eq!(calls.get(), 2);
            assert_eq!(r.assigned, 0);
            assert_eq!(r.reason, ConstructionTermination::TimeLimit);
        }
    }
}

#[test]
fn expiry_latches_when_a_clock_moves_backwards() {
    for mode in 0..6 {
        for order in ORDERS {
            let sample = Cell::new(0);
            let r = run(
                mode,
                order,
                ConstructionConfig {
                    max_time: Some(Duration::from_millis(3)),
                    ..Default::default()
                },
                || {},
                || {
                    sample.set(sample.get() + 1);
                    Duration::from_millis(match sample.get() {
                        1 => 1,
                        2 => 2,
                        3 => 3,
                        _ => 0,
                    })
                },
            );
            assert_eq!(r.evaluations, 2);
            assert_eq!(r.assigned, 1);
            assert!(!r.complete);
            assert_eq!(r.reason, ConstructionTermination::TimeLimit);
        }
    }
}

#[test]
fn first_observed_count_limit_wins_over_later_time_expiry() {
    for mode in 0..6 {
        for order in ORDERS {
            for moves in [false, true] {
                let sample = Cell::new(0);
                let r = run(
                    mode,
                    order,
                    ConstructionConfig {
                        max_time: Some(Duration::from_millis(4)),
                        max_evaluations: if moves { 100 } else { 2 },
                        max_move_evaluations: moves.then_some(1),
                        ..Default::default()
                    },
                    || {},
                    || {
                        sample.set(sample.get() + 1);
                        Duration::from_millis(sample.get())
                    },
                );
                assert_eq!(r.evaluations, 2);
                assert_eq!(
                    r.reason,
                    if moves {
                        ConstructionTermination::MoveEvaluationLimit
                    } else {
                        ConstructionTermination::EvaluationLimit
                    }
                );
            }
        }
    }
}

#[test]
fn zero_score_budget_and_cancellation_precede_time() {
    for mode in 0..6 {
        for order in ORDERS {
            let token = CancellationToken::new();
            token.cancel();
            for budget in [0, 100] {
                let r = run(
                    mode,
                    order,
                    ConstructionConfig {
                        cancellation: Some(&token),
                        max_time: Some(Duration::ZERO),
                        max_evaluations: budget,
                        ..Default::default()
                    },
                    || panic!("no callbacks"),
                    || panic!("no clock poll before cancellation"),
                );
                assert_eq!(r.evaluations, 0);
                assert_eq!(
                    r.reason,
                    if budget == 0 {
                        ConstructionTermination::EvaluationLimit
                    } else {
                        ConstructionTermination::Cancelled
                    }
                );
            }
            let token = CancellationToken::new();
            let r = run(
                mode,
                order,
                ConstructionConfig {
                    cancellation: Some(&token),
                    max_time: Some(Duration::ZERO),
                    ..Default::default()
                },
                || {
                    token.cancel();
                },
                || Duration::ZERO,
            );
            assert_eq!(r.evaluations, 1);
            assert_eq!(r.reason, ConstructionTermination::Cancelled);
        }
    }
}

#[test]
fn empty_and_optional_only_models_still_observe_expiry_after_initial_scoring() {
    for optional in [false, true] {
        let p = FiniteProblem::<usize>::new(if optional {
            vec![Variable::new(vec![0, 1]).allowing_unassigned()]
        } else {
            vec![]
        })
        .unwrap();
        let l = ListProblem::new(if optional { vec![7usize] } else { vec![] }, vec![])
            .unwrap()
            .allowing_unassigned();
        let config = ConstructionConfig {
            max_time: Some(Duration::ZERO),
            ..Default::default()
        };
        macro_rules! check {
            ($r:expr) => {{
                let r = $r.unwrap();
                assert_eq!(r.termination, ConstructionTermination::TimeLimit);
                assert_eq!(r.evaluations, 1);
                assert_eq!(r.move_evaluations(), 0);
                assert!(r.complete);
            }};
        }
        check!(greedy_construction(
            &p,
            &ConstraintNetwork::<_, SimpleScore>::new(p.variables().len()),
            config
        ));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, (usize, Option<usize>)>();
        check!(greedy_stream_construction(&p, b.build(), s, config));
        check!(greedy_list_construction(
            &l,
            &ListConstraintNetwork::<_, SimpleScore>::new(0, l.values().len()),
            config
        ));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::<usize>::new(&mut b);
        check!(greedy_list_stream_construction(&l, b.build(), s, config));
        let p = MixedProblem::new(p, l);
        let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(
            p.basic().variables().len(),
            0,
            p.lists().values().len(),
        );
        let c = MixedConstructionConfig {
            construction: config,
            ..Default::default()
        };
        check!(greedy_mixed_construction(&p, &n, c));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::<usize, usize>::new(&mut b);
        check!(greedy_mixed_stream_construction(&p, b.build(), s, c));
    }
}
