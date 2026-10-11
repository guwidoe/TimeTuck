// Adapted from Timefold Community tests at8b815bc6b5f3eb81feedc94dea0885a93b267762.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Native immutable moves capture owned metadata from the pre-move assignment;
// every source metadata assertion below is made after actual move execution.
use timetuck::*;
fn pos(entity: usize, index: usize) -> ListPosition {
    ListPosition { entity, index }
}
fn span(entity: usize, start: usize, end: usize) -> ListRange {
    ListRange { entity, start, end }
}
fn model(values: std::ops::RangeInclusive<i32>, rows: Vec<Vec<i32>>) -> ListProblem<i32> {
    ListProblem::new(
        values.collect(),
        rows.into_iter().map(ListEntity::new).collect(),
    )
    .unwrap()
}
fn apply(
    p: &ListProblem<i32>,
    movement: &ListMove<i32>,
) -> (Vec<usize>, Vec<i32>, ListAssignment<i32>) {
    let mut assignment = p.initial_assignment();
    let values = p.planning_values(&assignment, movement).unwrap();
    p.apply(&mut assignment, movement).unwrap().commit();
    (movement.touched_entities(), values, assignment)
}
fn change(
    entity: usize,
    start: usize,
    length: usize,
    destination: ListPosition,
    reverse: bool,
) -> ListMove<i32> {
    ListMove::SubListChange {
        entity,
        start,
        end: start + length,
        destination,
        reverse,
    }
}
fn swap(left: ListRange, right: ListRange, reverse: bool) -> ListMove<i32> {
    ListMove::SubListSwap {
        left,
        right,
        reverse,
    }
}
#[test]
fn list_change_tabu_introspection_two_entities() {
    let p = model(0..=4, vec![vec![1, 2], vec![3]]);
    let m = ListMove::Change {
        source: pos(0, 1),
        destination: pos(1, 1),
    };
    let (entities, values, a) = apply(&p, &m);
    assert_eq!(entities, [0, 1]);
    assert_eq!(values, [2]);
    assert_eq!(a.lists(), [vec![1], vec![3, 2]]);
}
#[test]
fn list_change_tabu_introspection_one_entity() {
    let p = model(0..=4, vec![vec![1, 2]]);
    let m = ListMove::Change {
        source: pos(0, 0),
        destination: pos(0, 1),
    };
    let (entities, values, a) = apply(&p, &m);
    assert_eq!(entities, [0]);
    assert_eq!(values, [1]);
    assert_eq!(a.lists(), [vec![2, 1]]);
}
#[test]
fn sublist_change_tabu_introspection_two_entities() {
    let p = model(1..=6, vec![vec![1, 2, 3, 4], vec![5], vec![]]);
    let m = change(0, 1, 3, pos(1, 0), false);
    let (entities, values, a) = apply(&p, &m);
    assert_eq!(entities, [0, 1]);
    assert_eq!(values, [2, 3, 4]);
    assert_eq!(a.lists(), [vec![1], vec![2, 3, 4, 5], vec![]]);
    // Preserve EVERY source equality distinction, even malformed comparison-only moves.
    for different in [
        change(0, 1, 3, pos(1, 0), true),
        change(0, 1, 3, pos(1, 1), false),
        change(0, 1, 3, pos(2, 0), false),
        change(0, 1, 2, pos(1, 0), false),
        change(0, 2, 4, pos(1, 0), false),
        change(2, 1, 3, pos(1, 0), false),
    ] {
        assert_ne!(different, m);
    }
    assert_eq!(change(0, 1, 3, pos(1, 0), false), m);
}
#[test]
fn sublist_change_tabu_introspection_one_entity() {
    let p = model(1..=6, vec![vec![1, 2, 3, 4]]);
    let (entities, values, a) = apply(&p, &change(0, 0, 2, pos(0, 2), false));
    assert_eq!(entities, [0]);
    assert_eq!(values, [1, 2]);
    assert_eq!(a.lists(), [vec![3, 4, 1, 2]]);
}
#[test]
fn list_swap_tabu_introspection_two_entities() {
    let p = model(1..=3, vec![vec![1, 2], vec![3]]);
    let (entities, mut values, a) = apply(
        &p,
        &ListMove::Swap {
            left: pos(0, 0),
            right: pos(1, 0),
        },
    );
    assert_eq!(entities, [0, 1]);
    values.sort_unstable();
    assert_eq!(values, [1, 3]);
    assert_eq!(a.lists(), [vec![3, 2], vec![1]]);
}
#[test]
fn sublist_swap_tabu_introspection_two_entities() {
    let p = model(1..=7, vec![vec![1, 2, 3, 4], vec![5], vec![]]);
    let m = swap(span(0, 0, 3), span(1, 0, 1), false);
    let (entities, values, a) = apply(&p, &m);
    assert_eq!(entities, [0, 1]);
    assert_eq!(values, [1, 2, 3, 5]);
    assert_eq!(a.lists(), [vec![5, 4], vec![1, 2, 3], vec![]]);
    for different in [
        swap(span(0, 0, 3), span(1, 0, 1), true),
        swap(span(0, 0, 3), span(1, 0, 2), false),
        swap(span(0, 0, 3), span(1, 1, 1), false),
        swap(span(0, 0, 3), span(2, 0, 1), false),
        swap(span(0, 1, 4), span(1, 0, 1), false),
        swap(span(0, 1, 3), span(1, 0, 1), false),
        swap(span(2, 0, 3), span(1, 0, 1), false),
    ] {
        assert_ne!(different, m);
    }
    assert_eq!(swap(span(0, 0, 3), span(1, 0, 1), false), m);
}
