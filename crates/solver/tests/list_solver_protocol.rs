//! Original development protocol regressions; no upstream method credit.
#[path = "../examples/list_solver_support/mod.rs"]
mod support;
use support::Case;
use timetuck::{HardSoftScore, ListAssignment};
const CASE: &str = "test\t1:2,4:6\t0:0,10:10\t1,1\t0;1\t10\t0,1";
#[test]
fn retains_initial_lists_full_pins_and_prefix_metadata() {
    let c = Case::parse(CASE).unwrap();
    assert_eq!(c.id, "test");
    assert_eq!(c.problem.initial_assignment().lists(), &[vec![0], vec![1]]);
    assert!(c.problem.entities()[0].is_pinned());
    assert_eq!(c.problem.entities()[1].pinned_prefix(), 1);
    assert_eq!(
        c.score(&c.problem.initial_assignment()),
        HardSoftScore::new(0, -26)
    );
    assert_eq!(
        Case::encode_assignment(&c.problem.initial_assignment()),
        "0;1"
    );
}
#[test]
fn rejects_invalid_identity_coordinates_shape_and_pins() {
    for text in [
        CASE.replace("0;1", "0;0"),
        CASE.replace("0;1", "0;2"),
        CASE.replace("1:2", "1000001:2"),
        CASE.replace("10\t0,1", "1x\t0,1"),
        CASE.replace("10\t0,1", "10\t1,1"),
        CASE.replace("10\t0,1", "00\t0,2"),
        CASE.replace("1,1\t0;1", "1\t0;1"),
        CASE.replace("1,1\t0;1", "-1,1\t0;1"),
        "bad".to_owned(),
    ] {
        assert!(Case::parse(&text).is_err(), "{text}");
    }
}
#[test]
fn capacity_penalty_and_distance_include_return_to_depot() {
    let c = Case::parse("test\t1:2,4:6\t0:0,10:10\t1,1\t-;-\t00\t0,0").unwrap();
    assert_eq!(
        c.score(&ListAssignment::new(vec![vec![0, 1], vec![]])),
        HardSoftScore::new(-1, -20)
    );
    assert_eq!(
        c.score(&ListAssignment::new(vec![vec![], vec![0, 1]])),
        HardSoftScore::new(-1, -34)
    );
}
#[test]
fn empty_range_preserves_empty_route_and_zero_score() {
    let c = Case::parse("empty\t-\t0:0\t0\t-\t0\t0").unwrap();
    assert!(c.problem.is_complete(&c.problem.initial_assignment()));
    assert_eq!(
        c.score(&c.problem.initial_assignment()),
        HardSoftScore::ZERO
    );
    assert_eq!(
        Case::encode_assignment(&c.problem.initial_assignment()),
        "-"
    );
}
