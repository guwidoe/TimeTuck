// Original independent subset/state references and solver integration checks.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
#[test]
fn finite_group_successors_match_bitmask_reference_for_ranges_pins_nulls_and_sizes() {
    let mut comparisons = 0;
    for state in 0..27 {
        for optional in 0..8 {
            for pins in 0..8 {
                let mut code = state;
                let mut vars = vec![];
                for i in 0..3 {
                    let value = match code % 3 {
                        0 => None,
                        1 => Some(i),
                        _ => Some(3),
                    };
                    code /= 3;
                    let mut v = Variable::new(vec![i, 3]);
                    if let Some(x) = value {
                        v = v.with_initial(x);
                    }
                    if optional & (1 << i) != 0 {
                        v = v.allowing_unassigned();
                    }
                    vars.push(v.with_pinned(pins & (1 << i) != 0));
                }
                let Ok(p) = FiniteProblem::new(vars) else {
                    continue;
                };
                let a = p.initial_assignment();
                for policy in [None, Some(false), Some(true)] {
                    for (min, max, all) in [
                        (0, 0, false),
                        (1, 1, false),
                        (2, 2, false),
                        (0, 3, false),
                        (3, usize::MAX, false),
                        (4, usize::MAX, false),
                        (0, 0, true),
                    ] {
                        let c = if all {
                            MassChangeConfig::all()
                        } else {
                            MassChangeConfig::between(min, max).unwrap()
                        };
                        let c = policy.map_or(c, |b| c.with_crossing_null(b));
                        let actual = mass_change_moves(&p, &a, c);
                        if policy == Some(true) && optional != 7 {
                            assert!(matches!(
                                actual,
                                Err(ModelError::NullCrossingRequired { .. })
                            ));
                            continue;
                        }
                        let eligible: Vec<_> = (0..3)
                            .filter(|&i| {
                                pins & (1 << i) == 0
                                    && (a.values()[i].is_some()
                                        || (policy != Some(false) && optional & (1 << i) != 0))
                            })
                            .collect();
                        let mut expected = BTreeSet::new();
                        for mask in 0..8 {
                            let group: Vec<_> = (0..3).filter(|i| mask & (1 << i) != 0).collect();
                            if group.len() < 2
                                || !group.iter().all(|i| eligible.contains(i))
                                || (all && group.len() != eligible.len())
                                || (!all && (group.len() < min || group.len() > max))
                            {
                                continue;
                            }
                            for target in [None, Some(0), Some(1), Some(2), Some(3)] {
                                if !group.iter().all(|&i| match target {
                                    None => policy != Some(false) && optional & (1 << i) != 0,
                                    Some(v) => v == i as i32 || v == 3,
                                }) {
                                    continue;
                                }
                                let mut values = a.values().to_vec();
                                for &i in &group {
                                    values[i] = target;
                                }
                                if values != a.values() {
                                    expected.insert(values);
                                }
                            }
                        }
                        let mut successors = BTreeSet::new();
                        for m in actual.unwrap() {
                            let mut expected = a.values().to_vec();
                            for &i in m.members() {
                                expected[i] = m.destination().copied();
                            }
                            let mut copy = a.clone();
                            {
                                let trial = p.apply(&mut copy, m.movement()).unwrap();
                                assert_eq!(trial.values(), expected);
                            }
                            assert_eq!(copy, a);
                            p.apply(&mut copy, m.movement()).unwrap().commit();
                            assert_ne!(copy, a);
                            assert_eq!(copy.values(), expected);
                            if p.is_complete(&a) {
                                assert!(p.is_complete(&copy));
                            }
                            successors.insert(copy.values().to_vec());
                        }
                        assert_eq!(
                            successors, expected,
                            "{state} {optional} {pins} {policy:?} {min} {max} {all}"
                        );
                        comparisons += 1;
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 20146);
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
fn large_subsets_are_lazy_and_invalid_bounds_and_empty_populations_terminate() {
    let count = Rc::new(Cell::new(0));
    let p = FiniteProblem::new(
        (0..128)
            .map(|_| {
                Variable::new(vec![Counted(0, count.clone()), Counted(1, count.clone())])
                    .with_initial(Counted(0, count.clone()))
            })
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    count.set(0);
    let mut moves =
        mass_change_moves(&p, &a, MassChangeConfig::between(64, usize::MAX).unwrap()).unwrap();
    assert_eq!(count.get(), 0);
    assert_eq!(moves.next().unwrap().members().len(), 64);
    assert!(count.get() < 100);
    assert_eq!(moves.next().unwrap().members().len(), 64);
    assert!(count.get() < 200);
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    let a = p.initial_assignment();
    let mut moves = mass_change_moves(&p, &a, MassChangeConfig::all()).unwrap();
    assert!(moves.next().is_none());
    assert!(moves.next().is_none());
    assert!(matches!(
        MassChangeConfig::between(3, 2),
        Err(ModelError::InvalidSampleSize {
            minimum: 3,
            maximum: 2
        })
    ));
    assert!(
        mass_change_moves(
            &p,
            &Assignment::new(vec![None]),
            MassChangeConfig::exactly(0)
        )
        .is_err()
    );
}
fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2]).with_initial(1),
    ])
    .unwrap()
}
fn reward(a: Option<i32>, b: Option<i32>) -> SimpleScore {
    SimpleScore::new(i64::from(a == Some(2) && b == Some(2)))
}
#[test]
fn arbitrary_groups_escape_plateaus_with_scoped_and_stream_basic_and_joint_mixed_search() {
    let p = problem();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("target", vec![0, 1], |v| {
        reward(v[0].copied(), v[1].copied())
    })
    .unwrap();
    for neighborhood in [
        Neighborhood::ChangeAndSwap,
        Neighborhood::Pillar { swaps: true },
        Neighborhood::MassChange(MassChangeConfig::all()),
        Neighborhood::MassChange(MassChangeConfig::exactly(2)),
    ] {
        for move_order in [
            MoveOrder::Original,
            MoveOrder::ShuffledVariables { seed: 17 },
            MoveOrder::ShuffledMoves {
                seed: 17,
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
                for budget in [0, 1, 2, 5, 100] {
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
                    let left = b.filter(s.port(), |(i, _)| *i == 0).unwrap();
                    let right = b.filter(s.port(), |(i, _)| *i == 1).unwrap();
                    let pair = b.join_equal(&left, &right, |_| 0, |_| 0).unwrap();
                    b.score("target", &pair, |((_, a), (_, b))| reward(*a, *b))
                        .unwrap();
                    let r = stream_local_search_from(&p, b.build(), s, p.initial_assignment(), c)
                        .unwrap();
                    assert_eq!(a.best, r.best);
                    assert_eq!(a.evaluations, r.evaluations);
                    assert_eq!(a.completed_steps, r.completed_steps);
                    assert_eq!(a.termination, r.termination);
                    assert!(a.evaluations <= budget);
                    if budget == 100 && acceptance == Acceptance::StrictImprovement {
                        assert_eq!(
                            a.best.unwrap().score,
                            SimpleScore::new(i64::from(matches!(
                                neighborhood,
                                Neighborhood::MassChange(_)
                            )))
                        );
                    }
                    let mixed = MixedProblem::new(
                        p.clone(),
                        ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap(),
                    );
                    let mut net = MixedConstraintNetwork::new(2, 1, 2);
                    net.add_constraint(
                        "target",
                        vec![
                            MixedScope::Basic(0),
                            MixedScope::Basic(1),
                            MixedScope::ListEntity(0),
                        ],
                        |f| {
                            let (
                                MixedFact::Basic(a),
                                MixedFact::Basic(b),
                                MixedFact::ListEntity(row),
                            ) = (&f[0], &f[1], &f[2])
                            else {
                                unreachable!()
                            };
                            if *row == [1, 0] {
                                reward(a.copied(), b.copied())
                            } else {
                                SimpleScore::ZERO
                            }
                        },
                    )
                    .unwrap();
                    let cfg = MixedLocalSearchConfig {
                        search: c,
                        joint_moves: true,
                    };
                    let a = mixed_local_search_from(&mixed, &net, mixed.initial_assignment(), cfg)
                        .unwrap();
                    let mut b = StreamGraphBuilder::new();
                    let s = MixedStreamSources::new(&mut b);
                    let left = b.filter(s.basic.port(), |(i, _)| *i == 0).unwrap();
                    let right = b.filter(s.basic.port(), |(i, _)| *i == 1).unwrap();
                    let pair = b.join_equal(&left, &right, |_| 0, |_| 0).unwrap();
                    let joined = b
                        .join_equal(&pair, s.lists.entities.port(), |_| 0, |_| 0)
                        .unwrap();
                    b.score("target", &joined, |(((_, a), (_, b)), (_, row))| {
                        if row == &[1, 0] {
                            reward(*a, *b)
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
                    assert_eq!(a.completed_steps, r.completed_steps);
                    assert_eq!(a.termination, r.termination);
                    assert!(a.evaluations <= budget);
                    if budget == 100 && acceptance == Acceptance::StrictImprovement {
                        assert_eq!(
                            a.best.unwrap().score,
                            SimpleScore::new(i64::from(matches!(
                                neighborhood,
                                Neighborhood::MassChange(_)
                            )))
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn invalid_null_crossing_is_rejected_before_basic_or_mixed_phase_work() {
    let p = problem();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("score", vec![0], |_| {
        calls.set(calls.get() + 1);
        SimpleScore::ZERO
    })
    .unwrap();
    let config = LocalSearchConfig {
        neighborhood: Neighborhood::MassChange(MassChangeConfig::all().with_crossing_null(true)),
        ..Default::default()
    };
    for budget in [0, 100] {
        let sequence = SolverSequence {
            phases: vec![
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::LocalSearch(config),
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
                    unreachable!("invalid plan factory")
                },
                sequence
            ),
            Err(SolverError::Phase { phase: 1, .. })
        ));
        assert!(matches!(
            local_search_from(
                &p,
                &n,
                p.initial_assignment(),
                LocalSearchConfig {
                    max_evaluations: budget,
                    ..config
                }
            ),
            Err(LocalSearchError::Scoring(ScoringError::Model(
                ModelError::NullCrossingRequired { .. }
            )))
        ));
        let mut builder = StreamGraphBuilder::new();
        let source = builder.source::<usize, (usize, Option<i32>)>();
        builder
            .score("score", source.port(), |_| {
                calls.set(calls.get() + 1);
                SimpleScore::ZERO
            })
            .unwrap();
        assert!(
            stream_local_search_from(
                &p,
                builder.build(),
                source,
                p.initial_assignment(),
                LocalSearchConfig {
                    max_evaluations: budget,
                    ..config
                }
            )
            .is_err()
        );
        let mixed = MixedProblem::new(p.clone(), ListProblem::<i32>::new(vec![], vec![]).unwrap());
        let mut net = MixedConstraintNetwork::new(2, 0, 0);
        net.add_constraint("score", vec![MixedScope::Basic(0)], |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
        let cfg = MixedLocalSearchConfig {
            search: config,
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
                    unreachable!("invalid mixed factory")
                },
                sequence
            ),
            Err(SolverError::Phase { phase: 1, .. })
        ));
        assert!(mixed_local_search_from(&mixed, &net, mixed.initial_assignment(), cfg).is_err());
        assert!(
            mixed_moves(
                &mixed,
                &mixed.initial_assignment(),
                config.neighborhood,
                true
            )
            .is_err()
        );
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::new(&mut b);
        b.score("score", s.basic.port(), |_| {
            calls.set(calls.get() + 1);
            SimpleScore::ZERO
        })
        .unwrap();
        assert!(
            mixed_stream_local_search_from(&mixed, b.build(), s, mixed.initial_assignment(), cfg)
                .is_err()
        );
    }
    assert_eq!(calls.get(), 0);
}
#[test]
fn optional_null_crossing_changes_search_and_list_only_paths_keep_ordinary_moves() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).allowing_unassigned(),
        Variable::new(vec![0]).with_initial(0).allowing_unassigned(),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("target", vec![0, 1], |v| {
        SimpleScore::new(i64::from(v.iter().all(|v| v.is_none())))
    })
    .unwrap();
    for policy in [None, Some(false), Some(true)] {
        let c = MassChangeConfig::all();
        let c = policy.map_or(c, |enabled| c.with_crossing_null(enabled));
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::MassChange(c),
            assert_scores: true,
            max_evaluations: 20,
            max_steps: 2,
            ..Default::default()
        };
        let a = local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        let left = b.filter(s.port(), |(i, _)| *i == 0).unwrap();
        let right = b.filter(s.port(), |(i, _)| *i == 1).unwrap();
        let pair = b.join_equal(&left, &right, |_| 0, |_| 0).unwrap();
        b.score("target", &pair, |((_, a), (_, b))| {
            SimpleScore::new(i64::from(a.is_none() && b.is_none()))
        })
        .unwrap();
        let r = stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config).unwrap();
        assert_eq!(a.best, r.best);
        assert_eq!(a.evaluations, r.evaluations);
        assert_eq!(
            a.best.unwrap().score,
            SimpleScore::new(i64::from(policy != Some(false)))
        );
        let lists = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])]).unwrap();
        let initial = lists.initial_assignment();
        assert_eq!(
            list_moves(&lists, &initial, config.neighborhood)
                .unwrap()
                .collect::<Vec<_>>(),
            list_moves(&lists, &initial, Neighborhood::ChangeAndSwap)
                .unwrap()
                .collect::<Vec<_>>()
        );
        let mut net = ListConstraintNetwork::new(1, 2);
        net.add_constraint("order", vec![ListScope::Entity(0)], |f| {
            let ListFact::Entity(row) = f[0] else {
                unreachable!()
            };
            SimpleScore::new(i64::from(row == [1, 0]))
        })
        .unwrap();
        let a = list_local_search_from(&lists, &net, initial.clone(), config).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = ListStreamSources::new(&mut b);
        b.score("order", s.entities.port(), |(_, row)| {
            SimpleScore::new(i64::from(row == &[1, 0]))
        })
        .unwrap();
        let r = list_stream_local_search_from(&lists, b.build(), s, initial, config).unwrap();
        assert_eq!(a.best, r.best);
        assert_eq!(a.evaluations, r.evaluations);
        assert_eq!(a.best.unwrap().score, SimpleScore::new(1));
    }
}
