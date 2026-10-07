use std::collections::BTreeMap;
use timetuck::*;
const RELATIONS: [JoinComparison; 5] = [
    JoinComparison::Equal,
    JoinComparison::LessThan,
    JoinComparison::LessThanOrEqual,
    JoinComparison::GreaterThan,
    JoinComparison::GreaterThanOrEqual,
];
fn accepts(c: JoinComparison, a: i32, b: i32) -> bool {
    match c {
        JoinComparison::Equal => a == b,
        JoinComparison::LessThan => a < b,
        JoinComparison::LessThanOrEqual => a <= b,
        JoinComparison::GreaterThan => a > b,
        JoinComparison::GreaterThanOrEqual => a >= b,
    }
}
#[test]
fn both_query_directions_match_full_scans_after_rekeys_and_removals() {
    for comparison in RELATIONS {
        let mut j = PartitionedJoinNode::new(comparison, |r: &(i32, i32)| *r, |r: &(i32, i32)| *r);
        let mut left = BTreeMap::new();
        let mut right = BTreeMap::new();
        for step in 0..120usize {
            let id = step % 11;
            let value = (!step.is_multiple_of(7))
                .then_some(((step % 3) as i32, ((step * 7) % 9) as i32 - 4));
            let rows = if step % 2 == 0 { &mut left } else { &mut right };
            if let Some(v) = value {
                rows.insert(id, v);
            } else {
                rows.remove(&id);
            }
            if step % 2 == 0 {
                j.apply_left(TupleChange { id, value });
            } else {
                j.apply_right(TupleChange { id, value });
            }
            let mut partitions = std::collections::BTreeSet::new();
            partitions.extend(left.values().map(|v| v.0));
            partitions.extend(right.values().map(|v| v.0));
            assert_eq!(j.partition_count(), partitions.len());
            for equal in -1..=3 {
                for key in -5..=5 {
                    let mut expected: Vec<_> = right
                        .iter()
                        .filter(|(_, r)| r.0 == equal && accepts(comparison, key, r.1))
                        .map(|(&id, &row)| (id, row))
                        .collect();
                    expected.sort_by_key(|(id, r)| (r.1, *id));
                    assert_eq!(
                        j.matching_right(&equal, &key)
                            .map(|(&id, &r)| (id, r))
                            .collect::<Vec<_>>(),
                        expected
                    );
                    let mut expected: Vec<_> = left
                        .iter()
                        .filter(|(_, r)| r.0 == equal && accepts(comparison, r.1, key))
                        .map(|(&id, &row)| (id, row))
                        .collect();
                    expected.sort_by_key(|(id, r)| (r.1, *id));
                    assert_eq!(
                        j.matching_left(&equal, &key)
                            .map(|(&id, &r)| (id, r))
                            .collect::<Vec<_>>(),
                        expected
                    );
                    let state = j.partition_state(&equal);
                    if let Some(state) = state {
                        assert_eq!(
                            state.left_rows,
                            left.values().filter(|r| r.0 == equal).count()
                        );
                        assert_eq!(
                            state.right_rows,
                            right.values().filter(|r| r.0 == equal).count()
                        );
                    } else {
                        assert!(!partitions.contains(&equal));
                    }
                }
            }
        }
    }
}
#[test]
fn empty_side_queries_do_not_initialize_and_live_side_survives_other_retraction() {
    let mut j = PartitionedJoinNode::new(
        JoinComparison::LessThan,
        |r: &(char, i32)| *r,
        |r: &(char, i32)| *r,
    );
    assert_eq!(j.matching_left(&'X', &0).count(), 0);
    assert_eq!(j.matching_right(&'X', &0).count(), 0);
    assert_eq!(j.partition_count(), 0);
    j.apply_left(TupleChange::put(0, ('X', 1)));
    assert_eq!(
        j.partition_state(&'X'),
        Some(JoinPartitionState {
            left_rows: 1,
            right_rows: 0,
            left_initialized: true,
            right_initialized: false
        })
    );
    j.apply_right(TupleChange::put(0, ('X', 2)));
    assert_eq!(j.partition_count(), 1);
    assert_eq!(j.matches().count(), 1);
    j.apply_left(TupleChange::retract(0));
    assert_eq!(
        j.partition_state(&'X'),
        Some(JoinPartitionState {
            left_rows: 0,
            right_rows: 1,
            left_initialized: true,
            right_initialized: true
        })
    );
    assert_eq!(j.matching_right(&'X', &0).count(), 1);
    j.apply_left(TupleChange::put(1, ('X', 1)));
    assert_eq!(j.matches().count(), 1);
    j.apply_right(TupleChange::put(0, ('Y', 2)));
    assert_eq!(j.partition_count(), 2);
    assert_eq!(j.matches().count(), 0);
    j.apply_left(TupleChange::retract(1));
    assert_eq!(j.partition_count(), 1);
    j.apply_right(TupleChange::retract(0));
    assert_eq!(j.partition_count(), 0);
}
