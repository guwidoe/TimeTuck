//! Original callback borrowing/order/transaction contracts; no method credit.
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Observation {
    Basic(Option<(usize, usize)>),
    Entity(Vec<usize>, usize),
    Value {
        value: usize,
        address: usize,
        position: Option<ListPosition>,
        previous: Option<(usize, usize)>,
        next: Option<(usize, usize)>,
    },
}
fn borrowed(value: Option<&usize>) -> Option<(usize, usize)> {
    value.map(|v| (*v, std::ptr::from_ref(v) as usize))
}
fn list_value(value: &usize, element: Option<ListElement<'_, usize>>) -> Observation {
    Observation::Value {
        value: *value,
        address: std::ptr::from_ref(value) as usize,
        position: element.as_ref().map(|e| e.position),
        previous: element.as_ref().and_then(|e| borrowed(e.previous)),
        next: element.as_ref().and_then(|e| borrowed(e.next)),
    }
}
fn list_fact(fact: &ListFact<'_, usize>) -> Observation {
    match fact {
        ListFact::Entity(row) => Observation::Entity(row.to_vec(), row.as_ptr() as usize),
        ListFact::Value { value, element } => list_value(value, *element),
    }
}
fn mixed_fact(fact: &MixedFact<'_, usize, usize>) -> Observation {
    match fact {
        MixedFact::Basic(value) => Observation::Basic(borrowed(*value)),
        MixedFact::ListEntity(row) => Observation::Entity(row.to_vec(), row.as_ptr() as usize),
        MixedFact::ListValue { value, element } => list_value(value, *element),
    }
}
// Independent list observation uses explicit rows, never element/location lookup.
fn expected_value(p: &ListProblem<usize>, a: &ListAssignment<usize>, id: usize) -> Observation {
    let mut position = None;
    let mut previous = None;
    let mut next = None;
    for (entity, row) in a.lists().iter().enumerate() {
        for (index, value) in row.iter().enumerate() {
            if *value == p.values()[id] {
                position = Some(ListPosition { entity, index });
                previous = index.checked_sub(1).and_then(|i| borrowed(row.get(i)));
                next = borrowed(row.get(index + 1));
            }
        }
    }
    Observation::Value {
        value: p.values()[id],
        address: std::ptr::from_ref(&p.values()[id]) as usize,
        position,
        previous,
        next,
    }
}
fn expected_list(
    p: &ListProblem<usize>,
    a: &ListAssignment<usize>,
    scope: ListScope,
) -> Observation {
    match scope {
        ListScope::Entity(entity) => Observation::Entity(
            a.lists()[entity].to_vec(),
            a.lists()[entity].as_ptr() as usize,
        ),
        ListScope::Value(value) => expected_value(p, a, value),
    }
}
fn contribution(facts: &[Observation]) -> SimpleScore {
    // Slot-dependent factors make reversed/repeated arguments observable in score.
    SimpleScore::new(
        3 + facts
            .iter()
            .enumerate()
            .map(|(slot, fact)| {
                let value = match fact {
                    Observation::Basic(value) => value.map_or(-1, |(value, _)| value as i64),
                    Observation::Entity(row, _) => row.iter().sum::<usize>() as i64,
                    Observation::Value {
                        value,
                        position,
                        previous,
                        next,
                        ..
                    } => {
                        *value as i64
                            + position.map_or(-100, |p| 100 * p.entity as i64 + p.index as i64)
                            + previous.map_or(-1, |(v, _)| v as i64)
                            + next.map_or(-1, |(v, _)| v as i64)
                    }
                };
                (slot as i64 + 1) * value
            })
            .sum::<i64>(),
    )
}
type Journal = RefCell<Vec<(usize, Vec<Observation>)>>;
fn basic_problem() -> FiniteProblem<usize> {
    FiniteProblem::new(vec![
        Variable::new(vec![10, 20]).with_initial(10),
        Variable::new(vec![11]).allowing_unassigned(),
        Variable::new(vec![30]).with_initial(30),
    ])
    .unwrap()
}
fn list_problem() -> ListProblem<usize> {
    ListProblem::new(
        vec![10, 20, 30, 40],
        vec![ListEntity::new(vec![10, 20, 30]), ListEntity::new(vec![])],
    )
    .unwrap()
    .allowing_unassigned()
}
fn relocation() -> ListMove<usize> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 1,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}

#[test]
fn basic_arguments_preserve_borrowed_slots_order_none_and_transaction_boundaries() {
    let p = basic_problem();
    let scopes = [
        vec![],
        vec![1],
        vec![1, 0],
        vec![1, 0, 1, 2],
        vec![2, 1, 0, 2, 1],
    ];
    let journal = Journal::default();
    let mut network = ConstraintNetwork::new(3);
    for (id, scope) in scopes.iter().enumerate() {
        let journal = &journal;
        network
            .add_constraint(format!("arity-{id}"), scope.clone(), move |facts| {
                let observed: Vec<_> = facts
                    .iter()
                    .map(|&v| Observation::Basic(borrowed(v)))
                    .collect();
                let score = contribution(&observed);
                journal.borrow_mut().push((id, observed));
                score
            })
            .unwrap();
    }
    network
        .add_weighted_constraint("zero", scopes[4].clone(), SimpleScore::ZERO, |_| {
            panic!("zero weight invokes no callback")
        })
        .unwrap();
    let check = |a: &Assignment<usize>, expected_ids: &[usize]| {
        let calls = journal.take();
        assert_eq!(
            calls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            expected_ids
        );
        for (id, actual) in calls {
            let expected: Vec<_> = scopes[id]
                .iter()
                .map(|&i| Observation::Basic(borrowed(a.values()[i].as_ref())))
                .collect();
            assert_eq!(actual, expected);
        }
        scopes
            .iter()
            .map(|scope| {
                contribution(
                    &scope
                        .iter()
                        .map(|&i| Observation::Basic(borrowed(a.values()[i].as_ref())))
                        .collect::<Vec<_>>(),
                )
                .score()
            })
            .sum::<i64>()
    };
    let mut d = ScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    assert_eq!(
        d.score(),
        SimpleScore::new(check(d.assignment(), &[0, 1, 2, 3, 4]))
    );
    let before = d.assignment().clone();
    let movement = Move::Change {
        variable: 0,
        value: Some(20),
    };
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(trial.constraint_evaluations(), 3);
        assert_eq!(
            trial.score(),
            SimpleScore::new(check(trial.assignment(), &[2, 3, 4]))
        );
    }
    assert_eq!(d.assignment(), &before);
    assert!(
        journal.borrow().is_empty(),
        "rollback does not call evaluators"
    );
    let trial = d.apply(&movement).unwrap();
    let expected = SimpleScore::new(check(trial.assignment(), &[2, 3, 4]));
    trial.commit();
    assert_eq!(d.score(), expected);
    d.assert_consistent().unwrap();
    check(d.assignment(), &[0, 1, 2, 3, 4]);
}

#[test]
fn list_arguments_preserve_registry_rows_missing_values_and_live_neighbors() {
    let p = list_problem();
    let scopes = [
        vec![],
        vec![ListScope::Value(3)],
        vec![ListScope::Value(1), ListScope::Entity(0)],
        vec![
            ListScope::Value(2),
            ListScope::Value(1),
            ListScope::Entity(0),
            ListScope::Value(2),
        ],
        vec![
            ListScope::Value(1),
            ListScope::Entity(1),
            ListScope::Value(3),
            ListScope::Value(1),
            ListScope::Entity(0),
        ],
    ];
    let journal = Journal::default();
    let mut network = ListConstraintNetwork::new(2, 4);
    for (id, scope) in scopes.iter().enumerate() {
        let journal = &journal;
        network
            .add_constraint(format!("arity-{id}"), scope.clone(), move |facts| {
                let observed: Vec<_> = facts.iter().map(list_fact).collect();
                let score = contribution(&observed);
                journal.borrow_mut().push((id, observed));
                score
            })
            .unwrap();
    }
    network
        .add_weighted_constraint("zero", scopes[4].clone(), SimpleScore::ZERO, |_| {
            panic!("zero weight invokes no callback")
        })
        .unwrap();
    let check = |a: &ListAssignment<usize>, expected_ids: &[usize]| {
        let calls = journal.take();
        assert_eq!(
            calls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            expected_ids
        );
        for (id, actual) in calls {
            let expected: Vec<_> = scopes[id]
                .iter()
                .map(|&scope| expected_list(&p, a, scope))
                .collect();
            assert_eq!(actual, expected);
        }
        scopes
            .iter()
            .map(|scope| {
                contribution(
                    &scope
                        .iter()
                        .map(|&scope| expected_list(&p, a, scope))
                        .collect::<Vec<_>>(),
                )
                .score()
            })
            .sum::<i64>()
    };
    let mut d = ListScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    assert_eq!(
        d.score(),
        SimpleScore::new(check(d.assignment(), &[0, 1, 2, 3, 4]))
    );
    let before = d.assignment().clone();
    {
        let trial = d.apply(&relocation()).unwrap();
        assert_eq!(trial.constraint_evaluations(), 3);
        assert_eq!(
            trial.score(),
            SimpleScore::new(check(trial.assignment(), &[2, 3, 4]))
        );
    }
    assert_eq!(d.assignment(), &before);
    assert!(journal.borrow().is_empty());
    let trial = d.apply(&relocation()).unwrap();
    let expected = SimpleScore::new(check(trial.assignment(), &[2, 3, 4]));
    trial.commit();
    assert_eq!(d.score(), expected);
    d.assert_consistent().unwrap();
    check(d.assignment(), &[0, 1, 2, 3, 4]);
}

#[test]
fn mixed_arguments_keep_basic_and_list_borrows_in_declared_cross_component_order() {
    let p = MixedProblem::new(basic_problem(), list_problem());
    let scopes = [
        vec![],
        vec![MixedScope::Basic(1)],
        vec![MixedScope::ListValue(1), MixedScope::ListEntity(0)],
        vec![
            MixedScope::Basic(0),
            MixedScope::ListValue(1),
            MixedScope::Basic(0),
            MixedScope::ListEntity(0),
        ],
        vec![
            MixedScope::ListValue(3),
            MixedScope::Basic(1),
            MixedScope::ListValue(1),
            MixedScope::ListEntity(1),
            MixedScope::ListValue(1),
        ],
    ];
    let journal = Journal::default();
    let mut network = MixedConstraintNetwork::new(3, 2, 4);
    for (id, scope) in scopes.iter().enumerate() {
        let journal = &journal;
        network
            .add_constraint(format!("arity-{id}"), scope.clone(), move |facts| {
                let observed: Vec<_> = facts.iter().map(mixed_fact).collect();
                let score = contribution(&observed);
                journal.borrow_mut().push((id, observed));
                score
            })
            .unwrap();
    }
    network
        .add_weighted_constraint("zero", scopes[4].clone(), SimpleScore::ZERO, |_| {
            panic!("zero weight invokes no callback")
        })
        .unwrap();
    let expected_fact = |a: &MixedAssignment<usize, usize>, scope| match scope {
        MixedScope::Basic(i) => Observation::Basic(borrowed(a.basic().values()[i].as_ref())),
        MixedScope::ListEntity(i) => expected_list(p.lists(), a.lists(), ListScope::Entity(i)),
        MixedScope::ListValue(i) => expected_value(p.lists(), a.lists(), i),
    };
    let check = |a: &MixedAssignment<usize, usize>, expected_ids: &[usize]| {
        let calls = journal.take();
        assert_eq!(
            calls.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            expected_ids
        );
        for (id, actual) in calls {
            let expected: Vec<_> = scopes[id]
                .iter()
                .map(|&scope| expected_fact(a, scope))
                .collect();
            assert_eq!(actual, expected);
        }
        scopes
            .iter()
            .map(|scope| {
                contribution(
                    &scope
                        .iter()
                        .map(|&scope| expected_fact(a, scope))
                        .collect::<Vec<_>>(),
                )
                .score()
            })
            .sum::<i64>()
    };
    let mut d = MixedScoreDirector::new(&p, &network, p.initial_assignment()).unwrap();
    assert_eq!(
        d.score(),
        SimpleScore::new(check(d.assignment(), &[0, 1, 2, 3, 4]))
    );
    let before = d.assignment().clone();
    let movement = MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(20),
        },
        lists: relocation(),
    };
    {
        let trial = d.apply(&movement).unwrap();
        assert_eq!(trial.constraint_evaluations(), 3);
        assert_eq!(
            trial.score(),
            SimpleScore::new(check(trial.assignment(), &[2, 3, 4]))
        );
    }
    assert_eq!(d.assignment(), &before);
    assert!(journal.borrow().is_empty());
    let trial = d.apply(&movement).unwrap();
    let expected = SimpleScore::new(check(trial.assignment(), &[2, 3, 4]));
    trial.commit();
    assert_eq!(d.score(), expected);
    d.assert_consistent().unwrap();
    check(d.assignment(), &[0, 1, 2, 3, 4]);
}

#[test]
fn weighted_overflow_structural_rejection_and_panic_leave_all_directors_reusable() {
    use std::cell::Cell;
    macro_rules! exercise {
        ($problem:expr, $network:expr, $scope:expr, $director:ident, $error:ident, $movement:expr) => {{
            let p = $problem;
            let scope = $scope;
            let mode = Cell::new(0);
            let mut network = $network;
            network
                .add_weighted_constraint(
                    "four-weighted",
                    vec![scope; 4],
                    SimpleScore::new(i64::MAX),
                    |facts| {
                        assert_eq!(facts.len(), 4);
                        if mode.get() == 1 { 2 } else { 0 }
                    },
                )
                .unwrap();
            network
                .add_constraint("five-direct", vec![scope; 5], |facts| {
                    assert_eq!(facts.len(), 5);
                    assert_ne!(mode.get(), 3, "intentional evaluator panic");
                    SimpleScore::new(1).with_structural(if mode.get() == 2 { -1 } else { 0 })
                })
                .unwrap();
            network
                .add_weighted_constraint("zero", vec![scope; 5], SimpleScore::ZERO, |_| {
                    panic!("zero callback")
                })
                .unwrap();
            let mut d = $director::new(&p, &network, p.initial_assignment()).unwrap();
            let before = d.assignment().clone();
            let score = d.score();
            let movement = $movement;
            mode.set(1);
            assert!(matches!(
                d.apply(&movement).err(),
                Some($error::ScoreOverflow {
                    constraint: Some(0)
                })
            ));
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), score);
            mode.set(2);
            assert!(matches!(
                d.apply(&movement).err(),
                Some($error::StructuralContribution { constraint: 1 })
            ));
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), score);
            mode.set(3);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = d.apply(&movement);
                }))
                .is_err()
            );
            assert_eq!(d.assignment(), &before);
            assert_eq!(d.score(), score);
            mode.set(0);
            d.apply(&movement).unwrap().commit();
            assert_ne!(d.assignment(), &before);
            assert_eq!(d.score(), score);
            d.assert_consistent().unwrap();
        }};
    }
    exercise!(
        basic_problem(),
        ConstraintNetwork::new(3),
        0,
        ScoreDirector,
        ScoringError,
        Move::Change {
            variable: 0,
            value: Some(20)
        }
    );
    exercise!(
        list_problem(),
        ListConstraintNetwork::new(2, 4),
        ListScope::Entity(0),
        ListScoreDirector,
        ListScoringError,
        relocation()
    );
    exercise!(
        MixedProblem::new(basic_problem(), list_problem()),
        MixedConstraintNetwork::new(3, 2, 4),
        MixedScope::Basic(0),
        MixedScoreDirector,
        MixedScoringError,
        MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(20)
            },
            lists: relocation()
        }
    );
}
