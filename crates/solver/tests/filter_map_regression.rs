// Original native integration regressions; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

#[test]
fn direct_non_clone_payloads_retract_only_accepted_identities() {
    struct Input(i64);
    struct Output(i64);
    let calls = Cell::new(0);
    let mut node = FilterMapNode::new(|value: &Input| {
        calls.set(calls.get() + 1);
        assert_ne!(value.0, 99, "projection panic");
        (value.0 > 0).then_some(Output(value.0 * 2))
    });
    let output = node.apply(TupleChange::put(1, Input(3))).unwrap();
    assert_eq!(output.id, 1);
    assert_eq!(output.value.unwrap().0, 6);
    assert_eq!(node.accepted_count(), 1);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || node.apply(TupleChange::put(1, Input(99)))
        ))
        .is_err()
    );
    assert_eq!(node.accepted_count(), 1);
    let output = node.apply(TupleChange::put(1, Input(0))).unwrap();
    assert_eq!(output.id, 1);
    assert!(output.value.is_none());
    assert_eq!(node.accepted_count(), 0);
    assert!(node.apply(TupleChange::retract(1)).is_none());
    assert!(node.apply(TupleChange::put(1, Input(-1))).is_none());
    assert!(node.apply(TupleChange::retract(1)).is_none());
    let _ = node.apply(TupleChange::put(1, Input(4))).unwrap();
    assert!(node.apply(TupleChange::retract(1)).unwrap().value.is_none());
    assert_eq!(calls.get(), 5);
}

#[test]
fn downstream_panics_rollback_membership_and_source_without_replay() {
    let calls = Cell::new(0);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, i64>();
    let mapped = builder
        .filter_map(source.port(), |v| {
            calls.set(calls.get() + 1);
            (*v > 0).then_some(*v)
        })
        .unwrap();
    builder
        .score("checked", &mapped, |v| {
            assert_ne!(*v, 99);
            SimpleScore::new(*v)
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, 99)).unwrap();
            tx.commit().unwrap();
        }))
        .is_err()
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(graph.score(), SimpleScore::new(5));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::ZERO);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 3);
    graph.assert_consistent().unwrap();
}

#[test]
fn rejected_rows_remain_structurally_active_and_foreign_ports_fail() {
    let calls = Cell::new(0);
    let initial = Cell::new(None);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let source = builder.source::<usize, i64>();
    let mapped = builder
        .filter_map(source.port(), |_| {
            calls.set(calls.get() + 1);
            None::<i64>
        })
        .unwrap();
    builder
        .score("none", &mapped, |_| SimpleScore::new(1))
        .unwrap();
    builder
        .after_initial_facts(&mapped, |can| initial.set(Some(can)))
        .unwrap();
    let mut other = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        other.filter_map(source.port(), |v| Some(*v)),
        Err(GraphError::ForeignPort)
    ));
    let mut graph = builder.build();
    assert_eq!(graph.activity().active_count(), 0);
    assert_eq!(calls.get(), 0);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 5)).unwrap();
    tx.commit().unwrap();
    assert!(graph.activity().get(&mapped).unwrap().active);
    assert_eq!(graph.finish_initial_facts(), 1);
    assert_eq!(initial.get(), Some(true));
    assert_eq!(graph.score(), SimpleScore::ZERO);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.activity().active_count(), 0);
    assert_eq!(graph.finish_initial_facts(), 0);
}
