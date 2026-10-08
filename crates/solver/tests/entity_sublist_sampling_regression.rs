//! Original entity-conditioned sublist sampling and solver regressions.
use std::{
    cell::Cell,
    iter,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
fn problem() -> ListProblem<i32> {
    ListProblem::new(
        (1..=9).collect(),
        vec![
            ListEntity::new(vec![1, 2, 3, 4]).with_pinned_prefix(1),
            ListEntity::new(vec![5, 6, 7]).with_pinned(true),
            ListEntity::new(vec![]),
            ListEntity::new(vec![8, 9]),
        ],
    )
    .unwrap()
}
#[test]
fn entity_ranks_match_an_independent_catalog_with_pins_and_bounds() {
    let p = problem();
    let a = p.initial_assignment();
    for minimum in 1..=5 {
        for maximum in minimum..=6 {
            let index =
                SubListSelection::new(&p, &a, SubListSize::new(minimum, maximum).unwrap()).unwrap();
            let mut total = 0;
            for entity in 0..=4 {
                let mut expected = vec![];
                if let Some(metadata) = p.entities().get(entity)
                    && !metadata.is_pinned()
                {
                    let list = &a.lists()[entity];
                    for length in (minimum..=maximum).rev() {
                        for start in metadata.pinned_prefix()..list.len() {
                            if start + length <= list.len() {
                                expected.push(ListRange {
                                    entity,
                                    start,
                                    end: start + length,
                                });
                            }
                        }
                    }
                }
                assert_eq!(index.entity_len(entity), expected.len() as u128);
                for (rank, span) in expected.iter().enumerate() {
                    assert_eq!(index.at_entity(entity, rank as u128), Some(*span));
                    assert_eq!(index.at(total + rank as u128), Some(*span));
                }
                assert_eq!(index.at_entity(entity, expected.len() as u128), None);
                assert_eq!(index.at_entity(entity, u128::MAX), None);
                total += expected.len() as u128;
            }
            assert_eq!(index.len(), total);
            assert_eq!(index.entity_len(usize::MAX), 0);
        }
    }
}
#[test]
fn filtering_is_lazy_bounded_and_fuses_nonfused_inputs() {
    let p = problem();
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(2, 3).unwrap()).unwrap();
    let reads = Cell::new(0);
    let draws = Cell::new(0);
    let entities = iter::repeat_with(|| {
        reads.set(reads.get() + 1);
        1
    });
    let mut spans = index.random_iter_for_entities_with(entities, 7, |_| {
        draws.set(draws.get() + 1);
        0
    });
    assert_eq!((reads.get(), draws.get()), (0, 0));
    assert_eq!(spans.next(), None);
    assert_eq!(spans.next(), None);
    assert_eq!((reads.get(), draws.get()), (7, 0));
    let entities = [usize::MAX, 1, 2, 3, 0, 0];
    let mut bounds = vec![];
    let spans = index
        .random_iter_for_entities_with(entities, 5, |n| {
            bounds.push(n);
            n - 1
        })
        .collect::<Vec<_>>();
    assert_eq!(bounds, [1, 3]);
    assert_eq!(
        spans,
        [
            ListRange {
                entity: 3,
                start: 0,
                end: 2
            },
            ListRange {
                entity: 0,
                start: 2,
                end: 4
            }
        ]
    );
    let mut calls = 0;
    let source = iter::from_fn(|| {
        calls += 1;
        if calls == 2 { None } else { Some(0) }
    });
    let mut spans = index.random_iter_for_entities_with(source, 10, |_| 0);
    assert!(spans.next().is_some());
    assert_eq!(spans.next(), None);
    assert_eq!(spans.next(), None);
    drop(spans);
    assert_eq!(calls, 2);
    let empty = SubListSelection::new(&p, &a, SubListSize::new(9, 10).unwrap()).unwrap();
    let mut spans = empty.random_iter_for_entities_with(
        iter::repeat_with(|| panic!("empty source read")),
        10,
        |_| panic!("empty draw"),
    );
    assert_eq!(spans.next(), None);
}
#[test]
fn changes_preserve_proposals_and_do_not_spend_reversal_draws_on_unassignment() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2, 3, 4]), ListEntity::new(vec![])],
    )
    .unwrap()
    .allowing_unassigned();
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(2, 2).unwrap()).unwrap();
    let positions = [
        Some(ListPosition {
            entity: 0,
            index: 3,
        }),
        None,
        Some(ListPosition {
            entity: 1,
            index: 0,
        }),
    ];
    let mut bounds = vec![];
    let proposals = index
        .change_samples_with::<i32>([1, 0, 1, 0, 0], positions, 5, true, |n| {
            bounds.push(n);
            if n == 2 { 1 } else { 0 }
        })
        .collect::<Vec<_>>();
    assert_eq!(bounds, [3, 2, 3, 3, 2]);
    assert_eq!(proposals.len(), 3);
    let mut trial = a.clone();
    assert!(p.apply(&mut trial, &proposals[0]).is_err());
    assert_eq!(trial, a);
    {
        let applied = p.apply(&mut trial, &proposals[1]).unwrap();
        assert_eq!(applied.lists()[0], [3, 4]);
    }
    assert_eq!(trial, a);
    {
        let applied = p.apply(&mut trial, &proposals[2]).unwrap();
        assert_eq!(applied.lists()[1], [2, 1]);
    }
    assert_eq!(trial, a);
    let no_op = index
        .change_samples_with::<i32>(
            [0],
            [Some(ListPosition {
                entity: 0,
                index: 0,
            })],
            1,
            false,
            |_| 0,
        )
        .next()
        .unwrap();
    {
        let applied = p.apply(&mut trial, &no_op).unwrap();
        assert_eq!(*applied, a);
    }
    let destinations = Cell::new(0);
    let mut never = index.change_samples_with::<i32>(
        iter::repeat(1),
        iter::repeat_with(|| {
            destinations.set(destinations.get() + 1);
            None
        }),
        4,
        true,
        |_| panic!("ineligible draw"),
    );
    assert!(never.next().is_none());
    assert_eq!(destinations.get(), 0);
}
#[test]
fn swaps_bound_each_side_keep_overlap_and_check_random_bounds() {
    let p = problem();
    let a = p.initial_assignment();
    let index = SubListSelection::new(&p, &a, SubListSize::new(1, 2).unwrap()).unwrap();
    let mut ranks = [4, 0, 1].into_iter();
    let movement = index
        .swap_samples_with::<i32>(&index, ([0], [0]), 1, true, |_| ranks.next().unwrap())
        .next()
        .unwrap();
    assert_eq!(
        movement,
        ListMove::SubListSwap {
            left: ListRange {
                entity: 0,
                start: 1,
                end: 3
            },
            right: ListRange {
                entity: 0,
                start: 3,
                end: 4
            },
            reverse: true
        }
    );
    let overlap = index
        .swap_samples_with::<i32>(&index, ([0], [0]), 1, false, |_| 0)
        .next()
        .unwrap();
    assert!(p.apply(&mut a.clone(), &overlap).is_err());
    let left = Cell::new(0);
    let right = Cell::new(0);
    let draws = Cell::new(0);
    let mut bounded = index.swap_samples_with::<i32>(
        &index,
        (
            iter::repeat_with(|| {
                left.set(left.get() + 1);
                0
            }),
            iter::repeat_with(|| {
                right.set(right.get() + 1);
                1
            }),
        ),
        3,
        false,
        |_| {
            draws.set(draws.get() + 1);
            0
        },
    );
    assert!(bounded.next().is_none());
    assert!(bounded.next().is_none());
    assert_eq!((left.get(), right.get(), draws.get()), (1, 3, 1));
    let empty = SubListSelection::new(&p, &a, SubListSize::new(20, 20).unwrap()).unwrap();
    assert!(
        index
            .swap_samples_with::<i32>(
                &empty,
                (
                    iter::repeat_with(|| panic!("left read")),
                    iter::repeat_with(|| panic!("right read"))
                ),
                5,
                true,
                |_| panic!("draw")
            )
            .next()
            .is_none()
    );
    assert!(
        catch_unwind(AssertUnwindSafe(|| index
            .random_iter_for_entities_with([0], 1, |n| n)
            .next()))
        .is_err()
    );
    let mut ranks = [0, 2].into_iter();
    assert!(
        catch_unwind(AssertUnwindSafe(|| index
            .change_samples_with::<i32>(
                [0],
                [Some(ListPosition {
                    entity: 3,
                    index: 0
                })],
                1,
                true,
                |_| ranks.next().unwrap()
            )
            .next()))
        .is_err()
    );
    let mut ranks = [0, 0, 2].into_iter();
    assert!(
        catch_unwind(AssertUnwindSafe(|| index
            .swap_samples_with::<i32>(&index, ([0], [3]), 1, true, |_| ranks.next().unwrap())
            .next()))
        .is_err()
    );
}
fn proposals(p: &ListProblem<i32>, a: &ListAssignment<i32>, _: u64) -> Vec<ListMove<i32>> {
    let index = SubListSelection::new(p, a, SubListSize::new(2, 2).unwrap()).unwrap();
    index
        .change_samples_with(
            [1, 0],
            [Some(ListPosition {
                entity: 1,
                index: 0,
            })],
            2,
            false,
            |_| 0,
        )
        .collect()
}
#[test]
fn entity_driven_proposals_improve_list_and_mixed_solver_results() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![ListEntity::new(vec![1, 2, 3, 4]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(2, 4);
    n.add_constraint(
        "balance",
        vec![ListScope::Entity(0), ListScope::Entity(1)],
        |rows| {
            let [ListFact::Entity(l), ListFact::Entity(r)] = rows else {
                panic!("entity facts")
            };
            SimpleScore::new(-(l.len() as i64 - r.len() as i64).abs())
        },
    )
    .unwrap();
    let mut provider = proposals;
    let r = list_local_search_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        LocalSearchConfig {
            max_steps: 1,
            assert_scores: true,
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.best.unwrap().score, SimpleScore::ZERO);
    let m = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![7]).with_initial(7).with_pinned(true),
        ])
        .unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 2, 4);
    n.add_constraint(
        "balance",
        vec![MixedScope::ListEntity(0), MixedScope::ListEntity(1)],
        |rows| {
            let [MixedFact::ListEntity(l), MixedFact::ListEntity(r)] = rows else {
                panic!("entity facts")
            };
            SimpleScore::new(-(l.len() as i64 - r.len() as i64).abs())
        },
    )
    .unwrap();
    let mut provider = |p: &MixedProblem<i32, i32>, a: &MixedAssignment<i32, i32>, step| {
        proposals(p.lists(), a.lists(), step)
            .into_iter()
            .map(|lists| MixedMove {
                basic: Move::Composite(vec![]),
                lists,
            })
            .collect::<Vec<_>>()
    };
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(0, &mut provider);
    let r = solve_mixed_with_extensions(
        &m,
        &n,
        MixedSolverSequence {
            phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 1,
                    assert_scores: true,
                    ..Default::default()
                },
                ..Default::default()
            })],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.score, Some(SimpleScore::ZERO));
    assert_eq!(r.assignment.basic().values(), &[Some(7)]);
}
