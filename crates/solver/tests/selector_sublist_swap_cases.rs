// Adapted from Community SelectorBasedSubListSwapMoveTest, Apache-2.0.
// See compat/provenance.json. Stable integer IDs replace entity/value objects.
use timetuck::*;
fn swap(left: (usize, usize, usize), right: (usize, usize, usize), reverse: bool) -> ListMove<i32> {
    ListMove::SubListSwap {
        left: ListRange {
            entity: left.0,
            start: left.1,
            end: left.2,
        },
        right: ListRange {
            entity: right.0,
            start: right.1,
            end: right.2,
        },
        reverse,
    }
}
fn eligible(
    movement: &ListMove<i32>,
    assignment: &ListAssignment<i32>,
    ranges: Option<&[Vec<i32>]>,
) -> Result<bool, ListError> {
    let ListMove::SubListSwap { left, right, .. } = movement else {
        unreachable!()
    };
    if let Some(ranges) = ranges {
        left.selector_swap_is_doable_with_ranges(*right, assignment, |entity, value| {
            ranges[entity].contains(value)
        })
    } else {
        Ok(left.selector_swap_is_doable(*right))
    }
}
#[test]
fn is_move_doable() {
    let assignment = ListAssignment::new(vec![vec![1, 2, 3, 4], vec![5]]);
    for (left, right, expected) in [
        ((0, 0, 3), (0, 2, 5), false),
        ((0, 0, 5), (0, 1, 2), false),
        ((0, 0, 1), (0, 4, 5), true),
        ((0, 0, 3), (0, 3, 5), true),
        ((0, 2, 5), (0, 0, 2), true),
        ((0, 0, 5), (1, 0, 1), true),
    ] {
        assert_eq!(
            eligible(&swap(left, right, false), &assignment, None).unwrap(),
            expected
        );
    }
    assert_eq!(assignment.lists(), &[vec![1, 2, 3, 4], vec![5]]);
}
#[test]
fn is_move_doable_value_range_provider_on_entity() {
    let ranges = vec![vec![1, 2, 3], vec![1, 3, 4, 5]];
    let assignment = ListAssignment::new(vec![vec![1, 4, 2], vec![3, 5]]);
    for reverse in [false, true] {
        assert!(
            eligible(
                &swap((0, 0, 2), (1, 0, 1), reverse),
                &assignment,
                Some(&ranges)
            )
            .unwrap()
        );
        assert!(
            !eligible(
                &swap((0, 0, 3), (1, 0, 1), reverse),
                &assignment,
                Some(&ranges)
            )
            .unwrap()
        );
        assert!(
            !eligible(
                &swap((0, 0, 2), (1, 0, 2), reverse),
                &assignment,
                Some(&ranges)
            )
            .unwrap()
        );
    }
    assert_eq!(assignment.lists(), &[vec![1, 4, 2], vec![3, 5]]);
}
