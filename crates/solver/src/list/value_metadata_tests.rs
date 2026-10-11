//! Original differential capture/error, clone-work and panic-safety checks.
use super::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
// The previous public implementation, retained as an independent transactional
// reference. It executes the actual move recorder, not the optimized checker.
fn transactional<V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
    m: &ListMove<V>,
) -> Result<Vec<V>, ListError> {
    let mut copy = a.clone();
    let mut out = vec![];
    {
        let _tx = p.apply_recording(&mut copy, m, |v| {
            if !out.contains(v) {
                out.push(v.clone());
            }
        })?;
    }
    Ok(out)
}
#[test]
fn strict_metadata_matches_transactional_values_and_exact_errors() {
    let mut valid = 0;
    let mut invalid = 0;
    for pins in [false, true] {
        for dependencies in [false, true] {
            let mut p = ListProblem::new(
                (0..5).collect(),
                vec![
                    ListEntity::new(vec![0, 1]).with_pinned_prefix(usize::from(pins)),
                    ListEntity::new(vec![2, 3])
                        .with_value_range(vec![1, 2, 3, 4])
                        .with_range_diagnostics("e1", "values", ToString::to_string),
                    ListEntity::new(vec![]).with_value_range(vec![0, 4]),
                ],
            )
            .unwrap();
            if dependencies {
                p = p
                    .with_dependencies(vec![vec![], vec![0], vec![], vec![2], vec![]])
                    .unwrap();
            }
            let a = p.initial_assignment();
            let mut moves = vec![ListMove::Composite(vec![])];
            for entity in 0..4 {
                for index in 0..4 {
                    moves.extend([
                        ListMove::Change {
                            source: pos(entity, index),
                            destination: pos(1, 0),
                        },
                        ListMove::Swap {
                            left: pos(entity, index),
                            right: pos(1, 1),
                        },
                        ListMove::Assign {
                            value: 4,
                            destination: pos(entity, index),
                        },
                        ListMove::Assign {
                            value: 99,
                            destination: pos(entity, index),
                        },
                        ListMove::Assign {
                            value: 2,
                            destination: pos(entity, index),
                        },
                        ListMove::Unassign {
                            entity,
                            start: index,
                            end: index + 1,
                        },
                        ListMove::SubListChange {
                            entity,
                            start: index,
                            end: index + 1,
                            destination: pos(0, 0),
                            reverse: true,
                        },
                        ListMove::SubListSwap {
                            left: ListRange {
                                entity,
                                start: index,
                                end: index + 1,
                            },
                            right: ListRange {
                                entity: 0,
                                start: 0,
                                end: 2,
                            },
                            reverse: true,
                        },
                    ]);
                }
            }
            moves.extend([
                ListMove::Composite(vec![
                    ListMove::Change {
                        source: pos(0, 0),
                        destination: pos(1, 0),
                    },
                    ListMove::Change {
                        source: pos(1, 0),
                        destination: pos(0, 0),
                    },
                ]),
                ListMove::Composite(vec![
                    ListMove::Assign {
                        value: 4,
                        destination: pos(2, 0),
                    },
                    ListMove::Change {
                        source: pos(2, 0),
                        destination: pos(1, 1),
                    },
                    ListMove::Unassign {
                        entity: 1,
                        start: 1,
                        end: 2,
                    },
                ]),
                ListMove::Composite(vec![
                    ListMove::Change {
                        source: pos(0, 0),
                        destination: pos(1, 0),
                    },
                    ListMove::Assign {
                        value: 99,
                        destination: pos(99, 99),
                    },
                ]),
                ListMove::SubListSwap {
                    left: ListRange {
                        entity: 99,
                        start: 1,
                        end: 0,
                    },
                    right: ListRange {
                        entity: 99,
                        start: 0,
                        end: 1,
                    },
                    reverse: false,
                },
                ListMove::SubListSwap {
                    left: ListRange {
                        entity: 0,
                        start: 0,
                        end: 2,
                    },
                    right: ListRange {
                        entity: 0,
                        start: 1,
                        end: 2,
                    },
                    reverse: false,
                },
            ]);
            for m in &moves {
                let expected = transactional(&p, &a, m);
                if expected.is_ok() {
                    valid += 1;
                } else {
                    invalid += 1;
                }
                assert_eq!(
                    p.planning_values(&a, m),
                    expected,
                    "pins={pins} dependencies={dependencies} move={m:?}"
                );
                assert_eq!(a, p.initial_assignment());
            }
            for malformed in [
                ListAssignment::new(vec![]),
                ListAssignment::new(vec![vec![0, 1], vec![1, 3], vec![]]),
                ListAssignment::new(vec![vec![0, 99], vec![2, 3], vec![]]),
                ListAssignment::new(vec![vec![1], vec![0, 2, 3], vec![]]),
            ] {
                for m in &moves[..9] {
                    assert_eq!(
                        p.planning_values(&malformed, m),
                        transactional(&p, &malformed, m)
                    );
                }
            }
        }
    }
    assert!(
        valid > 30 && invalid > 300,
        "counts: valid={valid} invalid={invalid}"
    );
}
#[derive(Debug)]
struct Value {
    id: usize,
    clones: Rc<Cell<usize>>,
    panic_at: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
impl Clone for Value {
    fn clone(&self) -> Self {
        let count = self.clones.get() + 1;
        self.clones.set(count);
        assert_ne!(count, self.panic_at.get(), "clone callback");
        Self {
            id: self.id,
            clones: self.clones.clone(),
            panic_at: self.panic_at.clone(),
        }
    }
}
#[test]
fn only_captured_outputs_clone_and_failures_leave_input_unchanged() {
    let clones = Rc::new(Cell::new(0));
    let panic_at = Rc::new(Cell::new(usize::MAX));
    let values: Vec<_> = (0..=256)
        .map(|id| Value {
            id,
            clones: clones.clone(),
            panic_at: panic_at.clone(),
        })
        .collect();
    let p = ListProblem::new(
        values.clone(),
        values[..256]
            .chunks(8)
            .map(|v| ListEntity::new(v.to_vec()))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    let ids = |a: &ListAssignment<Value>| {
        a.lists()
            .iter()
            .map(|row| row.iter().map(|v| v.id).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    };
    let before = ids(&a);
    let m = ListMove::Composite(vec![
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        },
        ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 1,
        },
        ListMove::Assign {
            value: values[256].clone(),
            destination: pos(1, 0),
        },
    ]);
    clones.set(0);
    let captured = p.planning_values(&a, &m).unwrap();
    assert_eq!(
        captured.iter().map(|v| v.id).collect::<Vec<_>>(),
        [0, 8, 256]
    );
    assert_eq!(clones.get(), 3);
    clones.set(0);
    assert!(
        p.planning_values(&a, &ListMove::Composite(vec![]))
            .unwrap()
            .is_empty()
    );
    assert_eq!(clones.get(), 0);
    clones.set(0);
    assert!(
        p.planning_values(
            &a,
            &ListMove::Change {
                source: pos(99, 0),
                destination: pos(0, 0)
            }
        )
        .is_err()
    );
    assert_eq!(clones.get(), 0);
    clones.set(0);
    panic_at.set(2);
    assert!(catch_unwind(AssertUnwindSafe(|| p.planning_values(&a, &m))).is_err());
    assert_eq!(ids(&a), before);
    panic_at.set(usize::MAX);
    clones.set(0);
    assert_eq!(p.planning_values(&a, &m).unwrap(), captured);
    assert_eq!(clones.get(), 3);
    assert_eq!(ids(&a), before);
}
