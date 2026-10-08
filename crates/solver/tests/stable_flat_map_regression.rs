//! Original keyed expansion, panic recovery and transactional graph regressions.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[test]
fn reorder_refreshes_payloads_and_matches_duplicates_in_previous_order() {
    let mut n = StableFlatMapNode::new(Clone::clone, |v: &(char, i32)| v.0);
    assert_eq!(
        n.apply(TupleChange::put(0, vec![('A', 1), ('B', 2), ('A', 3)])),
        vec![
            TupleChange::put((0, 0), ('A', 1)),
            TupleChange::put((0, 1), ('B', 2)),
            TupleChange::put((0, 2), ('A', 3))
        ]
    );
    assert_eq!(
        n.apply(TupleChange::put(0, vec![('B', 20), ('A', 10), ('A', 30)])),
        vec![
            TupleChange::put((0, 1), ('B', 20)),
            TupleChange::put((0, 0), ('A', 10)),
            TupleChange::put((0, 2), ('A', 30))
        ]
    );
    assert_eq!(
        n.apply(TupleChange::put(0, vec![('A', 40)])),
        vec![
            TupleChange::retract((0, 1)),
            TupleChange::retract((0, 2)),
            TupleChange::put((0, 0), ('A', 40))
        ]
    );
    assert_eq!(
        n.apply(TupleChange::put(0, vec![('A', 50), ('B', 60)]))[1].id,
        (0, 3)
    );
    n.apply(TupleChange::put(0, vec![]));
    assert_eq!(n.apply(TupleChange::put(0, vec![('A', 70)]))[0].id, (0, 4));
    n.apply(TupleChange::retract(0));
    assert!(n.apply(TupleChange::retract(0)).is_empty());
    assert_eq!(n.apply(TupleChange::put(0, vec![('A', 80)]))[0].id, (0, 0));
}
#[test]
fn queued_batch_failure_preserves_state_and_pending_work_for_retry() {
    let fail = Cell::new(false);
    let calls = Cell::new(0);
    let mut n = StableFlatMapNode::new(
        |v: &Vec<i32>| {
            calls.set(calls.get() + 1);
            v.clone()
        },
        |v| {
            assert!(!fail.get() || *v != 9);
            *v
        },
    );
    n.apply(TupleChange::put(0, vec![1]));
    n.enqueue(TupleChange::put(0, vec![2]));
    n.enqueue(TupleChange::put(1, vec![9]));
    assert_eq!(calls.get(), 1);
    assert_eq!(n.pending_len(), 2);
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| n.propagate())).is_err());
    assert_eq!(n.pending_len(), 2);
    assert!(catch_unwind(AssertUnwindSafe(|| n.apply(TupleChange::retract(0)))).is_err());
    fail.set(false);
    assert_eq!(
        n.propagate(),
        vec![
            TupleChange::retract((0, 0)),
            TupleChange::put((0, 1), 2),
            TupleChange::put((1, 0), 9)
        ]
    );
    assert_eq!(n.pending_len(), 0);
    assert!(n.propagate().is_empty());
    let old_calls = calls.get();
    n.enqueue(TupleChange::retract(0));
    n.enqueue(TupleChange::retract(1));
    assert_eq!(n.propagate().len(), 2);
    assert_eq!(calls.get(), old_calls);
}
#[test]
fn immediate_mapping_failure_does_not_consume_ids_and_payloads_need_not_clone() {
    struct Payload(i32);
    let fail = Cell::new(false);
    let mut n = StableFlatMapNode::new(
        |_: &()| {
            assert!(!fail.get());
            vec![Payload(3), Payload(3)]
        },
        |p| p.0,
    );
    assert_eq!(n.apply(TupleChange::put(0, ())).len(), 2);
    fail.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| n.apply(TupleChange::put(1, ())))).is_err());
    fail.set(false);
    assert_eq!(n.apply(TupleChange::put(1, ()))[0].id, (1, 0));
    assert_eq!(
        n.apply(TupleChange::retract(0))
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        [(0, 0), (0, 1)]
    );
}
#[test]
fn graph_scores_occurrences_and_restores_identity_after_rollback_and_callback_failure() {
    for prune in [false, true] {
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if prune {
            b = b.with_activity_pruning();
        }
        let s = b.source::<usize, Vec<(char, i64)>>();
        let out = b.flat_map_stable(s.port(), Clone::clone, |p| p.0).unwrap();
        let data = b.dataset(&out).unwrap();
        b.score("sum", &out, |v| {
            assert!(!fail.get() || v.1 != 99);
            SimpleScore::new(v.1)
        })
        .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(0, vec![('A', 1), ('B', 2)]))
            .unwrap();
        tx.commit().unwrap();
        let before = g.dataset_rows(&data).unwrap().clone();
        {
            let mut tx = g.transaction();
            tx.apply(
                &s,
                TupleChange::put(0, vec![('B', 20), ('X', 30), ('B', 40)]),
            )
            .unwrap();
            assert_eq!(tx.score().unwrap(), SimpleScore::new(90));
        }
        assert_eq!(*g.dataset_rows(&data).unwrap(), before);
        assert_eq!(g.score(), SimpleScore::new(3));
        fail.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut tx = g.transaction();
                tx.apply(&s, TupleChange::put(0, vec![('X', 99)])).unwrap();
                tx.commit().unwrap();
            }))
            .is_err()
        );
        fail.set(false);
        assert_eq!(*g.dataset_rows(&data).unwrap(), before);
        let mut tx = g.transaction();
        tx.apply(
            &s,
            TupleChange::put(0, vec![('B', 20), ('X', 30), ('B', 40)]),
        )
        .unwrap();
        tx.commit().unwrap();
        let rows = g.dataset_rows(&data).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[&(0, 1)], ('B', 20));
        assert_eq!(rows[&(0, 2)], ('X', 30));
        assert_eq!(rows[&(0, 3)], ('B', 40));
        assert_eq!(g.score(), SimpleScore::new(90));
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::retract(0)).unwrap();
        tx.commit().unwrap();
        assert!(g.dataset_rows(&data).unwrap().is_empty());
        assert_eq!(g.score(), SimpleScore::ZERO);
    }
}
