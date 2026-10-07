use timetuck::*;
fn provider() -> EntitySwapProvider {
    EntitySwapProvider::new(
        vec![vec![0, 1], vec![2, 3]],
        EntitySwapOrder::Unordered(vec![0, 1]),
    )
    .unwrap()
}
#[test]
fn aligned_multi_column_swaps_match_all_small_optional_states_and_undo() {
    for optional in [false, true] {
        let base = if optional { 3_usize } else { 2 };
        for code in 0..base.pow(4) {
            let mut code = code;
            let vars = (0..4)
                .map(|_| {
                    let value = code % base;
                    code /= base;
                    let v = Variable::new(vec![0, 1]);
                    let v = if optional { v.allowing_unassigned() } else { v };
                    if value < 2 { v.with_initial(value) } else { v }
                })
                .collect();
            let p = FiniteProblem::new(vars).unwrap();
            let a = p.initial_assignment();
            let mut expected = a.values().to_vec();
            expected.swap(0, 2);
            expected.swap(1, 3);
            let candidates: Vec<_> = provider().select(&p, &a, 0).unwrap().collect();
            assert_eq!(candidates.len(), usize::from(expected != a.values()));
            for candidate in candidates {
                let mut copy = a.clone();
                {
                    let applied = p.apply(&mut copy, &candidate.movement).unwrap();
                    assert_eq!(applied.values(), expected);
                }
                assert_eq!(copy, a);
                p.apply(&mut copy, &candidate.movement).unwrap().commit();
                assert_eq!(copy.values(), expected);
            }
        }
    }
}
#[test]
fn all_columns_must_accept_and_pins_exclude_entire_rows() {
    let mut vars = vec![
        Variable::new(vec![1, 3]).with_initial(1),
        Variable::new(vec![2]).with_initial(2),
        Variable::new(vec![1, 3]).with_initial(3),
        Variable::new(vec![2, 4]).with_initial(4),
    ];
    let p = FiniteProblem::new(vars.clone()).unwrap();
    assert_eq!(
        provider()
            .select(&p, &p.initial_assignment(), 0)
            .unwrap()
            .count(),
        0
    );
    vars[1] = Variable::new(vec![2, 4]).with_initial(2).with_pinned(true);
    let p = FiniteProblem::new(vars.clone()).unwrap();
    assert_eq!(
        provider()
            .select(&p, &p.initial_assignment(), 0)
            .unwrap()
            .count(),
        0
    );
    vars[1] = Variable::new(vec![2, 4]).with_initial(2);
    vars.push(Variable::new(vec![9]).with_initial(9).with_pinned(true));
    let p = FiniteProblem::new(vars).unwrap();
    assert_eq!(
        provider()
            .select(&p, &p.initial_assignment(), 0)
            .unwrap()
            .count(),
        1
    );
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]),
        Variable::new(vec![1]).with_initial(1),
    ])
    .unwrap();
    let selector = EntitySwapProvider::new(
        vec![vec![0], vec![1]],
        EntitySwapOrder::Unordered(vec![0, 1]),
    )
    .unwrap();
    assert_eq!(
        selector
            .select(&p, &p.initial_assignment(), 0)
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn paired_pin_filtering_and_bad_configuration_are_explicit() {
    assert!(
        EntitySwapProvider::new(vec![vec![0], vec![0]], EntitySwapOrder::Unordered(vec![]))
            .is_err()
    );
    assert!(EntitySwapProvider::new(vec![vec![]], EntitySwapOrder::Unordered(vec![])).is_err());
    assert!(
        EntitySwapProvider::new(
            vec![vec![0], vec![1, 2]],
            EntitySwapOrder::Unordered(vec![])
        )
        .is_err()
    );
    let p = FiniteProblem::new(
        (0..4)
            .map(|i| {
                Variable::new(vec![0, 1, 2, 3])
                    .with_initial(i)
                    .with_pinned(i == 1)
            })
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let rows = (0..4).map(|i| vec![i]).collect::<Vec<_>>();
    let s = EntitySwapProvider::new(
        rows.clone(),
        EntitySwapOrder::Paired {
            left: vec![1, 0, 2],
            right: vec![3, 1, 0],
        },
    )
    .unwrap();
    assert_eq!(
        s.select(&p, &a, 0)
            .unwrap()
            .map(|c| (c.left, c.right))
            .collect::<Vec<_>>(),
        [(0, 3), (2, 0)]
    );
    let s = EntitySwapProvider::new(
        rows,
        EntitySwapOrder::Cartesian {
            left: vec![],
            right: vec![4],
        },
    )
    .unwrap();
    assert!(matches!(
        s.select(&p, &a, 0),
        Err(EntitySwapError::UnknownEntity { .. })
    ));
    assert!(matches!(
        provider().select(&p, &Assignment::new(vec![]), 0),
        Err(EntitySwapError::Model(_))
    ));
    let s = EntitySwapProvider::new(vec![vec![4]], EntitySwapOrder::Unordered(vec![])).unwrap();
    assert!(matches!(
        s.select(&p, &a, 0),
        Err(EntitySwapError::Schema(_))
    ));
}
#[test]
fn sampled_pairs_replay_respect_each_order_and_stop_when_impossible() {
    let p = FiniteProblem::new(
        (0..4)
            .map(|i| Variable::new(vec![0, 1, 2, 3]).with_initial(i))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    for order in [
        EntitySwapOrder::Unordered(vec![3, 1, 0, 2]),
        EntitySwapOrder::Cartesian {
            left: vec![0, 1, 1],
            right: vec![2, 3],
        },
        EntitySwapOrder::Paired {
            left: vec![0, 1, 2],
            right: vec![3, 2, 0],
        },
    ] {
        let selector = EntitySwapProvider::new((0..4).map(|i| vec![i]).collect(), order).unwrap();
        let all: Vec<_> = selector.select(&p, &a, 0).unwrap().collect();
        let sampled = selector.with_sampling(u64::MAX, 300);
        for step in [0, 1, 8] {
            let draws: Vec<_> = sampled.select(&p, &a, step).unwrap().collect();
            assert_eq!(draws.len(), 300);
            assert!(draws.iter().all(|d| all.contains(d)));
            assert_eq!(
                sampled.select(&p, &a, step).unwrap().next(),
                draws.first().cloned()
            );
            assert_eq!(
                sampled.select(&p, &a, step).unwrap().collect::<Vec<_>>(),
                draws
            );
        }
    }
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0]).with_initial(0),
        Variable::new(vec![1]).with_initial(1),
    ])
    .unwrap();
    let s = EntitySwapProvider::new(
        vec![vec![0], vec![1]],
        EntitySwapOrder::Unordered(vec![0, 1]),
    )
    .unwrap()
    .with_sampling(1, 100);
    assert_eq!(s.select(&p, &p.initial_assignment(), 0).unwrap().count(), 0);
    let empty = EntitySwapProvider::new(vec![], EntitySwapOrder::Unordered(vec![]))
        .unwrap()
        .with_sampling(1, u64::MAX);
    assert_eq!(
        empty
            .select(&p, &p.initial_assignment(), 0)
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn basic_and_mixed_plans_exchange_entities_without_touching_other_variables() {
    for sampled in [false, true] {
        let p = FiniteProblem::new(vec![
            Variable::new(vec![1, 3]).with_initial(1),
            Variable::new(vec![2, 4]).with_initial(2),
            Variable::new(vec![1, 3]).with_initial(3),
            Variable::new(vec![2, 4]).with_initial(4),
            Variable::new(vec![9]).with_initial(9),
        ])
        .unwrap();
        let mut n = ConstraintNetwork::new(5);
        n.add_constraint("left", vec![0, 1], |values| {
            SimpleScore::new(values.iter().map(|v| i64::from(v.copied().unwrap())).sum())
        })
        .unwrap();
        let mut s = if sampled {
            provider().with_sampling(32, 100)
        } else {
            provider()
        };
        let search = LocalSearchConfig {
            max_steps: 2,
            assert_scores: true,
            ..Default::default()
        };
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(0, &mut s);
        let result = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(search)],
                ..Default::default()
            },
            ext,
        )
        .unwrap();
        assert_eq!(result.score, Some(SimpleScore::new(7)));
        assert_eq!(
            result.assignment.values(),
            [Some(3), Some(4), Some(1), Some(2), Some(9)]
        );
        let p = MixedProblem::new(
            p,
            ListProblem::new(vec![8], vec![ListEntity::new(vec![8])]).unwrap(),
        );
        let mut n = MixedConstraintNetwork::new(5, 1, 1);
        n.add_constraint(
            "left",
            vec![MixedScope::Basic(0), MixedScope::Basic(1)],
            |facts| {
                SimpleScore::new(
                    facts
                        .iter()
                        .map(|f| {
                            let MixedFact::Basic(v) = f else {
                                unreachable!()
                            };
                            i64::from(v.copied().unwrap())
                        })
                        .sum(),
                )
            },
        )
        .unwrap();
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(0, &mut s);
        let result = solve_mixed_with_extensions(
            &p,
            &n,
            MixedSolverSequence {
                phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search,
                    ..Default::default()
                })],
                ..Default::default()
            },
            ext,
        )
        .unwrap();
        assert_eq!(result.score, Some(SimpleScore::new(7)));
        assert_eq!(result.assignment.lists().lists(), [vec![8]]);
        assert_eq!(result.assignment.basic().values()[4], Some(9));
    }
}
