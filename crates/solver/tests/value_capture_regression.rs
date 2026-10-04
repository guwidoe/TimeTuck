// Original sparse-capture reference, clone-cost and panic-safety checks.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

fn reference(initial: &[Option<i32>], movement: &Move<i32>) -> Result<Vec<Option<i32>>, MoveError> {
    fn execute(
        a: &mut [Option<i32>],
        m: &Move<i32>,
        writes: &mut Vec<Option<i32>>,
    ) -> Result<(), MoveError> {
        let error = |i| MoveError {
            variable: i,
            variable_count: a.len(),
        };
        match m {
            Move::Change { variable, value } => {
                if *variable >= a.len() {
                    return Err(error(*variable));
                }
                a[*variable] = *value;
                writes.push(*value);
            }
            Move::Swap { left, right } => {
                let x = *a.get(*left).ok_or_else(|| error(*left))?;
                let y = *a.get(*right).ok_or_else(|| error(*right))?;
                a[*left] = y;
                a[*right] = x;
                writes.extend([y, x]);
            }
            Move::Composite(children) => {
                for child in children {
                    execute(a, child, writes)?;
                }
            }
        }
        Ok(())
    }
    let mut a = initial.to_vec();
    let mut writes = vec![];
    execute(&mut a, movement, &mut writes)?;
    Ok(writes
        .iter()
        .enumerate()
        .filter(|(i, x)| !writes[..*i].contains(x))
        .map(|(_, x)| *x)
        .collect())
}
#[test]
fn nested_composites_match_independent_full_array_execution_and_error_order() {
    let a = Assignment::new(vec![None, Some(0), Some(1), Some(0)]);
    let mut state = 928174u64;
    let mut draw = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        state >> 32
    };
    let (mut changes, mut swaps, mut valid_nonempty, mut invalid) = (0, 0, 0, 0);
    for case in 0..1500 {
        let mut children = vec![];
        for _ in 0..case % 13 {
            let i = (draw() % 6) as usize;
            let m = if draw() % 2 == 0 {
                changes += 1;
                Move::Change {
                    variable: i,
                    value: match draw() % 3 {
                        0 => None,
                        1 => Some(0),
                        _ => Some(2),
                    },
                }
            } else {
                swaps += 1;
                Move::Swap {
                    left: i,
                    right: (draw() % 6) as usize,
                }
            };
            if draw() % 3 == 0 {
                children.push(Move::Composite(vec![Move::Composite(vec![]), m]));
            } else {
                children.push(m);
            }
        }
        let m = Move::Composite(children);
        let expected = reference(a.values(), &m);
        if expected.is_err() {
            invalid += 1;
        } else if case % 13 != 0 {
            valid_nonempty += 1;
        }
        assert_eq!(a.planning_values(&m), expected, "case {case}");
        assert_eq!(a.values(), &[None, Some(0), Some(1), Some(0)]);
    }
    assert!(changes > 100 && swaps > 100 && valid_nonempty > 100 && invalid > 100);
}

#[derive(Debug, Default)]
struct Control {
    clones: Cell<usize>,
    panic_clone: Cell<Option<usize>>,
    panic_eq: Cell<Option<usize>>,
}
#[derive(Debug)]
struct Value {
    id: usize,
    control: Rc<Control>,
}
impl Clone for Value {
    fn clone(&self) -> Self {
        self.control.clones.set(self.control.clones.get() + 1);
        assert_ne!(
            self.control.panic_clone.get(),
            Some(self.id),
            "injected clone panic"
        );
        Self {
            id: self.id,
            control: self.control.clone(),
        }
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        assert!(
            !self
                .control
                .panic_eq
                .get()
                .is_some_and(|v| v == self.id || v == other.id),
            "injected equality panic"
        );
        self.id == other.id
    }
}
impl Eq for Value {}
fn value(id: usize, c: &Rc<Control>) -> Option<Value> {
    Some(Value {
        id,
        control: c.clone(),
    })
}
fn ids(v: &[Option<Value>]) -> Vec<Option<usize>> {
    v.iter().map(|v| v.as_ref().map(|v| v.id)).collect()
}

#[test]
fn capture_clones_only_distinct_outputs_instead_of_the_assignment() {
    let c = Rc::new(Control::default());
    let a = Assignment::new((0..10000).map(|i| value(i, &c)).collect());
    // An untouched input is not cloned, even if cloning it would panic.
    c.panic_clone.set(Some(9999));
    let m = Move::Change {
        variable: 0,
        value: value(20000, &c),
    };
    assert_eq!(ids(&a.planning_values(&m).unwrap()), vec![Some(20000)]);
    assert_eq!(c.clones.get(), 1);
    c.panic_clone.set(None);
    c.clones.set(0);
    assert_eq!(
        ids(&a
            .planning_values(&Move::Swap {
                left: 2,
                right: 9999
            })
            .unwrap()),
        vec![Some(9999), Some(2)]
    );
    assert_eq!(c.clones.get(), 2);
    c.clones.set(0);
    assert_eq!(
        ids(&a
            .planning_values(&Move::Swap { left: 7, right: 7 })
            .unwrap()),
        vec![Some(7)]
    );
    assert_eq!(c.clones.get(), 1);
    c.clones.set(0);
    let m = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: value(20, &c),
        },
        Move::Swap { left: 0, right: 1 },
        Move::Change {
            variable: 2,
            value: None,
        },
        Move::Swap { left: 1, right: 2 },
        Move::Composite(vec![
            Move::Change {
                variable: 0,
                value: value(1, &c),
            },
            Move::Swap { left: 0, right: 0 },
        ]),
    ]);
    assert_eq!(
        ids(&a.planning_values(&m).unwrap()),
        vec![Some(20), Some(1), None]
    );
    assert_eq!(c.clones.get(), 2);
    assert_eq!(ids(a.values()), (0..10000).map(Some).collect::<Vec<_>>());
}

#[test]
fn invalid_indices_and_panics_leave_input_intact_without_cloning_invalid_queries() {
    let c = Rc::new(Control::default());
    let a = Assignment::new((0..4).map(|i| value(i, &c)).collect());
    for bad in [4, usize::MAX] {
        for m in [
            Move::Change {
                variable: bad,
                value: value(2, &c),
            },
            Move::Swap {
                left: bad,
                right: bad,
            },
            Move::Swap {
                left: 0,
                right: bad,
            },
            Move::Composite(vec![
                Move::Change {
                    variable: 0,
                    value: value(7, &c),
                },
                Move::Swap {
                    left: 0,
                    right: bad,
                },
            ]),
        ] {
            c.panic_clone.set(Some(2));
            let error = a.planning_values(&m).unwrap_err();
            assert_eq!(
                error,
                MoveError {
                    variable: bad,
                    variable_count: 4
                }
            );
            assert_eq!(c.clones.get(), 0);
        }
    }
    assert!(
        catch_unwind(AssertUnwindSafe(
            || a.planning_values(&Move::Swap { left: 0, right: 2 })
        ))
        .is_err()
    );
    c.panic_clone.set(None);
    c.clones.set(0);
    c.panic_eq.set(Some(2));
    assert!(
        catch_unwind(AssertUnwindSafe(
            || a.planning_values(&Move::Swap { left: 0, right: 2 })
        ))
        .is_err()
    );
    assert_eq!(c.clones.get(), 0);
    c.panic_eq.set(None);
    assert_eq!(ids(a.values()), vec![Some(0), Some(1), Some(2), Some(3)]);
    assert_eq!(
        ids(&a
            .planning_values(&Move::Swap { left: 0, right: 2 })
            .unwrap()),
        vec![Some(2), Some(0)]
    );
}

#[test]
fn equal_identities_preserve_first_write_payload_through_later_overrides() {
    #[derive(Clone, Debug)]
    struct Tagged(i32, &'static str);
    impl PartialEq for Tagged {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Tagged {}
    let a = Assignment::new(vec![Some(Tagged(1, "input")), Some(Tagged(2, "second"))]);
    let m = Move::Composite(vec![
        Move::Change {
            variable: 0,
            value: Some(Tagged(1, "first")),
        },
        Move::Change {
            variable: 0,
            value: Some(Tagged(1, "later")),
        },
        Move::Swap { left: 0, right: 1 },
    ]);
    let out = a.planning_values(&m).unwrap();
    assert_eq!(
        out.iter()
            .map(|v| {
                let v = v.as_ref().unwrap();
                (v.0, v.1)
            })
            .collect::<Vec<_>>(),
        vec![(1, "first"), (2, "second")]
    );
    assert_eq!(a.values()[0].as_ref().unwrap().1, "input");
}
