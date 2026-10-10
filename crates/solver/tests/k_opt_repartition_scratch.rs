//! Original allocation/payload regression for k-opt relocation generation.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[derive(Debug)]
struct Value {
    id: usize,
    payload: usize,
    clones: Rc<Cell<usize>>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        Self {
            id: self.id,
            payload: self.payload,
            clones: self.clones.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
#[test]
fn scratch_tracks_only_suffix_indices_and_preserves_assigned_payloads() {
    let clones = Rc::new(Cell::new(0));
    let value = |id, payload| Value {
        id,
        payload,
        clones: clones.clone(),
    };
    let p = ListProblem::new(
        (0..134).map(|i| value(i, 0)).collect(),
        vec![
            ListEntity::new((0..3).map(|i| value(i, i + 1000)).collect()),
            ListEntity::new((3..6).map(|i| value(i, i + 1000)).collect()),
            ListEntity::new((6..134).map(|i| value(i, i + 1000)).collect()).with_pinned(true),
        ],
    )
    .unwrap();
    let before = p.initial_assignment();
    for lengths in [[0, 6], [6, 0], [2, 4], [3, 3]] {
        clones.set(0);
        let movement = p
            .k_opt_repartition_move(
                &before,
                &[0, 1],
                &[(1, 2), (4, 5)],
                &[(1, 4), (2, 5)],
                (0, 1),
                &lengths,
            )
            .unwrap();
        // Validation and relocation construction borrow all assigned payloads.
        assert_eq!(clones.get(), 0);
        let mut candidate = before.clone();
        {
            let trial = p.apply(&mut candidate, &movement).unwrap();
            assert_eq!(trial.lists()[0].len(), lengths[0]);
            assert_eq!(trial.lists()[1].len(), lengths[1]);
            assert_eq!(
                trial.lists()[..2]
                    .iter()
                    .flatten()
                    .map(|v| v.id)
                    .collect::<Vec<_>>(),
                vec![0, 1, 4, 3, 2, 5]
            );
            assert_eq!(trial.lists()[2], before.lists()[2]);
            for v in trial.lists().iter().flatten() {
                assert_eq!(v.payload, v.id + 1000);
            }
        }
        assert_eq!(candidate, before);
    }
}

#[test]
fn borrowed_targets_keep_owned_validation_errors_in_entity_order() {
    let clones = Rc::new(Cell::new(0));
    let values = || {
        (0..6)
            .map(|id| Value {
                id,
                payload: id + 100,
                clones: clones.clone(),
            })
            .collect::<Vec<_>>()
    };
    for restricted in [false, true] {
        for dependencies in [false, true] {
            let all = values();
            let mut entities = vec![
                ListEntity::new(all[..3].to_vec()),
                ListEntity::new(all[3..].to_vec()),
            ];
            if restricted {
                entities[0] = entities[0].clone().with_value_range(all[..4].to_vec());
                entities[1] = entities[1].clone().with_value_range(all[2..].to_vec());
            }
            let mut p = ListProblem::new(values(), entities).unwrap();
            if dependencies {
                p = p
                    .with_dependencies(
                        (0..6)
                            .map(|i| if i == 0 { vec![] } else { vec![i - 1] })
                            .collect(),
                    )
                    .unwrap();
            }
            let before = p.initial_assignment();
            for order in [[0, 1], [1, 0]] {
                // Edge replacement (1,2),(4,5) -> (1,4),(2,5) gives this
                // independently written tour over the concatenated suffixes.
                let ids: Vec<_> = order
                    .iter()
                    .flat_map(|&e| before.lists()[e].iter())
                    .collect();
                let tour = [0, 1, 4, 3, 2, 5];
                for left_length in 0..=6 {
                    let mut rows = vec![vec![]; 2];
                    rows[order[0]] = tour[..left_length]
                        .iter()
                        .map(|&i| ids[i].clone())
                        .collect();
                    rows[order[1]] = tour[left_length..]
                        .iter()
                        .map(|&i| ids[i].clone())
                        .collect();
                    let target = ListAssignment::new(rows);
                    let expected = p.validate(&target).map_err(ListKOptError::List);
                    clones.set(0);
                    let movement = p.k_opt_repartition_move(
                        &before,
                        &order,
                        &[(1, 2), (4, 5)],
                        &[(1, 4), (2, 5)],
                        (0, 1),
                        &[left_length, 6 - left_length],
                    );
                    assert_eq!(clones.get(), 0);
                    assert_eq!(
                        movement.as_ref().map(|_| ()).map_err(Clone::clone),
                        expected
                    );
                    if let Ok(movement) = movement {
                        let mut actual = before.clone();
                        p.apply(&mut actual, &movement).unwrap().commit();
                        assert_eq!(actual, target);
                    }
                }
            }
        }
    }
}

#[test]
fn sampled_repartition_validation_never_clones_rejected_or_yielded_payloads() {
    let clones = Rc::new(Cell::new(0));
    let all: Vec<_> = (0..12)
        .map(|id| Value {
            id,
            payload: id + 100,
            clones: clones.clone(),
        })
        .collect();
    let mut unrestricted_count = 0;
    for restricted in [false, true] {
        for dependencies in [false, true] {
            let entities = (0..3)
                .map(|e| {
                    let entity =
                        ListEntity::new(all[e * 4..e * 4 + 4].to_vec()).with_pinned_prefix(1);
                    if restricted {
                        entity.with_value_range(
                            all.iter()
                                .filter(|v| v.id != ((e + 1) % 3) * 4 + 1)
                                .cloned()
                                .collect(),
                        )
                    } else {
                        entity
                    }
                })
                .collect();
            let mut p = ListProblem::new(all.clone(), entities).unwrap();
            if dependencies {
                p = p
                    .with_dependencies(
                        (0..12)
                            .map(|i| if i % 4 == 0 { vec![] } else { vec![i - 1] })
                            .collect(),
                    )
                    .unwrap();
            }
            let a = p.initial_assignment();
            clones.set(0);
            let moves: Vec<_> = sample_list_k_opt_repartitions(
                &p,
                &a,
                ListKOptSampling::new(ListKOptConfig::new(2, 4).unwrap(), 731, 256),
            )
            .unwrap()
            .collect();
            assert_eq!(clones.get(), 0);
            assert!(!moves.is_empty());
            if !dependencies {
                if restricted {
                    assert!(moves.len() < unrestricted_count);
                } else {
                    unrestricted_count = moves.len();
                }
            }
            for m in moves {
                let mut target = a.clone();
                p.apply(&mut target, &m).unwrap().commit();
                p.validate(&target).unwrap();
                assert!(p.is_complete(&target));
            }
        }
    }
}
