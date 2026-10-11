//! Original reservation lifecycle, identity, transactional and joined-row checks.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Row = (i32, i32);
type Node<'a> = PartitionedJoinNode<'a, usize, usize, Row, Row, i32, i32>;
fn node() -> Node<'static> {
    PartitionedJoinNode::new(JoinComparison::LessThan, |r: &Row| *r, |r: &Row| *r)
}

#[test]
fn reservations_are_lazy_reusable_and_isolate_stale_foreign_and_wrong_key_tokens() {
    let mut a = node();
    let old = a.reserve_partition(1);
    assert_eq!(
        a.partition_state(&1),
        Some(JoinPartitionState {
            left_rows: 0,
            right_rows: 0,
            left_initialized: false,
            right_initialized: false
        })
    );
    assert_eq!(a.matching_left(&1, &5).count(), 0);
    assert_eq!(a.matching_right(&1, &5).count(), 0);
    assert_eq!(a.reserve_partition(1), old);
    assert!(a.remove_reserved_partition(&1, &old));
    let new = a.reserve_partition(1);
    assert_ne!(old, new);
    assert!(!a.remove_reserved_partition(&1, &old));
    let mut b = node();
    let foreign = b.reserve_partition(1);
    assert_ne!(new, foreign);
    assert!(!a.remove_reserved_partition(&1, &foreign));
    let other = a.reserve_partition(2);
    assert!(!a.remove_reserved_partition(&2, &new));
    assert!(!a.remove_reserved_partition(&1, &other));
    assert_eq!(a.partition_count(), 2);
    assert!(a.remove_reserved_partition(&2, &other));
    assert!(!a.remove_reserved_partition(&2, &other));
    // Token drop has no hidden lifetime effect on the node.
    drop(new.clone());
    assert_eq!(a.partition_reservation(&1), Some(new.clone()));
    assert!(a.remove_reserved_partition(&1, &new));
}

#[test]
fn reservation_of_live_group_retains_rows_and_only_reserved_empty_groups_survive() {
    let mut n = node();
    n.apply_left(TupleChange::put(0, (1, 10)));
    n.apply_right(TupleChange::put(0, (1, 20)));
    assert_eq!(n.partition_reservation(&1), None);
    let token = n.reserve_partition(1);
    assert_eq!(n.matches().count(), 1);
    assert!(!n.remove_reserved_partition(&1, &token));
    n.apply_left(TupleChange::put(0, (1, 15)));
    assert_eq!(n.partition_reservation(&1), Some(token.clone()));
    n.apply_batch(
        [TupleChange::put(0, (2, 15))],
        [TupleChange::put(0, (2, 20))],
    );
    assert_eq!(n.partition_count(), 2);
    assert_eq!(
        n.partition_state(&1),
        Some(JoinPartitionState {
            left_rows: 0,
            right_rows: 0,
            left_initialized: true,
            right_initialized: true
        })
    );
    assert_eq!(n.partition_reservation(&1), Some(token.clone()));
    n.apply_batch([TupleChange::retract(0)], [TupleChange::retract(0)]);
    assert_eq!(n.partition_state(&2), None);
    assert_eq!(n.partition_count(), 1);
    assert!(n.remove_reserved_partition(&1, &token));
    assert_eq!(n.partition_count(), 0);
}

#[test]
fn reservation_and_update_panics_preserve_rows_and_handle_identity() {
    let fail = Cell::new(false);
    let calls = Cell::new(0);
    let project = |r: &Row| {
        calls.set(calls.get() + 1);
        assert!(!fail.get(), "projection fault");
        *r
    };
    let mut n = PartitionedJoinNode::<usize, usize, _, _, _, _>::new(
        JoinComparison::LessThan,
        project,
        project,
    );
    let token = n.reserve_left_partition(&(1, 10));
    assert_eq!(calls.get(), 1);
    n.apply_batch(
        [TupleChange::put(0, (1, 10))],
        [TupleChange::put(0, (1, 20))],
    );
    let previous: Vec<_> = n
        .matches()
        .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
        .collect();
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| n.reserve_left_partition(&(2, 10)))).is_err());
    assert!(catch_unwind(AssertUnwindSafe(|| n.same_left_partition(&(1, 0), &(2, 0)))).is_err());
    assert!(
        catch_unwind(AssertUnwindSafe(|| n.apply_batch(
            [TupleChange::put(0, (2, 10))],
            [TupleChange::put(0, (2, 20))]
        )))
        .is_err()
    );
    assert_eq!(n.partition_reservation(&1), Some(token.clone()));
    assert_eq!(n.partition_count(), 1);
    assert_eq!(
        n.matches()
            .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
            .collect::<Vec<_>>(),
        previous
    );
    let calls_before = calls.get();
    assert_eq!(n.reserve_partition(1), token);
    assert!(!n.remove_reserved_partition(&1, &token));
    assert_eq!(calls.get(), calls_before);
    fail.set(false);
    n.apply_batch([TupleChange::retract(0)], [TupleChange::retract(0)]);
    assert!(n.remove_reserved_partition(&1, &token));
}

#[test]
fn reserved_and_automatic_partitions_match_independent_join_after_mixed_batches() {
    let mut n = node();
    let tokens: Vec<_> = (0..8).map(|key| (key, n.reserve_partition(key))).collect();
    let (mut left, mut right) = (BTreeMap::new(), BTreeMap::new());
    let mut seed = 0x56a77b9_u64;
    let mut random = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 32) as usize
    };
    for round in 0..250 {
        let mut change = || {
            let id = random() % 24;
            if random() % 4 == 0 {
                TupleChange::retract(id)
            } else {
                TupleChange::put(id, ((random() % 16) as i32, (random() % 50) as i32))
            }
        };
        let l = change();
        let r = change();
        for (map, change) in [(&mut left, &l), (&mut right, &r)] {
            if let Some(row) = change.value {
                map.insert(change.id, row);
            } else {
                map.remove(&change.id);
            }
        }
        if round % 3 == 0 {
            n.apply_batch([l], [r]);
        } else {
            n.apply_left(l);
            n.apply_right(r);
        }
        let actual: BTreeMap<_, _> = n
            .matches()
            .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
            .collect();
        let expected: BTreeMap<_, _> = left
            .iter()
            .flat_map(|(i, l)| {
                right.iter().filter_map(move |(j, r)| {
                    (l.0 == r.0 && l.1 < r.1).then_some(((*i, *j), (*l, *r)))
                })
            })
            .collect();
        assert_eq!(actual, expected);
        for (key, token) in &tokens {
            assert_eq!(n.partition_reservation(key), Some(token.clone()));
        }
        let expected_groups: std::collections::BTreeSet<_> = (0..8)
            .chain(left.values().map(|r| r.0))
            .chain(right.values().map(|r| r.0))
            .collect();
        assert_eq!(n.partition_count(), expected_groups.len());
    }
    n.apply_batch(
        left.keys().copied().map(TupleChange::retract),
        right.keys().copied().map(TupleChange::retract),
    );
    assert!(n.matches().next().is_none());
    assert_eq!(n.partition_count(), 8);
    for (key, token) in tokens {
        assert!(n.remove_reserved_partition(&key, &token));
    }
    assert_eq!(n.partition_count(), 0);
}
