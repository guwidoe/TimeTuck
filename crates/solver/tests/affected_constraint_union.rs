//! Original affected-scope regressions; no upstream source-method credit.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

const VARIABLES: usize = 64;
const CONSTRAINTS: usize = 512;
const FAIL_AT: usize = 101;

fn contribution(
    id: usize,
    ordinary: i64,
    calls: &RefCell<Vec<usize>>,
    failure: &Cell<u8>,
) -> SimpleScore {
    calls.borrow_mut().push(id);
    assert!(
        failure.get() != 1 || id != FAIL_AT,
        "injected evaluator panic"
    );
    SimpleScore::new(ordinary + id as i64)
        .with_structural(i64::from(failure.get() == 2 && id == FAIL_AT))
}

fn basic() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); VARIABLES]).unwrap()
}
fn basic_move() -> Move<i32> {
    Move::Composite(
        (0..VARIABLES)
            .flat_map(|variable| {
                [
                    Move::Change {
                        variable,
                        value: Some(1),
                    },
                    Move::Change {
                        variable,
                        value: if variable.is_multiple_of(2) {
                            None
                        } else {
                            Some(1)
                        },
                    },
                ]
            })
            .collect(),
    )
}
fn lists() -> ListProblem<i32> {
    ListProblem::new_indexed(
        (0..VARIABLES as i32).collect(),
        (0..VARIABLES)
            .map(|i| ListEntity::new(vec![i as i32]))
            .collect(),
    )
    .unwrap()
}
fn list_move() -> ListMove<i32> {
    ListMove::Composite(
        (0..VARIABLES)
            .map(|entity| ListMove::Unassign {
                entity,
                start: 0,
                end: 1,
            })
            .collect(),
    )
}
fn list_fact(fact: &ListFact<'_, i32>) -> i64 {
    match fact {
        ListFact::Entity(row) => row.iter().map(|v| i64::from(*v)).sum(),
        ListFact::Value { element, .. } => {
            element.as_ref().map_or(-3, |v| v.position.entity as i64)
        }
    }
}

// Each director is exercised through the same public transaction contract. The
// fresh calculation builds its own affected set by evaluating the full network.
macro_rules! exercise {
    ($director:ident, $problem:ident, $network:ident, $movement:ident, $calls:ident, $failure:ident) => {{
        let original = $director.assignment().clone();
        let original_score = $director.score();
        let expected: Vec<_> = (0..CONSTRAINTS).filter(|i| !i.is_multiple_of(13)).collect();
        $calls.borrow_mut().clear();
        let trial = $director.apply(&$movement).unwrap();
        assert_eq!(*$calls.borrow(), expected);
        assert_eq!(trial.constraint_evaluations(), expected.len());
        assert_eq!(
            trial.score(),
            $network.calculate(&$problem, trial.assignment()).unwrap()
        );
        drop(trial);
        assert_eq!($director.assignment(), &original);
        assert_eq!($director.score(), original_score);
        for failure in [2, 1] {
            $calls.borrow_mut().clear();
            $failure.set(failure);
            if failure == 1 {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| {
                        let _ = $director.apply(&$movement);
                    }))
                    .is_err()
                );
            } else {
                assert!($director.apply(&$movement).is_err());
            }
            assert_eq!(
                *$calls.borrow(),
                expected
                    .iter()
                    .copied()
                    .filter(|id| *id <= FAIL_AT)
                    .collect::<Vec<_>>()
            );
            assert_eq!($director.assignment(), &original);
            assert_eq!($director.score(), original_score);
            $failure.set(0);
            $director.assert_consistent().unwrap();
        }
        $calls.borrow_mut().clear();
        $director.apply(&$movement).unwrap().commit();
        assert_eq!(*$calls.borrow(), expected);
        $director.assert_consistent().unwrap();
    }};
}

#[test]
fn basic_overlap_preserves_order_filtering_and_transaction_failures() {
    let problem = basic();
    let calls = RefCell::new(Vec::new());
    let failure = Cell::new(0);
    let mut network = ConstraintNetwork::new(VARIABLES);
    for id in 0..CONSTRAINTS {
        let scope = vec![0, 1 + id % (VARIABLES - 1), 0];
        if id.is_multiple_of(13) {
            network
                .add_weighted_constraint(format!("disabled {id}"), scope, SimpleScore::ZERO, |_| {
                    panic!("zero weight callback")
                })
                .unwrap();
        } else {
            let (calls, failure) = (&calls, &failure);
            network
                .add_constraint(format!("active {id}"), scope, move |facts| {
                    assert_eq!(facts.len(), 3);
                    assert_eq!(facts[0], facts[2]);
                    let ordinary = facts
                        .iter()
                        .enumerate()
                        .map(|(i, v)| (i as i64 + 1) * v.map_or(-7, |v| i64::from(*v)))
                        .sum();
                    contribution(id, ordinary, calls, failure)
                })
                .unwrap();
        }
    }
    let mut director =
        ScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    let movement = basic_move();
    exercise!(director, problem, network, movement, calls, failure);
}

#[test]
fn list_entity_and_value_overlap_preserves_order_and_zero_weights() {
    let problem = lists();
    let calls = RefCell::new(Vec::new());
    let failure = Cell::new(0);
    let mut network = ListConstraintNetwork::new(VARIABLES, VARIABLES);
    for id in 0..CONSTRAINTS {
        let scope = vec![
            ListScope::Entity(0),
            ListScope::Entity(1 + id % (VARIABLES - 1)),
            ListScope::Value(id % VARIABLES),
            ListScope::Entity(0),
        ];
        if id.is_multiple_of(13) {
            network
                .add_weighted_constraint(format!("disabled {id}"), scope, SimpleScore::ZERO, |_| {
                    panic!("zero weight callback")
                })
                .unwrap();
        } else {
            let (calls, failure) = (&calls, &failure);
            network
                .add_constraint(format!("active {id}"), scope, move |facts| {
                    assert_eq!(facts.len(), 4);
                    assert_eq!(list_fact(&facts[0]), list_fact(&facts[3]));
                    let ordinary = facts
                        .iter()
                        .enumerate()
                        .map(|(i, f)| (i as i64 + 1) * list_fact(f))
                        .sum();
                    contribution(id, ordinary, calls, failure)
                })
                .unwrap();
        }
    }
    let mut director =
        ListScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    let movement = list_move();
    exercise!(director, problem, network, movement, calls, failure);
}

#[test]
fn mixed_basic_entity_and_value_overlap_preserves_evaluation_order() {
    let problem = MixedProblem::new(basic(), lists());
    let calls = RefCell::new(Vec::new());
    let failure = Cell::new(0);
    let mut network = MixedConstraintNetwork::new(VARIABLES, VARIABLES, VARIABLES);
    for id in 0..CONSTRAINTS {
        let scope = vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1 + id % (VARIABLES - 1)),
            MixedScope::ListEntity(0),
            MixedScope::ListEntity(1 + id % (VARIABLES - 1)),
            MixedScope::ListValue(id % VARIABLES),
            MixedScope::Basic(0),
        ];
        if id.is_multiple_of(13) {
            network
                .add_weighted_constraint(format!("disabled {id}"), scope, SimpleScore::ZERO, |_| {
                    panic!("zero weight callback")
                })
                .unwrap();
        } else {
            let (calls, failure) = (&calls, &failure);
            network
                .add_constraint(format!("active {id}"), scope, move |facts| {
                    assert_eq!(facts.len(), 6);
                    let ordinary = facts
                        .iter()
                        .enumerate()
                        .map(|(i, f)| {
                            (i as i64 + 1)
                                * match f {
                                    MixedFact::Basic(value) => value.map_or(-7, |v| i64::from(*v)),
                                    MixedFact::ListEntity(row) => {
                                        row.iter().map(|v| i64::from(*v)).sum()
                                    }
                                    MixedFact::ListValue { element, .. } => {
                                        element.as_ref().map_or(-3, |v| v.position.entity as i64)
                                    }
                                }
                        })
                        .sum();
                    contribution(id, ordinary, calls, failure)
                })
                .unwrap();
        }
    }
    let mut director =
        MixedScoreDirector::new(&problem, &network, problem.initial_assignment()).unwrap();
    let movement = MixedMove {
        basic: basic_move(),
        lists: list_move(),
    };
    exercise!(director, problem, network, movement, calls, failure);
}
