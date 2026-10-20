//! Original routing workspace regressions; no upstream source-method credit.
use super::*;
use crate::{SimpleScore, StreamBatch};
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn storage<S: AdditiveScore>(g: &StreamGraph<'_, S>) -> (usize, usize) {
    let w = &g.propagation_workspace;
    assert!(w.pending.is_empty());
    assert!(w.arena_pending.is_empty());
    assert!(w.arena.is_empty());
    assert!(w.changes.is_empty());
    (w.arena_pending.capacity(), w.changes.capacity())
}

#[test]
fn fifo_boundaries_reuse_buffers_across_commit_drop_and_nested_diagnostics() {
    let calls = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let mapped = b
        .map(source.port(), |v| {
            calls.borrow_mut().push(("map", *v));
            v + 1
        })
        .unwrap();
    b.score("mapped", &mapped, |v| {
        calls.borrow_mut().push(("mapped", *v));
        SimpleScore::new(*v)
    })
    .unwrap();
    b.score("direct", source.port(), |v| {
        calls.borrow_mut().push(("direct", *v));
        SimpleScore::new(*v)
    })
    .unwrap();
    let mut g = b.build();
    for (value, commit) in [(3, true), (7, false), (9, true)] {
        calls.borrow_mut().clear();
        let original = g.score();
        let old_storage = storage(&g);
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, value)).unwrap();
        assert_eq!(
            *calls.borrow(),
            [("map", value), ("direct", value), ("mapped", value + 1)]
        );
        assert_eq!(tx.score_evaluations(), 2);
        assert_eq!(tx.score().unwrap(), SimpleScore::new(value * 2 + 1));
        let allocated = storage(tx.graph);
        assert!(allocated.0 >= 2 && allocated.1 >= 2);
        if value != 3 {
            assert_eq!(allocated, old_storage);
        }
        // Diagnostic replay borrows this graph while the outer transaction still
        // owns live checkpoints, and must return its own empty routing storage.
        tx.assert_consistent().unwrap();
        assert_eq!(tx.score_evaluations(), 2);
        assert_eq!(storage(tx.graph), allocated);
        if commit {
            tx.commit().unwrap();
        } else {
            drop(tx);
        }
        assert_eq!(storage(&g), allocated);
        assert_eq!(
            g.score(),
            if commit {
                SimpleScore::new(value * 2 + 1)
            } else {
                original
            }
        );
    }
    assert_eq!(g.source_rows(&source).unwrap()[&0], 9);
    g.assert_consistent().unwrap();
}

#[test]
fn errors_and_panics_clear_pending_events_and_score_deltas_before_reuse() {
    for failure in 0..4 {
        let fail = Cell::new(false);
        let payload = Rc::new(7_i64);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, Rc<i64>>();
        b.score("first", source.port(), |v| {
            if fail.get() && failure == 0 {
                panic!("first callback");
            }
            SimpleScore::new(**v)
        })
        .unwrap();
        b.score("second", source.port(), |v| {
            if fail.get() && failure == 1 {
                panic!("second callback");
            }
            if fail.get() && failure == 2 {
                return SimpleScore::new(**v).with_structural(1);
            }
            if fail.get() && failure == 3 {
                return SimpleScore::new(i64::MAX);
            }
            SimpleScore::new(**v)
        })
        .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0, payload.clone()))
                .unwrap();
            tx.commit().unwrap();
        }
        let retained = Rc::strong_count(&payload);
        fail.set(true);
        let mut tx = g.transaction();
        let result = catch_unwind(AssertUnwindSafe(|| {
            tx.apply(&source, TupleChange::put(1, payload.clone()))
        }));
        if failure < 2 {
            assert!(result.is_err());
        } else if failure == 2 {
            assert!(matches!(
                result.unwrap(),
                Err(GraphError::StructuralContribution(_))
            ));
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(GraphError::ScoreOverflow { .. })
            ));
        }
        storage(tx.graph);
        assert!(matches!(tx.score(), Err(GraphError::FailedTransaction)));
        assert!(matches!(
            tx.apply(&source, TupleChange::retract(0)),
            Err(GraphError::FailedTransaction)
        ));
        drop(tx);
        assert_eq!(Rc::strong_count(&payload), retained);
        assert_eq!(g.score(), SimpleScore::new(14));
        assert_eq!(g.source_rows(&source).unwrap().len(), 1);
        fail.set(false);
        g.assert_consistent().unwrap();
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(1, payload.clone()))
            .unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(28));
        tx.commit().unwrap();
        storage(&g);
    }
}

#[test]
fn per_event_total_overflow_is_not_deferred_across_the_batch() {
    let calls = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    b.score("a", source.port(), |v| {
        calls.borrow_mut().push(("a", *v));
        SimpleScore::new(*v)
    })
    .unwrap();
    b.score("b", source.port(), |_| SimpleScore::new(1))
        .unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch.push(&source, TupleChange::put(0, i64::MAX)).unwrap();
    batch.push(&source, TupleChange::put(0, 0)).unwrap();
    let mut tx = g.transaction();
    assert!(matches!(
        tx.apply_updates(batch),
        Err(GraphError::ScoreOverflow { constraint: None })
    ));
    assert_eq!(*calls.borrow(), [("a", i64::MAX)]);
    storage(tx.graph);
    drop(tx);
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert!(g.source_rows(&source).unwrap().is_empty());
}
