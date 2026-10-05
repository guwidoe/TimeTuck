//! Original rank-space, candidate-validity and actual solver integration checks.
use std::{cell::Cell, collections::HashSet};
use timetuck::*;

#[test]
fn every_rank_matches_independent_enumeration_for_bounds_and_pins() {
    for n in 0..7 {
        for prefix in 0..=n {
            for whole in [false, true] {
                let entity = if whole {
                    ListEntity::new((0..n).collect()).with_pinned(true)
                } else {
                    ListEntity::new((0..n).collect()).with_pinned_prefix(prefix as usize)
                };
                let p = ListProblem::new(
                    (0..n + 3).collect(),
                    vec![
                        entity,
                        ListEntity::new(vec![n, n + 1]),
                        ListEntity::new(vec![]),
                    ],
                )
                .unwrap()
                .allowing_unassigned();
                for min in 1..9 {
                    for max in [min, min + 1, usize::MAX] {
                        let a = p.initial_assignment();
                        let s = SubListSelection::new(&p, &a, SubListSize::new(min, max).unwrap())
                            .unwrap();
                        let mut expected = vec![];
                        for (entity, list) in a.lists().iter().enumerate() {
                            if p.entities()[entity].is_pinned() {
                                continue;
                            }
                            for len in (1..=list.len())
                                .rev()
                                .filter(|len| (min..=max).contains(len))
                            {
                                for start in p.entities()[entity].pinned_prefix()..=list.len() - len
                                {
                                    expected.push(ListRange {
                                        entity,
                                        start,
                                        end: start + len,
                                    });
                                }
                            }
                        }
                        assert_eq!(s.len(), expected.len() as u128);
                        for (rank, span) in expected.iter().enumerate() {
                            assert_eq!(s.at(rank as u128), Some(*span));
                        }
                        assert_eq!(s.at(s.len()), None);
                        assert_eq!(s.at(u128::MAX), None);
                    }
                }
            }
        }
    }
}
#[test]
fn seeded_sampling_repeats_preserves_snapshot_and_does_no_eager_draws() {
    let p = ListProblem::new(
        (0..8).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2, 3]),
            ListEntity::new(vec![4, 5, 6, 7]),
        ],
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let s = SubListSelection::new(&p, &a, SubListSize::new(1, 4).unwrap()).unwrap();
    let draws = Cell::new(0);
    let mut scripted = s.random_iter_with(|n| {
        assert_eq!(n, 20);
        let i = draws.get();
        draws.set(i + 1);
        i % n
    });
    assert_eq!(draws.get(), 0);
    assert_eq!(scripted.next(), s.at(0));
    assert_eq!(draws.get(), 1);
    let run = |seed| s.random_iter(seed).take(4096).collect::<Vec<_>>();
    assert_eq!(run(81), run(81));
    assert_ne!(run(81), run(82));
    assert_eq!(run(81).into_iter().collect::<HashSet<_>>().len(), 20);
    p.apply(
        &mut a,
        &ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 4,
            destination: ListPosition {
                entity: 1,
                index: 4,
            },
            reverse: false,
        },
    )
    .unwrap()
    .commit();
    assert_eq!(s.len(), 20);
    let updated = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    assert_eq!(updated.len(), 36);
    assert_eq!(
        updated.at(0),
        Some(ListRange {
            entity: 1,
            start: 0,
            end: 8
        })
    );
}
#[test]
fn large_span_space_needs_no_value_clones_or_quadratic_catalog() {
    #[derive(Debug, Eq, PartialEq)]
    struct Value(usize, std::rc::Rc<Cell<usize>>);
    impl Clone for Value {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }
    let clones = std::rc::Rc::new(Cell::new(0));
    let value = |i| Value(i, clones.clone());
    let p = ListProblem::new(
        (0..1000).map(value).collect(),
        vec![ListEntity::new((0..1000).map(value).collect())],
    )
    .unwrap();
    let a = ListAssignment::new(vec![(0..1000).map(value).collect()]);
    clones.set(0);
    let s = SubListSelection::new(&p, &a, SubListSize::new(1, usize::MAX).unwrap()).unwrap();
    assert_eq!(s.len(), 500500);
    assert_eq!(
        s.at(500499),
        Some(ListRange {
            entity: 0,
            start: 999,
            end: 1000
        })
    );
    assert_eq!(s.random_iter(7).take(100).count(), 100);
    assert_eq!(clones.get(), 0);
}
#[test]
fn sampled_proposals_cover_legal_small_neighborhoods_and_rollback() {
    for optional in [false, true] {
        let mut p = ListProblem::new(
            (0..7).collect(),
            vec![
                ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
                ListEntity::new(vec![3, 4, 5]).with_value_range(vec![1, 2, 3, 4, 5]),
                ListEntity::new(vec![]),
                ListEntity::new(vec![6]).with_pinned(true),
            ],
        )
        .unwrap();
        if optional {
            p = p.allowing_unassigned();
        }
        for reverse in [false, true] {
            let c = SubListChangeConfig::new(2, 3)
                .unwrap()
                .with_reversing(reverse);
            let a = p.initial_assignment();
            let expected: HashSet<_> = list_sublist_change_moves_with_config(&p, &a, c)
                .unwrap()
                .collect();
            let sampled: HashSet<_> = sampled_sublist_change_moves(&p, &a, c, 123, 20000)
                .unwrap()
                .collect();
            assert_eq!(sampled, expected);
            for movement in &sampled {
                let mut trial = a.clone();
                {
                    let guard = p.apply(&mut trial, movement).unwrap();
                    assert_ne!(&*guard, &a);
                }
                assert_eq!(trial, a);
            }
        }
    }
}
#[test]
fn invalid_inputs_and_empty_or_impossible_sampling_terminate() {
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let a = p.initial_assignment();
    let c = SubListChangeConfig::new(2, 2)
        .unwrap()
        .with_reversing(false);
    assert_eq!(
        sampled_sublist_change_moves(&p, &a, c, 1, 10000)
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        sampled_sublist_change_moves(&p, &a, c.with_reversing(true), 1, 0)
            .unwrap()
            .count(),
        0
    );
    assert!(sampled_sublist_change_moves(&p, &a, c.with_unassignment(true), 1, 1).is_err());
    assert!(
        SubListSelection::new(
            &p,
            &ListAssignment::new(vec![vec![1, 1]]),
            SubListSize::new(1, 2).unwrap()
        )
        .is_err()
    );
    assert!(SubListSize::new(0, 5).is_err());
    assert!(SubListSize::new(2, 1).is_err());
    let p = ListProblem::<i32>::new(vec![], vec![]).unwrap();
    let a = p.initial_assignment();
    let mut moves = sampled_sublist_change_moves(&p, &a, c, 1, u64::MAX).unwrap();
    assert_eq!(moves.next(), None);
    assert_eq!(moves.next(), None);
}

struct SampledProvider {
    calls: usize,
}
impl MoveProvider<ListProblem<i32>, ListAssignment<i32>, ListMove<i32>> for SampledProvider {
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<i32>,
        a: &'a ListAssignment<i32>,
        step: u64,
    ) -> Box<dyn Iterator<Item = ListMove<i32>> + 'a>
    where
        ListMove<i32>: 'a,
    {
        self.calls += 1;
        Box::new(
            sampled_sublist_change_moves(
                p,
                a,
                SubListChangeConfig::new(2, 4).unwrap(),
                51u64.wrapping_add(step),
                512,
            )
            .unwrap(),
        )
    }
}
fn quality(v: &[i32]) -> i64 {
    v.iter()
        .enumerate()
        .map(|(i, v)| (i as i64 + 1) * i64::from(*v))
        .sum()
}
#[test]
fn lazy_provider_improves_list_order_through_both_scoring_backends() {
    let p = ListProblem::new(vec![1, 2, 3, 4], vec![ListEntity::new(vec![4, 3, 2, 1])]).unwrap();
    let mut n = ListConstraintNetwork::new(1, 4);
    n.add_constraint("order", vec![ListScope::Entity(0)], |facts| {
        let ListFact::Entity(v) = facts[0] else {
            unreachable!()
        };
        SimpleScore::new(quality(v))
    })
    .unwrap();
    for stream in [false, true] {
        let mut provider = SampledProvider { calls: 0 };
        let config = LocalSearchConfig {
            max_steps: 8,
            max_move_evaluations: Some(2000),
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            b.score("order", sources.entities.port(), |(_, v)| {
                SimpleScore::new(quality(v))
            })
            .unwrap();
            list_stream_local_search_from_with_provider(
                &p,
                b.build(),
                sources,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            list_local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.assignment.lists(), &[vec![1, 2, 3, 4]]);
        assert_eq!(best.score, SimpleScore::new(30));
        assert!(provider.calls >= 2);
    }
}

#[test]
fn sampled_swaps_reach_symmetric_and_asymmetric_neighbors_and_reject_overlap() {
    let p = ListProblem::new(
        (0..7).collect(),
        vec![
            ListEntity::new(vec![0, 1, 2]).with_pinned_prefix(1),
            ListEntity::new(vec![3, 4, 5]).with_value_range(vec![1, 2, 3, 4, 5]),
            ListEntity::new(vec![6]).with_pinned(true),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    for (lo, hi, ro, rhi) in [(1, 3, 1, 3), (1, 1, 2, 3), (2, 3, 1, 1), (1, 1, 1, 1)] {
        for reversing in [false, true] {
            let c = SubListSwapConfig::asymmetric(lo, hi, ro, rhi)
                .unwrap()
                .with_reversing(reversing);
            let expected: HashSet<_> = list_sublist_swap_moves_with_config(&p, &a, c)
                .unwrap()
                .collect();
            let sampled: HashSet<_> = sampled_sublist_swap_moves(&p, &a, c, 567, 20000)
                .unwrap()
                .collect();
            assert_eq!(sampled, expected);
            for movement in sampled {
                let mut trial = a.clone();
                {
                    let guard = p.apply(&mut trial, &movement).unwrap();
                    assert_ne!(&*guard, &a);
                }
                assert_eq!(trial, a);
            }
        }
    }
    let c = SubListSwapConfig::new(3, 3).unwrap();
    assert_eq!(
        sampled_sublist_swap_moves(&p, &a, c, 1, 1000)
            .unwrap()
            .count(),
        0
    );
    let c = SubListSwapConfig::new(9, 9).unwrap();
    assert_eq!(
        sampled_sublist_swap_moves(&p, &a, c, 1, u64::MAX)
            .unwrap()
            .next(),
        None
    );
    assert!(sampled_sublist_swap_moves(&p, &ListAssignment::new(vec![]), c, 1, 0).is_err());
}
