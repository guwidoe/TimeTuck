//! Original round semantics and failure recovery checks.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn net_events_are_balanced_and_coalesced_for_both_visibility_modes() {
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        for timing in [ExistenceEvaluation::Eager, ExistenceEvaluation::Deferred] {
            let mut n = BatchedExistsNode::new(
                mode,
                timing,
                |r: &(i32, i64)| r.0,
                |r: &i32| *r,
                |_, _| true,
            );
            n.enqueue_left(TupleChange::put(0, (1, 10)));
            n.enqueue_left(TupleChange::retract(0));
            assert_eq!(n.pending_len(), 2);
            assert!(n.propagate().is_empty());
            if mode == ExistenceMode::Exists {
                n.enqueue_right(TupleChange::put(0, 1));
            }
            n.enqueue_left(TupleChange::put(0, (1, 10)));
            assert_eq!(
                n.propagate(),
                vec![TupleLifecycleEvent::Insert {
                    id: 0,
                    value: (1, 10)
                }]
            );
            n.enqueue_left(TupleChange::put(0, (1, 20)));
            n.enqueue_left(TupleChange::retract(0));
            n.enqueue_left(TupleChange::put(0, (1, 30)));
            assert_eq!(n.rows().map(|(_, v)| *v).collect::<Vec<_>>(), vec![(1, 10)]);
            assert_eq!(
                n.propagate(),
                vec![TupleLifecycleEvent::Update {
                    id: 0,
                    value: (1, 30)
                }]
            );
            n.enqueue_right(TupleChange::put(1, 1));
            n.enqueue_right(TupleChange::retract(1));
            let events = n.propagate();
            if mode == ExistenceMode::Exists {
                assert!(events.is_empty());
            } else {
                assert_eq!(
                    events,
                    vec![TupleLifecycleEvent::Update {
                        id: 0,
                        value: (1, 30)
                    }]
                );
            }
            n.enqueue_left(TupleChange::retract(0));
            assert_eq!(n.propagate(), vec![TupleLifecycleEvent::Retract { id: 0 }]);
            assert!(n.propagate().is_empty());
            assert_eq!(n.pending_len(), 0);
        }
    }
}
#[test]
fn evaluation_timing_retry_and_discard_preserve_committed_rows() {
    for timing in [ExistenceEvaluation::Eager, ExistenceEvaluation::Deferred] {
        let fail = Cell::new(false);
        let calls = Cell::new(0);
        let mut n = BatchedExistsNode::new(
            ExistenceMode::Exists,
            timing,
            |_: &i32| (),
            |_: &i32| (),
            |l, r| {
                calls.set(calls.get() + 1);
                assert!(!fail.get());
                l > r
            },
        );
        n.enqueue_left(TupleChange::put(0, 3));
        n.enqueue_right(TupleChange::put(0, 1));
        assert_eq!(
            calls.get(),
            if timing == ExistenceEvaluation::Eager {
                1
            } else {
                0
            }
        );
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Insert { id: 0, value: 3 }]
        );
        assert_eq!(calls.get(), 1);
        n.enqueue_left(TupleChange::put(1, 5));
        fail.set(true);
        if timing == ExistenceEvaluation::Eager {
            assert!(
                catch_unwind(AssertUnwindSafe(|| n.enqueue_right(TupleChange::put(1, 2)))).is_err()
            );
            assert_eq!(n.pending_len(), 1);
        } else {
            n.enqueue_right(TupleChange::put(1, 2));
            assert!(catch_unwind(AssertUnwindSafe(|| n.propagate())).is_err());
            assert_eq!(n.pending_len(), 2);
        }
        assert_eq!(
            n.rows().map(|(id, v)| (*id, *v)).collect::<Vec<_>>(),
            vec![(0, 3)]
        );
        fail.set(false);
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Insert { id: 1, value: 5 }]
        );
        n.enqueue_left(TupleChange::retract(0));
        n.enqueue_right(TupleChange::retract(0));
        n.discard_pending();
        assert_eq!(n.pending_len(), 0);
        assert!(n.propagate().is_empty());
        assert_eq!(n.rows().count(), 2);
        n.enqueue_left(TupleChange::retract(0));
        assert_eq!(n.propagate(), vec![TupleLifecycleEvent::Retract { id: 0 }]);
    }
}
#[test]
fn eager_and_deferred_rounds_match_independent_scan_under_mixed_churn() {
    type Row = (i32, i32);
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        let make = |timing| {
            BatchedExistsNode::new(mode, timing, |r: &Row| r.0, |r: &Row| r.0, |l, r| l.1 < r.1)
        };
        let mut eager = make(ExistenceEvaluation::Eager);
        let mut deferred = make(ExistenceEvaluation::Deferred);
        let mut left = BTreeMap::new();
        let mut right = BTreeMap::new();
        let mut live = BTreeMap::new();
        let mut seed = 31289u64;
        for round in 0..160 {
            let mut nl = left.clone();
            let mut nr = right.clone();
            for step in 0..7 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let x = seed >> 32;
                let id = (x % 13) as usize;
                let value =
                    (!x.is_multiple_of(4)).then_some(((x / 13 % 3) as i32, (x / 39 % 9) as i32));
                let target = if step % 2 == 0 { &mut nl } else { &mut nr };
                if let Some(v) = value {
                    target.insert(id, v);
                } else {
                    target.remove(&id);
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
            if round % 9 == 0 {
                eager.discard_pending();
                deferred.discard_pending();
            } else {
                left = nl;
                right = nr;
            }
            let events = eager.propagate();
            assert_eq!(events, deferred.propagate());
            let mut ids = std::collections::BTreeSet::new();
            for event in events {
                assert!(
                    ids.insert(*event.id()),
                    "at most one event per ID per round"
                );
                match event {
                    TupleLifecycleEvent::Insert { id, value } => {
                        assert!(live.insert(id, value).is_none())
                    }
                    TupleLifecycleEvent::Update { id, value } => {
                        assert!(live.insert(id, value).is_some())
                    }
                    TupleLifecycleEvent::Retract { id } => assert!(live.remove(&id).is_some()),
                }
            }
            let wanted = left
                .iter()
                .filter(|(_, l)| {
                    right.values().any(|r| l.0 == r.0 && l.1 < r.1)
                        == (mode == ExistenceMode::Exists)
                })
                .map(|(id, v)| (*id, *v))
                .collect::<BTreeMap<_, _>>();
            assert_eq!(live, wanted);
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
    }
}

#[test]
fn propagation_clone_panic_retains_the_round_for_retry() {
    use std::rc::Rc;
    struct Row {
        value: i32,
        fail: Rc<Cell<bool>>,
    }
    impl Clone for Row {
        fn clone(&self) -> Self {
            assert!(!self.fail.get(), "payload clone panic");
            Self {
                value: self.value,
                fail: self.fail.clone(),
            }
        }
    }
    for timing in [ExistenceEvaluation::Eager, ExistenceEvaluation::Deferred] {
        let fail = Rc::new(Cell::new(false));
        let mut n = BatchedExistsNode::<usize, usize, Row, i32, ()>::new(
            ExistenceMode::NotExists,
            timing,
            |_: &Row| (),
            |_: &i32| (),
            |_, _| true,
        );
        n.enqueue_left(TupleChange::put(
            0,
            Row {
                value: 7,
                fail: fail.clone(),
            },
        ));
        fail.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| n.propagate())).is_err());
        assert_eq!(n.pending_len(), 1);
        assert_eq!(n.rows().count(), 0);
        fail.set(false);
        let events = n.propagate();
        assert_eq!(events.len(), 1);
        match &events[0] {
            TupleLifecycleEvent::Insert { id, value } => {
                assert_eq!(*id, 0);
                assert_eq!(value.value, 7);
            }
            _ => panic!("expected first insertion"),
        }
        assert_eq!(n.pending_len(), 0);
        assert_eq!(n.rows().count(), 1);
        assert!(n.propagate().is_empty());
    }
}
