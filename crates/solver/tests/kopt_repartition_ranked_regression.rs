//! Original current-index reference for multi-route ranked relocation construction.
use timetuck::*;

fn reference(
    before: &ListAssignment<usize>,
    target: &[Vec<usize>],
    entities: &[usize],
    prefixes: &[usize],
) -> ListMove<usize> {
    let mut rows = before.lists().to_vec();
    let mut leaves = vec![];
    for &entity in entities {
        for (index, &wanted) in target[entity].iter().enumerate().skip(prefixes[entity]) {
            let (from_row, from_index) = rows
                .iter()
                .enumerate()
                .find_map(|(e, row)| row.iter().position(|&v| v == wanted).map(|i| (e, i)))
                .unwrap();
            if (from_row, from_index) != (entity, index) {
                let value = rows[from_row].remove(from_index);
                rows[entity].insert(index, value);
                leaves.push(ListMove::Change {
                    source: ListPosition {
                        entity: from_row,
                        index: from_index,
                    },
                    destination: ListPosition { entity, index },
                });
            }
        }
    }
    assert_eq!(rows, target);
    ListMove::Composite(leaves)
}

#[test]
fn samples_preserve_exact_leaves_at_thresholds_with_empty_and_pinned_routes() {
    for n in [32, 127, 128, 129, 513] {
        for shape in [[n, 0, 0], [n / 2, n / 3, n - n / 2 - n / 3], [0, 1, n - 1]] {
            for restricted in [false, true] {
                let mut next = 0;
                let prefixes = [2, 0, 1, 1];
                let mut entities: Vec<_> = shape
                    .iter()
                    .enumerate()
                    .map(|(row, &length)| {
                        let count = length + prefixes[row];
                        let values: Vec<_> = (next..next + count).collect();
                        next += count;
                        let entity = ListEntity::new(values).with_pinned_prefix(prefixes[row]);
                        if restricted {
                            // Retain the original row while allowing most incoming identities.
                            entity.with_value_range(
                                (0..n + 3).filter(|&v| v < next || v % 17 != row).collect(),
                            )
                        } else {
                            entity
                        }
                    })
                    .collect();
                entities.push(ListEntity::new(vec![next]).with_pinned(true));
                let p = ListProblem::new_indexed((0..next + 2).rev().collect(), entities)
                    .unwrap()
                    .allowing_unassigned();
                let a = p.initial_assignment();
                let config = ListKOptSampling::new(ListKOptConfig::new(2, 4).unwrap(), 817, 64);
                let candidates: Vec<_> = sample_list_k_opt_repartitions(&p, &a, config.clone())
                    .unwrap()
                    .collect();
                if !restricted {
                    assert!(!candidates.is_empty());
                }
                // Opening and abandoning early cannot mutate or consume later openings.
                assert_eq!(
                    sample_list_k_opt_repartitions(&p, &a, config)
                        .unwrap()
                        .next(),
                    candidates.first().cloned()
                );
                for movement in candidates {
                    let mut state = a.clone();
                    {
                        let trial = p.apply(&mut state, &movement).unwrap();
                        assert_eq!(
                            movement,
                            reference(&a, trial.lists(), &[0, 1, 2], &prefixes)
                        );
                        for (row, &prefix) in prefixes.iter().enumerate() {
                            assert_eq!(&trial.lists()[row][..prefix], &a.lists()[row][..prefix]);
                        }
                        assert_eq!(trial.location(&(next + 1)), None);
                    }
                    assert_eq!(state, a);
                }
            }
        }
    }
}

#[test]
fn explicit_reversed_entity_order_preserves_exact_leaves_and_empty_targets() {
    let p = ListProblem::new_indexed(
        (0..259).rev().collect(),
        vec![
            ListEntity::new((0..129).collect()).with_pinned_prefix(1),
            ListEntity::new(vec![129]).with_pinned_prefix(1),
            ListEntity::new((130..259).collect()).with_pinned_prefix(1),
        ],
    )
    .unwrap();
    let a = p.initial_assignment();
    let order = [2, 1, 0];
    for lengths in [[0, 256, 0], [100, 0, 156], [256, 0, 0], [127, 1, 128]] {
        let movement = p
            .k_opt_repartition_move(
                &a,
                &order,
                &[(31, 32), (191, 192)],
                &[(31, 191), (32, 192)],
                (100, 99),
                &lengths,
            )
            .unwrap();
        let mut state = a.clone();
        {
            let trial = p.apply(&mut state, &movement).unwrap();
            assert_eq!(movement, reference(&a, trial.lists(), &order, &[1, 1, 1]));
        }
        assert_eq!(state, a);
    }
}
