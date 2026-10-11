//! Original persistent-consumer stream regression; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

type Row = (Option<usize>, i64);
#[test]
fn high_fanout_stream_trials_restore_routes_scores_and_failure_state() {
    let calls = RefCell::new(vec![]);
    let panic_at = Cell::new(None);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, Row>();
    let derived = builder
        .derive_dependencies_batched(
            source.port(),
            0..258,
            |row| row.0.into_iter().collect(),
            |key, row, inputs| {
                calls.borrow_mut().push(*key);
                assert_ne!(panic_at.get(), Some(*key), "supplier failure");
                Some(row.1 + inputs.first().and_then(|(_, v)| v.value).unwrap_or(0))
            },
        )
        .unwrap();
    let constraint = builder
        .score("derived", &derived, |(_, value)| {
            SimpleScore::new(value.value.unwrap_or(-1000))
        })
        .unwrap();
    let mut graph = builder.build();
    let mut initial = StreamBatch::new();
    initial
        .push(&source, TupleChange::put(0, (None, 1)))
        .unwrap();
    initial
        .push(&source, TupleChange::put(1, (None, 10)))
        .unwrap();
    for key in 2..258 {
        initial
            .push(&source, TupleChange::put(key, (Some(0), 1)))
            .unwrap();
    }
    let mut tx = graph.transaction();
    tx.apply_batch(initial).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(523));
    tx.commit().unwrap();
    let original = graph.constraint_matches(&constraint).unwrap().clone();
    let original_rows = graph.source_rows(&source).unwrap().clone();
    for target in [2, 128, 257, 2] {
        calls.borrow_mut().clear();
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(target, (Some(1), 1)))
            .unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(532));
        assert_eq!(*calls.borrow(), [target]);
        // Only the moved consumer follows source 1 in this trial.
        calls.borrow_mut().clear();
        tx.apply(&source, TupleChange::put(1, (None, 20))).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(552));
        assert_eq!(*calls.borrow(), [1, target]);
        drop(tx);
        assert_eq!(graph.constraint_matches(&constraint).unwrap(), &original);
        assert_eq!(graph.source_rows(&source).unwrap(), &original_rows);
    }
    // A supplier panic during a topology edit cannot publish its reverse links.
    panic_at.set(Some(128));
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                tx.apply(&source, TupleChange::put(128, (Some(1), 1)))
            }))
            .is_err()
        );
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
    }
    panic_at.set(None);
    assert_eq!(graph.constraint_matches(&constraint).unwrap(), &original);
    calls.borrow_mut().clear();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(1, (None, 20))).unwrap();
    assert_eq!(*calls.borrow(), [1]);
    assert_eq!(tx.score().unwrap(), SimpleScore::new(533));
    drop(tx);
    // Creating a real cycle suppresses all of source 0's consumers. Undo then
    // restores the complete high-fan-out reverse index for the next change.
    calls.borrow_mut().clear();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (Some(128), 1)))
        .unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(-256_990));
    assert!(calls.borrow().is_empty());
    drop(tx);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (None, 2))).unwrap();
    assert_eq!(
        *calls.borrow(),
        std::iter::once(0).chain(2..258).collect::<Vec<_>>()
    );
    assert_eq!(tx.score().unwrap(), SimpleScore::new(780));
    drop(tx);
    assert_eq!(graph.constraint_matches(&constraint).unwrap(), &original);
    graph.assert_consistent().unwrap();
}
