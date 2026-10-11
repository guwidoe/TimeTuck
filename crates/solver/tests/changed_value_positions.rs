//! Original transaction-position reuse regressions; no upstream source credit.
use std::{cell::RefCell, rc::Rc};
use timetuck::*;
type Calls = Rc<RefCell<Vec<(usize, Vec<i64>)>>>;

fn lists(indexed: bool) -> ListProblem<usize> {
    let entities = std::iter::once(ListEntity::new((0..8).collect()))
        .chain((8..256).map(|i| ListEntity::new(vec![i])))
        .collect();
    if indexed {
        ListProblem::new_indexed((0..260).collect(), entities)
    } else {
        ListProblem::new((0..260).collect(), entities)
    }
    .unwrap()
}
fn movements() -> Vec<ListMove<usize>> {
    let pos = |entity, index| ListPosition { entity, index };
    vec![
        ListMove::Change {
            source: pos(0, 0),
            destination: pos(0, 7),
        },
        ListMove::Unassign {
            entity: 0,
            start: 2,
            end: 5,
        },
        ListMove::Assign {
            value: 256,
            destination: pos(0, 0),
        },
        ListMove::Composite(vec![
            ListMove::Change {
                source: pos(0, 0),
                destination: pos(1, 1),
            },
            ListMove::Change {
                source: pos(1, 1),
                destination: pos(0, 0),
            },
        ]),
        ListMove::Composite(vec![
            ListMove::Assign {
                value: 259,
                destination: pos(0, 0),
            },
            ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1,
            },
        ]),
        ListMove::Composite(vec![]),
    ]
}
fn element_score(value: usize, element: &Option<ListElement<'_, usize>>) -> i64 {
    value as i64
        + element.as_ref().map_or(-10000, |e| {
            e.position.entity as i64 * 37
                + e.position.index as i64 * 19
                + e.previous.copied().unwrap_or(999) as i64 * 3
                + e.next.copied().unwrap_or(888) as i64 * 5
        })
}

#[test]
fn list_final_positions_match_direct_lookup_across_scopes_and_composites() {
    for wide in [false, true] {
        let make = |indexed| lists(indexed);
        let indexed = make(true);
        let direct = make(false);
        let indexed_calls = Rc::new(RefCell::new(vec![]));
        let direct_calls = Rc::new(RefCell::new(vec![]));
        let network = |calls: &Calls| {
            let mut n = ListConstraintNetwork::new(249, 260);
            for i in 0..64 {
                let scope = if wide && i == 0 {
                    (0..260).map(ListScope::Value).collect()
                } else {
                    vec![
                        ListScope::Value(i),
                        ListScope::Value(255),
                        ListScope::Value(258),
                        ListScope::Value(i),
                    ]
                };
                let calls = calls.clone();
                n.add_weighted_constraint(
                    format!("c{i}"),
                    scope,
                    SimpleScore::new(i64::from(i != 17)),
                    move |facts| {
                        assert_ne!(i, 17, "disabled callback");
                        let values: Vec<_> = facts
                            .iter()
                            .map(|f| match f {
                                ListFact::Value { value, element } => {
                                    element_score(**value, element)
                                }
                                _ => unreachable!(),
                            })
                            .collect();
                        let score = values
                            .iter()
                            .enumerate()
                            .map(|(i, v)| (i as i64 + 1) * v)
                            .sum();
                        calls.borrow_mut().push((i, values));
                        score
                    },
                )
                .unwrap();
            }
            n
        };
        let ni = network(&indexed_calls);
        let nd = network(&direct_calls);
        let mut a = ListScoreDirector::new(&indexed, &ni, indexed.initial_assignment()).unwrap();
        let mut b = ListScoreDirector::new(&direct, &nd, direct.initial_assignment()).unwrap();
        assert_eq!(a.score(), b.score());
        assert_eq!(*indexed_calls.borrow(), *direct_calls.borrow());
        for (step, movement) in movements().into_iter().enumerate() {
            // Rejection and commitment must see the same final-state facts.
            for commit in [false, true] {
                indexed_calls.borrow_mut().clear();
                direct_calls.borrow_mut().clear();
                let ca = a.apply(&movement).unwrap();
                let cb = b.apply(&movement).unwrap();
                assert_eq!(ca.score(), cb.score(), "step {step}");
                assert_eq!(ca.assignment(), cb.assignment());
                assert_eq!(ca.constraint_evaluations(), cb.constraint_evaluations());
                assert_eq!(
                    *indexed_calls.borrow(),
                    *direct_calls.borrow(),
                    "step {step}"
                );
                if commit {
                    ca.commit();
                    cb.commit();
                } else {
                    drop(ca);
                    drop(cb);
                }
                assert_eq!(a.assignment(), b.assignment());
                a.assert_consistent().unwrap();
                b.assert_consistent().unwrap();
            }
        }
    }
}

#[test]
fn mixed_final_positions_match_direct_lookup_across_scopes_and_composites() {
    for wide in [false, true] {
        let make = |indexed| {
            MixedProblem::new(
                FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap(),
                lists(indexed),
            )
        };
        let indexed = make(true);
        let direct = make(false);
        let indexed_calls = Rc::new(RefCell::new(vec![]));
        let direct_calls = Rc::new(RefCell::new(vec![]));
        let network = |calls: &Calls| {
            let mut n = MixedConstraintNetwork::new(1, 249, 260);
            for i in 0..64 {
                let scope = if wide && i == 0 {
                    (0..260).map(MixedScope::ListValue).collect()
                } else {
                    vec![
                        MixedScope::ListValue(i),
                        MixedScope::ListValue(255),
                        MixedScope::ListValue(258),
                        MixedScope::ListValue(i),
                    ]
                };
                let mut scope = scope;
                scope.push(MixedScope::Basic(0));
                let calls = calls.clone();
                n.add_weighted_constraint(
                    format!("c{i}"),
                    scope,
                    SimpleScore::new(i64::from(i != 17)),
                    move |facts| {
                        assert_ne!(i, 17, "disabled callback");
                        let values: Vec<_> = facts
                            .iter()
                            .map(|f| match f {
                                MixedFact::ListValue { value, element } => {
                                    element_score(**value, element)
                                }
                                MixedFact::Basic(value) => value.map_or(-100, |v| *v as i64),
                                _ => unreachable!(),
                            })
                            .collect();
                        let score = values
                            .iter()
                            .enumerate()
                            .map(|(i, v)| (i as i64 + 1) * v)
                            .sum();
                        calls.borrow_mut().push((i, values));
                        score
                    },
                )
                .unwrap();
            }
            n
        };
        let ni = network(&indexed_calls);
        let nd = network(&direct_calls);
        let mut a = MixedScoreDirector::new(&indexed, &ni, indexed.initial_assignment()).unwrap();
        let mut b = MixedScoreDirector::new(&direct, &nd, direct.initial_assignment()).unwrap();
        assert_eq!(a.score(), b.score());
        assert_eq!(*indexed_calls.borrow(), *direct_calls.borrow());
        for (step, movement) in movements().into_iter().enumerate() {
            let movement = MixedMove {
                basic: if step == 5 {
                    Move::Change {
                        variable: 0,
                        value: Some(2),
                    }
                } else {
                    Move::Composite(vec![])
                },
                lists: movement,
            };
            // Rejection and commitment must see the same final-state facts.
            for commit in [false, true] {
                indexed_calls.borrow_mut().clear();
                direct_calls.borrow_mut().clear();
                let ca = a.apply(&movement).unwrap();
                let cb = b.apply(&movement).unwrap();
                assert_eq!(ca.score(), cb.score(), "step {step}");
                assert_eq!(ca.assignment(), cb.assignment());
                assert_eq!(ca.constraint_evaluations(), cb.constraint_evaluations());
                assert_eq!(
                    *indexed_calls.borrow(),
                    *direct_calls.borrow(),
                    "step {step}"
                );
                if commit {
                    ca.commit();
                    cb.commit();
                } else {
                    drop(ca);
                    drop(cb);
                }
                assert_eq!(a.assignment(), b.assignment());
                a.assert_consistent().unwrap();
                b.assert_consistent().unwrap();
            }
        }
    }
}
