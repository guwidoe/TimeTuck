//! Original per-family grouping, aligned swap and repair integration regressions.
use timetuck::*;
fn successors(p: &MixedProblem<i32, i32>, c: MixedNeighborhood<'_>) -> Vec<Vec<Option<i32>>> {
    let a = p.initial_assignment();
    let mut result = mixed_moves_configured(p, &a, c)
        .unwrap()
        .map(|m| {
            let mut next = a.clone();
            p.apply(&mut next, &m).unwrap().commit();
            p.validate(&next).unwrap();
            assert_eq!(next.lists(), a.lists());
            next.basic().values().to_vec()
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}
fn row_swap(schema: &PillarSchema) -> MixedNeighborhood<'_> {
    let selection = SubPillarSelection::combinations(1, 1).unwrap();
    MixedNeighborhood {
        basic: Some(Neighborhood::SubPillar(SubPillarConfig {
            selection,
            changes: PillarChangeMode::Disabled,
            swap_selection: Some(selection),
        })),
        lists: None,
        pillar_schema: Some(schema),
        ..Default::default()
    }
}
#[test]
fn distinct_family_schemas_match_independent_successors_across_states_pins_and_ranges() {
    let rows = PillarSchema::new(vec![vec![0, 1], vec![2, 3], vec![4, 5]]).unwrap();
    let column = PillarSchema::new(vec![vec![0], vec![2], vec![4]]).unwrap();
    for mask in 0..64 {
        for pin in [false, true] {
            for restricted in [false, true] {
                let values = (0..6).map(|i| Some((mask >> i) & 1)).collect::<Vec<_>>();
                let variables = (0..6)
                    .map(|i| {
                        Variable::new(if restricted && i == 3 {
                            vec![values[i].unwrap()]
                        } else {
                            vec![0, 1]
                        })
                        .with_initial(values[i].unwrap())
                        .with_pinned(pin && i == 5)
                    })
                    .collect();
                let p = MixedProblem::new(
                    FiniteProblem::new(variables)
                        .unwrap()
                        .with_pillar_schema(rows.clone())
                        .unwrap(),
                    ListProblem::new(vec![10], vec![ListEntity::new(vec![10])]).unwrap(),
                );
                let change = MixedNeighborhood {
                    basic: Some(Neighborhood::Pillar { swaps: false }),
                    lists: None,
                    pillar_schema: Some(&column),
                    ..Default::default()
                };
                // Column-only grouping ignores different second-column values and pins.
                let mut expected = Vec::new();
                for old in [0, 1] {
                    let members = [0, 2, 4]
                        .into_iter()
                        .filter(|i| values[*i] == Some(old))
                        .collect::<Vec<_>>();
                    if members.is_empty() {
                        continue;
                    }
                    let mut next = values.clone();
                    for i in members {
                        next[i] = Some(1 - old);
                    }
                    expected.push(next);
                }
                expected.sort();
                assert_eq!(successors(&p, change.clone()), expected);
                let mut swaps = Vec::new();
                for l in 0..3 {
                    for r in 0..3 {
                        if l == r
                            || (pin && (l == 2 || r == 2))
                            || values[2 * l..2 * l + 2] == values[2 * r..2 * r + 2]
                        {
                            continue;
                        }
                        let mut next = values.clone();
                        for c in 0..2 {
                            next.swap(2 * l + c, 2 * r + c);
                        }
                        if restricted && next[3] != values[3] {
                            continue;
                        }
                        swaps.push(next);
                    }
                }
                swaps.sort();
                assert_eq!(successors(&p, row_swap(&rows)), swaps);
                assert_eq!(p.basic().pillar_schema(), Some(&rows));
                assert_eq!(p.initial_assignment().basic().values(), values);
                // Default keeps the model's row schema; independent union includes
                // both actual families, with all duplicate orientations retained.
                let default = MixedNeighborhood {
                    pillar_schema: None,
                    ..row_swap(&rows)
                };
                assert_eq!(successors(&p, default), swaps);
                let children = [(change, 1), (row_swap(&rows), 1)];
                let a = p.initial_assignment();
                let union =
                    mixed_moves_configured_union(&p, &a, &children, UnionOrder::Sequential, 0)
                        .unwrap()
                        .count();
                assert_eq!(union, expected.len() + swaps.len());
            }
        }
    }
}
#[test]
fn invalid_overrides_fail_before_scoring_and_disabled_overrides_are_ignored() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::<i32>::new(vec![], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let a = p.initial_assignment();
    let bad = PillarSchema::new(vec![vec![5]]).unwrap();
    let c = MixedNeighborhood {
        pillar_schema: Some(&bad),
        basic: Some(Neighborhood::Pillar { swaps: false }),
        lists: None,
        ..Default::default()
    };
    assert!(matches!(
        mixed_moves_configured(&p, &a, c.clone()),
        Err(MixedNeighborhoodError::Pillar(_))
    ));
    assert!(matches!(
        mixed_moves_configured(
            &p,
            &a,
            MixedNeighborhood {
                basic: Some(Neighborhood::Change),
                ..c.clone()
            }
        ),
        Err(MixedNeighborhoodError::SchemaRequiresPillarNeighborhood)
    ));
    assert_eq!(
        mixed_moves_configured(
            &p,
            &a,
            MixedNeighborhood {
                basic: None,
                ..c.clone()
            }
        )
        .unwrap()
        .count(),
        0
    );
    let children = [(c.clone(), 0)];
    assert_eq!(
        mixed_moves_configured_union(&p, &a, &children, UnionOrder::Sequential, 0)
            .unwrap()
            .count(),
        0
    );
    let families = [(RepairNeighborhood::Mixed(c), 1)];
    let plan = MixedSolverSequence {
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                search: RuinRecreateConfig {
                    additional_neighborhoods: &families,
                    ..Default::default()
                },
                ..Default::default()
            }),
        ],
        ..Default::default()
    };
    let mut n = MixedConstraintNetwork::new(1, 1, 0);
    n.add_constraint(
        "must not run",
        vec![MixedScope::Basic(0)],
        |_| -> SimpleScore { panic!("validation must precede scoring") },
    )
    .unwrap();
    assert!(solve_mixed(&p, &n, plan.clone()).is_err());
    assert!(
        solve_mixed_stream::<_, _, SimpleScore>(
            &p,
            || panic!("validation must precede graph construction"),
            plan
        )
        .is_err()
    );
}
#[test]
fn column_pillars_cross_a_local_barrier_in_both_complete_solver_backends() {
    let rows = PillarSchema::new(vec![vec![0, 1], vec![2, 3]]).unwrap();
    let column = PillarSchema::new(vec![vec![0], vec![2]]).unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(
            [0, 0, 0, 1]
                .map(|v| Variable::new(vec![0, 1]).with_initial(v))
                .to_vec(),
        )
        .unwrap()
        .with_pillar_schema(rows)
        .unwrap(),
        ListProblem::<i32>::new(vec![10], vec![ListEntity::new(vec![10])]).unwrap(),
    );
    fn score(a: Option<&i32>, b: Option<&i32>) -> SimpleScore {
        SimpleScore::new(match (a, b) {
            (Some(1), Some(1)) => 10,
            (Some(0), Some(0)) => 0,
            _ => -1,
        })
    }
    let mut n = MixedConstraintNetwork::new(4, 1, 1);
    n.add_constraint(
        "barrier",
        vec![MixedScope::Basic(0), MixedScope::Basic(2)],
        |f| {
            let MixedFact::Basic(a) = f[0] else { panic!() };
            let MixedFact::Basic(b) = f[1] else { panic!() };
            score(a, b)
        },
    )
    .unwrap();
    let graph = || {
        let mut g = StreamGraphBuilder::new();
        let sources = MixedStreamSources::<i32, i32>::new(&mut g);
        let a = g.filter(sources.basic.port(), |(i, _)| *i == 0)?;
        let b = g.filter(sources.basic.port(), |(i, _)| *i == 2)?;
        let pair = g.join_equal(&a, &b, |_| (), |_| ())?;
        g.score("barrier", &pair, |((_, a), (_, b))| {
            score(a.as_ref(), b.as_ref())
        })?;
        Ok((g.build(), sources))
    };
    for schema in [None, Some(&column)] {
        let families = [(
            RepairNeighborhood::Mixed(MixedNeighborhood {
                basic: Some(Neighborhood::Pillar { swaps: false }),
                lists: None,
                basic_variables: Some(&[0, 2]),
                pillar_schema: schema,
                ..Default::default()
            }),
            1,
        )];
        let plan = MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(Default::default()),
                MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                    search: RuinRecreateConfig {
                        additional_neighborhoods: &families,
                        move_weights: RepairMoveWeights {
                            change: 0,
                            swap: 0,
                            repair: 0,
                        },
                        max_attempts: 8,
                        max_move_evaluations: Some(8),
                        assert_scores: true,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let r = solve_mixed(&p, &n, plan.clone()).unwrap();
        let s = solve_mixed_stream(&p, graph, plan).unwrap();
        assert_eq!(r.assignment, s.assignment);
        assert_eq!(r.score, s.score);
        assert_eq!(r.work.move_evaluations, s.work.move_evaluations);
        assert_eq!(
            r.score,
            Some(SimpleScore::new(if schema.is_some() { 10 } else { 0 }))
        );
        assert_eq!(r.assignment.basic().values()[1], Some(0));
        assert_eq!(r.assignment.basic().values()[3], Some(1));
        assert_eq!(r.assignment.lists(), p.initial_assignment().lists());
    }
}
