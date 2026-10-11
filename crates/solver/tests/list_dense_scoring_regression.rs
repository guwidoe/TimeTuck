//! Original dense/sparse scoring regressions; no upstream-method credit.
use std::{
    cell::{Cell, RefCell},
    hash::{Hash, Hasher},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::{
    ListAssignment, ListConstraintNetwork, ListEntity, ListFact, ListMove, ListPosition,
    ListProblem, ListScope, ListScoreDirector, SimpleScore,
};

fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}

fn fact_score(facts: &[ListFact<'_, usize>]) -> i64 {
    facts
        .iter()
        .enumerate()
        .map(|(slot, fact)| {
            let score = match fact {
                ListFact::Entity(row) => row.iter().map(|v| *v as i64).sum(),
                ListFact::Value { value, element } => {
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

fn reference(a: &ListAssignment<usize>, scopes: &[Vec<ListScope>]) -> SimpleScore {
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
                        ListScope::Entity(e) => a.lists()[e].iter().map(|v| *v as i64).sum(),
                        ListScope::Value(v) => {
                            let mut shadow = -1000;
                            for (e, row) in a.lists().iter().enumerate() {
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
    SimpleScore::new(score)
        .with_structural(-((128 - a.lists().iter().map(Vec::len).sum::<usize>()) as i64))
}

#[test]
fn dense_moves_preserve_order_counts_unassigned_neighbors_and_explanations() {
    let scopes: Vec<_> = (0..128)
        .map(|i| {
            vec![
                ListScope::Value(i),
                ListScope::Entity(i % 4),
                ListScope::Value((i + 67) % 128),
                ListScope::Value(i),
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
        let calls = RefCell::new(vec![0; 128]);
        let mut net = ListConstraintNetwork::new(4, 128);
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
        let mut director = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
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
        for movement in &movements {
            let before = director.assignment().clone();
            let before_score = director.score();
            let touched = movement.touched_entities();
            // Test rejection and then commit the same candidate.
            for commit in [false, true] {
                let before_calls = calls.borrow().clone();
                let candidate = director.apply(movement).unwrap();
                let after = candidate.assignment();
                let expected_calls: Vec<_> = scopes
                    .iter()
                    .enumerate()
                    .map(|(i, scope)| {
                        usize::from(
                            i % 11 != 0
                                && scope.iter().any(|s| match *s {
                                    ListScope::Entity(e) => touched.contains(&e),
                                    ListScope::Value(v) => touched.iter().any(|&e| {
                                        before.lists()[e].contains(&v)
                                            || after.lists()[e].contains(&v)
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
                            (ListScope::Value(i), ListFact::Value { value, element }) => {
                                assert_eq!(*i, **value);
                                assert_eq!(*element, director.assignment().element(value));
                            }
                            (ListScope::Entity(e), ListFact::Entity(row)) => {
                                assert_eq!(*row, director.assignment().lists()[*e])
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
fn dense_callback_errors_and_panics_restore_assignment_and_contributions() {
    for panic in [false, true] {
        let p = ListProblem::new_indexed(
            (0..128).collect(),
            vec![ListEntity::new((0..128).collect()), ListEntity::new(vec![])],
        )
        .unwrap();
        let mut net = ListConstraintNetwork::new(2, 128);
        for i in 0..128 {
            net.add_constraint(format!("v{i}"), vec![ListScope::Value(i), ListScope::Value(0)], move |f| {
                if i == 127 && matches!(f[1], ListFact::Value { element: Some(e), .. } if e.position.entity == 1) {
                    assert!(!panic, "dense callback panic");
                    return SimpleScore::ZERO.with_structural(1);
                }
                SimpleScore::new(fact_score(f))
            }).unwrap();
        }
        let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        let before = d.assignment().clone();
        let score = d.score();
        let scores: Vec<_> = d.constraint_scores().map(|(_, s)| s).collect();
        let result = catch_unwind(AssertUnwindSafe(|| {
            d.apply(&ListMove::Change {
                source: pos(0, 0),
                destination: pos(1, 0),
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
        d.apply(&ListMove::Swap {
            left: pos(0, 0),
            right: pos(0, 127),
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
fn dense_lookups_are_linear_sparse_lookups_stay_direct_and_diagnostics_are_fresh() {
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
    for dense in [false, true] {
        let count = if dense { 256 } else { 2 };
        let offset = Cell::new(0);
        let mut net = ListConstraintNetwork::<Value, SimpleScore>::new(1, 256);
        for i in 0..count {
            let offset = &offset;
            net.add_constraint(format!("v{i}"), vec![ListScope::Value(i)], move |f| {
                let ListFact::Value {
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
            })
            .unwrap();
        }
        let mut d = ListScoreDirector::new(&p, &net, p.initial_assignment()).unwrap();
        comparisons.set(0);
        d.explain(|_, _, _, _| ((), Vec::<usize>::new()));
        assert!(comparisons.get() <= if dense { 512 } else { 3 });
        comparisons.set(0);
        drop(
            d.apply(&ListMove::Swap {
                left: pos(0, 0),
                right: pos(0, 1),
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
