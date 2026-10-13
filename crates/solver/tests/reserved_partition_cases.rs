// Adapted from Timefold Community FusedEqualIndexTest; Apache-2.0.
// Pinned commit: 8b815bc6b5f3eb81feedc94dea0885a93b267762.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn lazy_allocation_left_only_key_right_not_allocated() {
    use std::cell::Cell;
    let right_init = Cell::new(0);
    let mut index = PartitionedJoinNode::<'_, usize, usize, &str, &str, &str, ()>::new(
        JoinComparison::Equal,
        |_| ("X", ()),
        |_| ("X", ()),
    )
    .with_side_factories(JoinPartitionSide::new, || {
        right_init.set(right_init.get() + 1);
        JoinPartitionSide::new()
    });
    let bucket = index.reserve_partition("X");
    assert_eq!(right_init.get(), 0);
    index.apply_left(TupleChange::put(0, "leftTuple"));
    assert_eq!(right_init.get(), 0);
    let mut matches = 0;
    for _ in index.matching_right(&"X", &()) {
        matches += 1;
    }
    assert_eq!(matches, 0);
    assert_eq!(index.matching_right(&"X", &()).count(), 0);
    assert_eq!(right_init.get(), 0);
    let state = index.partition_state(&"X").unwrap();
    assert!(!(state.left_rows == 0 && state.right_rows == 0));
    assert!(!state.right_initialized);
    index.apply_left(TupleChange::retract(0));
    let state = index.partition_state(&"X").unwrap();
    assert!(state.left_rows == 0 && state.right_rows == 0);
    assert_eq!(index.partition_reservation(&"X"), Some(bucket));
    index.apply_right(TupleChange::put(0, "rightTuple"));
    assert_eq!(right_init.get(), 1);
    assert!(index.partition_state(&"X").unwrap().right_initialized);
}
type Row = (&'static str, i32, &'static str);
type Join = PartitionedJoinNode<'static, usize, usize, Row, Row, &'static str, i32>;
fn join(comparison: JoinComparison) -> Join {
    PartitionedJoinNode::new(
        comparison,
        move |r: &Row| {
            (
                r.0,
                if comparison == JoinComparison::Equal {
                    0
                } else {
                    r.1
                },
            )
        },
        move |r: &Row| {
            (
                r.0,
                if comparison == JoinComparison::Equal {
                    0
                } else {
                    r.1
                },
            )
        },
    )
}
fn right(j: &Join, gender: &'static str, age: i32) -> Vec<&'static str> {
    j.matching_right(&gender, &age).map(|(_, r)| r.2).collect()
}
fn left(j: &Join, gender: &'static str, age: i32) -> Vec<&'static str> {
    j.matching_left(&gender, &age).map(|(_, r)| r.2).collect()
}
fn empty(j: &Join, gender: &'static str) -> bool {
    let state = j.partition_state(&gender).expect("reserved bucket");
    state.left_rows == 0 && state.right_rows == 0
}
#[test]
fn single_equal_bucket_sharing_and_ops() {
    let mut j = join(JoinComparison::Equal);
    let bucket_f = j.reserve_partition("F");
    assert_eq!(j.reserve_partition("F"), bucket_f);
    assert_eq!(j.partition_reservation(&"F"), Some(bucket_f.clone()));
    assert_ne!(j.reserve_partition("M"), bucket_f);
    j.apply_left(TupleChange::put(0, ("F", 0, "L1")));
    j.apply_right(TupleChange::put(0, ("F", 0, "R1")));
    j.apply_right(TupleChange::put(1, ("F", 0, "R2")));
    assert_eq!(j.matching_right(&"F", &0).count(), 2);
    assert_eq!(right(&j, "F", 0), ["R1", "R2"]);
    assert_eq!(left(&j, "F", 0), ["L1"]);
    j.apply_right(TupleChange::retract(1));
    assert_eq!(j.matching_right(&"F", &0).count(), 1);
    assert!(!empty(&j, "F"));
    j.apply_left(TupleChange::retract(0));
    assert!(!empty(&j, "F"));
}
#[test]
fn empty_bucket_cleanup() {
    let mut j = join(JoinComparison::Equal);
    let bucket = j.reserve_partition("F");
    j.apply_left(TupleChange::put(0, ("F", 0, "L")));
    j.remove_reserved_partition(&"F", &bucket);
    assert_eq!(j.partition_reservation(&"F"), Some(bucket.clone()));
    j.apply_left(TupleChange::retract(0));
    assert!(empty(&j, "F"));
    j.remove_reserved_partition(&"F", &bucket);
    assert_eq!(j.partition_reservation(&"F"), None);
    assert_eq!(j.partition_count(), 0);
}
#[test]
fn equal_plus_equal_keyed_by_composite_key() {
    let mut j = PartitionedJoinNode::<'_, usize, usize, Row, Row, (&str, i32), ()>::new(
        JoinComparison::Equal,
        |r| ((r.0, r.1), ()),
        |r| ((r.0, r.1), ()),
    );
    let bucket = j.reserve_left_partition(&("F", 40, ""));
    assert_eq!(j.reserve_left_partition(&("F", 40, "")), bucket);
    assert_ne!(j.reserve_left_partition(&("F", 30, "")), bucket);
    assert_ne!(j.reserve_left_partition(&("M", 40, "")), bucket);
    j.apply_right(TupleChange::put(0, ("F", 40, "R")));
    assert_eq!(j.matching_right(&("F", 40), &()).count(), 1);
    assert_eq!(
        j.matching_right(&("F", 40), &())
            .map(|(_, r)| r.2)
            .collect::<Vec<_>>(),
        ["R"]
    );
}
#[test]
fn equal_plus_less_than_suffix_is_per_side_and_flipped() {
    let mut j = join(JoinComparison::LessThan);
    let bucket = j.reserve_left_partition(&("F", 40, ""));
    assert_eq!(j.reserve_left_partition(&("F", 99, "")), bucket);
    assert_ne!(j.reserve_left_partition(&("M", 40, "")), bucket);
    j.apply_right(TupleChange::put(0, ("F", 30, "rightAge30")));
    j.apply_right(TupleChange::put(1, ("F", 50, "rightAge50")));
    assert_eq!(right(&j, "F", 40), ["rightAge50"]);
    assert_eq!(right(&j, "F", 20), ["rightAge30", "rightAge50"]);
    assert!(right(&j, "F", 60).is_empty());
}
#[test]
fn has_suffix_and_is_same_bucket() {
    let j = join(JoinComparison::LessThan);
    assert!(j.has_ordered_suffix());
    assert!(j.same_left_partition(&("F", 30, ""), &("F", 50, "")));
    assert!(!j.same_left_partition(&("F", 30, ""), &("M", 30, "")));
    let pure = join(JoinComparison::Equal);
    assert!(!pure.has_ordered_suffix());
}
