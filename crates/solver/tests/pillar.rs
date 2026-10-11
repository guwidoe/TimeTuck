//! Original pillar validation, enumeration, transactions and solver regressions.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap()
}
fn trap(a: Option<i32>, b: Option<i32>, owner: usize) -> SimpleScore {
    SimpleScore::new(match (a, b, owner) {
        (Some(0), Some(0), 0) => 0,
        (Some(1), Some(1), 1) => 10,
        _ => -5,
    })
}
fn network() -> ConstraintNetwork<'static, i32, SimpleScore> {
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("pair", vec![0, 1], |v| {
        trap(
            v[0].copied(),
            v[1].copied(),
            if v[0] == Some(&0) && v[1] == Some(&0) {
                0
            } else {
                1
            },
        )
    })
    .unwrap();
    n
}
fn graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    let pairs = b.join_equal(s.port(), s.port(), |_| 0, |_| 0).unwrap();
    let pair = b.filter(&pairs, |(a, b)| a.0 == 0 && b.0 == 1).unwrap();
    b.score("pair", &pair, |(a, b)| {
        trap(
            a.1,
            b.1,
            if a.1 == Some(0) && b.1 == Some(0) {
                0
            } else {
                1
            },
        )
    })
    .unwrap();
    (b.build(), s)
}
#[test]
fn pillars_escape_a_group_trap_in_both_basic_backends_under_seeded_orders_and_budgets() {
    let p = basic();
    let n = network();
    let ordinary = local_search(&p, &n, LocalSearchConfig::default()).unwrap();
    assert_eq!(ordinary.best.unwrap().score, SimpleScore::ZERO);
    for order in [
        MoveOrder::Original,
        MoveOrder::ShuffledVariables { seed: 71 },
        MoveOrder::ShuffledMoves {
            seed: 71,
            window: 2.try_into().unwrap(),
        },
    ] {
        for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
            for cap in [1, 2, 20] {
                let c = LocalSearchConfig {
                    neighborhood: Neighborhood::Pillar { swaps: true },
                    max_evaluations: cap,
                    max_steps: 3,
                    move_order: order,
                    selection,
                    assert_scores: true,
                    ..Default::default()
                };
                let scoped = local_search(&p, &n, c.clone()).unwrap();
                let (g, s) = graph();
                let stream = stream_local_search_from(&p, g, s, p.initial_assignment(), c).unwrap();
                let expected = if cap == 1 { 0 } else { 10 };
                assert_eq!(
                    scoped.best.as_ref().unwrap().score,
                    SimpleScore::new(expected)
                );
                assert_eq!(
                    stream.best.as_ref().unwrap().score,
                    SimpleScore::new(expected)
                );
                assert_eq!(
                    scoped.best.unwrap().assignment,
                    stream.best.unwrap().assignment
                );
                assert!(scoped.evaluations <= cap && stream.evaluations <= cap);
            }
        }
    }
}
#[test]
fn joint_pillar_and_list_moves_escape_a_mixed_trap_in_both_backends() {
    let p = MixedProblem::new(
        basic(),
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(2, 2, 1);
    n.add_constraint(
        "joint",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListValue(0),
        ],
        |f| {
            let MixedFact::Basic(a) = f[0] else { panic!() };
            let MixedFact::Basic(b) = f[1] else { panic!() };
            let MixedFact::ListValue { ref element, .. } = f[2] else {
                panic!()
            };
            trap(
                a.copied(),
                b.copied(),
                element.as_ref().unwrap().position.entity,
            )
        },
    )
    .unwrap();
    for neighborhood in [
        Neighborhood::ChangeAndSwap,
        Neighborhood::Pillar { swaps: true },
        Neighborhood::SubPillar(SubPillarConfig::new(
            SubPillarSelection::combinations(2, 2).unwrap(),
        )),
    ] {
        for joint_moves in [false, true] {
            let c = MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    neighborhood: neighborhood.clone(),
                    max_steps: 3,
                    max_evaluations: 100,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves,
            };
            let scoped =
                mixed_local_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::<i32, i32>::new(&mut b);
            let pairs = b
                .join_equal(s.basic.port(), s.basic.port(), |_| 0, |_| 0)
                .unwrap();
            let pair = b.filter(&pairs, |(a, b)| a.0 == 0 && b.0 == 1).unwrap();
            let joined = b
                .join_equal(&pair, s.lists.values.port(), |_| 0, |_| 0)
                .unwrap();
            b.score("joint", &joined, |((a, b), v)| {
                trap(a.1, b.1, v.position.unwrap().entity)
            })
            .unwrap();
            let streamed =
                mixed_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c)
                    .unwrap();
            let expected = if matches!(
                neighborhood,
                Neighborhood::Pillar { .. } | Neighborhood::SubPillar(_)
            ) && joint_moves
            {
                10
            } else {
                0
            };
            assert_eq!(
                scoped.best.as_ref().unwrap().score,
                SimpleScore::new(expected)
            );
            assert_eq!(
                streamed.best.as_ref().unwrap().score,
                SimpleScore::new(expected)
            );
            assert_eq!(
                scoped.best.unwrap().assignment,
                streamed.best.unwrap().assignment
            );
        }
    }
}
#[test]
fn providers_match_a_small_independent_successor_oracle_with_ranges_pins_and_optional_values() {
    for a in [None, Some(0), Some(1)] {
        for b in [None, Some(1), Some(2)] {
            for c in [None, Some(0), Some(1), Some(2)] {
                let p = FiniteProblem::new(vec![
                    Variable::new(vec![0, 1]).allowing_unassigned(),
                    Variable::new(vec![1, 2]).allowing_unassigned(),
                    Variable::new(vec![0, 1, 2]).allowing_unassigned(),
                    Variable::new(vec![1]).with_initial(1).with_pinned(true),
                ])
                .unwrap();
                let before = Assignment::new(vec![a, b, c, Some(1)]);
                for swaps in [false, true] {
                    let generated: Vec<_> = pillar_moves(&p, &before, swaps).unwrap().collect();
                    let mut actual = BTreeSet::new();
                    for m in &generated {
                        let mut state = before.clone();
                        p.apply(&mut state, m).unwrap().commit();
                        assert!(p.is_complete(&state));
                        assert_ne!(state, before);
                        actual.insert(state.values().to_vec());
                    }
                    assert_eq!(actual.len(), generated.len());
                    let mut expected = BTreeSet::new();
                    for value in [None, Some(0), Some(1), Some(2)] {
                        if !before.values()[..3].contains(&value) {
                            continue;
                        }
                        for target in [None, Some(0), Some(1), Some(2)] {
                            if value == target {
                                continue;
                            }
                            let mut state = before.values().to_vec();
                            for v in &mut state[..3] {
                                if *v == value {
                                    *v = target;
                                }
                            }
                            if p.validate(&Assignment::new(state.clone())).is_ok() {
                                expected.insert(state);
                            }
                        }
                        if swaps {
                            for other in [None, Some(0), Some(1), Some(2)] {
                                if value >= other || !before.values()[..3].contains(&other) {
                                    continue;
                                }
                                let mut state = before.values().to_vec();
                                for v in &mut state[..3] {
                                    if *v == value {
                                        *v = other;
                                    } else if *v == other {
                                        *v = value;
                                    }
                                }
                                if p.validate(&Assignment::new(state.clone())).is_ok() {
                                    expected.insert(state);
                                }
                            }
                        }
                    }
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
#[test]
fn malformed_or_nonuniform_pillars_are_rejected_before_mutation_and_member_order_is_canonical() {
    assert_eq!(Pillar::new([]), Err(PillarError::Empty));
    assert_eq!(Pillar::from_rows(vec![vec![]]), Err(PillarError::EmptyRow));
    assert!(matches!(
        Pillar::from_rows(vec![vec![0, 1], vec![2]]),
        Err(PillarError::Arity { .. })
    ));
    assert_eq!(
        Pillar::new([0, 0]),
        Err(PillarError::DuplicateVariable { variable: 0 })
    );
    assert_eq!(Pillar::new([1, 0]).unwrap(), Pillar::new([0, 1]).unwrap());
    let a = Assignment::new(vec![Some(0), Some(1), Some(0)]);
    assert!(matches!(
        Pillar::new([0, 1]).unwrap().change_move(&a, &[Some(2)]),
        Err(PillarError::NotUniform { .. })
    ));
    assert!(matches!(
        Pillar::new([4]).unwrap().change_move(&a, &[None]),
        Err(PillarError::Move(_))
    ));
    let left = Pillar::new([0, 2]).unwrap();
    let right = Pillar::new([1, 2]).unwrap();
    assert_eq!(
        left.swap_move(&right, &a),
        Err(PillarError::Overlap { variable: 2 })
    );
    assert!(matches!(
        left.change_move(&a, &[Some(1), Some(2)]),
        Err(PillarError::Arity { .. })
    ));
    assert_eq!(a.values(), &[Some(0), Some(1), Some(0)]);
}
#[test]
fn pillar_transactions_update_all_scopes_and_rollback_or_reject_atomically() {
    let p = basic();
    let n = network();
    let a = p.initial_assignment();
    let m = Pillar::new([0, 1])
        .unwrap()
        .change_move(&a, &[Some(1)])
        .unwrap();
    let mut d = ScoreDirector::new(&p, &n, a.clone()).unwrap();
    {
        let t = d.apply(&m).unwrap();
        assert_eq!(t.score(), SimpleScore::new(10));
    }
    assert_eq!(d.assignment(), &a);
    d.assert_consistent().unwrap();
    d.apply(&m).unwrap().commit();
    d.assert_consistent().unwrap();
    let (g, s) = graph();
    let mut d = StreamScoreDirector::new(&p, g, s, a.clone()).unwrap();
    {
        let t = d.apply(&m).unwrap();
        assert_eq!(t.score(), SimpleScore::new(10));
    }
    assert_eq!(d.assignment(), &a);
    d.assert_consistent().unwrap();
    d.apply(&m).unwrap().commit();
    d.assert_consistent().unwrap();
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0]).with_initial(0),
    ])
    .unwrap();
    let mut state = p.initial_assignment();
    assert!(p.apply(&mut state, &m).is_err());
    assert_eq!(state.values(), &[Some(0), Some(0)]);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(0).with_pinned(true),
    ])
    .unwrap();
    let mut state = p.initial_assignment();
    assert!(p.apply(&mut state, &m).is_err());
    assert_eq!(state.values(), &[Some(0), Some(0)]);
}
#[test]
fn candidate_values_are_cloned_lazily_instead_of_materializing_the_neighborhood() {
    struct Value(usize, Rc<Cell<usize>>);
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    impl PartialEq for Value {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Value {}
    let calls = Rc::new(Cell::new(0));
    let range: Vec<_> = (0..100).map(|i| Value(i, calls.clone())).collect();
    let p = FiniteProblem::new(vec![
        Variable::new(range.clone()).with_initial(range[0].clone()),
        Variable::new(range.clone()).with_initial(range[0].clone()),
    ])
    .unwrap();
    let a = p.initial_assignment();
    calls.set(0);
    let mut iter = pillar_moves(&p, &a, true).unwrap();
    assert_eq!(calls.get(), 0);
    assert!(iter.next().is_some());
    assert_eq!(calls.get(), 2);
}
