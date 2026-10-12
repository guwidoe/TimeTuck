//! Original move description boundaries, immutability and retained-record checks.
use std::cell::{Cell, RefCell};
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn range(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
#[test]
fn diagnostic_formatting_borrows_nonclone_noneq_values_and_preserves_unicode_labels() {
    struct Value(&'static str);
    let a = ListAssignment::new(vec![vec![Value("α{1}"), Value("β[2]")], vec![]]);
    let calls = RefCell::new(vec![]);
    let described = describe_list_move(
        &ListMove::Change {
            source: pos(0, 1),
            destination: pos(1, usize::MAX),
        },
        &a,
        |e| {
            calls.borrow_mut().push(e);
            if e == 0 {
                "row[左]".into()
            } else {
                "row{右}".into()
            }
        },
        |v| v.0.to_owned(),
    )
    .unwrap();
    assert_eq!(
        described,
        format!("β[2] {{row[左][1] -> row{{右}}[{}]}}", usize::MAX)
    );
    assert_eq!(*calls.borrow(), vec![0, 1]);
    assert_eq!(a.lists()[0][0].0, "α{1}");
    assert_eq!(a.lists()[0][1].0, "β[2]");
    let text = describe_list_move(
        &ListMove::SubListChange {
            entity: 0,
            start: 0,
            end: 2,
            destination: pos(1, 0),
            reverse: true,
        },
        &a,
        |_| "列表[x]".into(),
        |_| panic!("span formatting does not read values"),
    )
    .unwrap();
    assert_eq!(text, "|2| {列表[x][0..2] -reversing-> 列表[x][0]}");
}
#[test]
fn missing_value_malformed_span_and_composite_reject_without_mutating_source() {
    let a = ListAssignment::new(vec![vec![1_i32]]);
    let original = a.clone();
    let labels = Cell::new(0);
    let describe = |m: &ListMove<i32>| {
        describe_list_move(
            m,
            &a,
            |e| {
                labels.set(labels.get() + 1);
                e.to_string()
            },
            i32::to_string,
        )
    };
    assert!(matches!(
        describe(&ListMove::Change {
            source: pos(0, 1),
            destination: pos(0, 0)
        }),
        Err(ListMoveDescriptionError::List(ListError::ElementIndex(_)))
    ));
    assert!(matches!(
        describe(&ListMove::Swap {
            left: pos(2, 0),
            right: pos(0, 0)
        }),
        Err(ListMoveDescriptionError::List(
            ListError::EntityIndex { .. }
        ))
    ));
    assert!(matches!(
        describe(&ListMove::SubListChange {
            entity: 0,
            start: 2,
            end: 1,
            destination: pos(0, 0),
            reverse: false
        }),
        Err(ListMoveDescriptionError::InvalidSpan(_))
    ));
    assert!(matches!(
        describe(&ListMove::SubListSwap {
            left: range(0, 2, 1),
            right: range(0, 0, 1),
            reverse: false
        }),
        Err(ListMoveDescriptionError::InvalidSpan(_))
    ));
    assert!(matches!(
        describe(&ListMove::Unassign {
            entity: 0,
            start: 0,
            end: usize::MAX
        }),
        Err(ListMoveDescriptionError::InvalidSpan(_))
    ));
    assert_eq!(
        describe(&ListMove::Composite(vec![])),
        Err(ListMoveDescriptionError::Composite)
    );
    assert_eq!(labels.get(), 0);
    assert_eq!(a, original);
}
#[test]
fn records_normalize_nested_swaps_keep_values_and_compare_movement_only() {
    let p = ListProblem::new(vec![1, 2, 3, 4], vec![ListEntity::new(vec![1, 2, 3, 4])]).unwrap();
    let before = p.initial_assignment();
    let mut a = before.clone();
    let forward = ListMove::SubListSwap {
        left: range(0, 0, 2),
        right: range(0, 3, 4),
        reverse: true,
    };
    let reversed = ListMove::SubListSwap {
        left: range(0, 3, 4),
        right: range(0, 0, 2),
        reverse: true,
    };
    let record = RecordedListMove::prepare(
        &p,
        &before,
        ListMove::Composite(vec![ListMove::Composite(vec![reversed])]),
    )
    .unwrap();
    let canonical = RecordedListMove::prepare(
        &p,
        &before,
        ListMove::Composite(vec![ListMove::Composite(vec![forward])]),
    )
    .unwrap();
    assert_eq!(record, canonical);
    assert_eq!(record.planning_entities(), &[0]);
    p.apply(&mut a, record.movement()).unwrap().commit();
    assert_eq!(a.lists(), &[vec![4, 3, 2, 1]]);
    assert_eq!(record.planning_values(&a).unwrap(), vec![&1, &2, &4]);
    let current_record = RecordedListMove::prepare(&p, &a, canonical.movement().clone()).unwrap();
    assert_eq!(record, current_record);
    assert_ne!(
        record.planning_values(&a).unwrap(),
        current_record.planning_values(&a).unwrap()
    );
    assert_eq!(before, p.initial_assignment());
}
#[test]
fn record_preparation_retains_model_checks_and_live_swap_inspection_checks_positions() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1, 2]).with_pinned_prefix(1)],
    )
    .unwrap();
    let before = p.initial_assignment();
    assert!(
        RecordedListMove::prepare(
            &p,
            &before,
            ListMove::Swap {
                left: pos(0, 0),
                right: pos(0, 1)
            }
        )
        .is_err()
    );
    assert_eq!(before, p.initial_assignment());
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1, 2])]).unwrap();
    let before = p.initial_assignment();
    let record = RecordedListMove::prepare(
        &p,
        &before,
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(0, 1),
        },
    )
    .unwrap();
    assert_eq!(record.planning_values(&before).unwrap(), vec![&1, &2]);
    assert!(matches!(
        record.planning_values(&ListAssignment::new(vec![vec![]])),
        Err(ListError::ElementIndex(_))
    ));
    assert_eq!(
        p.planning_values(&before, record.movement()).unwrap(),
        vec![1, 2]
    );
}
