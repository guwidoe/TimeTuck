// Complete adapted pinned Community list-move diagnostics/inspection/lifecycle TESTS.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;
#[derive(Debug, Eq, PartialEq)]
struct Value {
    code: i32,
}
fn values(n: i32) -> Vec<Arc<Value>> {
    (1..=n).map(|code| Arc::new(Value { code })).collect()
}
fn assignment(v: &[Arc<Value>], rows: &[&[usize]]) -> ListAssignment<Arc<Value>> {
    ListAssignment::new(
        rows.iter()
            .map(|r| r.iter().map(|&i| v[i - 1].clone()).collect())
            .collect(),
    )
}
fn p(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn r(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn describe(m: &ListMove<Arc<Value>>, a: &ListAssignment<Arc<Value>>, labels: &[&str]) -> String {
    describe_list_move(m, a, |i| labels[i].to_owned(), |v| v.code.to_string()).unwrap()
}
#[test]
fn list_assign_to_string() {
    let v = values(1);
    let a = assignment(&v, &[&[]]);
    assert_eq!(
        describe(
            &ListMove::Assign {
                value: v[0].clone(),
                destination: p(0, 15)
            },
            &a,
            &["E1"]
        ),
        "1 {null -> E1[15]}"
    );
}
#[test]
fn list_change_to_string() {
    let v = values(3);
    let a = assignment(&v, &[&[1, 2], &[3]]);
    assert_eq!(
        describe(
            &ListMove::Change {
                source: p(0, 1),
                destination: p(0, 0)
            },
            &a,
            &["e1", "e2"]
        ),
        "2 {e1[1] -> e1[0]}"
    );
    assert_eq!(
        describe(
            &ListMove::Change {
                source: p(0, 0),
                destination: p(1, 1)
            },
            &a,
            &["e1", "e2"]
        ),
        "1 {e1[0] -> e2[1]}"
    );
}
#[test]
fn list_swap_to_string() {
    let v = values(3);
    let a = assignment(&v, &[&[1, 2], &[3]]);
    assert_eq!(
        describe(
            &ListMove::Swap {
                left: p(0, 0),
                right: p(0, 1)
            },
            &a,
            &["e1", "e2"]
        ),
        "1 {e1[0]} <-> 2 {e1[1]}"
    );
    assert_eq!(
        describe(
            &ListMove::Swap {
                left: p(0, 1),
                right: p(1, 0)
            },
            &a,
            &["e1", "e2"]
        ),
        "2 {e1[1]} <-> 3 {e2[0]}"
    );
}
#[test]
fn list_unassign_to_string() {
    let v = values(1);
    let a = assignment(&v, &[&[1]]);
    assert_eq!(
        describe(
            &ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1
            },
            &a,
            &["E1"]
        ),
        "1 {E1[0] -> null}"
    );
}
#[test]
fn sub_list_change_to_string() {
    let v = values(5);
    let a = assignment(&v, &[&[1, 2, 3, 4], &[5]]);
    assert_eq!(
        describe(
            &ListMove::SubListChange {
                entity: 0,
                start: 1,
                end: 4,
                destination: p(0, 0),
                reverse: false
            },
            &a,
            &["e1", "e2"]
        ),
        "|3| {e1[1..4] -> e1[0]}"
    );
    for reverse in [false, true] {
        assert_eq!(
            describe(
                &ListMove::SubListChange {
                    entity: 0,
                    start: 0,
                    end: 1,
                    destination: p(1, 1),
                    reverse
                },
                &a,
                &["e1", "e2"]
            ),
            if reverse {
                "|1| {e1[0..1] -reversing-> e2[1]}"
            } else {
                "|1| {e1[0..1] -> e2[1]}"
            }
        );
    }
}
#[test]
fn sub_list_swap_to_string() {
    let a = ListAssignment::<Arc<Value>>::new(vec![vec![], vec![]]);
    assert_eq!(
        describe(
            &ListMove::SubListSwap {
                left: r(0, 1, 4),
                right: r(0, 0, 1),
                reverse: false
            },
            &a,
            &["e1", "e2"]
        ),
        "{e1[0..1]} <-> {e1[1..4]}"
    );
    for reverse in [false, true] {
        assert_eq!(
            describe(
                &ListMove::SubListSwap {
                    left: r(0, 0, 1),
                    right: r(1, 1, 6),
                    reverse
                },
                &a,
                &["e1", "e2"]
            ),
            if reverse {
                "{e1[0..1]} <-reversing-> {e2[1..6]}"
            } else {
                "{e1[0..1]} <-> {e2[1..6]}"
            }
        );
    }
}
fn problem(v: &[Arc<Value>], rows: &[&[usize]]) -> ListProblem<Arc<Value>> {
    ListProblem::new(
        v.to_vec(),
        assignment(v, rows)
            .lists()
            .iter()
            .cloned()
            .map(ListEntity::new)
            .collect(),
    )
    .unwrap()
}
#[test]
fn list_swap_tabu_introspection_one_entity() {
    let v = values(2);
    let problem = problem(&v, &[&[1, 2]]);
    let mut a = problem.initial_assignment();
    let record = RecordedListMove::prepare(
        &problem,
        &a,
        ListMove::Swap {
            left: p(0, 0),
            right: p(0, 1),
        },
    )
    .unwrap();
    problem.apply(&mut a, record.movement()).unwrap().commit();
    assert_eq!(record.planning_entities(), &[0]);
    let actual = record.planning_values(&a).unwrap();
    assert_eq!(
        actual.iter().map(|v| v.code).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert!(Arc::ptr_eq(actual[0], &v[1]));
    assert!(Arc::ptr_eq(actual[1], &v[0]));
}
#[test]
fn sub_list_swap_tabu_introspection_one_entity() {
    let v = values(4);
    let problem = problem(&v, &[&[1, 2, 3, 4]]);
    let before = problem.initial_assignment();
    let mut a = before.clone();
    let record = RecordedListMove::prepare(
        &problem,
        &before,
        ListMove::SubListSwap {
            left: r(0, 3, 4),
            right: r(0, 0, 2),
            reverse: false,
        },
    )
    .unwrap();
    problem.apply(&mut a, record.movement()).unwrap().commit();
    assert_eq!(record.planning_entities(), &[0]);
    let actual = record.planning_values(&a).unwrap();
    assert_eq!(
        actual.iter().map(|v| v.code).collect::<Vec<_>>(),
        vec![1, 2, 4]
    );
    for (actual, expected) in actual.iter().zip([0, 1, 3]) {
        assert!(Arc::ptr_eq(actual, &v[expected]));
    }
    let normalized = RecordedListMove::prepare(
        &problem,
        &a,
        ListMove::SubListSwap {
            left: r(0, 0, 2),
            right: r(0, 3, 4),
            reverse: false,
        },
    )
    .unwrap();
    assert_eq!(record, normalized);
}
#[test]
fn list_unassign_do_move() {
    let v = values(3);
    let problem = problem(&v, &[&[1, 2, 3]]);
    let mut a = problem.initial_assignment();
    let plan = ListLifecyclePlan::prepare(
        &problem,
        &a,
        ListMove::Unassign {
            entity: 0,
            start: 2,
            end: 3,
        },
    )
    .unwrap();
    let mut events = vec![];
    let mut flushes = 0;
    plan.apply(&mut a, &mut |e: ListLifecycleEvent<'_, Arc<Value>>| {
        assert!(!e.undo);
        match e.change {
            ListLifecycleChange::Changed(range) => events.push((e.phase, Some(range), None)),
            ListLifecycleChange::Unassigned(value) => {
                assert!(Arc::ptr_eq(value, &v[2]));
                events.push((e.phase, None, Some(value.code)));
            }
            ListLifecycleChange::Flush => flushes += 1,
            _ => panic!("unexpected assignment callback"),
        }
    })
    .unwrap()
    .commit();
    assert_eq!(a.lists()[0], v[..2]);
    assert_eq!(events.len(), 4);
    for expected in [
        (ListChangePhase::Before, Some(r(0, 2, 3)), None),
        (ListChangePhase::Before, None, Some(3)),
        (ListChangePhase::After, None, Some(3)),
        (ListChangePhase::After, Some(r(0, 2, 2)), None),
    ] {
        assert!(events.contains(&expected));
    }
    assert_eq!(flushes, 1); // Native flush is explicit and additional to source callbacks.
    for _ in 0..2 {
        let plan = ListLifecyclePlan::prepare(
            &problem,
            &a,
            ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1,
            },
        )
        .unwrap();
        plan.apply(&mut a, &mut |_: ListLifecycleEvent<'_, Arc<Value>>| {})
            .unwrap()
            .commit();
    }
    assert!(a.lists()[0].is_empty());
}
