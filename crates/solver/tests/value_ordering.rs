//! Original ordering, null, grouping, validation and solver regressions.
use timetuck::*;
#[test]
fn every_optional_target_permutation_preserves_family_semantics_and_pillar_alignment() {
    let permutations = [
        [None, Some(0), Some(1)],
        [None, Some(1), Some(0)],
        [Some(0), None, Some(1)],
        [Some(0), Some(1), None],
        [Some(1), None, Some(0)],
        [Some(1), Some(0), None],
    ];
    for old in [None, Some(10), Some(20)] {
        for permutation in permutations {
            for schema in [false, true] {
                let p =
                    FiniteProblem::new(vec![Variable::new(vec![10, 20]).allowing_unassigned(); 2])
                        .unwrap();
                let p = if schema {
                    p.with_pillar_schema(PillarSchema::new(vec![vec![0], vec![1]]).unwrap())
                        .unwrap()
                } else {
                    p
                };
                let a = Assignment::new(vec![old, old]);
                let orders = [
                    BasicValueOrder::new(0, permutation.to_vec()),
                    BasicValueOrder::new(1, permutation.to_vec()),
                ];
                let target_values = permutation.map(|i| i.map(|i| [10, 20][i]));
                for n in [
                    Neighborhood::Change,
                    Neighborhood::ChangeAndSwap,
                    Neighborhood::Assign,
                    Neighborhood::Unassign,
                    Neighborhood::Pillar { swaps: true },
                    Neighborhood::SubPillar(SubPillarConfig {
                        selection: SubPillarSelection::combinations(2, 2).unwrap(),
                        changes: PillarChangeMode::All,
                        swap_selection: None,
                    }),
                    Neighborhood::PillarChange(PillarChangeConfig::default()),
                    Neighborhood::PillarUnassign,
                ] {
                    let grouped = matches!(
                        n,
                        Neighborhood::Pillar { .. }
                            | Neighborhood::SubPillar(_)
                            | Neighborhood::PillarChange(_)
                            | Neighborhood::PillarUnassign
                    );
                    let targets = target_values
                        .into_iter()
                        .filter(|v| *v != old)
                        .filter(|v| match n {
                            Neighborhood::Assign => old.is_none() && v.is_some(),
                            Neighborhood::Unassign | Neighborhood::PillarUnassign => {
                                old.is_some() && v.is_none()
                            }
                            _ => true,
                        })
                        .collect::<Vec<_>>();
                    let mut expected = vec![];
                    if grouped {
                        for v in &targets {
                            expected.push(vec![*v, *v]);
                        }
                    } else {
                        for variable in 0..2 {
                            for v in &targets {
                                let mut next = vec![old, old];
                                next[variable] = *v;
                                expected.push(next);
                            }
                        }
                    }
                    let c = BasicNeighborhood {
                        neighborhood: n.clone(),
                        value_orders: &orders,
                        ..Default::default()
                    };
                    let actual = basic_moves_configured(&p, &a, c)
                        .unwrap()
                        .map(|m| {
                            let mut next = a.clone();
                            p.apply(&mut next, &m).unwrap().commit();
                            next.values().to_vec()
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        actual, expected,
                        "{old:?} {permutation:?} {n:?} schema={schema}"
                    );
                }
            }
        }
    }
}
#[test]
fn stable_sorting_does_not_clone_values_and_group_targets_keep_pins_and_ranges() {
    #[derive(Debug, Eq, PartialEq)]
    struct NonClone {
        key: i32,
        id: i32,
    }
    let p = FiniteProblem::new(vec![
        Variable::new(vec![
            NonClone { key: 2, id: 0 },
            NonClone { key: 1, id: 1 },
            NonClone { key: 1, id: 2 },
        ])
        .allowing_unassigned(),
    ])
    .unwrap();
    let order =
        BasicValueOrder::sorted_by(&p, 0, |a, b| a.map(|v| v.key).cmp(&b.map(|v| v.key))).unwrap();
    assert_eq!(order.values, vec![None, Some(1), Some(2), Some(0)]);
    assert_eq!(p.variables()[0].domain()[0].id, 0);
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0, 1, 2]).with_initial(0),
        Variable::new(vec![0, 2]).with_initial(0),
        Variable::new(vec![0, 1, 2])
            .with_initial(0)
            .with_pinned(true),
    ])
    .unwrap();
    let orders = [BasicValueOrder::new(0, vec![Some(2), Some(1), Some(0)])];
    let a = p.initial_assignment();
    let moves = basic_moves_configured(
        &p,
        &a,
        BasicNeighborhood {
            neighborhood: Neighborhood::Pillar { swaps: false },
            value_orders: &orders,
            ..Default::default()
        },
    )
    .unwrap()
    .collect::<Vec<_>>();
    assert_eq!(moves.len(), 1);
    let mut next = a.clone();
    p.apply(&mut next, &moves[0]).unwrap().commit();
    assert_eq!(next.values(), &[Some(2), Some(2), Some(0)]);
    assert_eq!(p.variables()[0].domain(), &[0, 1, 2]);
}
#[test]
fn malformed_orders_fail_before_solver_callbacks_and_disabled_families_ignore_them() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let a = p.initial_assignment();
    for orders in [
        vec![BasicValueOrder::new(1, vec![])],
        vec![BasicValueOrder::new(0, vec![Some(0)])],
        vec![BasicValueOrder::new(0, vec![Some(0), Some(2)])],
        vec![BasicValueOrder::new(0, vec![Some(0), None])],
        vec![BasicValueOrder::new(0, vec![Some(0), Some(0)])],
        vec![BasicValueOrder::new(0, vec![Some(0), Some(1)]); 2],
    ] {
        let c = BasicNeighborhood {
            neighborhood: Neighborhood::Change,
            value_orders: &orders,
            ..Default::default()
        };
        assert!(matches!(
            basic_moves_configured(&p, &a, c.clone()),
            Err(BasicNeighborhoodError::ValueOrder(_))
        ));
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
        n.add_constraint("no callback", vec![0], |_| -> SimpleScore {
            panic!("invalid before scoring")
        })
        .unwrap();
        assert!(solve(&p, &n, plan.clone()).is_err());
        assert!(
            solve_stream::<_, SimpleScore>(&p, || panic!("invalid before graph"), plan).is_err()
        );
    }
    let orders = [BasicValueOrder::new(0, vec![Some(0), Some(1)])];
    assert!(matches!(
        basic_moves_configured(
            &p,
            &a,
            BasicNeighborhood {
                neighborhood: Neighborhood::Swap,
                value_orders: &orders,
                ..Default::default()
            }
        ),
        Err(BasicNeighborhoodError::ValueOrderingUnsupported)
    ));
    let mixed = MixedProblem::new(
        p,
        ListProblem::<i32>::new(vec![], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let a = mixed.initial_assignment();
    let disabled = MixedNeighborhood {
        basic: None,
        lists: None,
        basic_value_orders: &orders,
        ..Default::default()
    };
    assert_eq!(
        mixed_moves_configured(&mixed, &a, disabled)
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn family_order_changes_budgeted_search_results_in_both_backends_and_preserves_union_children() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 3, 1, 2]).with_initial(0)]).unwrap();
    let ascending = [BasicValueOrder::sorted_by(&p, 0, |a, b| a.cmp(&b)).unwrap()];
    let descending = [BasicValueOrder::sorted_by(&p, 0, |a, b| b.cmp(&a)).unwrap()];
    let families = [
        (
            BasicNeighborhood {
                neighborhood: Neighborhood::Change,
                value_orders: &ascending,
                ..Default::default()
            },
            1,
        ),
        (
            BasicNeighborhood {
                neighborhood: Neighborhood::Change,
                value_orders: &descending,
                ..Default::default()
            },
            1,
        ),
    ];
    let a = p.initial_assignment();
    let targets = basic_moves_configured_union(&p, &a, &families, UnionOrder::Sequential, 0)
        .unwrap()
        .map(|m| {
            let Move::Change { value, .. } = m else {
                panic!()
            };
            value.unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(targets, vec![1, 2, 3, 3, 2, 1]);
    let mixed = MixedProblem::new(
        p.clone(),
        ListProblem::new(vec![10], vec![ListEntity::new(vec![10])]).unwrap(),
    );
    let a = mixed.initial_assignment();
    let children = families.clone().map(|(c, w)| {
        (
            MixedNeighborhood {
                basic: Some(c.neighborhood),
                lists: None,
                basic_value_orders: c.value_orders,
                ..Default::default()
            },
            w,
        )
    });
    let actual = mixed_moves_configured_union(&mixed, &a, &children, UnionOrder::Sequential, 0)
        .unwrap()
        .map(|m| {
            let Move::Change { value, .. } = m.basic else {
                panic!()
            };
            value.unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, targets);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0) as i64)
    })
    .unwrap();
    for (index, (c, _)) in families.into_iter().enumerate() {
        let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            basic_moves_configured(p, a, c.clone())
                .unwrap()
                .collect::<Vec<_>>()
        };
        let plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_move_evaluations: Some(1),
                    max_steps: 1,
                    assert_scores: true,
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(1, &mut provider);
        let r = solve_with_extensions(&p, &n, plan.clone(), ext).unwrap();
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(1, &mut provider);
        let s = solve_stream_with_extensions(
            &p,
            || {
                let mut g = StreamGraphBuilder::new();
                let source = g.source::<usize, (usize, Option<i32>)>();
                g.score("value", source.port(), |(_, v)| {
                    SimpleScore::new(v.unwrap_or(0) as i64)
                })?;
                Ok((g.build(), source))
            },
            plan,
            ext,
        )
        .unwrap();
        assert_eq!(r.assignment, s.assignment);
        assert_eq!(r.score, s.score);
        assert_eq!(r.phases[1].work.move_evaluations, 1);
        assert_eq!(
            r.score,
            Some(SimpleScore::new(if index == 0 { 1 } else { 3 }))
        );
    }
}
