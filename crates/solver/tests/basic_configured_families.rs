//! Original scoped basic-family enumeration and solver regressions.
use timetuck::*;
#[test]
fn scoped_changes_and_swaps_match_independent_ordered_candidates() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned(); 3]).unwrap();
    for x in [None, Some(0), Some(1)] {
        for y in [None, Some(0), Some(1)] {
            for z in [None, Some(0), Some(1)] {
                let a = Assignment::new(vec![x, y, z]);
                for mask in 0..8 {
                    let ids = (0..3)
                        .rev()
                        .filter(|i| mask & (1 << i) != 0)
                        .collect::<Vec<_>>();
                    let reference = &a;
                    let changes = ids
                        .iter()
                        .flat_map(|&i| {
                            [Some(0), Some(1), None]
                                .into_iter()
                                .filter(move |v| *v != reference.values()[i])
                                .map(move |v| Move::Change {
                                    variable: i,
                                    value: v,
                                })
                        })
                        .collect::<Vec<_>>();
                    let mut swaps = vec![];
                    for &i in &ids {
                        for &j in &ids {
                            if i != j && a.values()[i] != a.values()[j] {
                                swaps.push(Move::Swap { left: i, right: j });
                            }
                        }
                    }
                    for (neighborhood, expected) in
                        [(Neighborhood::Change, changes), (Neighborhood::Swap, swaps)]
                    {
                        let c = BasicNeighborhood {
                            neighborhood,
                            variables: Some(&ids),
                            ..Default::default()
                        };
                        assert_eq!(
                            basic_moves_configured(&p, &a, c)
                                .unwrap()
                                .collect::<Vec<_>>(),
                            expected
                        );
                    }
                }
            }
        }
    }
}
type BasicGraph = (
    StreamGraph<'static, SimpleScore>,
    StreamSource<usize, (usize, Option<i32>)>,
);
fn graph() -> Result<BasicGraph, GraphError> {
    let mut g = StreamGraphBuilder::new();
    let s = g.source::<usize, (usize, Option<i32>)>();
    let a = g.filter(s.port(), |(i, _)| *i == 0)?;
    let b = g.filter(s.port(), |(i, _)| *i == 2)?;
    let pair = g.join_equal(&a, &b, |_| (), |_| ())?;
    g.score("barrier", &pair, |((_, a), (_, b))| {
        score(a.as_ref(), b.as_ref())
    })?;
    Ok((g.build(), s))
}
fn score(a: Option<&i32>, b: Option<&i32>) -> SimpleScore {
    SimpleScore::new(match (a, b) {
        (Some(1), Some(1)) => 10,
        (Some(0), Some(0)) => 0,
        _ => -1,
    })
}
#[test]
fn independently_grouped_basic_families_solve_with_both_backends_and_preserve_exclusions() {
    let rows = PillarSchema::new(vec![vec![0, 1], vec![2, 3]]).unwrap();
    let column = PillarSchema::new(vec![vec![0], vec![2]]).unwrap();
    let p = FiniteProblem::new(
        [0, 0, 0, 1]
            .map(|v| Variable::new(vec![0, 1]).with_initial(v))
            .to_vec(),
    )
    .unwrap()
    .with_pillar_schema(rows.clone())
    .unwrap();
    let mut n = ConstraintNetwork::new(4);
    n.add_constraint("barrier", vec![0, 2], |v| score(v[0], v[1]))
        .unwrap();
    for schema in [None, Some(&column)] {
        let c = BasicNeighborhood {
            value_orders: &[],

            neighborhood: Neighborhood::Pillar { swaps: false },
            variables: Some(&[0, 2]),
            pillar_schema: schema,
        };
        let children = [(c.clone(), 1), (c.clone(), 1)];
        let a = p.initial_assignment();
        let expected = basic_moves_configured(&p, &a, c.clone())
            .unwrap()
            .chain(basic_moves_configured(&p, &a, c.clone()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            basic_moves_configured_union(&p, &a, &children, UnionOrder::Sequential, 0)
                .unwrap()
                .collect::<Vec<_>>(),
            expected
        );
        for step in 0..4 {
            let run = || {
                basic_moves_configured_union(
                    &p,
                    &a,
                    &children,
                    UnionOrder::Random { seed: 91 },
                    step,
                )
                .unwrap()
                .collect::<Vec<_>>()
            };
            let actual = run();
            assert_eq!(actual, run());
            let mut remaining = expected.clone();
            for m in actual {
                let at = remaining.iter().position(|x| x == &m).unwrap();
                remaining.remove(at);
            }
            assert!(remaining.is_empty());
        }
        let families = [(RepairNeighborhood::Basic(c), 1)];
        for budget in 0..5 {
            let plan = SolverSequence {
                limits: SolverLimits {
                    max_move_evaluations: Some(budget),
                    ..Default::default()
                },
                phases: vec![
                    SolverPhase::Construction(Default::default()),
                    SolverPhase::RuinRecreate(RuinRecreateConfig {
                        additional_neighborhoods: &families,
                        move_weights: RepairMoveWeights {
                            change: 0,
                            swap: 0,
                            repair: 0,
                        },
                        max_attempts: 4,
                        assert_scores: true,
                        ..Default::default()
                    }),
                ],
            };
            let r = solve(&p, &n, plan.clone()).unwrap();
            let s = solve_stream(&p, graph, plan).unwrap();
            assert_eq!(r.assignment, s.assignment);
            assert_eq!(r.score, s.score);
            assert_eq!(r.work.move_evaluations, s.work.move_evaluations);
            assert!(r.work.move_evaluations <= budget);
            assert_eq!(
                r.score,
                Some(SimpleScore::new(if schema.is_some() && budget > 0 {
                    10
                } else {
                    0
                }))
            );
            assert_eq!(r.assignment.values()[1], Some(0));
            assert_eq!(r.assignment.values()[3], Some(1));
        }
    }
    assert_eq!(p.pillar_schema(), Some(&rows));
}
#[test]
fn invalid_basic_family_is_rejected_before_callbacks_and_disabled_children_skip_it() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let a = p.initial_assignment();
    let bad_schema = PillarSchema::new(vec![vec![5]]).unwrap();
    for c in [
        BasicNeighborhood {
            variables: Some(&[1]),
            ..Default::default()
        },
        BasicNeighborhood {
            variables: Some(&[0, 0]),
            ..Default::default()
        },
        BasicNeighborhood {
            pillar_schema: Some(&bad_schema),
            ..Default::default()
        },
        BasicNeighborhood {
            neighborhood: Neighborhood::Pillar { swaps: false },
            pillar_schema: Some(&bad_schema),
            ..Default::default()
        },
    ] {
        assert!(basic_moves_configured(&p, &a, c.clone()).is_err());
        let children = [(c.clone(), 0)];
        assert_eq!(
            basic_moves_configured_union(&p, &a, &children, UnionOrder::Sequential, 0)
                .unwrap()
                .count(),
            0
        );
        let families = [(RepairNeighborhood::Basic(c), 1)];
        let plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::RuinRecreate(RuinRecreateConfig {
                    additional_neighborhoods: &families,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("no scoring", vec![0], |_| -> SimpleScore {
            panic!("invalid before scoring")
        })
        .unwrap();
        assert!(solve(&p, &n, plan.clone()).is_err());
        assert!(
            solve_stream::<_, SimpleScore>(&p, || panic!("invalid before graph"), plan).is_err()
        );
    }
    let lists = ListProblem::new(vec![10], vec![ListEntity::new(vec![10])]).unwrap();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 1);
    let families = [(RepairNeighborhood::Basic(BasicNeighborhood::default()), 1)];
    assert!(matches!(
        ruin_recreate_list_search_from(
            &lists,
            &n,
            lists.initial_assignment(),
            RuinRecreateConfig {
                additional_neighborhoods: &families,
                ..Default::default()
            }
        ),
        Err(RuinRecreateError::BasicFamilyRequiresBasicVariables)
    ));
}
#[test]
fn basic_repair_family_on_mixed_models_preserves_lists_and_matches_mixed_configuration() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap(),
        ListProblem::new(
            vec![10, 20],
            vec![ListEntity::new(vec![10]), ListEntity::new(vec![20])],
        )
        .unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(2, 2, 2);
    n.add_constraint("selected", vec![MixedScope::Basic(1)], |f| {
        let MixedFact::Basic(v) = f[0] else { panic!() };
        SimpleScore::new(i64::from(v == Some(&1)))
    })
    .unwrap();
    let configured = BasicNeighborhood {
        neighborhood: Neighborhood::Change,
        variables: Some(&[1]),
        ..Default::default()
    };
    let mut results = vec![];
    for family in [
        RepairNeighborhood::Basic(configured),
        RepairNeighborhood::Mixed(MixedNeighborhood {
            basic: Some(Neighborhood::Change),
            lists: None,
            basic_variables: Some(&[1]),
            ..Default::default()
        }),
    ] {
        let families = [(family, 1)];
        let c = MixedRuinRecreateConfig {
            search: RuinRecreateConfig {
                additional_neighborhoods: &families,
                move_weights: RepairMoveWeights {
                    change: 0,
                    swap: 0,
                    repair: 0,
                },
                max_attempts: 8,
                assert_scores: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let r = ruin_recreate_mixed_search_from(&p, &n, p.initial_assignment(), c.clone()).unwrap();
        let s = ruin_recreate_mixed_stream_search_from(
            &p,
            || {
                let mut g = StreamGraphBuilder::new();
                let sources = MixedStreamSources::<i32, i32>::new(&mut g);
                let selected = g.filter(sources.basic.port(), |(i, _)| *i == 1)?;
                g.score("selected", &selected, |(_, v)| {
                    SimpleScore::new(i64::from(*v == Some(1)))
                })?;
                Ok((g.build(), sources))
            },
            p.initial_assignment(),
            c,
        )
        .unwrap();
        assert_eq!(r, s);
        let best = r.best.as_ref().unwrap();
        assert_eq!(best.score, SimpleScore::new(1));
        assert_eq!(best.assignment.basic().values(), &[Some(0), Some(1)]);
        assert_eq!(best.assignment.lists(), p.initial_assignment().lists());
        results.push(r);
    }
    assert_eq!(results[0], results[1]);
}
