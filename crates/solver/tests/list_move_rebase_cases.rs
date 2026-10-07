// Adapted pinned Community selector-based list-move rebase cases. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::rc::Rc;
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn owner(entity: usize) -> Result<usize, ()> {
    Ok(1 - entity)
}
fn destination_state() -> ([Rc<String>; 3], ListAssignment<Rc<String>>) {
    let values: [Rc<String>; 3] = [
        Rc::new("1".into()),
        Rc::new("2".into()),
        Rc::new("3".into()),
    ];
    let a = ListAssignment::new(vec![
        vec![values[2].clone()],
        vec![values[0].clone(), values[1].clone()],
    ]);
    (values, a)
}
#[test]
fn assign_rebase() {
    let value = Rc::new("1".to_string());
    let m = ListMove::Assign {
        value: 1u8,
        destination: pos(0, 0),
    };
    let ListMove::Assign {
        value: actual,
        destination,
    } = rebase_list_move(&m, owner, |_| Ok(value.clone())).unwrap()
    else {
        panic!("expected assignment")
    };
    assert!(Rc::ptr_eq(&actual, &value));
    assert_eq!(destination, pos(1, 0));
}
#[test]
fn unassign_rebase() {
    let source: Box<ListMove<u8>> = Box::new(ListMove::Unassign {
        entity: 0,
        start: 0,
        end: 1,
    });
    let result = rebase_list_move(&source, owner, |v| Ok(*v)).unwrap();
    assert!(!std::ptr::eq(source.as_ref(), &result));
    assert_eq!(
        result,
        ListMove::Unassign {
            entity: 1,
            start: 0,
            end: 1
        }
    );
    assert_eq!(
        *source,
        ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 1
        }
    );
}
#[test]
fn change_rebase() {
    let (values, a) = destination_state();
    for (source, destination, expected_source, expected_destination, value) in [
        (pos(0, 0), pos(1, 1), pos(1, 0), pos(0, 1), 0),
        (pos(1, 0), pos(1, 0), pos(0, 0), pos(0, 0), 2),
    ] {
        let movement: ListMove<u8> = ListMove::Change {
            source,
            destination,
        };
        let ListMove::Change {
            source,
            destination,
        } = rebase_list_move(&movement, owner, |v| Ok(*v)).unwrap()
        else {
            panic!("expected change")
        };
        assert_eq!(source, expected_source);
        assert_eq!(destination, expected_destination);
        assert!(Rc::ptr_eq(
            &a.list(source.entity).unwrap()[source.index],
            &values[value]
        ));
    }
}
#[test]
fn swap_rebase() {
    let (values, a) = destination_state();
    for (left, right, expected_left, expected_right, left_value, right_value) in [
        (pos(0, 1), pos(1, 0), pos(1, 1), pos(0, 0), 1, 2),
        (pos(0, 0), pos(0, 1), pos(1, 0), pos(1, 1), 0, 1),
    ] {
        let movement: ListMove<u8> = ListMove::Swap { left, right };
        let ListMove::Swap { left, right } =
            rebase_list_move(&movement, owner, |v| Ok(*v)).unwrap()
        else {
            panic!("expected swap")
        };
        assert_eq!(left, expected_left);
        assert_eq!(right, expected_right);
        assert!(Rc::ptr_eq(
            &a.list(left.entity).unwrap()[left.index],
            &values[left_value]
        ));
        assert!(Rc::ptr_eq(
            &a.list(right.entity).unwrap()[right.index],
            &values[right_value]
        ));
    }
}
#[test]
fn sublist_change_rebase() {
    let movement: ListMove<u8> = ListMove::SubListChange {
        entity: 0,
        start: 3,
        end: 8,
        destination: pos(1, 7),
        reverse: false,
    };
    let ListMove::SubListChange {
        entity,
        start,
        end,
        destination,
        reverse,
    } = rebase_list_move(&movement, owner, |v| Ok(*v)).unwrap()
    else {
        panic!("expected sublist change")
    };
    assert_eq!(entity, 1);
    assert_eq!(start, 3);
    assert_eq!(end - start, 5);
    assert_eq!(destination, pos(0, 7));
    assert!(!reverse);
}
#[test]
fn sublist_swap_rebase() {
    let movement: ListMove<u8> = ListMove::SubListSwap {
        left: ListRange {
            entity: 0,
            start: 7,
            end: 11,
        },
        right: ListRange {
            entity: 1,
            start: 3,
            end: 9,
        },
        reverse: false,
    };
    let ListMove::SubListSwap {
        left,
        right,
        reverse,
    } = rebase_list_move(&movement, owner, |v| Ok(*v)).unwrap()
    else {
        panic!("expected sublist swap")
    };
    assert_eq!(
        left,
        ListRange {
            entity: 1,
            start: 7,
            end: 11
        }
    );
    assert_eq!(
        right,
        ListRange {
            entity: 0,
            start: 3,
            end: 9
        }
    );
    assert!(!reverse);
}
