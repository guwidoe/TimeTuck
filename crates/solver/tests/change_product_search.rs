//! Original search traps require simultaneous changes to disjoint value ranges.
use timetuck::*;
fn reward(a: Option<i32>, b: Option<i32>) -> SimpleScore {
    SimpleScore::new(match (a, b) {
        (Some(0), Some(0)) => 0,
        (Some(1), Some(2)) => 10,
        _ => -1,
    })
}
fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 2]).with_initial(0),
    ])
    .unwrap()
}
#[test]
fn independent_targets_escape_traps_in_both_basic_scoring_backends() {
    let p = problem();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("trap", vec![0, 1], |v| reward(v[0].copied(), v[1].copied()))
        .unwrap();
    let product = Neighborhood::ChangeProduct(SubPillarSelection::combinations(2, 2).unwrap());
    for neighborhood in [
        Neighborhood::ChangeAndSwap,
        Neighborhood::MassChange(MassChangeConfig::all()),
        product,
    ] {
        for order in [
            MoveOrder::Original,
            MoveOrder::ShuffledVariables { seed: 23 },
            MoveOrder::ShuffledMoves {
                seed: 91,
                window: 5.try_into().unwrap(),
            },
        ] {
            for budget in [1, 2, 100] {
                let children = [(neighborhood, 1), (Neighborhood::Change, 0)];
                let union = NeighborhoodUnion {
                    children: &children,
                    order: UnionOrder::Random { seed: 42 },
                };
                let c = LocalSearchConfig {
                    neighborhood_union: Some(&union),
                    max_evaluations: budget,
                    move_order: order,
                    assert_scores: true,
                    ..Default::default()
                };
                let a = local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
                let mut builder = StreamGraphBuilder::new();
                let source = builder.source::<usize, (usize, Option<i32>)>();
                let left = builder.filter(source.port(), |(i, _)| *i == 0).unwrap();
                let right = builder.filter(source.port(), |(i, _)| *i == 1).unwrap();
                let pair = builder.join_equal(&left, &right, |_| 0, |_| 0).unwrap();
                builder
                    .score("trap", &pair, |((_, a), (_, b))| reward(*a, *b))
                    .unwrap();
                let b = stream_local_search_from(
                    &p,
                    builder.build(),
                    source,
                    p.initial_assignment(),
                    c,
                )
                .unwrap();
                assert_eq!(a.best, b.best);
                assert_eq!(a.evaluations, b.evaluations);
                assert_eq!(a.termination, b.termination);
                assert!(a.evaluations <= budget);
                assert_eq!(
                    a.best.unwrap().score,
                    SimpleScore::new(if neighborhood == product && budget > 1 {
                        10
                    } else {
                        0
                    })
                );
            }
        }
    }
}
fn mixed_reward(a: Option<i32>, b: Option<i32>, row: &[i32]) -> SimpleScore {
    SimpleScore::new(if a == Some(0) && b == Some(0) && row == [7, 8] {
        0
    } else if a == Some(1) && b == Some(2) && row == [8, 7] {
        10
    } else {
        -1
    })
}
#[test]
fn products_combine_with_list_moves_and_respect_budgets_in_both_mixed_backends() {
    let p = MixedProblem::new(
        problem(),
        ListProblem::new(vec![7, 8], vec![ListEntity::new(vec![7, 8])]).unwrap(),
    );
    let mut n = MixedConstraintNetwork::new(2, 1, 2);
    n.add_constraint(
        "trap",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListEntity(0),
        ],
        |f| {
            let (MixedFact::Basic(a), MixedFact::Basic(b), MixedFact::ListEntity(row)) =
                (&f[0], &f[1], &f[2])
            else {
                unreachable!()
            };
            mixed_reward(a.copied(), b.copied(), row)
        },
    )
    .unwrap();
    for joint_moves in [false, true] {
        for budget in [1, 2, 100] {
            let c = MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    neighborhood: Neighborhood::ChangeProduct(
                        SubPillarSelection::combinations(2, 2).unwrap(),
                    ),
                    max_evaluations: budget,
                    assert_scores: true,
                    ..Default::default()
                },
                joint_moves,
            };
            let a = mixed_local_search_from(&p, &n, p.initial_assignment(), c).unwrap();
            let mut builder = StreamGraphBuilder::new();
            let source = MixedStreamSources::new(&mut builder);
            let left = builder
                .filter(source.basic.port(), |(i, _)| *i == 0)
                .unwrap();
            let right = builder
                .filter(source.basic.port(), |(i, _)| *i == 1)
                .unwrap();
            let pair = builder.join_equal(&left, &right, |_| 0, |_| 0).unwrap();
            let joined = builder
                .join_equal(&pair, source.lists.entities.port(), |_| 0, |_| 0)
                .unwrap();
            builder
                .score("trap", &joined, |(((_, a), (_, b)), (_, row))| {
                    mixed_reward(*a, *b, row)
                })
                .unwrap();
            let b = mixed_stream_local_search_from(
                &p,
                builder.build(),
                source,
                p.initial_assignment(),
                c,
            )
            .unwrap();
            assert_eq!(a.best, b.best);
            assert_eq!(a.evaluations, b.evaluations);
            assert_eq!(a.termination, b.termination);
            assert!(a.evaluations <= budget);
            if budget == 100 {
                assert_eq!(
                    a.best.unwrap().score,
                    SimpleScore::new(if joint_moves { 10 } else { 0 })
                );
            }
        }
    }
}
