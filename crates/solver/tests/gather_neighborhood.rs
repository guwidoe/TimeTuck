// Original neighborhood enumeration, laziness and solver integration checks.
use std::{cell::Cell, collections::BTreeSet, rc::Rc};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn samples(values: &[i32], max: usize) -> Vec<Vec<i32>> {
    fn extend(values: &[i32], n: usize, prefix: &mut Vec<i32>, out: &mut Vec<Vec<i32>>) {
        if prefix.len() == n {
            out.push(prefix.clone());
            return;
        }
        for v in values {
            if !prefix.contains(v) {
                prefix.push(*v);
                extend(values, n, prefix, out);
                prefix.pop();
            }
        }
    }
    let mut out = vec![];
    for n in 1..=max.min(values.len()) {
        extend(values, n, &mut vec![], &mut out);
    }
    out
}
fn outcome(
    a: &ListAssignment<i32>,
    group: &[i32],
    destination: Option<ListPosition>,
) -> Vec<Vec<i32>> {
    a.lists()
        .iter()
        .enumerate()
        .map(|(e, row)| {
            let mut output = vec![];
            for i in 0..=row.len() {
                if destination == Some(pos(e, i)) {
                    output.extend_from_slice(group);
                }
                if let Some(v) = row.get(i).filter(|v| !group.contains(v)) {
                    output.push(*v);
                }
            }
            output
        })
        .collect()
}
#[test]
fn small_neighborhoods_match_independent_successor_sets_with_pins_and_ranges() {
    let mut models = 0;
    for mut layout in 0..27 {
        let mut lists = [vec![], vec![]];
        for v in 0..3 {
            let e = layout % 3;
            layout /= 3;
            if e < 2 {
                lists[e].push(v);
            }
        }
        lists[0].reverse();
        for optional in [false, true] {
            for pinned in [false, true] {
                let p = ListProblem::new(
                    vec![2, 0, 1],
                    vec![
                        ListEntity::new(lists[0].clone()).with_pinned_prefix(if pinned {
                            lists[0].len().min(1)
                        } else {
                            0
                        }),
                        ListEntity::new(lists[1].clone()).with_value_range(
                            lists[1]
                                .iter()
                                .copied()
                                .chain([2])
                                .collect::<BTreeSet<_>>()
                                .into_iter()
                                .collect(),
                        ),
                    ],
                )
                .unwrap();
                let p = if optional { p.allowing_unassigned() } else { p };
                let a = p.initial_assignment();
                let eligible: Vec<_> = p
                    .values()
                    .iter()
                    .copied()
                    .filter(|v| {
                        a.location(v)
                            .is_none_or(|q| q.index >= p.entities()[q.entity].pinned_prefix())
                    })
                    .collect();
                for max in [0, 1, 2, usize::MAX] {
                    let mut expected = BTreeSet::new();
                    for group in samples(&eligible, max) {
                        let destinations = std::iter::once(None).filter(|_| optional).chain(
                            a.lists()
                                .iter()
                                .enumerate()
                                .flat_map(|(e, l)| (0..=l.len()).map(move |i| Some(pos(e, i)))),
                        );
                        for destination in destinations {
                            let next = outcome(&a, &group, destination);
                            if next == a.lists() {
                                continue;
                            }
                            let state = ListAssignment::new(next.clone());
                            if p.validate(&state).is_ok() {
                                expected.insert(next);
                            }
                        }
                    }
                    let actual: BTreeSet<_> = list_gather_moves(&p, &a, max)
                        .unwrap()
                        .map(|m| {
                            let mut copy = a.clone();
                            p.apply(&mut copy, &m).unwrap().commit();
                            assert_ne!(copy, a);
                            if p.is_complete(&a) {
                                assert!(p.is_complete(&copy));
                            }
                            copy.lists().to_vec()
                        })
                        .collect();
                    assert_eq!(
                        actual, expected,
                        "optional={optional},pinned={pinned},max={max},state={a:?}"
                    );
                    assert_eq!(
                        list_gather_moves(&p, &a, max).unwrap().collect::<Vec<_>>(),
                        list_moves(&p, &a, Neighborhood::Gather { max_size: max })
                            .unwrap()
                            .collect::<Vec<_>>()
                    );
                }
                models += 1;
            }
        }
    }
    assert_eq!(models, 108);
}
#[test]
fn fixed_order_zero_bounds_and_frozen_lists_are_explicit() {
    let p = ListProblem::new(vec![2, 0, 1], vec![ListEntity::new(vec![0, 1, 2])]).unwrap();
    let a = p.initial_assignment();
    let successors: Vec<_> = list_gather_moves(&p, &a, 1)
        .unwrap()
        .map(|m| {
            let mut c = a.clone();
            p.apply(&mut c, &m).unwrap().commit();
            c.lists().to_vec()
        })
        .collect();
    assert_eq!(
        successors,
        vec![
            vec![vec![2, 0, 1]],
            vec![vec![1, 0, 2]],
            vec![vec![0, 2, 1]],
            vec![vec![1, 0, 2]],
            vec![vec![1, 2, 0]],
            vec![vec![0, 2, 1]]
        ]
    );
    assert_eq!(list_gather_moves(&p, &a, 0).unwrap().count(), 0);
    assert!(list_gather_moves(&p, &ListAssignment::new(vec![]), 0).is_err());
    let frozen = ListProblem::new(
        vec![0, 1],
        vec![
            ListEntity::new(vec![0]).with_pinned(true),
            ListEntity::new(vec![1]).with_pinned(true),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    assert_eq!(
        list_gather_moves(&frozen, &frozen.initial_assignment(), usize::MAX)
            .unwrap()
            .count(),
        0
    );
    let empty = ListProblem::<i32>::new(vec![], vec![]).unwrap();
    let a = empty.initial_assignment();
    let mut moves = list_gather_moves(&empty, &a, usize::MAX).unwrap();
    assert!(moves.next().is_none());
    assert!(moves.next().is_none());
}
#[derive(Debug)]
struct Value(usize, Rc<Cell<usize>>);
impl Clone for Value {
    fn clone(&self) -> Self {
        self.1.set(self.1.get() + 1);
        Self(self.0, self.1.clone())
    }
}
impl PartialEq for Value {
    fn eq(&self, b: &Self) -> bool {
        self.0 == b.0
    }
}
impl Eq for Value {}
#[test]
fn huge_group_bounds_do_not_materialize_permutations_or_clone_before_polling() {
    let clones = Rc::new(Cell::new(0));
    let p = ListProblem::new(
        (0..80).map(|i| Value(i, clones.clone())).collect(),
        vec![ListEntity::new(
            (0..80).map(|i| Value(i, clones.clone())).collect(),
        )],
    )
    .unwrap();
    let a = p.initial_assignment();
    clones.set(0);
    let mut moves = list_gather_moves(&p, &a, usize::MAX).unwrap();
    assert_eq!(clones.get(), 0);
    assert!(moves.next().is_some());
    assert!(clones.get() < 100);
    assert!(moves.next().is_some());
    assert!(clones.get() < 200);
}
fn reward(route: &[i32]) -> SimpleScore {
    SimpleScore::new(i64::from(route == [2, 0, 3, 1]))
}
#[test]
fn gather_search_escapes_plateaus_in_list_and_joint_mixed_backends() {
    let p = ListProblem::new((0..4).collect(), vec![ListEntity::new((0..4).collect())]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 4);
    n.add_constraint("target", vec![ListScope::Entity(0)], |f| {
        let ListFact::Entity(row) = f[0] else {
            unreachable!()
        };
        reward(row)
    })
    .unwrap();
    for neighborhood in [
        Neighborhood::ChangeAndSwap,
        Neighborhood::Gather { max_size: 0 },
        Neighborhood::Gather { max_size: 3 },
    ] {
        for order in [
            MoveOrder::Original,
            MoveOrder::ShuffledVariables { seed: 29 },
            MoveOrder::ShuffledMoves {
                seed: 29,
                window: 7.try_into().unwrap(),
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
                Acceptance::FadingValueTabu {
                    hard_tenure: 1,
                    fading_tenure: 2,
                    aspiration: true,
                    seed: 7,
                },
            ] {
                for budget in [0, 1, 2, 17, 1000] {
                    let config = LocalSearchConfig {
                        neighborhood,
                        move_order: order,
                        max_evaluations: budget,
                        max_steps: 4,
                        acceptance,
                        selection: MoveSelection::BestAccepted,
                        assert_scores: true,
                        ..Default::default()
                    };
                    let a = list_local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
                    let mut b = StreamGraphBuilder::new();
                    let s = ListStreamSources::new(&mut b);
                    b.score("target", s.entities.port(), |(_, row)| reward(row))
                        .unwrap();
                    let c = list_stream_local_search_from(
                        &p,
                        b.build(),
                        s,
                        p.initial_assignment(),
                        config,
                    )
                    .unwrap();
                    assert_eq!(a.best, c.best);
                    assert_eq!(a.evaluations, c.evaluations);
                    assert_eq!(a.completed_steps, c.completed_steps);
                    assert_eq!(a.termination, c.termination);
                    assert!(a.evaluations <= budget);
                    if budget == 1000
                        && (acceptance == Acceptance::StrictImprovement
                            || neighborhood == Neighborhood::Gather { max_size: 3 })
                    {
                        assert_eq!(
                            a.best.unwrap().score,
                            SimpleScore::new(i64::from(matches!(
                                neighborhood,
                                Neighborhood::Gather { max_size: 3 }
                            )))
                        );
                    }
                    let mixed = MixedProblem::new(
                        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)])
                            .unwrap(),
                        p.clone(),
                    );
                    let mut net = MixedConstraintNetwork::new(1, 1, 4);
                    net.add_constraint(
                        "joint",
                        vec![MixedScope::Basic(0), MixedScope::ListEntity(0)],
                        |f| {
                            let (MixedFact::Basic(v), MixedFact::ListEntity(row)) = (&f[0], &f[1])
                            else {
                                unreachable!()
                            };
                            if v.copied() == Some(1) {
                                reward(row)
                            } else {
                                SimpleScore::new(0)
                            }
                        },
                    )
                    .unwrap();
                    let cfg = MixedLocalSearchConfig {
                        search: config,
                        joint_moves: true,
                    };
                    let a = mixed_local_search_from(&mixed, &net, mixed.initial_assignment(), cfg)
                        .unwrap();
                    let mut b = StreamGraphBuilder::new();
                    let s = MixedStreamSources::new(&mut b);
                    let joined = b
                        .join_equal(
                            s.basic.port(),
                            s.lists.entities.port(),
                            |(i, _)| *i,
                            |(i, _)| *i,
                        )
                        .unwrap();
                    b.score("joint", &joined, |((_, v), (_, row))| {
                        if *v == Some(1) {
                            reward(row)
                        } else {
                            SimpleScore::new(0)
                        }
                    })
                    .unwrap();
                    let c = mixed_stream_local_search_from(
                        &mixed,
                        b.build(),
                        s,
                        mixed.initial_assignment(),
                        cfg,
                    )
                    .unwrap();
                    assert_eq!(a.best, c.best);
                    assert_eq!(a.evaluations, c.evaluations);
                    assert_eq!(a.completed_steps, c.completed_steps);
                    assert_eq!(a.termination, c.termination);
                    assert!(a.evaluations <= budget);
                    if budget == 1000
                        && (acceptance == Acceptance::StrictImprovement
                            || neighborhood == Neighborhood::Gather { max_size: 3 })
                    {
                        assert_eq!(
                            a.best.unwrap().score,
                            SimpleScore::new(i64::from(matches!(
                                neighborhood,
                                Neighborhood::Gather { max_size: 3 }
                            )))
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn basic_backends_retain_changes_and_swaps_when_gather_is_selected() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let mut n = ConstraintNetwork::new(2);
    for i in 0..2 {
        n.add_constraint(format!("value{i}"), vec![i], |v| {
            SimpleScore::new(i64::from(*v[0].unwrap()))
        })
        .unwrap();
    }
    for max_size in [0, 3] {
        let config = LocalSearchConfig {
            neighborhood: Neighborhood::Gather { max_size },
            max_steps: 3,
            max_evaluations: 50,
            assert_scores: true,
            ..Default::default()
        };
        let a = local_search_from(&p, &n, p.initial_assignment(), config).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i32>)>();
        b.score("values", s.port(), |(_, v)| {
            SimpleScore::new(i64::from(v.unwrap()))
        })
        .unwrap();
        let c = stream_local_search_from(&p, b.build(), s, p.initial_assignment(), config).unwrap();
        let control = local_search_from(
            &p,
            &n,
            p.initial_assignment(),
            LocalSearchConfig {
                neighborhood: Neighborhood::ChangeAndSwap,
                ..config
            },
        )
        .unwrap();
        assert_eq!(a.best, control.best);
        assert_eq!(a.evaluations, control.evaluations);
        assert_eq!(a.best, c.best);
        assert_eq!(a.evaluations, c.evaluations);
        assert_eq!(a.best.unwrap().score, SimpleScore::new(2));
    }
}
