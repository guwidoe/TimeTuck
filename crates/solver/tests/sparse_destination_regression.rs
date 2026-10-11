//! Original adaptive receiver index and gather integration regressions.
use timetuck::*;

fn problem(indexed: bool, shape: usize) -> ListProblem<usize> {
    let n = 128;
    let rows = (0..n)
        .map(|entity| {
            let pin = entity % 17 == 0;
            let mut row = ListEntity::new(vec![2 * entity, 2 * entity + 1])
                .with_pinned(pin)
                .with_pinned_prefix(if pin { 0 } else { entity % 3 });
            if shape == 1 || (shape == 2 && entity % 13 != 0) {
                // Asymmetric ranges, including co-values absent from the assignment.
                row = row.with_value_range(vec![
                    2 * entity,
                    2 * entity + 1,
                    2 * ((entity + 1) % n),
                    256,
                ]);
            } else if shape == 3 {
                row = row.with_value_range(
                    (0..n)
                        .map(|offset| (2 * entity + offset) % (2 * n))
                        .collect(),
                );
            }
            row
        })
        .collect();
    let values = (0..258).rev().collect();
    if indexed {
        ListProblem::new_indexed(values, rows)
    } else {
        ListProblem::new(values, rows)
    }
    .unwrap()
    .allowing_unassigned()
}

fn reference(
    p: &ListProblem<usize>,
    a: &ListAssignment<usize>,
    group: &[usize],
    unassigned: bool,
) -> Vec<Option<ListPosition>> {
    unassigned
        .then_some(None)
        .into_iter()
        .chain(
            p.entities()
                .iter()
                .enumerate()
                .filter(|(_, meta)| {
                    !meta.is_pinned()
                        && meta
                            .value_range()
                            .is_none_or(|range| group.iter().all(|value| range.contains(value)))
                })
                .flat_map(|(entity, meta)| {
                    (meta.pinned_prefix()..=a.lists()[entity].len())
                        .map(move |index| Some(ListPosition { entity, index }))
                }),
        )
        .collect()
}

#[test]
fn adaptive_queries_match_direct_reference_for_ranges_pins_order_and_duplicates() {
    for shape in 0..4 {
        let p = problem(true, shape);
        let a = p.initial_assignment();
        for unassigned in [false, true] {
            let index = ListDestinationIndex::new(&p, &a, unassigned).unwrap();
            for origin in 0..258 {
                for group in [
                    vec![],
                    vec![origin],
                    vec![origin, (origin + 2) % 258],
                    vec![256, origin, origin],
                    vec![257, origin],
                ] {
                    let expected = reference(&p, &a, &group, unassigned);
                    let actual = index.for_values(&group).unwrap();
                    assert_eq!(actual.len(), expected.len());
                    assert_eq!(actual.iter().collect::<Vec<_>>(), expected);
                    assert_eq!(
                        actual.iter().rev().collect::<Vec<_>>(),
                        expected.iter().rev().copied().collect::<Vec<_>>()
                    );
                    assert_eq!(actual.get(actual.len()), None);
                    let mut entities: Vec<_> =
                        expected.iter().flatten().map(|p| p.entity).collect();
                    entities.dedup();
                    assert_eq!(actual.entities().collect::<Vec<_>>(), entities);
                }
            }
            assert!(matches!(
                index.for_values(&[257, usize::MAX]),
                Err(ListDestinationError::Model(ListError::ValueOutsideDomain))
            ));
        }
    }
}

#[test]
fn sparse_gather_sampling_preserves_seeded_moves_and_partial_iteration() {
    for shape in [0, 1, 2] {
        let indexed = problem(true, shape);
        let direct = problem(false, shape);
        let before = indexed.initial_assignment();
        for unassigned in [false, true] {
            for seed in [0, 819, u64::MAX] {
                let mut config = GatherSamplingConfig::new(1, 3, 48, seed).unwrap();
                config.unassignment = unassigned;
                let expected: Vec<_> = sampled_list_gather_moves(&direct, &before, config)
                    .unwrap()
                    .collect();
                let actual: Vec<_> = sampled_list_gather_moves(&indexed, &before, config)
                    .unwrap()
                    .collect();
                assert_eq!(actual, expected);
                assert_eq!(
                    sampled_list_gather_moves(&indexed, &before, config)
                        .unwrap()
                        .next(),
                    expected.first().cloned()
                );
                // Dropping a partial cursor cannot retain receiver state or draws.
                assert_eq!(
                    sampled_list_gather_moves(&indexed, &before, config)
                        .unwrap()
                        .collect::<Vec<_>>(),
                    expected
                );
                if let Some(movement) = actual.first() {
                    let mut assignment = before.clone();
                    indexed.apply(&mut assignment, movement).unwrap().commit();
                    let fresh =
                        ListDestinationIndex::new(&indexed, &assignment, unassigned).unwrap();
                    for group in [vec![2], vec![2, 4], vec![256]] {
                        assert_eq!(
                            fresh.for_values(&group).unwrap().iter().collect::<Vec<_>>(),
                            reference(&indexed, &assignment, &group, unassigned)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn all_pinned_queries_still_validate_every_identity_and_keep_optional_unassignment() {
    let p = ListProblem::new_indexed(vec![1, 2], vec![ListEntity::new(vec![1]).with_pinned(true)])
        .unwrap()
        .allowing_unassigned();
    let a = p.initial_assignment();
    for unassigned in [false, true] {
        let index = ListDestinationIndex::new(&p, &a, unassigned).unwrap();
        for group in [vec![], vec![1], vec![2], vec![1, 2, 2]] {
            assert_eq!(
                index.for_values(&group).unwrap().iter().collect::<Vec<_>>(),
                if unassigned { vec![None] } else { vec![] }
            );
        }
        assert!(matches!(
            index.for_values(&[2, 99]),
            Err(ListDestinationError::Model(ListError::ValueOutsideDomain))
        ));
    }
}
