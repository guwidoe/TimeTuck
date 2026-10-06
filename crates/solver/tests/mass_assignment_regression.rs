// Original independent references and finite/seeded group integration checks.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
#[test]
fn assignment_and_unassignment_match_independent_bitmask_successors() {
    let mut comparisons = 0;
    for state in 0..27 {
        for optional in 0..8 {
            for pins in 0..8 {
                let mut code = state;
                let mut vars = vec![];
                for i in 0..3 {
                    let initial = match code % 3 {
                        0 => None,
                        1 => Some(i),
                        _ => Some(3),
                    };
                    code /= 3;
                    let mut v = Variable::new(vec![i, 3]);
                    if let Some(vv) = initial {
                        v = v.with_initial(vv);
                    }
                    if optional & (1 << i) != 0 {
                        v = v.allowing_unassigned();
                    }
                    vars.push(v.with_pinned(pins & (1 << i) != 0));
                }
                let p = FiniteProblem::new(vars).unwrap();
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
                            MassAssignmentSize::all()
                        } else {
                            MassAssignmentSize::between(min, max).unwrap()
                        };
                        if unassign && optional != 7 {
                            assert!(mass_unassign_moves(&p, &a, size).is_err());
                            continue;
                        }
                        let eligible: Vec<_> = (0..3)
                            .filter(|&i| {
                                pins & (1 << i) == 0 && a.values()[i].is_some() == unassign
                            })
                            .collect();
                        let mut expected = BTreeSet::new();
                        let mut metadata = BTreeSet::new();
                        for mask in 1..8 {
                            let group: Vec<_> = (0..3).filter(|i| mask & (1 << i) != 0).collect();
                            if group.len() < (if unassign { 1 } else { 2 })
                                || !group.iter().all(|i| eligible.contains(i))
                                || (all && group.len() != eligible.len())
                                || (!all && (group.len() < min || group.len() > max))
                            {
                                continue;
                            }
                            for target in [None, Some(0), Some(1), Some(2), Some(3)] {
                                if unassign != target.is_none()
                                    || !group
                                        .iter()
                                        .all(|&i| target.is_none_or(|v| v == i as i32 || v == 3))
                                {
                                    continue;
                                }
                                let mut next = a.values().to_vec();
                                for &i in &group {
                                    next[i] = target;
                                }
                                expected.insert(next);
                                metadata.insert((group.clone(), target));
                            }
                        }
                        let actual: Vec<_> = if unassign {
                            mass_unassign_moves(&p, &a, size).unwrap().collect()
                        } else {
                            mass_assign_moves(&p, &a, size).unwrap().collect()
                        };
                        let mut seen = BTreeSet::new();
                        let mut seen_metadata = BTreeSet::new();
                        for m in actual {
                            assert!(
                                seen_metadata
                                    .insert((m.members().to_vec(), m.destination().copied()))
                            );
                            let mut copy = a.clone();
                            {
                                let trial = p.apply(&mut copy, m.movement()).unwrap();
                                seen.insert(trial.values().to_vec());
                            }
                            assert_eq!(copy, a);
                            p.apply(&mut copy, m.movement()).unwrap().commit();
                            assert_ne!(copy, a);
                            if p.is_complete(&a) {
                                assert!(p.is_complete(&copy));
                            }
                        }
                        assert_eq!(seen, expected);
                        assert_eq!(seen_metadata, metadata);
                        comparisons += 1;
                    }
                }
            }
        }
    }
    // All pin/null combinations are valid partial models now. Optional-only
    // unassignment/null-crossing policies add one extra branch per pin pattern.
    assert_eq!(comparisons, 27 * 8 * 8 * 7 + 27 * 8 * 7);
}
#[test]
fn sampled_unassignment_is_bounded_reproducible_and_matches_finite_candidate_sets() {
    let mut vars = vec![Variable::new(vec![0, 1]).allowing_unassigned()];
    for i in 1..7 {
        vars.push(
            Variable::new(vec![0, 1])
                .with_initial(i % 2)
                .allowing_unassigned()
                .with_pinned(i == 6),
        );
    }
    let p = FiniteProblem::new(vars).unwrap();
    let a = p.initial_assignment();
    let mut distinct = BTreeSet::new();
    for size in [
        MassAssignmentSize::exactly(0),
        MassAssignmentSize::exactly(1),
        MassAssignmentSize::exactly(2),
        MassAssignmentSize::between(0, usize::MAX).unwrap(),
        MassAssignmentSize::exactly(6),
        MassAssignmentSize::all(),
    ] {
        let allowed: BTreeSet<_> = mass_unassign_moves(&p, &a, size)
            .unwrap()
            .map(|m| m.members().to_vec())
            .collect();
        for seed in 0..32 {
            let draw = |n| {
                sample_mass_unassign_moves(&p, &a, size, seed, n)
                    .unwrap()
                    .collect::<Vec<_>>()
            };
            let fifty = draw(50);
            assert_eq!(fifty, draw(50));
            assert_eq!(draw(7), fifty.iter().take(7).cloned().collect::<Vec<_>>());
            assert!(draw(0).is_empty());
            assert_eq!(fifty.len(), if allowed.is_empty() { 0 } else { 50 });
            for m in fifty {
                assert!(allowed.contains(m.members()));
                assert!(m.destination().is_none());
                distinct.insert(m.members().to_vec());
                let mut next = a.clone();
                p.apply(&mut next, m.movement()).unwrap().commit();
                assert!(p.is_complete(&next));
                for i in 0..7 {
                    assert_eq!(
                        next.values()[i],
                        if m.members().contains(&i) {
                            None
                        } else {
                            a.values()[i]
                        }
                    );
                }
            }
        }
    }
    assert!(distinct.len() > 20);
    let empty = FiniteProblem::<i32>::new(vec![]).unwrap();
    let a = empty.initial_assignment();
    let mut samples =
        sample_mass_unassign_moves(&empty, &a, MassAssignmentSize::all(), 1, usize::MAX).unwrap();
    assert!(samples.next().is_none());
    assert!(samples.next().is_none());
    assert!(matches!(
        MassAssignmentSize::between(2, 1),
        Err(ModelError::InvalidSampleSize { .. })
    ));
    assert!(
        mass_assign_moves(
            &empty,
            &Assignment::new(vec![None]),
            MassAssignmentSize::exactly(0)
        )
        .is_err()
    );
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
fn huge_group_spaces_allocate_only_polled_candidates() {
    let count = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(
        (0..128)
            .map(|_| Variable::new(vec![Counted(0, count.clone()), Counted(1, count.clone())]))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    count.set(0);
    let mut moves =
        mass_assign_moves(&p, &a, MassAssignmentSize::between(64, usize::MAX).unwrap()).unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(moves.next().unwrap().members().len(), 64);
    assert!(count.get() < 100);
    assert_eq!(moves.next().unwrap().members().len(), 64);
    assert!(count.get() < 200);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0])
            .with_initial(0)
            .allowing_unassigned();
        128
    ])
    .unwrap();
    let a = p.initial_assignment();
    let mut draws =
        sample_mass_unassign_moves(&p, &a, MassAssignmentSize::all(), 5, usize::MAX).unwrap();
    assert_eq!(draws.next().unwrap().members().len(), 128);
}
fn reward(a: Option<i32>, b: Option<i32>, unassign: bool) -> SimpleScore {
    SimpleScore::new(i64::from(if unassign {
        a.is_none() && b.is_none()
    } else {
        a == Some(1) && b == Some(1)
    }))
}
#[test]
fn group_assignment_search_obeys_budgets_and_joint_scoring_in_both_backends() {
    for unassign in [false, true] {
        let v = Variable::new(vec![1]).allowing_unassigned();
        let v = if unassign { v.with_initial(1) } else { v };
        let p = FiniteProblem::new(vec![v; 2]).unwrap();
        let family = if unassign {
            Neighborhood::MassUnassign(MassAssignmentSize::all())
        } else {
            Neighborhood::MassAssign(MassAssignmentSize::all())
        };
        let mut n = ConstraintNetwork::new(2);
        n.add_constraint("target", vec![0, 1], move |v| {
            reward(v[0].copied(), v[1].copied(), unassign)
        })
        .unwrap();
        for neighborhood in [Neighborhood::ChangeAndSwap, family] {
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
                            neighborhood,
                            move_order,
                            acceptance,
                            max_steps: 3,
                            max_evaluations: budget,
                            assert_scores: true,
                            ..Default::default()
                        };
                        let a = local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
                        let mut b = StreamGraphBuilder::new();
                        let s = b.source::<usize, (usize, Option<i32>)>();
                        let l = b.filter(s.port(), |(i, _)| *i == 0).unwrap();
                        let r = b.filter(s.port(), |(i, _)| *i == 1).unwrap();
                        let pair = b.join_equal(&l, &r, |_| 0, |_| 0).unwrap();
                        b.score("target", &pair, move |((_, a), (_, b))| {
                            reward(*a, *b, unassign)
                        })
                        .unwrap();
                        let r =
                            stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c)
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
                        let lists = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
                            .unwrap();
                        let la = lists.initial_assignment();
                        assert_eq!(
                            list_moves(&lists, &la, family).unwrap().collect::<Vec<_>>(),
                            list_moves(&lists, &la, Neighborhood::ChangeAndSwap)
                                .unwrap()
                                .collect::<Vec<_>>()
                        );
                        let mixed = MixedProblem::new(p.clone(), lists);
                        let mut net = MixedConstraintNetwork::new(2, 1, 2);
                        net.add_constraint(
                            "target",
                            vec![
                                MixedScope::Basic(0),
                                MixedScope::Basic(1),
                                MixedScope::ListEntity(0),
                            ],
                            move |f| {
                                let (
                                    MixedFact::Basic(a),
                                    MixedFact::Basic(b),
                                    MixedFact::ListEntity(row),
                                ) = (&f[0], &f[1], &f[2])
                                else {
                                    unreachable!()
                                };
                                if *row == [1, 0] {
                                    reward(a.copied(), b.copied(), unassign)
                                } else {
                                    SimpleScore::ZERO
                                }
                            },
                        )
                        .unwrap();
                        for joint_moves in [false, true] {
                            let cfg = MixedLocalSearchConfig {
                                search: c,
                                joint_moves,
                            };
                            let a = mixed_local_search_from(
                                &mixed,
                                &net,
                                mixed.initial_assignment(),
                                cfg,
                            )
                            .unwrap();
                            let mut b = StreamGraphBuilder::new();
                            let s = MixedStreamSources::new(&mut b);
                            let l = b.filter(s.basic.port(), |(i, _)| *i == 0).unwrap();
                            let r = b.filter(s.basic.port(), |(i, _)| *i == 1).unwrap();
                            let pair = b.join_equal(&l, &r, |_| 0, |_| 0).unwrap();
                            let joined = b
                                .join_equal(&pair, s.lists.entities.port(), |_| 0, |_| 0)
                                .unwrap();
                            b.score("target", &joined, move |(((_, a), (_, b)), (_, row))| {
                                if row == &[1, 0] {
                                    reward(*a, *b, unassign)
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
fn required_partial_assignments_and_global_population_ranges_have_explicit_boundaries() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]),
        Variable::new(vec![1]),
        Variable::new(vec![2]),
    ])
    .unwrap();
    let a = p.initial_assignment();
    assert!(!p.is_complete(&a));
    assert_eq!(
        mass_assign_moves(&p, &a, MassAssignmentSize::all())
            .unwrap()
            .count(),
        0
    );
    let m: Vec<_> = mass_assign_moves(&p, &a, MassAssignmentSize::exactly(2))
        .unwrap()
        .collect();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].members(), [0, 1]);
    let mut partial = a.clone();
    p.apply(&mut partial, m[0].movement()).unwrap().commit();
    assert!(!p.is_complete(&partial));
    assert_eq!(partial.values(), [Some(1), Some(1), None]);
    assert_eq!(
        mass_assign_moves(&p, &partial, MassAssignmentSize::all())
            .unwrap()
            .count(),
        0
    );
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 2]).unwrap();
    let mut a = p.initial_assignment();
    let m = mass_assign_moves(&p, &a, MassAssignmentSize::all())
        .unwrap()
        .next()
        .unwrap();
    p.apply(&mut a, m.movement()).unwrap().commit();
    assert!(p.is_complete(&a));
    let p = FiniteProblem::new(vec![Variable::<i32>::new(vec![]), Variable::new(vec![1])]).unwrap();
    let a = p.initial_assignment();
    let mut m = mass_assign_moves(&p, &a, MassAssignmentSize::all()).unwrap();
    assert!(m.next().is_none());
    assert!(m.next().is_none());
}
#[test]
fn invalid_unassignment_fails_before_any_basic_or_mixed_scoring_or_factory() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("score", vec![0], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    for size in [
        MassAssignmentSize::exactly(0),
        MassAssignmentSize::exactly(1),
        MassAssignmentSize::all(),
    ] {
        assert!(mass_unassign_moves(&p, &p.initial_assignment(), size).is_err());
        assert!(sample_mass_unassign_moves(&p, &p.initial_assignment(), size, 0, 0).is_err());
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::MassUnassign(size),
            ..Default::default()
        };
        for budget in [0, 100] {
            let c = LocalSearchConfig {
                max_evaluations: budget,
                ..config
            };
            let sequence = SolverSequence {
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig::default()),
                    SolverPhase::LocalSearch(c),
                ],
                limits: SolverLimits {
                    max_evaluations: budget,
                    ..Default::default()
                },
            };
            assert!(matches!(
                solve(&p, &n, sequence.clone()),
                Err(SolverError::Phase { phase: 1, .. })
            ));
            assert!(matches!(
                solve_stream::<i32, SimpleScore>(
                    &p,
                    || {
                        calls.set(calls.get() + 1);
                        unreachable!()
                    },
                    sequence
                ),
                Err(SolverError::Phase { phase: 1, .. })
            ));
            assert!(local_search_from(&p, &n, p.initial_assignment(), c).is_err());
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<i32>)>();
            b.score("score", s.port(), |_| {
                calls.set(calls.get() + 1);
                SimpleScore::ZERO
            })
            .unwrap();
            assert!(stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c).is_err());
            let mixed =
                MixedProblem::new(p.clone(), ListProblem::<i32>::new(vec![], vec![]).unwrap());
            let mut net = MixedConstraintNetwork::new(2, 0, 0);
            net.add_constraint("score", vec![MixedScope::Basic(0)], |_| {
                calls.set(calls.get() + 1);
                SimpleScore::ZERO
            })
            .unwrap();
            let cfg = MixedLocalSearchConfig {
                search: c,
                joint_moves: true,
            };
            let sequence = MixedSolverSequence {
                phases: vec![
                    MixedSolverPhase::Construction(MixedConstructionConfig::default()),
                    MixedSolverPhase::LocalSearch(cfg),
                ],
                limits: SolverLimits {
                    max_evaluations: budget,
                    ..Default::default()
                },
            };
            assert!(matches!(
                solve_mixed(&mixed, &net, sequence.clone()),
                Err(SolverError::Phase { phase: 1, .. })
            ));
            assert!(matches!(
                solve_mixed_stream::<i32, i32, SimpleScore>(
                    &mixed,
                    || {
                        calls.set(calls.get() + 1);
                        unreachable!()
                    },
                    sequence
                ),
                Err(SolverError::Phase { phase: 1, .. })
            ));
            assert!(
                mixed_local_search_from(&mixed, &net, mixed.initial_assignment(), cfg).is_err()
            );
            assert!(
                mixed_moves(&mixed, &mixed.initial_assignment(), c.neighborhood, true).is_err()
            );
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::new(&mut b);
            b.score("score", s.basic.port(), |_| {
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
    assert_eq!(calls.get(), 0);
}
