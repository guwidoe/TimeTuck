//! Original schema, multicolumn successor and solver regressions.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
fn rows(n: usize, width: usize) -> Vec<Vec<usize>> {
    (0..n)
        .map(|r| (r * width..(r + 1) * width).collect())
        .collect()
}
#[test]
fn schema_validation_cloning_removal_and_selected_pin_scope() {
    assert!(PillarSchema::new(vec![]).is_err());
    assert!(PillarSchema::new(vec![vec![]]).is_err());
    assert!(PillarSchema::new(vec![vec![0, 1], vec![2]]).is_err());
    assert!(PillarSchema::new(vec![vec![0], vec![0]]).is_err());
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 4]).unwrap();
    assert!(
        p.clone()
            .with_pillar_schema(PillarSchema::new(vec![vec![usize::MAX]]).unwrap())
            .is_err()
    );
    let p = p
        .with_pillar_schema(PillarSchema::new(vec![vec![2, 3], vec![0, 1]]).unwrap())
        .unwrap();
    assert_eq!(p.clone().pillar_schema().unwrap().rows(), &rows(2, 2));
    assert!(p.clone().without_pillar_schema().pillar_schema().is_none());
    assert!(p.pillar_schema().is_some());
    for pin_selected in [false, true] {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![0, 1])
                .with_initial(0)
                .with_pinned(pin_selected),
            Variable::new(vec![0, 1]).with_initial(0),
            Variable::new(vec![5]).with_initial(5).with_pinned(true),
            Variable::new(vec![0, 1]).with_initial(1),
            Variable::new(vec![0, 1]).with_initial(1),
            Variable::new(vec![5]).with_initial(5),
        ])
        .unwrap()
        .with_pillar_schema(PillarSchema::new(vec![vec![0, 1], vec![3, 4]]).unwrap())
        .unwrap();
        let count = pillar_swap_moves(&p, &p.initial_assignment())
            .unwrap()
            .count();
        assert_eq!(count, if pin_selected { 0 } else { 2 });
    }
}
#[test]
fn schema_swaps_match_exhaustive_mask_oracle_for_selected_columns_domains_and_pins() {
    for bits in 0..256 {
        let initial: Vec<_> = (0..8).map(|i| Some((bits >> i) & 1)).collect();
        for pin in [false, true] {
            for columns in [vec![0], vec![0, 1]] {
                for subsets in [false, true] {
                    let p = FiniteProblem::new(
                        initial
                            .iter()
                            .enumerate()
                            .map(|(i, &v)| {
                                Variable::new(if i == 3 { vec![v.unwrap()] } else { vec![0, 1] })
                                    .with_initial(v.unwrap())
                                    .with_pinned(pin && i == 4)
                            })
                            .collect(),
                    )
                    .unwrap()
                    .with_pillar_schema(
                        PillarSchema::new(
                            (0..4)
                                .map(|r| columns.iter().map(|c| r * 2 + c).collect())
                                .collect(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    let a = p.initial_assignment();
                    let key = |r: usize| {
                        columns
                            .iter()
                            .map(|c| initial[r * 2 + c])
                            .collect::<Vec<_>>()
                    };
                    let eligible = |r: usize| !pin || r != 2;
                    let selected = |mask: usize| -> Option<Vec<usize>> {
                        let members: Vec<_> = (0..4).filter(|r| mask & (1 << r) != 0).collect();
                        if members
                            .iter()
                            .any(|&r| !eligible(r) || key(r) != key(members[0]))
                        {
                            return None;
                        }
                        if subsets {
                            if members.len() > 2 {
                                return None;
                            }
                        } else if (0..4).any(|r| {
                            eligible(r) && key(r) == key(members[0]) && !members.contains(&r)
                        }) {
                            return None;
                        }
                        Some(members)
                    };
                    let mut expected = BTreeSet::new();
                    for left in 1..16 {
                        let Some(l) = selected(left) else { continue };
                        for right in 1..16 {
                            let Some(r) = selected(right) else { continue };
                            if left & right != 0 || key(l[0]) == key(r[0]) {
                                continue;
                            }
                            let mut state = initial.clone();
                            for (members, source) in [(&l, r[0]), (&r, l[0])] {
                                for &row in members {
                                    for &col in &columns {
                                        state[row * 2 + col] = initial[source * 2 + col];
                                    }
                                }
                            }
                            if p.validate(&Assignment::new(state.clone())).is_ok() {
                                expected.insert(state);
                            }
                        }
                    }
                    let moves: Box<dyn Iterator<Item = Move<i32>>> = if subsets {
                        Box::new(
                            subpillar_moves(
                                &p,
                                &a,
                                SubPillarConfig {
                                    changes: PillarChangeMode::Disabled,
                                    ..SubPillarConfig::new(
                                        SubPillarSelection::combinations(1, 2).unwrap(),
                                    )
                                },
                            )
                            .unwrap(),
                        )
                    } else {
                        Box::new(pillar_swap_moves(&p, &a).unwrap())
                    };
                    let mut actual = BTreeSet::new();
                    for m in moves {
                        let mut state = a.clone();
                        p.apply(&mut state, &m).unwrap().commit();
                        assert_ne!(state, a);
                        actual.insert(state.values().to_vec());
                    }
                    assert_eq!(actual, expected);
                }
            }
        }
    }
}
#[test]
fn schema_changes_update_one_column_and_optional_targets_obey_each_selected_slot() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1])
            .with_initial(0)
            .allowing_unassigned(),
        Variable::new(vec![0, 2]).with_initial(0),
        Variable::new(vec![0, 1])
            .with_initial(0)
            .allowing_unassigned(),
        Variable::new(vec![0]).with_initial(0),
    ])
    .unwrap()
    .with_pillar_schema(PillarSchema::new(rows(2, 2)).unwrap())
    .unwrap();
    let a = p.initial_assignment();
    let movements: Vec<_> = pillar_moves(&p, &a, false).unwrap().collect();
    assert_eq!(movements.len(), 2);
    for m in &movements {
        assert_eq!(m.touched_variables(), vec![0, 2]);
        let mut t = a.clone();
        p.apply(&mut t, m).unwrap().commit();
        assert_eq!(t.values()[1], Some(0));
        assert_eq!(t.values()[3], Some(0));
    }
    let states: BTreeSet<_> = movements
        .iter()
        .map(|m| {
            let mut t = a.clone();
            p.apply(&mut t, m).unwrap().commit();
            t.values().to_vec()
        })
        .collect();
    assert!(states.contains(&vec![Some(1), Some(0), Some(1), Some(0)]));
    assert!(states.contains(&vec![None, Some(0), None, Some(0)]));
}
fn initial() -> Vec<Option<i32>> {
    vec![Some(0), Some(0), Some(1), Some(1), Some(0), Some(1)]
}
fn trap(v: &[Option<i32>], owner: usize) -> SimpleScore {
    SimpleScore::new(if v == initial() && owner == 0 {
        0
    } else if v == [Some(1), Some(1), Some(0), Some(0), Some(0), Some(1)] && owner == 1 {
        10
    } else {
        -5
    })
}
fn model(schema: bool) -> FiniteProblem<i32> {
    let p = FiniteProblem::new(
        initial()
            .into_iter()
            .map(|v| Variable::new(vec![0, 1]).with_initial(v.unwrap()))
            .collect(),
    )
    .unwrap();
    if schema {
        p.with_pillar_schema(PillarSchema::new(rows(3, 2)).unwrap())
            .unwrap()
    } else {
        p
    }
}
fn graph() -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i32>) {
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<i32>)>();
    let all = b.collect_list(s.port(), |v| v.1).unwrap();
    b.score("trap", &all, |v| {
        let v: Vec<_> = v.iter().copied().collect();
        trap(&v, usize::from(v != initial()))
    })
    .unwrap();
    (b.build(), s)
}
#[test]
fn schema_neighborhoods_solve_multicolumn_trap_in_both_backends_and_rollback_scores() {
    for schema in [false, true] {
        let p = model(schema);
        let mut n = ConstraintNetwork::new(6);
        n.add_constraint("trap", (0..6).collect(), |v| {
            let v: Vec<_> = v.iter().map(|v| v.copied()).collect();
            trap(&v, usize::from(v != initial()))
        })
        .unwrap();
        for neighborhood in [
            Neighborhood::Pillar { swaps: true },
            Neighborhood::SubPillar(SubPillarConfig::new(
                SubPillarSelection::combinations(1, 1).unwrap(),
            )),
        ] {
            for order in [
                MoveOrder::Original,
                MoveOrder::ShuffledVariables { seed: 41 },
                MoveOrder::ShuffledMoves {
                    seed: 41,
                    window: 3.try_into().unwrap(),
                },
            ] {
                let c = LocalSearchConfig {
                    neighborhood,
                    move_order: order,
                    max_steps: 3,
                    max_evaluations: 200,
                    assert_scores: true,
                    ..Default::default()
                };
                let result = local_search(&p, &n, c).unwrap();
                let (g, s) = graph();
                let streamed =
                    stream_local_search_from(&p, g, s, p.initial_assignment(), c).unwrap();
                assert_eq!(
                    result.best.as_ref().unwrap().score,
                    SimpleScore::new(if schema { 10 } else { 0 })
                );
                assert_eq!(
                    result.best.unwrap().assignment,
                    streamed.best.unwrap().assignment
                );
            }
        }
        if schema {
            let a = p.initial_assignment();
            let movement = pillar_swap_moves(&p, &a)
                .unwrap()
                .find(|m| m.touched_variables() == [0, 1, 2, 3])
                .unwrap();
            let mut d = ScoreDirector::new(&p, &n, a.clone()).unwrap();
            {
                let trial = d.apply(&movement).unwrap();
                assert_eq!(trial.score(), SimpleScore::new(10));
            }
            assert_eq!(d.assignment(), &a);
            d.assert_consistent().unwrap();
            let (g, s) = graph();
            let mut d = StreamScoreDirector::new(&p, g, s, a.clone()).unwrap();
            {
                let trial = d.apply(&movement).unwrap();
                assert_eq!(trial.score(), SimpleScore::new(10));
            }
            assert_eq!(d.assignment(), &a);
            d.assert_consistent().unwrap();
        }
    }
}
#[test]
fn schema_joint_moves_escape_mixed_trap_in_both_backends() {
    let p = MixedProblem::new(
        model(true),
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(6, 2, 1);
    let mut scopes: Vec<_> = (0..6).map(MixedScope::Basic).collect();
    scopes.push(MixedScope::ListValue(0));
    n.add_constraint("trap", scopes, |f| {
        let v: Vec<_> = f[..6]
            .iter()
            .map(|f| {
                let MixedFact::Basic(v) = f else { panic!() };
                v.copied()
            })
            .collect();
        let MixedFact::ListValue { ref element, .. } = f[6] else {
            panic!()
        };
        trap(&v, element.as_ref().unwrap().position.entity)
    })
    .unwrap();
    for joint in [false, true] {
        for neighborhood in [
            Neighborhood::Pillar { swaps: true },
            Neighborhood::SubPillar(SubPillarConfig::new(
                SubPillarSelection::combinations(1, 1).unwrap(),
            )),
        ] {
            let c = MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    neighborhood,
                    max_steps: 3,
                    max_evaluations: 500,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves: joint,
            };
            let result = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::<i32, i32>::new(&mut b);
            let all = b.collect_list(s.basic.port(), |v| v.1).unwrap();
            let both = b
                .join_equal(&all, s.lists.values.port(), |_| 0, |_| 0)
                .unwrap();
            b.score("trap", &both, |(v, l)| {
                trap(
                    &v.iter().copied().collect::<Vec<_>>(),
                    l.position.unwrap().entity,
                )
            })
            .unwrap();
            let streamed =
                mixed_stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c)
                    .unwrap();
            assert_eq!(
                result.best.as_ref().unwrap().score,
                SimpleScore::new(if joint { 10 } else { 0 })
            );
            assert_eq!(
                result.best.unwrap().assignment,
                streamed.best.unwrap().assignment
            );
        }
    }
}
#[test]
fn schema_candidates_clone_values_lazily_and_skip_undersized_swap_partners() {
    struct V(usize, Rc<Cell<usize>>);
    impl Clone for V {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    impl PartialEq for V {
        fn eq(&self, r: &Self) -> bool {
            self.0 == r.0
        }
    }
    impl Eq for V {}
    let count = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(
        (0..256)
            .map(|i| {
                Variable::new(vec![V(0, count.clone()), V(1, count.clone())])
                    .with_initial(V(usize::from(i >= 128), count.clone()))
            })
            .collect(),
    )
    .unwrap()
    .with_pillar_schema(PillarSchema::new(rows(128, 2)).unwrap())
    .unwrap();
    let a = p.initial_assignment();
    count.set(0);
    let mut moves = pillar_swap_moves(&p, &a).unwrap();
    assert_eq!(count.get(), 0);
    assert!(moves.next().is_some());
    assert_eq!(count.get(), 256);
    let mut vars = vec![Variable::new(vec![0, 1]).with_initial(0); 256];
    vars.extend(vec![Variable::new(vec![0, 1]).with_initial(1); 2]);
    let p = FiniteProblem::new(vars)
        .unwrap()
        .with_pillar_schema(PillarSchema::new(rows(129, 2)).unwrap())
        .unwrap();
    let a = p.initial_assignment();
    let c = SubPillarConfig {
        changes: PillarChangeMode::Disabled,
        ..SubPillarConfig::new(SubPillarSelection::combinations(64, 64).unwrap())
    };
    assert_eq!(subpillar_moves(&p, &a, c).unwrap().count(), 0);
}
