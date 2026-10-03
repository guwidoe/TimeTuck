// Adapted from pinned Community SubListSwapMoveTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
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
fn swap(left: (usize, usize, usize), right: (usize, usize, usize)) -> ListMove<char> {
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
        reverse: false,
    }
}
#[test]
fn swap_non_overlapping_spans() {
    let (p, mut a) = fixture(&["ABCDEF"]);
    p.apply(&mut a, &swap((0, 1, 3), (0, 4, 6)))
        .unwrap()
        .commit();
    assert_eq!(a.list(0).unwrap(), &['A', 'E', 'F', 'D', 'B', 'C']);
}
#[test]
fn swap_normalizes_out_of_order_spans() {
    let (p, mut a) = fixture(&["ABCDEF"]);
    p.apply(&mut a, &swap((0, 4, 6), (0, 1, 3)))
        .unwrap()
        .commit();
    assert_eq!(a.list(0).unwrap(), &['A', 'E', 'F', 'D', 'B', 'C']);
}
#[test]
fn swap_spans_of_differing_length() {
    // A1..A4 -> A..D; B1..B3 -> E..G.
    let (p, mut a) = fixture(&["ABCD", "EFG"]);
    p.apply(&mut a, &swap((0, 1, 3), (1, 0, 1)))
        .unwrap()
        .commit();
    assert_eq!(a.list(0).unwrap(), &['A', 'E', 'D']);
    assert_eq!(a.list(1).unwrap(), &['B', 'C', 'F', 'G']);
}
#[test]
fn execute_temporarily_then_undo() {
    let (p, mut a) = fixture(&["ABCD"]);
    {
        let trial = p.apply(&mut a, &swap((0, 0, 1), (0, 3, 4))).unwrap();
        assert_eq!(trial.list(0).unwrap(), &['D', 'B', 'C', 'A']);
    }
    assert_eq!(a.list(0).unwrap(), &['A', 'B', 'C', 'D']);
}
#[test]
fn get_planning_entities_for_different_entities_swap() {
    assert_eq!(swap((0, 0, 1), (1, 0, 1)).touched_entities(), vec![0, 1]);
}
