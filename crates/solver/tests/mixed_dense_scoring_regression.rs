//! Original dense/sparse mixed scoring regressions; no upstream-method credit.
use std::{
    cell::{Cell, RefCell},
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    FiniteProblem, ListEntity, ListMove, ListPosition, ListProblem, MixedAssignment,
    MixedConstraintNetwork, MixedFact, MixedMove, MixedProblem, MixedScope, MixedScoreDirector,
    Move, SimpleScore, Variable,
};

fn basic_problem() -> FiniteProblem<usize> {
    FiniteProblem::new(
        (0..4)
            .map(|_| Variable::new(vec![1, 2]).with_initial(1))
            .collect(),
    )
    .unwrap()
}

fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}

fn fact_score(facts: &[MixedFact<'_, usize, usize>]) -> i64 {
    facts
        .iter()
        .enumerate()
        .map(|(slot, fact)| {
            let score = match fact {
                MixedFact::Basic(value) => value.map_or(-100, |v| *v as i64 * 17),
                MixedFact::ListEntity(row) => row.iter().map(|v| *v as i64).sum(),
                MixedFact::ListValue { value, element } => {
                    **value as i64
                        + element.as_ref().map_or(-1000, |e| {
                            100 * e.position.entity as i64
                                + 7 * e.position.index as i64
                                + e.previous.copied().unwrap_or(999) as i64
                                + 3 * e.next.copied().unwrap_or(999) as i64
                        })
                }
            };
            (slot as i64 + 1) * score
        })
        .sum()
}

fn reference(a: &MixedAssignment<usize, usize>, scopes: &[Vec<MixedScope>]) -> SimpleScore {
    // Derive every shadow directly from the rows; do not use element/location or
    // the network's contribution/fact-building paths.
    let score = scopes
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 11 != 0)
        .map(|(_, scope)| {
            scope
                .iter()
                .enumerate()
                .map(|(slot, scope)| {
                    let value = match *scope {
                        MixedScope::Basic(i) => {
                            a.basic().values()[i].map_or(-100, |v| v as i64 * 17)
                        }
                        MixedScope::ListEntity(e) => {
                            a.lists().lists()[e].iter().map(|v| *v as i64).sum()
                        }
                        MixedScope::ListValue(v) => {
                            let mut shadow = -1000;
                            for (e, row) in a.lists().lists().iter().enumerate() {
                                for (i, &value) in row.iter().enumerate() {
                                    if value == v {
                                        shadow = 100 * e as i64
                                            + 7 * i as i64
                                            + if i == 0 { 999 } else { row[i - 1] as i64 }
                                            + 3 * row.get(i + 1).copied().unwrap_or(999) as i64;
                                    }
                                }
                            }
                            v as i64 + shadow
                        }
                    };
                    (slot as i64 + 1) * value
                })
                .sum::<i64>()
        })
        .sum();
    SimpleScore::new(score).with_structural(
        -((128 - a.lists().lists().iter().map(Vec::len).sum::<usize>()
            + a.basic().values().iter().filter(|v| v.is_none()).count()) as i64),
    )
}

#[test]
fn mixed_dense_scopes_preserve_joint_basic_and_list_edits() {
    let scopes: Vec<_> = (0..128)
        .map(|i| {
            vec![
                MixedScope::Basic(i % 4),
                MixedScope::ListValue(i),
                MixedScope::ListEntity(i % 4),
                MixedScope::ListValue((i + 67) % 128),
                MixedScope::ListValue(i),
            ]
        })
        .collect();
    for indexed in [false, true] {
        let entities: Vec<_> = (0..4)
            .map(|e| ListEntity::new((e * 24..(e + 1) * 24).collect()))
            .collect();
        let p = if indexed {
            ListProblem::new_indexed((0..128).collect(), entities)
        } else {
            ListProblem::new((0..128).collect(), entities)
        }
        .unwrap();
        let p = MixedProblem::new(basic_problem(), p);
        let calls = RefCell::new(vec![0; 128]);
        let mut net = MixedConstraintNetwork::new(4, 4, 128);
        for (i, scope) in scopes.iter().enumerate() {
            let calls = &calls;
            net.add_weighted_constraint(
                format!("v{i}"),
                scope.clone(),
                SimpleScore::new(i64::from(i % 11 != 0)),
                move |f| {
                    assert_ne!(i % 11, 0, "disabled callback");
                    calls.borrow_mut()[i] += 1;
                    fact_score(f)
                },
            )
            .unwrap();
        }
        let mut director = MixedScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        assert_eq!(director.score(), reference(director.assignment(), &scopes));
        let movements = [
            ListMove::Change {
                source: pos(0, 3),
                destination: pos(1, 7),
            },
            ListMove::Unassign {
                entity: 1,
                start: 2,
                end: 17,
            },
            ListMove::Assign {
                value: 127,
                destination: pos(2, 0),
            },
            ListMove::Swap {
                left: pos(2, 7),
                right: pos(3, 4),
            },
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
            ListMove::Composite(vec![]),
        ];
        let movements = movements
            .into_iter()
            .enumerate()
            .map(|(i, lists)| MixedMove {
                basic: Move::Change {
                    variable: i % 4,
                    value: if i % 2 == 0 { Some(2) } else { None },
                },
                lists,
            })
            .chain([
                MixedMove {
                    basic: Move::Change {
                        variable: 0,
                        value: Some(1),
                    },
                    lists: ListMove::Composite(vec![]),
                },
                MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Swap {
                        left: pos(2, 0),
                        right: pos(3, 0),
                    },
                },
                MixedMove {
                    basic: Move::Composite(vec![]),
                    lists: ListMove::Composite(vec![]),
                },
            ]);
        for movement in movements {
            let before = director.assignment().clone();
            let before_score = director.score();
            let touched = movement.lists.touched_entities();
            let basic_touched = movement.basic.touched_variables();
            // Test rejection and then commit the same candidate.
            for commit in [false, true] {
                let before_calls = calls.borrow().clone();
                let candidate = director.apply(&movement).unwrap();
                let after = candidate.assignment();
                let expected_calls: Vec<_> = scopes
                    .iter()
                    .enumerate()
                    .map(|(i, scope)| {
                        usize::from(
                            i % 11 != 0
                                && scope.iter().any(|s| match *s {
                                    MixedScope::Basic(i) => basic_touched.contains(&i),
                                    MixedScope::ListEntity(e) => touched.contains(&e),
                                    MixedScope::ListValue(v) => touched.iter().any(|&e| {
                                        before.lists().lists()[e].contains(&v)
                                            || after.lists().lists()[e].contains(&v)
                                    }),
                                }),
                        )
                    })
                    .collect();
                assert_eq!(
                    candidate.constraint_evaluations(),
                    expected_calls.iter().sum::<usize>()
                );
                for i in 0..128 {
                    assert_eq!(calls.borrow()[i] - before_calls[i], expected_calls[i]);
                }
                assert_eq!(candidate.score(), reference(after, &scopes));
                if commit {
                    candidate.commit();
                } else {
                    drop(candidate);
                    assert_eq!(director.assignment(), &before);
                    assert_eq!(director.score(), before_score);
                }
                let calls_before_explain = calls.borrow().clone();
                let mut explanations = 0;
                let explained = director.explain(|_, scope, facts, score| {
                    explanations += 1;
                    assert_eq!(scope.len(), facts.len());
                    assert_eq!(*score, SimpleScore::new(fact_score(facts)));
                    for (scope, fact) in scope.iter().zip(facts) {
                        match (scope, fact) {
                            (MixedScope::Basic(i), MixedFact::Basic(value)) => assert_eq!(
                                value.copied(),
                                director.assignment().basic().values()[*i]
                            ),
                            (MixedScope::ListValue(i), MixedFact::ListValue { value, element }) => {
                                assert_eq!(*i, **value);
                                assert_eq!(*element, director.assignment().lists().element(value));
                            }
                            (MixedScope::ListEntity(e), MixedFact::ListEntity(row)) => {
                                assert_eq!(*row, director.assignment().lists().lists()[*e])
                            }
                            _ => panic!("fact order changed"),
                        }
                    }
                    ((), Vec::<usize>::new())
                });
                assert_eq!(explanations, 116);
                assert_eq!(explained.score, director.score());
                assert_eq!(*calls.borrow(), calls_before_explain);
                assert_eq!(
                    net.calculate(&p, director.assignment()).unwrap(),
                    director.score()
                );
                director.assert_consistent().unwrap();
            }
        }
    }
}

#[test]
fn mixed_dense_scoring_errors_and_panics_restore_both_components() {
    for panic in [false, true] {
        let p = ListProblem::new_indexed(
            (0..128).collect(),
            vec![ListEntity::new((0..128).collect()), ListEntity::new(vec![])],
        )
        .unwrap();
        let p = MixedProblem::new(basic_problem(), p);
        let mut net = MixedConstraintNetwork::new(4, 2, 128);
        for i in 0..128 {
            net.add_constraint(format!("v{i}"), vec![MixedScope::Basic(0), MixedScope::ListValue(i), MixedScope::ListValue(0)], move |f| {
                if i == 127 && matches!(f[2], MixedFact::ListValue { element: Some(e), .. } if e.position.entity == 1) {
                    assert!(!panic, "dense callback panic");
                    return SimpleScore::ZERO.with_structural(1);
                }
                SimpleScore::new(fact_score(f))
            }).unwrap();
        }
        let mut d = MixedScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        let before = d.assignment().clone();
        let score = d.score();
        let scores: Vec<_> = d.constraint_scores().map(|(_, s)| s).collect();
        let result = catch_unwind(AssertUnwindSafe(|| {
            d.apply(&MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(2),
                },
                lists: ListMove::Change {
                    source: pos(0, 0),
                    destination: pos(1, 0),
                },
            })
            .map(drop)
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert_eq!(d.assignment(), &before);
        assert_eq!(d.score(), score);
        assert_eq!(
            d.constraint_scores().map(|(_, s)| s).collect::<Vec<_>>(),
            scores
        );
        d.assert_consistent().unwrap();
        d.apply(&MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(2),
            },
            lists: ListMove::Swap {
                left: pos(0, 0),
                right: pos(0, 127),
            },
        })
        .unwrap()
        .commit();
        d.assert_consistent().unwrap();
    }
}

#[derive(Clone, Debug)]
struct Value {
    id: usize,
    payload: usize,
    comparisons: Rc<Cell<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.comparisons.set(self.comparisons.get() + 1);
        self.id == other.id
    }
}
impl Eq for Value {}
impl Hash for Value {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.id.hash(h);
    }
}

#[test]
fn mixed_dense_basic_only_edits_are_linear_and_diagnostics_remain_independent() {
    let comparisons = Rc::new(Cell::new(0));
    let values: Vec<_> = (0..256)
        .map(|id| Value {
            id,
            payload: 1000 + id,
            comparisons: comparisons.clone(),
        })
        .collect();
    let row: Vec<_> = values
        .iter()
        .cloned()
        .map(|mut v| {
            v.payload = v.id;
            v
        })
        .collect();
    let p = ListProblem::new_indexed(values, vec![ListEntity::new(row)]).unwrap();
    let p = MixedProblem::new(basic_problem(), p);
    for dense in [false, true] {
        let count = if dense { 256 } else { 2 };
        let offset = Cell::new(0);
        let mut net = MixedConstraintNetwork::<usize, Value, SimpleScore>::new(4, 1, 256);
        for i in 0..count {
            let offset = &offset;
            net.add_constraint(
                format!("v{i}"),
                vec![MixedScope::ListValue(i), MixedScope::Basic(0)],
                move |f| {
                    let MixedFact::ListValue {
                        value,
                        element: Some(element),
                    } = &f[0]
                    else {
                        panic!("assigned value")
                    };
                    assert_eq!(value.payload, 1000 + i);
                    for neighbor in [element.previous, element.next].into_iter().flatten() {
                        assert_eq!(neighbor.payload, neighbor.id);
                    }
                    SimpleScore::new(element.position.index as i64 + offset.get())
                },
            )
            .unwrap();
        }
        let mut d = MixedScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        comparisons.set(0);
        d.explain(|_, _, _, _| ((), Vec::<usize>::new()));
        assert!(comparisons.get() <= if dense { 512 } else { 3 });
        comparisons.set(0);
        drop(
            d.apply(&MixedMove {
                basic: Move::Change {
                    variable: 0,
                    value: Some(2),
                },
                lists: ListMove::Composite(vec![]),
            })
            .unwrap(),
        );
        assert!(
            comparisons.get() < 256 * 25,
            "{} comparisons",
            comparisons.get()
        );
        comparisons.set(0);
        offset.set(1);
        let report = d.consistency_report().unwrap();
        assert!(!report.is_consistent());
        if dense {
            assert!(comparisons.get() >= 256 * 128);
        }
        offset.set(0);
        d.assert_consistent().unwrap();
    }
}
