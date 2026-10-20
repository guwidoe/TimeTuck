//! Original scratch-buffer controls; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

fn apply<I: 'static, T: 'static>(
    node: &mut impl Node<SimpleScore>,
    right: bool,
    change: TupleChange<I, T>,
) -> Outputs {
    let event = Rc::new(change) as Event;
    node.apply(
        right,
        &event,
        NodeArithmetic {
            impact: crate::ScoringArithmetic::Checked,
            totals: crate::ScoringArithmetic::Checked,
        },
    )
    .unwrap()
}

fn rows(outputs: &Outputs) -> Vec<TupleChange<(usize, usize), (i64, i64)>> {
    outputs
        .iter()
        .map(|event| typed::<(usize, usize), (i64, i64)>(event).clone())
        .collect()
}

#[test]
fn join_buffer_reuses_both_directions_and_preserves_order_and_snapshots() {
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        for bound in [false, true] {
            let mut node = Join::new(JoinNode::<usize, usize, _, _, _>::comparing(
                comparison,
                |v: &i64| *v,
                |v: &i64| *v,
            ));
            let mut reference = JoinNode::comparing(comparison, |v: &i64| *v, |v: &i64| *v);
            if bound {
                node.0.bind_fixed_input(false, 8);
                node.0.bind_fixed_input(true, 8);
                reference.bind_fixed_input(false, 8);
                reference.bind_fixed_input(true, 8);
            }
            let ids = if bound {
                [0usize, 3, 7]
            } else {
                [0usize, 3, usize::MAX]
            };
            for (i, id) in ids.into_iter().enumerate() {
                let change = TupleChange::put(id, i as i64);
                assert_eq!(
                    rows(&apply(&mut node, true, change.clone())),
                    reference.apply_right(change)
                );
                let change = TupleChange::put(id, 2 - i as i64);
                assert_eq!(
                    rows(&apply(&mut node, false, change.clone())),
                    reference.apply_left(change)
                );
            }
            for (right, id, value) in [(false, ids[0], -2i64), (true, ids[2], 4i64)] {
                let change = TupleChange::put(id, value);
                let expected = if right {
                    reference.apply_right(change.clone())
                } else {
                    reference.apply_left(change.clone())
                };
                assert_eq!(rows(&apply(&mut node, right, change)), expected);
            }
            // Repeated same-size updates retain the actual allocation on both sides.
            for right in [false, true] {
                apply(&mut node, right, TupleChange::put(ids[1], 1i64));
                let buffer = if right { &node.2 } else { &node.1 };
                let (pointer, capacity) = (buffer.as_ptr(), buffer.capacity());
                assert!(capacity > 0);
                apply(&mut node, right, TupleChange::put(ids[1], 1i64));
                let buffer = if right { &node.2 } else { &node.1 };
                assert_eq!((buffer.as_ptr(), buffer.capacity()), (pointer, capacity));
                assert!(buffer.is_empty());
            }
            // Start a common base after the reuse probes, then exercise saved
            // alternate trials and an outer rollback without checkpointing scratch.
            let original = node
                .0
                .matches()
                .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
                .collect::<Vec<_>>();
            let outer = <_ as Node<SimpleScore>>::begin_checkpoint(&mut node);
            apply(&mut node, false, TupleChange::put(ids[1], 4i64));
            let saved = <_ as Node<SimpleScore>>::capture_checkpoint(&node, outer.as_ref());
            let expected = node
                .0
                .matches()
                .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
                .collect::<Vec<_>>();
            apply(&mut node, true, TupleChange::<usize, i64>::retract(ids[0]));
            <_ as Node<SimpleScore>>::restore(&mut node, saved);
            assert_eq!(
                node.0
                    .matches()
                    .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
                    .collect::<Vec<_>>(),
                expected
            );
            <_ as Node<SimpleScore>>::restore(&mut node, outer);
            assert_eq!(
                node.0
                    .matches()
                    .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
                    .collect::<Vec<_>>(),
                original
            );
        }
    }
}

#[test]
fn join_buffer_discards_oversized_capacity_and_empty_retractions_emit_nothing() {
    let mut node = Join::new(JoinNode::<usize, usize, _, _, _>::new(
        |_: &i64| 0usize,
        |_: &i64| 0usize,
    ));
    for id in 0usize..2048 {
        node.0.apply_right(TupleChange::put(id, id as i64));
    }
    assert_eq!(
        apply(&mut node, false, TupleChange::put(usize::MAX, 1i64)).len(),
        2048
    );
    assert_eq!(node.1.capacity(), 0);
    assert_eq!(
        apply(
            &mut node,
            false,
            TupleChange::<usize, i64>::retract(usize::MAX)
        )
        .len(),
        2048
    );
    assert_eq!(node.1.capacity(), 0);
    assert!(
        apply(
            &mut node,
            false,
            TupleChange::<usize, i64>::retract(usize::MAX)
        )
        .is_empty()
    );
    let retained = retain_join_buffer::<u8>(Vec::with_capacity(JOIN_BUFFER_BYTES));
    assert_eq!(retained.capacity(), JOIN_BUFFER_BYTES);
    assert_eq!(
        retain_join_buffer::<u8>(Vec::with_capacity(JOIN_BUFFER_BYTES + 1)).capacity(),
        0
    );
}

thread_local! {
    static TRACE: RefCell<Vec<(&'static str, i64)>> = const { RefCell::new(Vec::new()) };
    static CLONES: Cell<usize> = const { Cell::new(0) };
    static FAIL_CLONE: Cell<usize> = const { Cell::new(usize::MAX) };
    static FAIL_KEY: Cell<bool> = const { Cell::new(false) };
}
#[derive(Debug)]
struct Row(i64); // NoDrop, but Clone is deliberately observable and fallible.
impl Clone for Row {
    fn clone(&self) -> Self {
        TRACE.with(|trace| trace.borrow_mut().push(("clone", self.0)));
        CLONES.with(|count| {
            let next = count.get() + 1;
            count.set(next);
            FAIL_CLONE.with(|fail| assert_ne!(next, fail.get(), "clone failure"));
        });
        Self(self.0)
    }
}
fn key(row: &Row) -> usize {
    TRACE.with(|trace| trace.borrow_mut().push(("key", row.0)));
    FAIL_KEY.with(|fail| assert!(!fail.get(), "key failure"));
    0
}
type Observation = (
    bool,
    Vec<(&'static str, i64)>,
    Vec<((usize, usize), (i64, i64))>,
);
fn observe(fast: bool, right: bool, failure: u8) -> Observation {
    FAIL_CLONE.with(|fail| fail.set(usize::MAX));
    FAIL_KEY.with(|fail| fail.set(false));
    let mut node = Join::new(JoinNode::<usize, usize, Row, Row, usize>::new(key, key));
    node.3 = fast;
    for id in 0usize..3 {
        apply(&mut node, true, TupleChange::put(id, Row(10 + id as i64)));
        apply(&mut node, false, TupleChange::put(id, Row(20 + id as i64)));
    }
    let original = node
        .0
        .matches()
        .map(|((i, j), (l, r))| ((*i, *j), (l.0, r.0)))
        .collect::<Vec<_>>();
    let saved = node.0.checkpoint();
    TRACE.with(|trace| trace.borrow_mut().clear());
    CLONES.with(|count| count.set(0));
    FAIL_CLONE.with(|fail| fail.set(if failure == 1 { 4 } else { usize::MAX }));
    FAIL_KEY.with(|fail| fail.set(failure == 2));
    let result = catch_unwind(AssertUnwindSafe(|| {
        apply(&mut node, right, TupleChange::put(1usize, Row(99)))
    }));
    if fast && result.is_err() {
        assert_eq!(
            if right {
                node.2.capacity()
            } else {
                node.1.capacity()
            },
            0
        );
        assert_eq!(
            node.0
                .matches()
                .map(|((i, j), (l, r))| ((*i, *j), (l.0, r.0)))
                .collect::<Vec<_>>(),
            original
        );
    }
    let outputs = result.as_ref().ok().map(|out| {
        out.iter()
            .map(|event| {
                let change = typed::<(usize, usize), (Row, Row)>(event);
                (change.id, change.value.as_ref().map(|(l, r)| (l.0, r.0)))
            })
            .collect::<Vec<_>>()
    });
    let trace = TRACE.with(|trace| trace.borrow().clone());
    FAIL_CLONE.with(|fail| fail.set(usize::MAX));
    FAIL_KEY.with(|fail| fail.set(false));
    node.0.restore(saved);
    assert_eq!(
        node.0
            .matches()
            .map(|((i, j), (l, r))| ((*i, *j), (l.0, r.0)))
            .collect::<Vec<_>>(),
        original
    );
    (
        result.is_err(),
        trace,
        outputs
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, row)| row.map(|row| (id, row)))
            .collect(),
    )
}

#[test]
fn join_buffer_preserves_nodrop_clone_callback_order_and_panics() {
    for right in [false, true] {
        for failure in 0..3 {
            assert_eq!(
                observe(true, right, failure),
                observe(false, right, failure)
            );
        }
    }
}

#[derive(Clone)]
struct Dropped(i64);
impl Drop for Dropped {
    fn drop(&mut self) {
        TRACE.with(|trace| trace.borrow_mut().push(("drop", self.0)));
    }
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CustomId(usize); // Even NoDrop arbitrary IDs keep the legacy path.

#[test]
fn join_buffer_custom_ownership_and_identity_keep_legacy_path() {
    let mut node = Join::new(JoinNode::<usize, usize, Dropped, Dropped, usize>::new(
        |_| 0,
        |_| 0,
    ));
    assert!(!node.3);
    apply(&mut node, true, TupleChange::put(0usize, Dropped(1)));
    apply(&mut node, false, TupleChange::put(0usize, Dropped(2)));
    assert_eq!((node.1.capacity(), node.2.capacity()), (0, 0));
    let node = Join::new(JoinNode::<CustomId, usize, i64, i64, usize>::new(
        |_| 0,
        |_| 0,
    ));
    assert!(!node.3);
    let node = Join::new(JoinNode::<
        (usize, usize),
        ((usize, usize), usize),
        i64,
        i64,
        usize,
    >::new(|_| 0, |_| 0));
    assert!(node.3);
}

#[test]
fn join_buffer_default_batch_preserves_self_join_fifo() {
    fn run(fast: bool) -> Vec<TupleChange<(usize, usize), (i64, i64)>> {
        let mut node = Join::new(JoinNode::<usize, usize, i64, i64, usize>::new(|_| 0, |_| 0));
        node.3 = fast;
        let inputs = [
            TupleChange::put(0usize, 4i64),
            TupleChange::put(1, 8),
            TupleChange::put(0, 9),
            TupleChange::retract(1),
        ]
        .into_iter()
        .flat_map(|change| {
            let event = Rc::new(change) as Event;
            [(false, event.clone()), (true, event)]
        })
        .collect::<Vec<_>>();
        let outputs = <_ as Node<SimpleScore>>::apply_batch(
            &mut node,
            &inputs,
            NodeArithmetic {
                impact: crate::ScoringArithmetic::Checked,
                totals: crate::ScoringArithmetic::Checked,
            },
        )
        .unwrap();
        outputs
            .iter()
            .map(|event| typed::<(usize, usize), (i64, i64)>(event).clone())
            .collect()
    }
    let fast = run(true);
    assert_eq!(fast, run(false));
    assert_eq!(fast.first(), Some(&TupleChange::put((0, 0), (4, 4))));
    assert_eq!(fast.last(), Some(&TupleChange::retract((0, 1))));
}
