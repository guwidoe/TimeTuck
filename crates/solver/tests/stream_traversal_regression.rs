// Original native controls; no source-method credit.
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};
use timetuck::*;
#[test]
fn role_visits_have_real_inputs_updates_and_removals_with_unique_publication() {
    let visits = RefCell::new(vec![]);
    let mut n = BatchedJoinNode::new(
        JoinEvaluation::DeferredByInputRole,
        |_: &i32| (),
        |_: &i32| (),
        |l, r| {
            visits.borrow_mut().push((*l, *r));
            l < r
        },
    );
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    for (ls, rs) in [
        (
            vec![(0, Some(1)), (1, Some(4))],
            vec![(0, Some(3)), (1, Some(8)), (2, Some(2))],
        ),
        (vec![(0, Some(9))], vec![(1, Some(10))]),
        (vec![(1, None)], vec![(0, None), (2, Some(12))]),
    ] {
        visits.borrow_mut().clear();
        for (id, v) in ls {
            if let Some(v) = v {
                left.insert(id, v);
            } else {
                left.remove(&id);
            }
            n.enqueue_left(TupleChange { id, value: v });
        }
        for (id, v) in rs {
            if let Some(v) = v {
                right.insert(id, v);
            } else {
                right.remove(&id);
            }
            n.enqueue_right(TupleChange { id, value: v });
        }
        let events = n.propagate();
        let mut ids = BTreeSet::new();
        for e in events {
            let id = match e {
                TupleLifecycleEvent::Insert { id, .. }
                | TupleLifecycleEvent::Update { id, .. }
                | TupleLifecycleEvent::Retract { id } => id,
            };
            assert!(ids.insert(id));
        }
        let actual: BTreeMap<_, _> = n.rows().map(|(id, pair)| (*id, *pair)).collect();
        let expected: BTreeMap<_, _> = left
            .iter()
            .flat_map(|(&i, &l)| {
                right
                    .iter()
                    .filter_map(move |(&j, &r)| (l < r).then_some(((i, j), (l, r))))
            })
            .collect();
        assert_eq!(actual, expected);
        assert!(
            visits
                .borrow()
                .iter()
                .all(|(l, r)| left.values().any(|v| v == l) && right.values().any(|v| v == r))
        );
    }
}
#[test]
fn role_preload_eager_read_restores_independent_reconciliation() {
    let count = std::cell::Cell::new(0);
    let mut n = BatchedJoinNode::new(
        JoinEvaluation::DeferredByInputRole,
        |_: &i32| (),
        |_: &i32| (),
        |_, _| {
            count.set(count.get() + 1);
            true
        },
    );
    n.begin_preload();
    for i in 0..2 {
        n.enqueue_left(TupleChange::put(i, i));
    }
    for j in 0..3 {
        n.enqueue_right(TupleChange::put(j, j));
    }
    n.propagate();
    assert_eq!(count.get(), 6);
    n.end_preload();
    count.set(0);
    n.enqueue_left(TupleChange::put(2, 2));
    n.enqueue_right(TupleChange::put(3, 3));
    n.propagate();
    // Left dirty visits four rights; right dirty visits three lefts, overlap
    // (2,3) receives both actual evaluations; no repeated output insertion.
    assert_eq!(count.get(), 7);
    assert_eq!(n.rows().count(), 12);
}
#[test]
fn backend_traversal_follows_promotion_and_edits_without_changing_standard_order() {
    let mut map = ScalingMap::with_config(ScalingMapConfig {
        array_threshold: 2,
        churn_tolerance: 1,
        maximum_array_size: 4,
    });
    for k in [3, 1, 2] {
        map.get_or_insert_with(k, || k * 10);
    }
    assert!(map.is_array_based());
    assert_eq!(
        map.storage_traversal(true)
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        [(1, 10), (2, 20), (3, 30)]
    );
    assert_eq!(
        map.iter().rev().map(|(k, _)| *k).collect::<Vec<_>>(),
        [3, 2, 1]
    );
    map.remove(&1);
    map.get_or_insert_with(4, || 40);
    assert!(!map.is_array_based());
    assert_eq!(
        map.storage_traversal(true)
            .map(|(k, _)| *k)
            .collect::<Vec<_>>(),
        [4, 3, 2]
    );
    map.remove(&3);
    *map.get_mut(&2).unwrap() = 99;
    assert_eq!(
        map.storage_traversal(false)
            .map(|(k, v)| (*k, *v))
            .collect::<Vec<_>>(),
        [(2, 99), (4, 40)]
    );
    map.clear();
    assert!(!map.is_array_based());
    assert!(map.storage_traversal(true).next().is_none());
}
