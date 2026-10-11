//! Original weighted selection and all-backend integration regressions.
use std::{cell::Cell, collections::BTreeMap};
use timetuck::{variable_neighborhood_descent, *};
#[test]
fn seeded_weights_are_repeatable_and_proportional_at_different_scales() {
    let run = |scale| {
        MoveUnion::new(
            [(0, 0), (1, scale), (2, 9 * scale)].map(|(i, w)| (std::iter::repeat(i), w)),
            UnionOrder::Random { seed: 83 },
        )
        .take(20000)
        .collect::<Vec<_>>()
    };
    let values = run(1);
    assert_eq!(values, run(1));
    // Integer draws need not have the same trace after rescaling weights.
    for values in [values, run(100), run(u32::MAX / 9)] {
        let counts = values.into_iter().fold(BTreeMap::new(), |mut m, v| {
            *m.entry(v).or_insert(0) += 1;
            m
        });
        assert_eq!(counts.len(), 2);
        assert!((1700..2300).contains(&counts[&1]), "{counts:?}");
    }
}
#[test]
fn union_is_lazy_disables_children_and_fuses_exhaustion() {
    let pulls = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let make = |i: usize, w| {
        (
            std::iter::repeat(i).inspect(|&i| pulls[i].set(pulls[i].get() + 1)),
            w,
        )
    };
    let mut union = MoveUnion::new(
        [make(0, 0), make(1, 1), make(2, 1)],
        UnionOrder::Random { seed: 7 },
    );
    assert_eq!(pulls.each_ref().map(Cell::get), [0, 0, 0]);
    assert!(union.next().is_some());
    assert_eq!(pulls[0].get(), 0);
    assert_eq!(pulls[1].get() + pulls[2].get(), 1);
    drop(union);
    let mut state = 0;
    let nonfused = std::iter::from_fn(move || {
        state += 1;
        (state % 2 == 1).then_some(state)
    });
    let mut union = MoveUnion::new([(nonfused, 1)], UnionOrder::Sequential);
    assert_eq!(union.next(), Some(1));
    assert_eq!(union.next(), None);
    assert_eq!(union.next(), None);
    assert_eq!(
        MoveUnion::new([(0..3, 2), (9..11, 0), (3..5, 1)], UnionOrder::Sequential)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4]
    );
}
fn basic() -> FiniteProblem<usize> {
    FiniteProblem::new(vec![
        Variable::new(vec![0, 1]).with_initial(0),
        Variable::new(vec![0, 1]).with_initial(1),
    ])
    .unwrap()
}
fn lists() -> ListProblem<usize> {
    ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![0]), ListEntity::new(vec![1])],
    )
    .unwrap()
}
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn list_value(f: &ListFact<'_, usize>) -> i64 {
    match f {
        ListFact::Value { value, element } => {
            i64::from(element.as_ref().unwrap().position.entity == 1 - **value)
        }
        _ => unreachable!(),
    }
}
#[test]
fn unions_drive_swaps_across_all_models_and_scoring_backends() {
    union_swaps(1);
}
#[test]
fn indexed_unions_drive_swaps_across_all_models_and_scoring_backends() {
    union_swaps(40);
}
fn union_swaps(copies: usize) {
    let p = basic();
    let l = lists();
    let m = MixedProblem::new(p.clone(), l.clone());
    let mut bn = ConstraintNetwork::new(2);
    bn.add_constraint("target", vec![0, 1], |v| {
        s(i64::from(v[0] == Some(&1)) + i64::from(v[1] == Some(&0)))
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::new(2, 2);
    ln.add_constraint(
        "target",
        vec![ListScope::Value(0), ListScope::Value(1)],
        |f| s(f.iter().map(list_value).sum()),
    )
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(2, 2, 2);
    mn.add_constraint(
        "target",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListValue(0),
            MixedScope::ListValue(1),
        ],
        |f| {
            s(f.iter()
                .enumerate()
                .map(|(i, f)| match f {
                    MixedFact::Basic(v) => i64::from(*v == Some(&(1 - i))),
                    MixedFact::ListValue { value, element } => {
                        i64::from(element.as_ref().unwrap().position.entity == 1 - **value)
                    }
                    _ => unreachable!(),
                })
                .sum())
        },
    )
    .unwrap();
    for order in [UnionOrder::Sequential, UnionOrder::Random { seed: 17 }] {
        for enabled in [false, true] {
            let children: Vec<_> = (0..copies)
                .flat_map(|_| {
                    [
                        (Neighborhood::Change, 0),
                        (Neighborhood::Swap, u32::from(enabled)),
                    ]
                })
                .collect();
            let config = LocalSearchConfig {
                neighborhood_union: Some(&NeighborhoodUnion {
                    children: &children,
                    order,
                }),
                selection: MoveSelection::BestAccepted,
                assert_scores: true,
                ..Default::default()
            };
            let b = local_search_from(&p, &bn, p.initial_assignment(), config.clone()).unwrap();
            let a =
                list_local_search_from(&l, &ln, l.initial_assignment(), config.clone()).unwrap();
            let mixed = MixedLocalSearchConfig {
                search: config.clone(),
                joint_moves: true,
            };
            let c =
                mixed_local_search_from(&m, &mn, m.initial_assignment(), mixed.clone()).unwrap();
            let mut builder = StreamGraphBuilder::new();
            let source = builder.source::<usize, (usize, Option<usize>)>();
            builder
                .score("target", source.port(), |(i, v)| {
                    s(i64::from(*v == Some(1 - i)))
                })
                .unwrap();
            let bg = stream_local_search_from(
                &p,
                builder.build(),
                source,
                p.initial_assignment(),
                config.clone(),
            )
            .unwrap();
            let mut builder = StreamGraphBuilder::new();
            let source = ListStreamSources::<usize>::new(&mut builder);
            builder
                .score("target", source.values.port(), |v| {
                    s(i64::from(v.position.unwrap().entity == 1 - v.value))
                })
                .unwrap();
            let lg = list_stream_local_search_from(
                &l,
                builder.build(),
                source,
                l.initial_assignment(),
                config,
            )
            .unwrap();
            let mut builder = StreamGraphBuilder::new();
            let source = MixedStreamSources::<usize, usize>::new(&mut builder);
            builder
                .score("basic", source.basic.port(), |(i, v)| {
                    s(i64::from(*v == Some(1 - i)))
                })
                .unwrap();
            builder
                .score("list", source.lists.values.port(), |v| {
                    s(i64::from(v.position.unwrap().entity == 1 - v.value))
                })
                .unwrap();
            let mg = mixed_stream_local_search_from(
                &m,
                builder.build(),
                source,
                m.initial_assignment(),
                mixed,
            )
            .unwrap();
            for (score, steps, want) in [
                (b.best.unwrap().score, b.completed_steps, 2),
                (bg.best.unwrap().score, bg.completed_steps, 2),
                (a.best.unwrap().score, a.completed_steps, 2),
                (lg.best.unwrap().score, lg.completed_steps, 2),
                (c.best.unwrap().score, c.completed_steps, 4),
                (mg.best.unwrap().score, mg.completed_steps, 4),
            ] {
                assert_eq!(score, s(if enabled { want } else { 0 }));
                assert_eq!(steps, u64::from(enabled));
            }
        }
    }
}
#[test]
fn enabled_union_children_are_validated_even_when_primary_is_valid() {
    let p = basic();
    let n = ConstraintNetwork::<usize, SimpleScore>::new(2);
    for weight in [0, 1] {
        let children = [(
            Neighborhood::MassUnassign(MassAssignmentSize::all()),
            weight,
        )];
        let config = LocalSearchConfig {
            neighborhood_union: Some(&NeighborhoodUnion {
                children: &children,
                order: UnionOrder::Sequential,
            }),
            ..Default::default()
        };
        assert_eq!(
            local_search_from(&p, &n, p.initial_assignment(), config.clone()).is_err(),
            weight != 0
        );
        let phases = SolverSequence {
            phases: vec![SolverPhase::LocalSearch(config)],
            ..Default::default()
        };
        assert_eq!(solve(&p, &n, phases).is_err(), weight != 0);
    }
}

#[test]
fn combining_single_and_group_changes_escapes_a_local_trap() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("landscape", vec![0, 1], |v| {
        s(match (v[0], v[1]) {
            (Some(0), Some(0)) => 0,
            (Some(1), Some(1)) => 5,
            (Some(2), Some(1)) => 10,
            _ => -10,
        })
    })
    .unwrap();
    let children = [
        (Neighborhood::Change, 1),
        (Neighborhood::MassChange(MassChangeConfig::all()), 1),
    ];
    for order in [UnionOrder::Sequential, UnionOrder::Random { seed: 91 }] {
        let neighborhoods = NeighborhoodUnion {
            children: &children,
            order,
        };
        let base = if order == UnionOrder::Sequential {
            variable_neighborhood_descent(&neighborhoods).unwrap()
        } else {
            LocalSearchConfig {
                neighborhood_union: Some(&neighborhoods),
                ..Default::default()
            }
        };
        let config = LocalSearchConfig {
            assert_scores: true,
            ..base
        };
        let union = local_search_from(&p, &n, p.initial_assignment(), config.clone()).unwrap();
        assert_eq!(union.best.unwrap().score, s(10));
        assert_eq!(union.completed_steps, 2);
        let single = local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                neighborhood_union: None,
                ..config
            },
        )
        .unwrap();
        assert_eq!(single.best.unwrap().score, s(0));
    }
}
