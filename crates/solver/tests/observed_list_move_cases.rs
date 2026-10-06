// Adapted from Community MoveDirectorTest, Apache-2.0; compat/provenance.json.
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn span(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
#[derive(Clone)]
enum Edit {
    Raw(ListMove<u8>),
    Move(ListPosition, ListPosition),
    Shift(ListPosition, isize),
    Span(ListRange, ListPosition, bool),
}
fn case(
    initial: Vec<Vec<u8>>,
    edit: Edit,
    moved: Vec<u8>,
    expected: Vec<Vec<u8>>,
    before: Vec<ListRange>,
    after: Vec<ListRange>,
) {
    let p = ListProblem::new(
        initial.iter().flatten().copied().collect(),
        initial.iter().cloned().map(ListEntity::new).collect(),
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let mut events = vec![];
    let mut observer = |e: ListChangeEvent<'_, u8>| {
        events.push((e.phase, e.undo, e.ranges.to_vec(), e.assignment.clone()))
    };
    let (actual, guard) = match edit {
        Edit::Raw(m) => (vec![], p.apply_observed(&mut a, &m, &mut observer).unwrap()),
        Edit::Move(from, to) => {
            let (v, g) = p
                .relocate_observed(&mut a, from, to, &mut observer)
                .unwrap();
            (vec![v], g)
        }
        Edit::Shift(from, offset) => {
            let (v, g) = p
                .shift_observed(&mut a, from, offset, &mut observer)
                .unwrap();
            (vec![v], g)
        }
        Edit::Span(from, to, reverse) => p
            .relocate_span_observed(&mut a, from, to, reverse, &mut observer)
            .unwrap(),
    };
    assert_eq!(actual, moved);
    assert_eq!(guard.lists(), expected);
    guard.rollback();
    assert_eq!(a.lists(), initial);
    assert_eq!(
        events,
        vec![
            (
                ListChangePhase::Before,
                false,
                before.clone(),
                ListAssignment::new(initial.clone())
            ),
            (
                ListChangePhase::After,
                false,
                after.clone(),
                ListAssignment::new(expected.clone())
            ),
            (
                ListChangePhase::Before,
                true,
                after,
                ListAssignment::new(expected)
            ),
            (
                ListChangePhase::After,
                true,
                before,
                ListAssignment::new(initial)
            ),
        ]
    );
}
fn within(
    initial: Vec<u8>,
    edit: Edit,
    moved: Vec<u8>,
    expected: Vec<u8>,
    start: usize,
    end: usize,
) {
    case(
        vec![initial],
        edit,
        moved,
        vec![expected],
        vec![span(0, start, end)],
        vec![span(0, start, end)],
    );
}
#[test]
fn move_value_in_list() {
    within(
        vec![1, 2, 3],
        Edit::Move(pos(0, 2), pos(0, 0)),
        vec![3],
        vec![3, 1, 2],
        0,
        3,
    );
    within(
        vec![1, 2, 3],
        Edit::Move(pos(0, 2), pos(0, 1)),
        vec![3],
        vec![1, 3, 2],
        1,
        3,
    );
}
#[test]
fn move_value_in_list_to_end() {
    within(
        vec![1, 2, 3],
        Edit::Move(pos(0, 0), pos(0, 2)),
        vec![1],
        vec![2, 3, 1],
        0,
        3,
    );
}
#[test]
fn shift_value_right() {
    within(
        vec![1, 2, 3, 4],
        Edit::Shift(pos(0, 1), 2),
        vec![2],
        vec![1, 3, 4, 2],
        1,
        4,
    );
}
#[test]
fn shift_value_right_by_one() {
    within(
        vec![1, 2, 3],
        Edit::Shift(pos(0, 0), 1),
        vec![1],
        vec![2, 1, 3],
        0,
        2,
    );
}
#[test]
fn shift_value_left() {
    within(
        vec![1, 2, 3, 4],
        Edit::Shift(pos(0, 2), -2),
        vec![3],
        vec![3, 1, 2, 4],
        0,
        3,
    );
}
#[test]
fn shift_value_left_by_one() {
    within(
        vec![1, 2, 3],
        Edit::Shift(pos(0, 2), -1),
        vec![3],
        vec![1, 3, 2],
        1,
        3,
    );
}
#[test]
fn shift_value_to_end() {
    within(
        vec![1, 2, 3],
        Edit::Shift(pos(0, 0), 2),
        vec![1],
        vec![2, 3, 1],
        0,
        3,
    );
}
#[test]
fn shift_value_zero_offset_fails() {
    let p = ListProblem::new(vec![1], vec![ListEntity::new(vec![1])]).unwrap();
    let mut a = p.initial_assignment();
    assert!(matches!(
        p.shift_observed(&mut a, pos(0, 0), 0, &mut |_| {}),
        Err(ListEditError::InvalidShift { offset: 0, .. })
    ));
}
#[test]
fn move_value_between_lists() {
    case(
        vec![vec![1, 2, 3], vec![4, 5, 6]],
        Edit::Move(pos(0, 1), pos(1, 2)),
        vec![2],
        vec![vec![1, 3], vec![4, 5, 2, 6]],
        vec![span(0, 1, 2), span(1, 2, 2)],
        vec![span(0, 1, 1), span(1, 2, 3)],
    );
}
#[test]
fn move_value_between_lists_to_end() {
    case(
        vec![vec![1, 2, 3], vec![4, 5, 6]],
        Edit::Move(pos(0, 1), pos(1, 3)),
        vec![2],
        vec![vec![1, 3], vec![4, 5, 6, 2]],
        vec![span(0, 1, 2), span(1, 3, 3)],
        vec![span(0, 1, 1), span(1, 3, 4)],
    );
}
#[test]
fn move_value_to_empty_list() {
    case(
        vec![vec![1, 2], vec![]],
        Edit::Move(pos(0, 0), pos(1, 0)),
        vec![1],
        vec![vec![2], vec![1]],
        vec![span(0, 0, 1), span(1, 0, 0)],
        vec![span(0, 0, 0), span(1, 0, 1)],
    );
}
#[test]
fn move_value_in_list_adjacent_swap() {
    within(
        vec![1, 2, 3],
        Edit::Move(pos(0, 0), pos(0, 1)),
        vec![1],
        vec![2, 1, 3],
        0,
        2,
    );
}
#[test]
fn move_value_in_list_rotate() {
    let initial: Vec<u8> = (1..=17).collect();
    let mut forward = vec![2, 3, 1];
    forward.extend(4..=17);
    within(
        initial.clone(),
        Edit::Move(pos(0, 0), pos(0, 2)),
        vec![1],
        forward,
        0,
        3,
    );
    let mut backward = vec![3, 1, 2];
    backward.extend(4..=17);
    within(
        initial,
        Edit::Move(pos(0, 2), pos(0, 0)),
        vec![3],
        backward,
        0,
        3,
    );
}
#[test]
fn move_values_in_list_forward() {
    within(
        vec![1, 2, 3, 4, 5],
        Edit::Span(span(0, 1, 3), pos(0, 3), false),
        vec![2, 3],
        vec![1, 4, 5, 2, 3],
        1,
        5,
    );
}
#[test]
fn move_values_in_list_backward_reversing() {
    within(
        vec![1, 2, 3, 4, 5],
        Edit::Span(span(0, 3, 5), pos(0, 0), true),
        vec![5, 4],
        vec![5, 4, 1, 2, 3],
        0,
        5,
    );
}

fn fails(initial: Vec<Vec<u8>>, edit: Edit, expected: ListError) {
    let p = ListProblem::new(
        initial.iter().flatten().copied().collect(),
        initial.iter().cloned().map(ListEntity::new).collect(),
    )
    .unwrap();
    let mut a = p.initial_assignment();
    let mut calls = 0;
    let mut observer = |_: ListChangeEvent<'_, u8>| calls += 1;
    let error = match edit {
        Edit::Span(from, to, reverse) => p
            .relocate_span_observed(&mut a, from, to, reverse, &mut observer)
            .err()
            .unwrap(),
        Edit::Raw(m) => p
            .apply_observed(&mut a, &m, &mut observer)
            .err()
            .unwrap()
            .into(),
        _ => panic!("unexpected failure fixture"),
    };
    assert_eq!(error, ListEditError::Model(expected));
    assert_eq!(calls, 0);
    assert_eq!(a.lists(), initial);
}
#[test]
fn move_values_in_list_throws_on_empty_span() {
    fails(
        vec![vec![1, 2]],
        Edit::Span(span(0, 1, 1), pos(0, 0), false),
        ListError::InvalidRange {
            entity: 0,
            start: 1,
            end: 1,
        },
    );
}
#[test]
fn move_values_in_list_throws_on_destination_index_out_of_bounds() {
    fails(
        vec![vec![1, 2, 3]],
        Edit::Span(span(0, 0, 1), pos(0, 3), false),
        ListError::InsertionIndex(pos(0, 3)),
    );
}
#[test]
fn move_values_between_lists() {
    case(
        vec![vec![1, 2, 3, 4], vec![5, 6, 7]],
        Edit::Span(span(0, 1, 3), pos(1, 1), false),
        vec![2, 3],
        vec![vec![1, 4], vec![5, 2, 3, 6, 7]],
        vec![span(0, 1, 3), span(1, 1, 1)],
        vec![span(0, 1, 1), span(1, 1, 3)],
    );
}
#[test]
fn move_values_between_lists_reversing_to_end() {
    case(
        vec![vec![1, 2, 3], vec![4, 5]],
        Edit::Span(span(0, 0, 2), pos(1, 2), true),
        vec![2, 1],
        vec![vec![3], vec![4, 5, 2, 1]],
        vec![span(0, 0, 2), span(1, 2, 2)],
        vec![span(0, 0, 0), span(1, 2, 4)],
    );
}
#[test]
fn move_values_between_lists_throws_on_destination_index_out_of_bounds() {
    fails(
        vec![vec![1], vec![2]],
        Edit::Span(span(0, 0, 1), pos(1, 2), false),
        ListError::InsertionIndex(pos(1, 2)),
    );
}
#[test]
fn move_values_between_lists_throws_on_empty_span() {
    fails(
        vec![vec![1], vec![2]],
        Edit::Span(span(0, 1, 1), pos(1, 0), false),
        ListError::InvalidRange {
            entity: 0,
            start: 1,
            end: 1,
        },
    );
}
fn swap(a: ListPosition, b: ListPosition) -> Edit {
    Edit::Raw(ListMove::Swap { left: a, right: b })
}
fn swap_spans(a: ListRange, b: ListRange, reverse: bool) -> Edit {
    Edit::Raw(ListMove::SubListSwap {
        left: a,
        right: b,
        reverse,
    })
}
#[test]
fn swap_values_in_list() {
    within(
        vec![1, 2, 3],
        swap(pos(0, 0), pos(0, 2)),
        vec![],
        vec![3, 2, 1],
        0,
        3,
    );
    within(
        vec![1, 2, 3],
        swap(pos(0, 2), pos(0, 1)),
        vec![],
        vec![1, 3, 2],
        1,
        3,
    );
}
#[test]
fn swap_adjacent_values_in_list() {
    within(
        vec![1, 2, 3],
        swap(pos(0, 0), pos(0, 1)),
        vec![],
        vec![2, 1, 3],
        0,
        2,
    );
}
#[test]
fn swap_values_between_lists() {
    case(
        vec![vec![1, 2, 3], vec![4, 5, 6]],
        swap(pos(0, 1), pos(1, 2)),
        vec![],
        vec![vec![1, 6, 3], vec![4, 5, 2]],
        vec![span(0, 1, 2), span(1, 2, 3)],
        vec![span(0, 1, 2), span(1, 2, 3)],
    );
}
#[test]
fn swap_first_and_last_values_between_lists() {
    case(
        vec![vec![1, 2], vec![3, 4, 5]],
        swap(pos(0, 0), pos(1, 2)),
        vec![],
        vec![vec![5, 2], vec![3, 4, 1]],
        vec![span(0, 0, 1), span(1, 2, 3)],
        vec![span(0, 0, 1), span(1, 2, 3)],
    );
}
#[test]
fn swap_values_in_list_non_adjacent() {
    within(
        vec![1, 2, 3, 4, 5, 6],
        swap_spans(span(0, 1, 3), span(0, 4, 6), false),
        vec![],
        vec![1, 5, 6, 4, 2, 3],
        1,
        6,
    );
}
#[test]
fn swap_values_in_list_adjacent() {
    within(
        vec![1, 2, 3, 4, 5],
        swap_spans(span(0, 0, 2), span(0, 2, 4), false),
        vec![],
        vec![3, 4, 1, 2, 5],
        0,
        4,
    );
}
#[test]
fn swap_values_in_list_reversing() {
    within(
        vec![1, 2, 3, 4, 5, 6],
        swap_spans(span(0, 1, 3), span(0, 4, 6), true),
        vec![],
        vec![1, 6, 5, 4, 3, 2],
        1,
        6,
    );
}
#[test]
fn swap_values_in_list_throws_when_spans_out_of_order() {
    fails(
        vec![vec![1, 2, 3, 4, 5]],
        swap_spans(span(0, 2, 4), span(0, 1, 3), false),
        ListError::OverlappingRanges {
            left: span(0, 2, 4),
            right: span(0, 1, 3),
        },
    );
}
#[test]
fn swap_values_in_list_throws_on_empty_span() {
    fails(
        vec![vec![1, 2, 3, 4]],
        swap_spans(span(0, 1, 1), span(0, 2, 3), false),
        ListError::InvalidRange {
            entity: 0,
            start: 1,
            end: 1,
        },
    );
}
#[test]
fn swap_values_between_lists_differing_lengths() {
    case(
        vec![vec![1, 2, 3, 4], vec![5, 6, 7]],
        swap_spans(span(0, 1, 3), span(1, 0, 1), false),
        vec![],
        vec![vec![1, 5, 4], vec![2, 3, 6, 7]],
        vec![span(0, 1, 3), span(1, 0, 1)],
        vec![span(0, 1, 2), span(1, 0, 2)],
    );
}
#[test]
fn swap_values_between_lists_throws_on_empty_span() {
    fails(
        vec![vec![1], vec![2]],
        swap_spans(span(0, 0, 0), span(1, 0, 1), false),
        ListError::InvalidRange {
            entity: 0,
            start: 0,
            end: 0,
        },
    );
}
