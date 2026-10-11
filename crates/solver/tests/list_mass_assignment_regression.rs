// Original independent list insertion/removal and search integration checks.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
fn permutations(values: &[i32]) -> Vec<Vec<i32>> {
    fn fill(values: &[i32], out: &mut Vec<Vec<i32>>, row: &mut Vec<i32>) {
        if row.len() == values.len() {
            out.push(row.clone());
            return;
        }
        for v in values {
            if !row.contains(v) {
                row.push(*v);
                fill(values, out, row);
                row.pop();
            }
        }
    }
    let mut out = vec![];
    fill(values, &mut out, &mut vec![]);
    out
}
#[test]
fn list_groups_match_independent_state_and_metadata_reference_across_pins_ranges_and_sizes() {
    let mut comparisons = 0;
    for layout in 0..27 {
        for pins in 0..3 {
            for ranged in [false, true] {
                let mut code = layout;
                let mut lists = [vec![], vec![]];
                for v in 0..3 {
                    let e = code % 3;
                    code /= 3;
                    if e < 2 {
                        lists[e].push(v);
                    }
                }
                lists[0].reverse();
                let first = ListEntity::new(lists[0].clone()).with_pinned_prefix(if pins == 1 {
                    lists[0].len().min(1)
                } else {
                    0
                });
                let mut second = ListEntity::new(lists[1].clone()).with_pinned(pins == 2);
                if ranged {
                    second = second.with_value_range(
                        lists[1]
                            .iter()
                            .copied()
                            .chain([2])
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .collect(),
                    );
                }
                let p = ListProblem::new(vec![2, 0, 1], vec![first, second])
                    .unwrap()
                    .allowing_unassigned();
                let a = p.initial_assignment();
                for unassign in [false, true] {
                    for (min, max, all) in [
                        (0, 0, false),
                        (1, 1, false),
                        (2, 2, false),
                        (0, 3, false),
                        (3, usize::MAX, false),
                        (4, usize::MAX, false),
                        (0, 0, true),
                    ] {
                        let size = if all {
                            ListSampleSize::all()
                        } else {
                            ListSampleSize::between(min, max).unwrap()
                        };
                        let eligible: Vec<_> = p
                            .values()
                            .iter()
                            .copied()
                            .filter(|v| {
                                let at = lists.iter().enumerate().find_map(|(e, row)| {
                                    row.iter().position(|x| x == v).map(|i| (e, i))
                                });
                                if unassign {
                                    at.is_some_and(|(e, i)| {
                                        !p.entities()[e].is_pinned()
                                            && i >= p.entities()[e].pinned_prefix()
                                    })
                                } else {
                                    at.is_none()
                                }
                            })
                            .collect();
                        let mut expected = BTreeSet::new();
                        let mut metadata = BTreeSet::new();
                        for mask in 0..1usize << eligible.len() {
                            let group: Vec<_> = eligible
                                .iter()
                                .enumerate()
                                .filter(|(i, _)| mask & (1 << i) != 0)
                                .map(|(_, v)| *v)
                                .collect();
                            if group.len() < (if unassign { 1 } else { 2 })
                                || (all && group.len() != eligible.len())
                                || (!all && (group.len() < min || group.len() > max))
                            {
                                continue;
                            }
                            if unassign {
                                let next: Vec<Vec<_>> = lists
                                    .iter()
                                    .map(|row| {
                                        row.iter().copied().filter(|v| !group.contains(v)).collect()
                                    })
                                    .collect();
                                expected.insert(next);
                                metadata.insert((group, None));
                            } else {
                                for ordered in permutations(&group) {
                                    for (e, row) in lists.iter().enumerate() {
                                        if p.entities()[e].is_pinned()
                                            || !ordered.iter().all(|v| {
                                                p.entities()[e]
                                                    .value_range()
                                                    .is_none_or(|r| r.contains(v))
                                            })
                                        {
                                            continue;
                                        }
                                        for i in p.entities()[e].pinned_prefix()..=row.len() {
                                            let mut next = lists.to_vec();
                                            next[e].splice(i..i, ordered.clone());
                                            expected.insert(next);
                                            metadata.insert((ordered.clone(), Some((e, i))));
                                        }
                                    }
                                }
                            }
                        }
                        let actual: Vec<_> = if unassign {
                            list_mass_unassign_moves(&p, &a, size).unwrap().collect()
                        } else {
                            list_mass_assign_moves(&p, &a, size).unwrap().collect()
                        };
                        let mut seen = BTreeSet::new();
                        let mut seen_metadata = BTreeSet::new();
                        let neighborhood = if unassign {
                            Neighborhood::MassListUnassign(size)
                        } else {
                            Neighborhood::MassListAssign(size)
                        };
                        assert_eq!(
                            actual
                                .iter()
                                .map(|m| m.movement().clone())
                                .collect::<Vec<_>>(),
                            list_moves(&p, &a, neighborhood)
                                .unwrap()
                                .collect::<Vec<_>>()
                        );
                        for m in actual {
                            assert!(seen_metadata.insert((
                                m.members().to_vec(),
                                m.destination().map(|p| (p.entity, p.index))
                            )));
                            let mut copy = a.clone();
                            {
                                let trial = p.apply(&mut copy, m.movement()).unwrap();
                                seen.insert(trial.lists().to_vec());
                            }
                            assert_eq!(copy, a);
                            p.apply(&mut copy, m.movement()).unwrap().commit();
                            assert!(p.is_complete(&copy));
                            assert_ne!(copy, a);
                        }
                        assert_eq!(seen, expected);
                        assert_eq!(seen_metadata, metadata);
                        comparisons += 1;
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 2268);
}
#[test]
fn sampled_list_unassignment_preserves_draw_count_seed_prefix_and_legal_members() {
    let p = ListProblem::new(
        (0..9).rev().collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]).with_pinned_prefix(2),
            ListEntity::new(vec![4, 5]).with_pinned(true),
            ListEntity::new(vec![6, 7]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let mut distinct = BTreeSet::new();
    for size in [
        ListSampleSize::exactly(0),
        ListSampleSize::exactly(1),
        ListSampleSize::exactly(2),
        ListSampleSize::between(0, usize::MAX).unwrap(),
        ListSampleSize::exactly(5),
        ListSampleSize::all(),
    ] {
        let allowed: BTreeSet<_> = list_mass_unassign_moves(&p, &a, size)
            .unwrap()
            .map(|m| m.members().to_vec())
            .collect();
        for seed in 0..32 {
            let draws = |n| {
                sample_list_mass_unassign_moves(&p, &a, size, seed, n)
                    .unwrap()
                    .collect::<Vec<_>>()
            };
            let fifty = draws(50);
            assert_eq!(fifty, draws(50));
            assert_eq!(draws(7), fifty.iter().take(7).cloned().collect::<Vec<_>>());
            assert!(draws(0).is_empty());
            assert_eq!(fifty.len(), if allowed.is_empty() { 0 } else { 50 });
            for m in fifty {
                assert!(allowed.contains(m.members()));
                assert!(m.destination().is_none());
                assert!(m.members().iter().all(|v| [2, 3, 6, 7].contains(v)));
                distinct.insert(m.members().to_vec());
                let mut next = a.clone();
                p.apply(&mut next, m.movement()).unwrap().commit();
                let expected: Vec<Vec<_>> = a
                    .lists()
                    .iter()
                    .map(|row| {
                        row.iter()
                            .copied()
                            .filter(|v| !m.members().contains(v))
                            .collect()
                    })
                    .collect();
                assert_eq!(next.lists(), expected);
            }
        }
    }
    assert_eq!(distinct.len(), 15);
    let empty = ListProblem::<i32>::new(vec![], vec![])
        .unwrap()
        .allowing_unassigned();
    let a = empty.initial_assignment();
    let mut m =
        sample_list_mass_unassign_moves(&empty, &a, ListSampleSize::all(), 1, usize::MAX).unwrap();
    assert!(m.next().is_none());
    assert!(m.next().is_none());
}
#[derive(Debug)]
struct Counted(usize, Rc<Cell<usize>>);
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.1.set(self.1.get() + 1);
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Counted {
    fn eq(&self, o: &Self) -> bool {
        self.0 == o.0
    }
}
impl Eq for Counted {}
#[test]
fn list_group_enumeration_is_lazy_and_all_uses_global_missing_population() {
    let count = Rc::new(Cell::new(0));
    let p = ListProblem::new(
        (0..128).map(|i| Counted(i, count.clone())).collect(),
        vec![ListEntity::new(vec![])],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    count.set(0);
    let mut m = list_mass_assign_moves(&p, &a, ListSampleSize::exactly(64)).unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(m.next().unwrap().members().len(), 64);
    assert!(count.get() < 400);
    assert!(m.next().is_some());
    assert!(count.get() < 800);
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![ListEntity::new(vec![]).with_value_range(vec![0, 1])],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    assert_eq!(
        list_mass_assign_moves(&p, &a, ListSampleSize::all())
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        list_mass_assign_moves(&p, &a, ListSampleSize::exactly(2))
            .unwrap()
            .count(),
        2
    );
    assert_eq!(
        list_mass_assign_moves(&p, &a, ListSampleSize::exactly(1))
            .unwrap()
            .count(),
        0
    );
    assert!(
        list_mass_assign_moves(&p, &ListAssignment::new(vec![]), ListSampleSize::exactly(0))
            .is_err()
    );
}
fn target(row: &[i32], unassign: bool) -> SimpleScore {
    SimpleScore::new(i64::from(if unassign {
        row.is_empty()
    } else {
        row == [1, 0]
    }))
}
#[test]
fn list_and_mixed_group_search_matches_both_scoring_backends_at_every_budget() {
    for unassign in [false, true] {
        let p = ListProblem::new(
            vec![0, 1],
            vec![ListEntity::new(if unassign { vec![0, 1] } else { vec![] })],
        )
        .unwrap()
        .allowing_unassigned();
        let family = if unassign {
            Neighborhood::MassListUnassign(ListSampleSize::all())
        } else {
            Neighborhood::MassListAssign(ListSampleSize::all())
        };
        let mut n = ListConstraintNetwork::new(1, 2);
        n.add_constraint("target", vec![ListScope::Entity(0)], move |f| {
            let ListFact::Entity(row) = f[0] else {
                unreachable!()
            };
            target(row, unassign)
        })
        .unwrap();
        for neighborhood in [Neighborhood::ChangeAndSwap, family.clone()] {
            for move_order in [
                MoveOrder::Original,
                MoveOrder::ShuffledVariables { seed: 19 },
                MoveOrder::ShuffledMoves {
                    seed: 19,
                    window: 3.try_into().unwrap(),
                },
            ] {
                for acceptance in [
                    Acceptance::StrictImprovement,
                    Acceptance::ValueTabu {
                        tenure: 2,
                        aspiration: true,
                    },
                    Acceptance::MoveTabu {
                        tenure: 2,
                        aspiration: true,
                    },
                ] {
                    for budget in [0, 1, 2, 50] {
                        let c = LocalSearchConfig {
                            neighborhood: neighborhood.clone(),
                            move_order,
                            acceptance,
                            max_steps: 3,
                            max_evaluations: budget,
                            assert_scores: true,
                            ..Default::default()
                        };
                        let a = list_local_search_from(&p, &n, p.initial_assignment(), c.clone())
                            .unwrap();
                        let mut b = StreamGraphBuilder::new();
                        let s = ListStreamSources::new(&mut b);
                        b.score("target", s.entities.port(), move |(_, row)| {
                            target(row, unassign)
                        })
                        .unwrap();
                        let r = list_stream_local_search_from(
                            &p,
                            b.build(),
                            s,
                            p.initial_assignment(),
                            c.clone(),
                        )
                        .unwrap();
                        assert_eq!(a.best, r.best);
                        assert_eq!(a.evaluations, r.evaluations);
                        assert_eq!(a.termination, r.termination);
                        assert!(a.evaluations <= budget);
                        if budget == 50 && acceptance == Acceptance::StrictImprovement {
                            assert_eq!(
                                a.best.unwrap().score,
                                SimpleScore::new(i64::from(neighborhood == family))
                            );
                        }
                        let mixed = MixedProblem::new(
                            FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)])
                                .unwrap(),
                            p.clone(),
                        );
                        let mut net = MixedConstraintNetwork::new(1, 1, 2);
                        net.add_constraint(
                            "joint",
                            vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
                            move |f| {
                                let (MixedFact::Basic(v), MixedFact::ListEntity(row)) =
                                    (&f[0], &f[1])
                                else {
                                    unreachable!()
                                };
                                if v.copied() == Some(1) {
                                    target(row, unassign)
                                } else {
                                    SimpleScore::ZERO
                                }
                            },
                        )
                        .unwrap();
                        for joint_moves in [false, true] {
                            let cfg = MixedLocalSearchConfig {
                                search: c.clone(),
                                joint_moves,
                            };
                            let a = mixed_local_search_from(
                                &mixed,
                                &net,
                                mixed.initial_assignment(),
                                cfg.clone(),
                            )
                            .unwrap();
                            let mut b = StreamGraphBuilder::new();
                            let s = MixedStreamSources::new(&mut b);
                            let joined = b
                                .join_equal(s.basic.port(), s.lists.entities.port(), |_| 0, |_| 0)
                                .unwrap();
                            b.score("target", &joined, move |((_, v), (_, row))| {
                                if *v == Some(1) {
                                    target(row, unassign)
                                } else {
                                    SimpleScore::ZERO
                                }
                            })
                            .unwrap();
                            let r = mixed_stream_local_search_from(
                                &mixed,
                                b.build(),
                                s,
                                mixed.initial_assignment(),
                                cfg,
                            )
                            .unwrap();
                            assert_eq!(a.best, r.best);
                            assert_eq!(a.evaluations, r.evaluations);
                            assert_eq!(a.termination, r.termination);
                            assert!(a.evaluations <= budget);
                            if budget == 50 && acceptance == Acceptance::StrictImprovement {
                                assert_eq!(
                                    a.best.unwrap().score,
                                    SimpleScore::new(i64::from(
                                        neighborhood == family && joint_moves
                                    ))
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn required_lists_fail_before_every_local_or_phase_scoring_path() {
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
    let calls = Cell::new(0);
    let mut n = ListConstraintNetwork::new(1, 2);
    n.add_constraint("score", vec![ListScope::Entity(0)], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    for size in [
        ListSampleSize::exactly(0),
        ListSampleSize::exactly(1),
        ListSampleSize::all(),
    ] {
        assert!(list_mass_assign_moves(&p, &p.initial_assignment(), size).is_err());
        assert!(list_mass_unassign_moves(&p, &p.initial_assignment(), size).is_err());
        assert!(sample_list_mass_unassign_moves(&p, &p.initial_assignment(), size, 0, 0).is_err());
        for neighborhood in [
            Neighborhood::MassListAssign(size),
            Neighborhood::MassListUnassign(size),
        ] {
            for budget in [0, 100] {
                let config = LocalSearchConfig {
                    neighborhood: neighborhood.clone(),
                    max_evaluations: budget,
                    ..Default::default()
                };
                let seq = SolverSequence {
                    phases: vec![
                        SolverPhase::Construction(ConstructionConfig::default()),
                        SolverPhase::LocalSearch(config.clone()),
                    ],
                    limits: SolverLimits {
                        max_evaluations: budget,
                        ..Default::default()
                    },
                };
                assert!(matches!(
                    solve_list(&p, &n, seq.clone()),
                    Err(SolverError::Phase { phase: 1, .. })
                ));
                assert!(matches!(
                    solve_list_stream::<i32, SimpleScore>(
                        &p,
                        || {
                            calls.set(calls.get() + 1);
                            unreachable!()
                        },
                        seq
                    ),
                    Err(SolverError::Phase { phase: 1, .. })
                ));
                assert!(
                    list_local_search_from(&p, &n, p.initial_assignment(), config.clone()).is_err()
                );
                assert!(list_moves(&p, &p.initial_assignment(), neighborhood.clone()).is_err());
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                b.score("score", s.entities.port(), |_| {
                    calls.set(calls.get() + 1);
                    SimpleScore::ZERO
                })
                .unwrap();
                assert!(
                    list_stream_local_search_from(
                        &p,
                        b.build(),
                        s,
                        p.initial_assignment(),
                        config.clone()
                    )
                    .is_err()
                );
                let mixed =
                    MixedProblem::new(FiniteProblem::<i32>::new(vec![]).unwrap(), p.clone());
                let mut net = MixedConstraintNetwork::new(0, 1, 2);
                net.add_constraint("score", vec![MixedScope::ListEntity(0)], |_| {
                    calls.set(calls.get() + 1);
                    SimpleScore::ZERO
                })
                .unwrap();
                let cfg = MixedLocalSearchConfig {
                    search: config,
                    joint_moves: true,
                };
                let seq = MixedSolverSequence {
                    phases: vec![
                        MixedSolverPhase::Construction(MixedConstructionConfig::default()),
                        MixedSolverPhase::LocalSearch(cfg.clone()),
                    ],
                    limits: SolverLimits {
                        max_evaluations: budget,
                        ..Default::default()
                    },
                };
                assert!(matches!(
                    solve_mixed(&mixed, &net, seq.clone()),
                    Err(SolverError::Phase { phase: 1, .. })
                ));
                assert!(matches!(
                    solve_mixed_stream::<i32, i32, SimpleScore>(
                        &mixed,
                        || {
                            calls.set(calls.get() + 1);
                            unreachable!()
                        },
                        seq
                    ),
                    Err(SolverError::Phase { phase: 1, .. })
                ));
                assert!(
                    mixed_local_search_from(&mixed, &net, mixed.initial_assignment(), cfg.clone())
                        .is_err()
                );
                assert!(
                    mixed_moves(
                        &mixed,
                        &mixed.initial_assignment(),
                        neighborhood.clone(),
                        true
                    )
                    .is_err()
                );
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::new(&mut b);
                b.score("score", s.lists.entities.port(), |_| {
                    calls.set(calls.get() + 1);
                    SimpleScore::ZERO
                })
                .unwrap();
                assert!(
                    mixed_stream_local_search_from(
                        &mixed,
                        b.build(),
                        s,
                        mixed.initial_assignment(),
                        cfg
                    )
                    .is_err()
                );
            }
        }
    }
    assert_eq!(calls.get(), 0);
}
