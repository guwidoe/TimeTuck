// Adapted from pinned Community list and sublist move doability tests.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
// Native checked moves expose invalid candidates as errors. For upstream boolean
// doability assertions, both an invalid candidate and an unchanged one are false.
fn doable(p: &ListProblem<i32>, movement: ListMove<i32>) -> bool {
    p.is_move_doable(&p.initial_assignment(), &movement) == Ok(true)
}
#[test]
fn list_change_doability() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    for (source, destination, expected) in [
        (pos(0, 1), pos(0, 1), false),
        (pos(0, 0), pos(0, 1), true),
        (pos(0, 0), pos(0, 2), false),
        (pos(0, 0), pos(1, 0), true),
    ] {
        assert_eq!(
            doable(
                &p,
                ListMove::Change {
                    source,
                    destination
                }
            ),
            expected
        );
    }
}
#[test]
fn list_swap_doability() {
    let p = ListProblem::new(
        vec![1, 2, 3],
        vec![ListEntity::new(vec![1, 2]), ListEntity::new(vec![3])],
    )
    .unwrap();
    for (left, right, expected) in [
        (pos(0, 1), pos(0, 1), false),
        (pos(0, 0), pos(0, 1), true),
        (pos(0, 0), pos(1, 0), true),
    ] {
        assert_eq!(doable(&p, ListMove::Swap { left, right }), expected);
    }
}
#[test]
fn list_swap_entity_ranges() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 2]).with_value_range(vec![1, 2, 3]),
            ListEntity::new(vec![3, 4]).with_value_range(vec![1, 3, 4]),
        ],
    )
    .unwrap();
    assert!(doable(
        &p,
        ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0)
        }
    ));
}
#[test]
fn sublist_change_doability() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4, 5],
        vec![ListEntity::new(vec![1, 2, 3, 4]), ListEntity::new(vec![5])],
    )
    .unwrap();
    for (start, length, destination, expected) in [
        (1, 2, pos(0, 1), false),
        (1, 2, pos(0, 0), true),
        (0, 3, pos(0, 1), true),
        (0, 3, pos(0, 2), false),
        (1, 2, pos(1, 0), true),
    ] {
        assert_eq!(
            doable(
                &p,
                ListMove::SubListChange {
                    entity: 0,
                    start,
                    end: start + length,
                    destination,
                    reverse: false
                }
            ),
            expected
        );
    }
}
#[test]
fn sublist_change_entity_ranges() {
    let p = ListProblem::new(
        vec![1, 2, 3, 4],
        vec![
            ListEntity::new(vec![1, 4, 2]).with_value_range(vec![1, 2, 4]),
            ListEntity::new(vec![3]).with_value_range(vec![1, 3, 4]),
        ],
    )
    .unwrap();
    for (start, length, expected) in [(0, 2, true), (0, 3, false), (1, 2, false)] {
        for reverse in [false, true] {
            assert_eq!(
                doable(
                    &p,
                    ListMove::SubListChange {
                        entity: 0,
                        start,
                        end: start + length,
                        destination: pos(1, 0),
                        reverse
                    }
                ),
                expected
            );
        }
    }
}
