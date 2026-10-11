//! Original directed-neighborhood and solver integration tests.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
fn reward(row: &[i32]) -> SimpleScore {
    SimpleScore::new(i64::from(row == [1, 5, 3, 4, 2]))
}
fn problem() -> ListProblem<i32> {
    ListProblem::new((1..=5).collect(), vec![ListEntity::new((1..=5).collect())]).unwrap()
}
#[test]
fn list_search_finds_a_wrapped_improvement_on_both_backends() {
    let p = problem();
    let mut network = ListConstraintNetwork::new(1, 5);
    network
        .add_constraint("target", vec![ListScope::Entity(0)], |facts| {
            let ListFact::Entity(row) = facts[0] else {
                unreachable!()
            };
            reward(row)
        })
        .unwrap();
    let config = LocalSearchConfig {
        max_steps: 2,
        max_evaluations: 1000,
        assert_scores: true,
        neighborhood: Neighborhood::TwoOpt {
            cross_entity: false,
        },
        ..Default::default()
    };
    let ordinary =
        list_local_search_from(&p, &network, p.initial_assignment(), config.clone()).unwrap();
    assert_eq!(ordinary.best.unwrap().score, SimpleScore::ZERO);
    for stream in [false, true] {
        let mut provider = DirectedTwoOptProvider::new(false);
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            b.score("target", sources.entities.port(), |(_, row)| reward(row))
                .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config.clone(),
                &mut provider,
            )
        } else {
            list_local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config.clone(),
                &mut provider,
            )
        }
        .unwrap();
        assert_eq!(r.best.as_ref().unwrap().score, SimpleScore::new(1));
        assert_eq!(r.best.unwrap().assignment.lists(), &[vec![1, 5, 3, 4, 2]]);
    }
}
#[test]
fn mixed_search_uses_wrapped_moves_and_preserves_basic_assignments_on_both_backends() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![7, 8]).with_initial(7)]).unwrap(),
        problem(),
    );
    let mut network = MixedConstraintNetwork::new(1, 1, 5);
    network
        .add_constraint("target", vec![MixedScope::ListEntity(0)], |facts| {
            let MixedFact::ListEntity(row) = facts[0] else {
                unreachable!()
            };
            reward(row)
        })
        .unwrap();
    let config = MixedLocalSearchConfig {
        search: LocalSearchConfig {
            max_steps: 2,
            max_evaluations: 1000,
            assert_scores: true,
            ..Default::default()
        },
        joint_moves: true,
    };
    for stream in [false, true] {
        let mut provider = DirectedTwoOptProvider::new(false);
        let r = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = MixedStreamSources::new(&mut b);
            b.score("target", sources.lists.entities.port(), |(_, row)| {
                reward(row)
            })
            .unwrap();
            mixed_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config.clone(),
                &mut provider,
            )
        } else {
            mixed_local_search_from_with_provider(
                &p,
                &network,
                p.initial_assignment(),
                config.clone(),
                &mut provider,
            )
        }
        .unwrap();
        let best = r.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(1));
        assert_eq!(best.assignment.lists().lists(), &[vec![1, 5, 3, 4, 2]]);
        assert_eq!(best.assignment.basic().values(), &[Some(7)]);
    }
}
#[test]
fn provider_includes_empty_receivers_but_omits_invalid_ranges_full_pins_and_noops() {
    let p = ListProblem::new(
        vec![0, 1, 2, 3],
        vec![
            ListEntity::new(vec![0, 1, 2])
                .with_pinned_prefix(1)
                .with_value_range(vec![0, 1, 2]),
            ListEntity::new(vec![]).with_value_range(vec![2]),
            ListEntity::new(vec![3]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let successors = |cross| {
        list_directed_two_opt_moves(&p, &a, cross)
            .unwrap()
            .map(|m| {
                let mut next = a.clone();
                p.apply(&mut next, &m).unwrap().commit();
                assert_ne!(next, a);
                assert_eq!(next.lists()[0][0], 0);
                assert_eq!(next.lists()[2], vec![3]);
                next
            })
            .collect::<Vec<_>>()
    };
    let local = successors(false);
    assert!(local.iter().all(|a| a.lists()[1].is_empty()));
    let cross = successors(true);
    assert!(
        cross
            .iter()
            .any(|a| a.lists() == [vec![0, 1], vec![2], vec![3]])
    );
    for a in cross {
        assert!(a.lists()[1].iter().all(|&v| v == 2));
    }
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])
        .unwrap()
        .with_dependencies(vec![vec![], vec![0]])
        .unwrap();
    assert_eq!(
        list_directed_two_opt_moves(&p, &p.initial_assignment(), true)
            .unwrap()
            .count(),
        0
    );
    assert!(list_directed_two_opt_moves(&p, &ListAssignment::new(vec![vec![99]]), true).is_err());
}
#[derive(Debug)]
struct Counted {
    id: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Counted {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Counted {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Counted {}
#[test]
fn opening_is_lazy_and_search_limits_bound_scored_candidates() {
    let count = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..30)
        .map(|id| Counted {
            id,
            clones: count.clone(),
        })
        .collect();
    let p = ListProblem::new(values.clone(), vec![ListEntity::new(values)]).unwrap();
    let a = p.initial_assignment();
    count.set(0);
    let mut moves = list_directed_two_opt_moves(&p, &a, true).unwrap();
    assert_eq!(count.get(), 0);
    assert!(moves.next().is_some());
    let first = count.get();
    assert_eq!(first, 0);
    assert!(moves.count() > 0);

    assert_eq!(count.get(), first);
    let p = problem();
    let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 5);
    let mut provider = DirectedTwoOptProvider::new(true);
    let r = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_evaluations: 2,
            max_steps: 10,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert!(r.evaluations <= 2);
}

#[test]
fn borrowed_generation_matches_lifecycle_plans_in_order_and_multiplicity() {
    for n in 0..=6 {
        for split in 0..=n {
            for policy in 0..4 {
                for dependencies in [false, true] {
                    let mut entities = vec![
                        ListEntity::new((0..split).collect()),
                        ListEntity::new((split..n).collect()),
                    ];
                    if policy == 1 && split > 0 {
                        entities[0] = entities[0].clone().with_pinned_prefix(1);
                    }
                    if policy == 2 {
                        entities[0] = entities[0].clone().with_pinned(true);
                    }
                    if policy == 3 {
                        entities[0] = entities[0].clone().with_value_range((0..split).collect());
                        entities[1] = entities[1].clone().with_value_range((split..n).collect());
                    }
                    // Registry value n deliberately remains unassigned.
                    let mut p = ListProblem::new((0..=n).collect(), entities)
                        .unwrap()
                        .allowing_unassigned();
                    if dependencies {
                        p = p
                            .with_dependencies(
                                (0..=n)
                                    .map(|i| {
                                        if i > 0 && i % 3 != 0 {
                                            vec![i - 1]
                                        } else {
                                            vec![]
                                        }
                                    })
                                    .collect(),
                            )
                            .unwrap();
                    }
                    let a = p.initial_assignment();
                    for cross in [false, true] {
                        let mut expected = Vec::new();
                        let mut record = |first, second| {
                            if let Ok(plan) = list_directed_two_opt_plan(&p, &a, first, second)
                                && plan.assignment() != &a
                            {
                                expected.push(plan.movement().clone());
                            }
                        };
                        for e in 0..2 {
                            if p.entities()[e].is_pinned() {
                                continue;
                            }
                            let prefix = p.entities()[e].pinned_prefix();
                            let len = a.lists()[e].len();
                            for i in prefix..=len {
                                for j in prefix..=len {
                                    let span = if i <= j { j - i } else { len - i + j - prefix };
                                    if span >= 2 {
                                        record(
                                            ListPosition {
                                                entity: e,
                                                index: i,
                                            },
                                            ListPosition {
                                                entity: e,
                                                index: j,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                        if cross && p.entities().iter().all(|e| !e.is_pinned()) {
                            for i in p.entities()[0].pinned_prefix()..=split {
                                for j in p.entities()[1].pinned_prefix()..=n - split {
                                    record(
                                        ListPosition {
                                            entity: 0,
                                            index: i,
                                        },
                                        ListPosition {
                                            entity: 1,
                                            index: j,
                                        },
                                    );
                                }
                            }
                        }
                        let actual: Vec<_> = list_directed_two_opt_moves(&p, &a, cross)
                            .unwrap()
                            .collect();
                        assert_eq!(
                            actual, expected,
                            "n={n},split={split},policy={policy},dependencies={dependencies},cross={cross}"
                        );
                        assert_eq!(p.initial_assignment(), a);
                    }
                }
            }
        }
    }
}

#[test]
fn dependency_and_range_filtering_never_clone_payloads() {
    let clones = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..6)
        .map(|id| Counted {
            id,
            clones: clones.clone(),
        })
        .collect();
    for dependencies in [false, true] {
        let mut p = ListProblem::new(
            values.clone(),
            vec![
                ListEntity::new(values[..3].to_vec())
                    .with_pinned_prefix(1)
                    .with_value_range(values[..4].to_vec()),
                ListEntity::new(values[3..].to_vec()).with_value_range(values[2..].to_vec()),
            ],
        )
        .unwrap();
        if dependencies {
            p = p
                .with_dependencies(vec![vec![], vec![0], vec![], vec![], vec![3], vec![]])
                .unwrap();
        }
        let a = p.initial_assignment();
        clones.set(0);
        assert!(list_directed_two_opt_moves(&p, &a, true).unwrap().count() > 0);
        assert_eq!(clones.get(), 0);
    }
}
