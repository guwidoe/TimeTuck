//! Original coalescing, indexing, lifecycle and atomicity checks.
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn deferred_batches_project_final_inputs_and_check_each_pair_once() {
    let keys = Cell::new(0);
    let pairs = Cell::new(0);
    let mut n = BatchedJoinNode::new(
        JoinEvaluation::Deferred,
        |r: &(i32, i32)| {
            keys.set(keys.get() + 1);
            r.0
        },
        |r: &(i32, i32)| {
            keys.set(keys.get() + 1);
            r.0
        },
        |l, r| {
            pairs.set(pairs.get() + 1);
            l.1 < r.1
        },
    );
    for i in 0..100 {
        n.enqueue_left(TupleChange::put(0, (0, i)));
    }
    n.enqueue_left(TupleChange::put(1, (0, 1)));
    for i in 0..3 {
        n.enqueue_right(TupleChange::put(i, (0, 100)));
    }
    for i in 3..40 {
        n.enqueue_right(TupleChange::put(i, (1, 100)));
    }
    assert_eq!(keys.get(), 0);
    assert_eq!(pairs.get(), 0);
    let out = n.propagate();
    assert_eq!(out.len(), 6);
    assert_eq!(keys.get(), 42);
    assert_eq!(pairs.get(), 6);
    keys.set(0);
    pairs.set(0);
    n.enqueue_left(TupleChange::put(0, (0, 200)));
    n.enqueue_right(TupleChange::put(0, (0, 300)));
    let out = n.propagate();
    assert_eq!(keys.get(), 2);
    assert_eq!(pairs.get(), 4); // 3 + 2 - 1 overlapping pair.
    assert_eq!(out.len(), 4);
    assert_eq!(n.rows().count(), 4);
    pairs.set(0);
    keys.set(0);
    n.enqueue_right(TupleChange::retract(0));
    n.propagate();
    assert_eq!(keys.get(), 0);
    assert_eq!(pairs.get(), 0);
    assert_eq!(n.rows().count(), 2);
}
#[test]
fn randomized_rounds_match_independent_scan_with_exact_lifecycle_balance() {
    type Row = (i32, i32);
    let make =
        |timing| BatchedJoinNode::new(timing, |r: &Row| r.0, |r: &Row| r.0, |l, r| l.1 < r.1);
    let mut eager = make(JoinEvaluation::Eager);
    let mut deferred = make(JoinEvaluation::Deferred);
    let mut left = BTreeMap::new();
    let mut right = BTreeMap::new();
    let mut live = BTreeMap::new();
    let mut seed = 993762u64;
    let mut positive = false;
    for round in 0..220 {
        let mut nl = left.clone();
        let mut nr = right.clone();
        for step in 0..9 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = seed >> 32;
            let id = (x % 11) as usize;
            let value =
                (!x.is_multiple_of(4)).then_some(((x / 11 % 3) as i32, (x / 33 % 13) as i32));
            let side = if step % 2 == 0 { &mut nl } else { &mut nr };
            if let Some(v) = value {
                side.insert(id, v);
            } else {
                side.remove(&id);
            }
            let change = TupleChange { id, value };
            if step % 2 == 0 {
                eager.enqueue_left(change.clone());
                deferred.enqueue_left(change);
            } else {
                eager.enqueue_right(change.clone());
                deferred.enqueue_right(change);
            }
        }
        if round % 7 == 0 {
            eager.discard_pending();
            deferred.discard_pending();
        } else {
            left = nl;
            right = nr;
        }
        let e = eager.propagate();
        let d = deferred.propagate();
        // Eager may refresh a pair after a transient match change; both modes
        // must publish balanced lifecycle events and the same final payloads.
        let mut dlive = live.clone();
        for (events, rows) in [(e, &mut live), (d, &mut dlive)] {
            let mut seen = BTreeSet::new();
            for event in events {
                assert!(seen.insert(*event.id()));
                match event {
                    TupleLifecycleEvent::Insert { id, value } => {
                        assert!(rows.insert(id, value).is_none())
                    }
                    TupleLifecycleEvent::Update { id, value } => {
                        assert!(rows.insert(id, value).is_some())
                    }
                    TupleLifecycleEvent::Retract { id } => assert!(rows.remove(&id).is_some()),
                }
            }
        }
        let mut wanted = BTreeMap::new();
        for (i, l) in &left {
            for (j, r) in &right {
                if l.0 == r.0 && l.1 < r.1 {
                    wanted.insert((*i, *j), (*l, *r));
                }
            }
        }
        positive |= !wanted.is_empty();
        assert_eq!(live, wanted);
        assert_eq!(dlive, wanted);
        assert_eq!(
            eager
                .rows()
                .map(|(id, v)| (*id, *v))
                .collect::<BTreeMap<_, _>>(),
            wanted
        );
        assert_eq!(
            deferred
                .rows()
                .map(|(id, v)| (*id, *v))
                .collect::<BTreeMap<_, _>>(),
            wanted
        );
    }
    assert!(positive);
}
#[test]
fn retry_discard_and_preload_boundaries_preserve_committed_state() {
    for timing in [JoinEvaluation::Eager, JoinEvaluation::Deferred] {
        let fail = Cell::new(false);
        let mut n = BatchedJoinNode::new(
            timing,
            |_: &i32| (),
            |_: &i32| (),
            |l, r| {
                assert!(!fail.get());
                l < r
            },
        );
        n.begin_preload();
        assert!(n.is_preloading());
        assert!(catch_unwind(AssertUnwindSafe(|| n.begin_preload())).is_err());
        n.enqueue_left(TupleChange::put(0, 1));
        assert!(catch_unwind(AssertUnwindSafe(|| n.end_preload())).is_err());
        n.enqueue_right(TupleChange::put(0, 2));
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Insert {
                id: (0, 0),
                value: (1, 2)
            }]
        );
        n.end_preload();
        assert!(!n.is_preloading());
        assert!(catch_unwind(AssertUnwindSafe(|| n.begin_preload())).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| n.end_preload())).is_err());
        fail.set(true);
        if timing == JoinEvaluation::Eager {
            assert!(
                catch_unwind(AssertUnwindSafe(|| n.enqueue_right(TupleChange::put(0, 3)))).is_err()
            );
            assert_eq!(n.pending_len(), 0);
        } else {
            n.enqueue_right(TupleChange::put(0, 3));
            assert!(catch_unwind(AssertUnwindSafe(|| n.propagate())).is_err());
            assert_eq!(n.pending_len(), 1);
        }
        assert_eq!(
            n.rows().map(|(id, v)| (*id, *v)).collect::<Vec<_>>(),
            vec![((0, 0), (1, 2))]
        );
        fail.set(false);
        if timing == JoinEvaluation::Eager {
            n.enqueue_right(TupleChange::put(0, 3));
        }
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Update {
                id: (0, 0),
                value: (1, 3)
            }]
        );
        n.enqueue_left(TupleChange::retract(0));
        n.discard_pending();
        assert!(n.propagate().is_empty());
        assert_eq!(n.rows().count(), 1);
        n.enqueue_left(TupleChange::retract(0));
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Retract { id: (0, 0) }]
        );
        assert!(
            catch_unwind(AssertUnwindSafe(|| n.begin_preload())).is_err(),
            "unmatched right input still exists"
        );
    }
}
#[test]
fn transient_pairs_are_suppressed_and_both_payloads_refresh_once() {
    for timing in [JoinEvaluation::Eager, JoinEvaluation::Deferred] {
        let mut n = BatchedJoinNode::new(timing, |_: &i32| (), |_: &i32| (), |_, _| true);
        n.enqueue_left(TupleChange::put(0, 1));
        n.enqueue_right(TupleChange::put(0, 2));
        n.enqueue_right(TupleChange::retract(0));
        assert!(n.propagate().is_empty());
        n.enqueue_right(TupleChange::put(0, 3));
        n.propagate();
        n.enqueue_left(TupleChange::put(0, 4));
        n.enqueue_right(TupleChange::put(0, 5));
        n.enqueue_left(TupleChange::retract(0));
        n.enqueue_left(TupleChange::put(0, 6));
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Update {
                id: (0, 0),
                value: (6, 5)
            }]
        );
    }
}
