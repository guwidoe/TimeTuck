// Adapted from pinned Community SubListChangeMoveTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Stable character values and entity indices replace fixture objects/metamodels.
use timetuck::*;
fn fixture(rows: &[&str]) -> (ListProblem<char>, ListAssignment<char>) {
    let p = ListProblem::new(
        rows.iter().flat_map(|s| s.chars()).collect(),
        rows.iter()
            .map(|s| ListEntity::new(s.chars().collect()))
            .collect(),
    )
    .unwrap();
    let a = p.initial_assignment();
    (p, a)
}
fn span(
    entity: usize,
    start: usize,
    end: usize,
    dest: usize,
    index: usize,
    reverse: bool,
) -> ListMove<char> {
    ListMove::SubListChange {
        entity,
        start,
        end,
        destination: ListPosition {
            entity: dest,
            index,
        },
        reverse,
    }
}
#[test]
fn move_span_forward() {
    let (p, mut a) = fixture(&["ABCDE"]);
    p.apply(&mut a, &span(0, 1, 3, 0, 3, false))
        .unwrap()
        .commit();
    assert_eq!(a.list(0).unwrap(), &['A', 'D', 'E', 'B', 'C']);
}
#[test]
fn move_span_backward_reversing() {
    let (p, mut a) = fixture(&["ABCDE"]);
    p.apply(&mut a, &span(0, 3, 5, 0, 0, true))
        .unwrap()
        .commit();
    assert_eq!(a.list(0).unwrap(), &['E', 'D', 'A', 'B', 'C']);
}
#[test]
fn move_span_to_other_entity() {
    // A1..A4 -> A..D; B1..B3 -> E..G, preserving distinct identities.
    let (p, mut a) = fixture(&["ABCD", "EFG"]);
    p.apply(&mut a, &span(0, 1, 3, 1, 1, false))
        .unwrap()
        .commit();
    assert_eq!(a.list(0).unwrap(), &['A', 'D']);
    assert_eq!(a.list(1).unwrap(), &['E', 'B', 'C', 'F', 'G']);
}
#[test]
fn move_span_to_unassigned_is_sublist_unassign_move() {
    let (p, mut a) = fixture(&["ABC"]);
    p.apply(
        &mut a,
        &ListMove::Unassign {
            entity: 0,
            start: 0,
            end: 2,
        },
    )
    .unwrap()
    .commit();
    assert_eq!(a.list(0).unwrap(), &['C']);
}
#[test]
fn execute_temporarily_then_undo() {
    let (p, mut a) = fixture(&["ABC"]);
    {
        let trial = p.apply(&mut a, &span(0, 0, 2, 0, 1, false)).unwrap();
        assert_eq!(trial.list(0).unwrap(), &['C', 'A', 'B']);
    }
    assert_eq!(a.list(0).unwrap(), &['A', 'B', 'C']);
}
#[test]
fn get_planning_entities_for_same_entity_move() {
    // Metadata is observable even for a destination not valid for execution.
    assert_eq!(span(0, 0, 1, 0, 2, false).touched_entities(), vec![0]);
}
#[test]
fn get_planning_entities_for_different_entities_move() {
    assert_eq!(span(0, 0, 1, 1, 0, false).touched_entities(), vec![0, 1]);
}
