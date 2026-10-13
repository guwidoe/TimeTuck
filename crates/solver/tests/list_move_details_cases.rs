// Complete adaptations of pinned Community TESTS; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    any::Any,
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use timetuck::*;
fn p(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn r(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn hash(m: &ListMove<&str>) -> u64 {
    let mut h = DefaultHasher::new();
    m.hash(&mut h);
    h.finish()
}
// Native nullable/dynamic boundary preserves the source null/other-type assertions.
fn dynamic_equal(left: &ListMove<&'static str>, right: Option<&dyn Any>) -> bool {
    right
        .and_then(|v| v.downcast_ref::<ListMove<&'static str>>())
        .is_some_and(|v| left == v)
}
fn equal_pair(a: &ListMove<&'static str>, b: &ListMove<&'static str>) {
    assert_eq!(a, b);
    assert_eq!(hash(a), hash(b));
    assert!(!dynamic_equal(a, None));
    assert!(!dynamic_equal(a, Some(&"not a move")));
}
#[test]
fn list_change_equals_and_hash_code() {
    let a = ListMove::Change {
        source: p(0, 0),
        destination: p(1, 0),
    };
    let b = ListMove::Change {
        source: p(0, 0),
        destination: p(1, 0),
    };
    equal_pair(&a, &b);
    assert_ne!(
        a,
        ListMove::Change {
            source: p(0, 0),
            destination: p(1, 1)
        }
    );
    assert_ne!(
        a,
        ListMove::Change {
            source: p(1, 0),
            destination: p(0, 0)
        }
    );
}
#[test]
fn sub_list_change_equals_and_hash_code() {
    let make = |reverse| ListMove::SubListChange {
        entity: 0,
        start: 0,
        end: 1,
        destination: p(1, 0),
        reverse,
    };
    equal_pair(&make(false), &make(false));
    assert_ne!(make(false), make(true));
}
#[test]
fn sub_list_swap_equals_and_hash_code() {
    let make = |reverse| ListMove::SubListSwap {
        left: r(0, 0, 1),
        right: r(1, 0, 1),
        reverse,
    };
    equal_pair(&make(false), &make(false));
    assert_ne!(make(false), make(true));
}
#[test]
fn sub_list_unassign_equals_and_hash_code() {
    let make = |end| ListMove::Unassign {
        entity: 0,
        start: 0,
        end,
    };
    equal_pair(&make(1), &make(1));
    assert_ne!(make(1), make(2));
}
#[test]
fn list_change_to_string_contains_move_details() {
    let problem = ListProblem::new(
        vec!["A", "B"],
        vec![ListEntity::new(vec!["A"]), ListEntity::new(vec!["B"])],
    )
    .unwrap();
    let mut current = problem.initial_assignment();
    let record = RecordedListMove::prepare(
        &problem,
        &current,
        ListMove::Change {
            source: p(0, 0),
            destination: p(1, 1),
        },
    )
    .unwrap();
    problem
        .apply(&mut current, record.movement())
        .unwrap()
        .commit();
    assert!(current.lists()[0].is_empty());
    let text = record
        .describe(
            &current,
            |i| ["Entity1", "Entity2"][i].to_owned(),
            |v| v.to_string(),
        )
        .unwrap();
    for detail in ["Entity1", "Entity2", "0", "1"] {
        assert!(text.contains(detail));
    }
    assert_eq!(text, "A {Entity1[0] -> Entity2[1]}");
}
#[test]
fn sub_list_change_to_string_contains_move_details() {
    let current = ListAssignment::new(vec![vec!["A", "B"], vec!["C"]]);
    let movement = ListMove::SubListChange {
        entity: 0,
        start: 0,
        end: 2,
        destination: p(1, 1),
        reverse: true,
    };
    let text = describe_list_move(
        &movement,
        &current,
        |i| ["EntityA", "EntityB"][i].to_owned(),
        |v| v.to_string(),
    )
    .unwrap();
    for detail in ["EntityA", "EntityB", "reversing-"] {
        assert!(text.contains(detail));
    }
}
#[test]
fn sub_list_swap_to_string_contains_move_details() {
    let current = ListAssignment::new(vec![vec!["A"], vec!["B"]]);
    let movement = ListMove::SubListSwap {
        left: r(0, 0, 1),
        right: r(1, 0, 1),
        reverse: true,
    };
    let text = describe_list_move(
        &movement,
        &current,
        |i| ["EntityA", "EntityB"][i].to_owned(),
        |v| v.to_string(),
    )
    .unwrap();
    for detail in ["EntityA", "EntityB", "reversing-"] {
        assert!(text.contains(detail));
    }
}
