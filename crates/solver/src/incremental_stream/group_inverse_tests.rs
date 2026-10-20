//! Per-edit inverse group controls; no upstream-method credit.
use super::*;
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

fn node(inverse: bool) -> GroupNode<usize, (usize, usize)> {
    let mut node = GroupNode::new();
    assert!(node.bind_primitive_group_keys());
    if !inverse {
        node.groups.disable_inverse_for_tests();
    }
    assert!(node.bind_fixed_inputs(8));
    for (id, key, value) in [
        (0usize, (0usize, 15usize), 1i64),
        (1, (0, 15), 2),
        (2, (usize::MAX, usize::MAX), 7),
    ] {
        node.apply(TupleChange::put(id, (key, value)));
    }
    node
}
fn contents(node: &GroupNode<usize, (usize, usize)>) -> Vec<((usize, usize), IntSummary)> {
    node.groups()
        .map(|(key, summary)| (*key, summary))
        .collect()
}
fn apply_pair(
    a: &mut GroupNode<usize, (usize, usize)>,
    b: &mut GroupNode<usize, (usize, usize)>,
    change: TupleChange<usize, ((usize, usize), i64)>,
) {
    assert_eq!(a.apply_inline(change.clone()), b.apply_inline(change));
    assert_eq!(contents(a), contents(b));
}

#[test]
fn inverse_group_mixed_edits_nested_captures_and_singleton_rebirth_match_original_group_route() {
    let mut fast = node(true);
    let mut old = node(false);
    let original = contents(&fast);
    let outer = fast.begin_journal_checkpoint();
    let old_outer = old.begin_journal_checkpoint();
    apply_pair(
        &mut fast,
        &mut old,
        TupleChange::put(0usize, ((0usize, 15usize), 3i64)),
    );
    apply_pair(
        &mut fast,
        &mut old,
        TupleChange::put(1usize, ((0usize, 15usize), 4i64)),
    );
    // First full observer appears only after both in-place edits.
    let full = fast.checkpoint();
    let old_full = old.checkpoint();
    let middle = contents(&fast);
    let inner = fast.begin_journal_checkpoint();
    let old_inner = old.begin_journal_checkpoint();
    apply_pair(
        &mut fast,
        &mut old,
        TupleChange::put(0usize, ((usize::MAX, usize::MAX), i64::MAX)),
    );
    apply_pair(&mut fast, &mut old, TupleChange::retract(2usize));
    // The sole remaining occurrence replaces its key representative as before.
    let owner = fast
        .groups
        .get_key_value(&(usize::MAX, usize::MAX))
        .unwrap()
        .0 as *const _;
    apply_pair(
        &mut fast,
        &mut old,
        TupleChange::put(0usize, ((usize::MAX, usize::MAX), i64::MIN)),
    );
    assert_ne!(
        fast.groups
            .get_key_value(&(usize::MAX, usize::MAX))
            .unwrap()
            .0 as *const _,
        owner
    );
    fast.commit_journal_checkpoint(inner);
    old.commit_journal_checkpoint(old_inner);
    let saved = fast.capture_checkpoint(outer.as_ref());
    let old_saved = old.capture_checkpoint(old_outer.as_ref());
    let expected = contents(&fast);
    fast.restore_checkpoint(outer);
    old.restore_checkpoint(old_outer);
    assert_eq!(contents(&fast), original);
    assert_eq!(contents(&fast), contents(&old));
    for value in [10i64, 20, 30] {
        let trial = fast.begin_journal_checkpoint();
        let old_trial = old.begin_journal_checkpoint();
        apply_pair(
            &mut fast,
            &mut old,
            TupleChange::put(1usize, ((0usize, 15usize), value)),
        );
        fast.restore_checkpoint(trial);
        old.restore_checkpoint(old_trial);
    }
    let enclosing = fast.begin_journal_checkpoint();
    let old_enclosing = old.begin_journal_checkpoint();
    fast.restore_checkpoint(saved);
    old.restore_checkpoint(old_saved);
    assert_eq!(contents(&fast), expected);
    assert_eq!(contents(&fast), contents(&old));
    fast.restore_checkpoint(full);
    old.restore_checkpoint(old_full);
    assert_eq!(contents(&fast), middle);
    assert_eq!(contents(&fast), contents(&old));
    fast.restore_checkpoint(enclosing);
    old.restore_checkpoint(old_enclosing);
    assert_eq!(contents(&fast), original);
    assert_eq!(contents(&fast), contents(&old));
    // The full frequency state must support subsequent removals, not just a
    // summary equal at the capture boundary.
    apply_pair(&mut fast, &mut old, TupleChange::retract(0usize));
    apply_pair(&mut fast, &mut old, TupleChange::retract(1usize));
}

#[test]
fn inverse_group_bound_publication_failure_poison_and_rollback_match_original_group_route() {
    let mut fast = node(true);
    let mut old = node(false);
    let original = contents(&fast);
    for node in [&mut fast, &mut old] {
        let checkpoint = node.begin_journal_checkpoint();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                // Group mutation succeeds before fixed-input publication rejects ID.
                node.apply_inline(TupleChange::put(8usize, ((0usize, 15usize), 9i64)));
            }))
            .is_err()
        );
        assert!(node.is_poisoned());
        node.restore_checkpoint(checkpoint);
        assert!(!node.is_poisoned());
        assert_eq!(contents(node), original);
    }
    apply_pair(
        &mut fast,
        &mut old,
        TupleChange::put(0usize, ((0usize, 15usize), 5i64)),
    );
}

#[test]
fn inverse_graph_capture_after_multiple_scalar_edits_installs_without_callback_replay() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut builder = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
    let source = builder.source::<usize, (usize, i64)>();
    let grouped = builder
        .group_collect(source.port(), {
            let trace = trace.clone();
            move |&(key, value)| {
                trace.borrow_mut().push(("map", value));
                ((key, usize::MAX), value)
            }
        })
        .unwrap();
    let dataset = builder.dataset(&grouped).unwrap();
    builder
        .score("sum", &grouped, {
            let trace = trace.clone();
            move |(_, summary)| {
                trace.borrow_mut().push(("score", summary.sum));
                crate::SimpleScore::new(summary.sum)
            }
        })
        .unwrap();
    let mut graph = builder.build();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 1i64)))
            .unwrap();
        tx.apply(&source, TupleChange::put(1usize, (0usize, 2i64)))
            .unwrap();
        tx.commit().unwrap();
    }
    trace.borrow_mut().clear();
    let (captured, full, expected) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 4i64)))
            .unwrap();
        tx.apply(&source, TupleChange::put(1usize, (0usize, 5i64)))
            .unwrap();
        assert_eq!(
            &*trace.borrow(),
            &[("map", 4), ("score", 6), ("map", 5), ("score", 9)]
        );
        let full = tx.graph().unwrap().capture_full();
        let expected = tx.graph().unwrap().dataset_rows(&dataset).unwrap().clone();
        let captured = tx.capture().unwrap();
        tx.apply(&source, TupleChange::put(0usize, (1usize, 7i64)))
            .unwrap();
        (captured, full, expected)
    };
    assert_eq!(graph.score(), crate::SimpleScore::new(3));
    for value in [8i64, 9] {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, value)))
            .unwrap();
    }
    let before = trace.borrow().clone();
    graph.install(captured);
    assert_eq!(*trace.borrow(), before);
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &expected);
    assert_eq!(graph.score(), crate::SimpleScore::new(9));
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::retract(1usize)).unwrap();
        tx.commit().unwrap();
    }
    let before = trace.borrow().clone();
    graph.install(full);
    assert_eq!(*trace.borrow(), before);
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &expected);
    assert_eq!(graph.score(), crate::SimpleScore::new(9));
    graph.assert_consistent().unwrap();
}

#[test]
fn inverse_group_graph_total_overflow_restores_inputs_collectors_matches_and_callback_phase() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut builder = crate::StreamGraphBuilder::<crate::SimpleScore>::new();
    let source = builder.source::<usize, (usize, i64)>();
    let constant = builder.source::<usize, i64>();
    let grouped = builder
        .group_collect(source.port(), |&(key, value)| (key, value))
        .unwrap();
    let dataset = builder.dataset(&grouped).unwrap();
    builder
        .score("groups", &grouped, {
            let trace = trace.clone();
            move |(_, summary)| {
                trace.borrow_mut().push(summary.sum);
                crate::SimpleScore::new(summary.sum)
            }
        })
        .unwrap();
    builder
        .score("constant", constant.port(), |value| {
            crate::SimpleScore::new(*value)
        })
        .unwrap();
    let mut graph = builder.build();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 1i64)))
            .unwrap();
        tx.apply(&source, TupleChange::put(1usize, (0usize, 2i64)))
            .unwrap();
        tx.apply(&constant, TupleChange::put(0usize, i64::MAX - 5))
            .unwrap();
        tx.commit().unwrap();
    }
    let baseline = graph.dataset_rows(&dataset).unwrap().clone();
    trace.borrow_mut().clear();
    {
        let mut tx = graph.transaction();
        assert!(
            tx.apply(&source, TupleChange::put(0usize, (0usize, 11i64)))
                .is_err()
        );
    }
    assert_eq!(
        *trace.borrow(),
        [13],
        "the delivered scoring callback precedes total overflow"
    );
    assert_eq!(graph.score(), crate::SimpleScore::new(i64::MAX - 2));
    assert_eq!(graph.dataset_rows(&dataset).unwrap(), &baseline);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0usize, (0usize, 0i64)))
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), crate::SimpleScore::new(i64::MAX - 3));
    graph.assert_consistent().unwrap();
}
