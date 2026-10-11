//! Original differential primitive-arrival validation; no upstream source credit.
use std::{
    cell::Cell,
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn candidates() -> Vec<ListMove<usize>> {
    let lengths = [3, 2, 0];
    let mut moves = Vec::new();
    for (a, &len) in lengths.iter().enumerate() {
        for i in 0..=len + 1 {
            moves.push(ListMove::Assign {
                value: 5,
                destination: pos(a, i),
            });
            for j in i..=len + 1 {
                moves.push(ListMove::Unassign {
                    entity: a,
                    start: i,
                    end: j,
                });
                for (b, &other) in lengths.iter().enumerate() {
                    for k in 0..=other + 1 {
                        moves.push(ListMove::Change {
                            source: pos(a, i),
                            destination: pos(b, k),
                        });
                        moves.push(ListMove::Swap {
                            left: pos(a, i),
                            right: pos(b, k),
                        });
                        for reverse in [false, true] {
                            moves.push(ListMove::SubListChange {
                                entity: a,
                                start: i,
                                end: j,
                                destination: pos(b, k),
                                reverse,
                            });
                            for l in k..=other + 1 {
                                moves.push(ListMove::SubListSwap {
                                    left: ListRange {
                                        entity: a,
                                        start: i,
                                        end: j,
                                    },
                                    right: ListRange {
                                        entity: b,
                                        start: k,
                                        end: l,
                                    },
                                    reverse,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    moves
}
#[test]
fn primitives_match_full_final_validation_and_restore_on_every_exit() {
    let initial = vec![vec![0, 1, 2], vec![3, 4], vec![]];
    let free = ListProblem::new_indexed(
        (0..6).collect(),
        initial.iter().cloned().map(ListEntity::new).collect(),
    )
    .unwrap();
    for indexed in [false, true] {
        for pin in 0..3 {
            let entities = vec![
                ListEntity::new(initial[0].clone())
                    .with_value_range(vec![0, 1, 2, 3])
                    .with_pinned_prefix(usize::from(pin == 1)),
                ListEntity::new(initial[1].clone())
                    .with_value_range(vec![0, 3, 4, 5])
                    .with_pinned_prefix(usize::from(pin == 2)),
                ListEntity::new(vec![]).with_value_range(vec![1, 5]),
            ];
            let p = if indexed {
                ListProblem::new_indexed((0..6).collect(), entities)
            } else {
                ListProblem::new((0..6).collect(), entities)
            }
            .unwrap();
            let n = ListConstraintNetwork::<usize, SimpleScore>::new(3, 6);
            let mut d = ListScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
            for movement in candidates() {
                let mut a = free.initial_assignment();
                let expected = match free.apply(&mut a, &movement) {
                    Ok(trial) => p.validate(&trial).map(|()| trial.lists().to_vec()),
                    Err(error) => Err(error),
                };
                let actual = d
                    .apply(&movement)
                    .map(|trial| trial.assignment().lists().to_vec());
                assert_eq!(
                    actual,
                    expected.clone().map_err(ListScoringError::Model),
                    "{movement:?}, pin={pin}"
                );
                assert_eq!(d.assignment().lists(), initial);
                // The composite deliberately retains full-row final validation.
                let actual = d
                    .apply(&ListMove::Composite(vec![movement.clone()]))
                    .map(|trial| trial.assignment().lists().to_vec());
                assert_eq!(actual, expected.map_err(ListScoringError::Model));
                assert_eq!(d.assignment().lists(), initial);
            }
            d.assert_consistent().unwrap();
        }
    }
}
#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: usize,
    hashes: Rc<Cell<usize>>,
    panic_on: Rc<Cell<Option<usize>>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.hashes.set(self.hashes.get() + 1);
        assert_ne!(self.panic_on.get(), Some(self.id), "hash panic");
        self.id.hash(h);
    }
}
#[test]
fn retained_list_and_mixed_check_only_arrivals_and_keep_actual_payloads() {
    let hashes = Rc::new(Cell::new(0));
    let panic_on = Rc::new(Cell::new(None));
    let make = |id, payload| Value {
        id,
        payload,
        hashes: hashes.clone(),
        panic_on: panic_on.clone(),
    };
    let p = ListProblem::new_indexed(
        (0..128).map(|i| make(i, i)).collect(),
        (0..2)
            .map(|e| {
                ListEntity::new((e * 64..(e + 1) * 64).map(|i| make(i, 1000 + i)).collect())
                    .with_value_range((0..128).map(|i| make(i, i)).collect())
            })
            .collect(),
    )
    .unwrap();
    let net = ListConstraintNetwork::<Value, SimpleScore>::new(2, 128);
    let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
    let swap = ListMove::Swap {
        left: pos(0, 1),
        right: pos(1, 2),
    };
    hashes.set(0);
    {
        let trial = d.apply(&swap).unwrap();
        assert_eq!(trial.assignment().lists()[0][1].payload, 1066);
        assert_eq!(trial.assignment().lists()[1][2].payload, 1001);
    }
    assert_eq!(hashes.get(), 2);
    hashes.set(0);
    drop(
        d.apply(&ListMove::SubListChange {
            entity: 0,
            start: 1,
            end: 40,
            destination: pos(0, 0),
            reverse: true,
        })
        .unwrap(),
    );
    assert_eq!(hashes.get(), 0);
    panic_on.set(Some(66));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = d.apply(&swap);
        }))
        .is_err()
    );
    panic_on.set(None);
    assert_eq!(d.assignment().lists()[0][1].payload, 1001);
    assert_eq!(d.assignment().lists()[1][2].payload, 1066);
    d.assert_consistent().unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1]).with_initial(1)]).unwrap(),
        p,
    );
    let n = MixedConstraintNetwork::<usize, Value, SimpleScore>::new(1, 2, 128);
    let mut d = MixedScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    hashes.set(0);
    d.apply(&MixedMove {
        basic: Move::Composite(vec![]),
        lists: swap,
    })
    .unwrap()
    .commit();
    assert_eq!(hashes.get(), 2);
    assert_eq!(d.assignment().lists().lists()[0][1].payload, 1066);
    d.assert_consistent().unwrap();
}
#[test]
fn public_apply_still_rejects_invalid_untouched_source_rows() {
    let p = ListProblem::new_indexed(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![0, 1]),
            ListEntity::new(vec![]).with_value_range(vec![]),
        ],
    )
    .unwrap();
    let mut a = ListAssignment::new(vec![vec![0, 1], vec![2]]);
    let original = a.clone();
    assert!(matches!(
        p.apply(
            &mut a,
            &ListMove::Swap {
                left: pos(0, 0),
                right: pos(0, 1)
            }
        ),
        Err(ListError::ValueOutsideEntityRange { entity: 1 })
    ));
    assert_eq!(a, original);
}
