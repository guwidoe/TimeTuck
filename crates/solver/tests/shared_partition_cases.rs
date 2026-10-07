// Partial behavioral fixtures from FusedEqualIndexTest (Apache-2.0).
// Java bucket-handle identity, explicit empty-bucket creation/cleanup and custom
// factory invocation contracts remain pending. See compat/provenance.json.
use timetuck::*;
type Row = (&'static str, i32, &'static str);
type Join = PartitionedJoinNode<'static, usize, usize, Row, Row, &'static str, i32>;
fn join(comparison: JoinComparison) -> Join {
    PartitionedJoinNode::new(comparison, |r: &Row| (r.0, r.1), |r: &Row| (r.0, r.1))
}
fn right(j: &Join, gender: &'static str, age: i32) -> Vec<&'static str> {
    j.matching_right(&gender, &age).map(|(_, r)| r.2).collect()
}
fn left(j: &Join, gender: &'static str, age: i32) -> Vec<&'static str> {
    j.matching_left(&gender, &age).map(|(_, r)| r.2).collect()
}
#[test]
fn single_equal_bucket_ops_partial() {
    let mut j = join(JoinComparison::Equal);
    j.apply_left(TupleChange::put(0, ("F", 0, "L1")));
    j.apply_right(TupleChange::put(0, ("F", 0, "R1")));
    j.apply_right(TupleChange::put(1, ("F", 0, "R2")));
    assert_eq!(j.partition_count(), 1);
    assert_eq!(j.matching_right(&"F", &0).count(), 2);
    assert_eq!(right(&j, "F", 0), ["R1", "R2"]);
    assert_eq!(left(&j, "F", 0), ["L1"]);
    j.apply_right(TupleChange::retract(1));
    assert_eq!(j.matching_right(&"F", &0).count(), 1);
    assert_eq!(j.partition_count(), 1);
    j.apply_left(TupleChange::retract(0));
    assert_eq!(j.partition_count(), 1);
    assert_eq!(right(&j, "F", 0), ["R1"]);
}
#[test]
fn empty_bucket_cleanup_partial() {
    let mut j = join(JoinComparison::Equal);
    j.apply_left(TupleChange::put(0, ("F", 0, "L")));
    assert_eq!(j.partition_count(), 1);
    j.apply_left(TupleChange::retract(0));
    assert_eq!(j.partition_state(&"F"), None);
    assert_eq!(j.partition_count(), 0);
}
#[test]
fn composite_equal_key_partial() {
    let mut j = PartitionedJoinNode::<'_, usize, usize, Row, Row, (&str, i32), ()>::new(
        JoinComparison::Equal,
        |r| ((r.0, r.1), ()),
        |r| ((r.0, r.1), ()),
    );
    j.apply_right(TupleChange::put(0, ("F", 40, "R")));
    assert_eq!(j.matching_right(&("F", 40), &()).count(), 1);
    assert_eq!(
        j.matching_right(&("F", 40), &())
            .map(|(_, r)| r.2)
            .collect::<Vec<_>>(),
        ["R"]
    );
    assert_eq!(j.partition_state(&("F", 30)), None);
    assert_eq!(j.partition_state(&("M", 40)), None);
}
#[test]
fn equal_plus_less_than_suffix_partial() {
    let mut j = join(JoinComparison::LessThan);
    j.apply_right(TupleChange::put(0, ("F", 30, "rightAge30")));
    j.apply_right(TupleChange::put(1, ("F", 50, "rightAge50")));
    assert_eq!(right(&j, "F", 40), ["rightAge50"]);
    assert_eq!(right(&j, "F", 20), ["rightAge30", "rightAge50"]);
    assert!(right(&j, "F", 60).is_empty());
    assert_eq!(j.partition_count(), 1);
    assert_eq!(j.partition_state(&"M"), None);
}
#[test]
fn lazy_opposite_side_partial() {
    let mut j = join(JoinComparison::Equal);
    j.apply_left(TupleChange::put(0, ("X", 0, "leftTuple")));
    let state = j.partition_state(&"X").unwrap();
    assert!(state.left_initialized);
    assert!(!state.right_initialized);
    assert_eq!(state.left_rows, 1);
    assert_eq!(state.right_rows, 0);
    assert_eq!(j.matching_right(&"X", &0).count(), 0);
    assert!(right(&j, "X", 0).is_empty());
    assert_eq!(j.partition_state(&"X"), Some(state));
    j.apply_left(TupleChange::retract(0));
    assert_eq!(j.partition_state(&"X"), None);
    j.apply_right(TupleChange::put(0, ("X", 0, "rightTuple")));
    let state = j.partition_state(&"X").unwrap();
    assert!(!state.left_initialized);
    assert!(state.right_initialized);
    assert_eq!(state.left_rows, 0);
    assert_eq!(state.right_rows, 1);
}
