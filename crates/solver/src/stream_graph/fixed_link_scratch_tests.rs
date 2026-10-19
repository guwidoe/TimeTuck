//! Original fixed-link scratch/ownership controls; no upstream source credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Trace = Rc<RefCell<Vec<(&'static str, i64, i64)>>>;
struct Row(i64, Trace, Rc<Cell<u8>>);
impl Clone for Row {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push(("clone", self.0, 0));
        assert!(!(self.2.get() == 2 && self.0 == 9), "endpoint clone");
        Self(self.0, self.1.clone(), self.2.clone())
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        self.1.borrow_mut().push(("drop", self.0, 0));
    }
}
type Links = FixedLinks<'static, String, String, String, Row, Row, Row>;
fn node(trace: &Trace, failure: &Rc<Cell<u8>>) -> Links {
    let calls = trace.clone();
    let fail = failure.clone();
    let projected = trace.clone();
    let projected_fail = failure.clone();
    let row = |v| Row(v, trace.clone(), failure.clone());
    let mut left = Slots::new(2);
    left.set(0, Some(row(1)));
    left.set(1, Some(row(2)));
    let mut right = Slots::new(2);
    right.set(0, Some(row(3)));
    right.set(1, Some(row(4)));
    FixedLinks {
        direct_left: false,
        direct_right: false,
        links: vec![
            (("e0".to_owned(), "a".to_owned()), "z".to_owned()),
            (("e1".to_owned(), "z".to_owned()), "a".to_owned()),
            (("e2".to_owned(), "a".to_owned()), "a".to_owned()),
        ],
        endpoints: vec![(0, 1), (1, 0), (0, 0)],
        by_left: BTreeMap::from([("a".to_owned(), 0), ("z".to_owned(), 1)]),
        by_right: BTreeMap::from([("a".to_owned(), 0), ("z".to_owned(), 1)]),
        left_links: vec![vec![0, 2], vec![1]],
        right_links: vec![vec![1, 2], vec![0]],
        left,
        right,
        checkpoint_box: CheckpointBox::new(),
        left_changes: Vec::new(),
        right_changes: Vec::new(),
        affected: Vec::new(),
        left_inputs: vec![usize::MAX; 2],
        right_inputs: vec![usize::MAX; 2],
        inputs_indexed: false,
        predicate: Box::new(move |_, a, b| {
            calls.borrow_mut().push(("predicate", a.0, b.0));
            assert!(!(fail.get() == 1 && a.0 == 9), "predicate");
            true
        }),
        projection: Box::new(move |_, a, b| {
            projected.borrow_mut().push(("projection", a.0, b.0));
            assert!(!(projected_fail.get() == 3 && a.0 == 9), "projection");
            Row(a.0 + b.0, projected.clone(), projected_fail.clone())
        }),
    }
}
fn inputs(
    trace: &Trace,
    failure: &Rc<Cell<u8>>,
    rows: &[(bool, &str, Option<i64>)],
) -> Vec<(bool, Event)> {
    rows.iter()
        .map(|&(side, id, value)| {
            (
                side,
                Rc::new(TupleChange {
                    id: id.to_owned(),
                    value: value.map(|v| Row(v, trace.clone(), failure.clone())),
                }) as Event,
            )
        })
        .collect()
}
fn apply(node: &mut Links, rows: &[(bool, Event)]) -> BatchOutputs {
    <Links as Node<SimpleScore>>::apply_batch(
        node,
        rows,
        NodeArithmetic {
            impact: crate::ScoringArithmetic::Checked,
            totals: crate::ScoringArithmetic::Checked,
        },
    )
    .unwrap()
}
fn marker_address(saved: &dyn Any) -> *const LinkCheckpoint {
    CheckpointBox::<LinkCheckpoint>::get(saved).unwrap()
}
fn values(outputs: &[Event]) -> Vec<(String, Option<i64>)> {
    outputs
        .iter()
        .map(|event| {
            let c = typed::<LinkId<String, String, String>, Row>(event);
            (c.id.0.0.clone(), c.value.as_ref().map(|v| v.0))
        })
        .collect()
}

#[test]
fn mixed_side_last_updates_unknown_ids_retractions_and_empty_batches_keep_order() {
    let trace = Trace::default();
    let fail = Rc::new(Cell::new(0));
    let mut n = node(&trace, &fail);
    let batch = inputs(
        &trace,
        &fail,
        &[
            (false, "a", Some(5)),
            (true, "z", None),
            (false, "a", Some(7)),
            (true, "unknown", Some(99)),
            (false, "unknown", None),
            (true, "z", Some(8)),
            (true, "a", Some(4)),
            (false, "z", None),
        ],
    );
    let output = apply(&mut n, &batch);
    assert_eq!(
        values(&output),
        [
            ("e0".to_owned(), Some(15)),
            ("e1".to_owned(), None),
            ("e2".to_owned(), Some(11))
        ]
    );
    let observed = trace.borrow().clone();
    assert_eq!(
        &observed[..5],
        [
            ("predicate", 7, 8),
            ("projection", 7, 8),
            ("predicate", 2, 3),
            ("predicate", 7, 4),
            ("projection", 7, 4)
        ]
    );
    assert_eq!(
        observed
            .iter()
            .filter(|v| v.0 == "clone")
            .map(|v| v.1)
            .collect::<Vec<_>>(),
        [7, 4, 8]
    );
    let scratch = (
        n.left_changes.as_ptr(),
        n.right_changes.as_ptr(),
        n.affected.as_ptr(),
    );
    assert!(apply(&mut n, &[]).is_empty());
    let unknown = inputs(&trace, &fail, &[(true, "unknown", Some(1))]);
    assert!(apply(&mut n, &unknown).is_empty());
    let again = inputs(&trace, &fail, &[(false, "a", Some(6))]);
    assert_eq!(
        values(&apply(&mut n, &again)),
        [("e0".to_owned(), Some(14)), ("e2".to_owned(), Some(10))]
    );
    assert_eq!(
        scratch,
        (
            n.left_changes.as_ptr(),
            n.right_changes.as_ptr(),
            n.affected.as_ptr()
        )
    );
}

#[test]
fn panic_dirty_scratch_restores_then_retries_with_fresh_callback_clone_drop_trace() {
    for failure in [1, 2, 3] {
        let trace = Trace::default();
        let fail = Rc::new(Cell::new(0));
        let mut n = node(&trace, &fail);
        // Recover from each user panic after recycling a completed marker too.
        let warm = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
        let address = marker_address(warm.as_ref());
        <Links as Node<SimpleScore>>::commit_checkpoint(&mut n, warm);
        let saved = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
        assert_eq!(address, marker_address(saved.as_ref()));
        let bad = inputs(
            &trace,
            &fail,
            &[
                (false, "z", Some(8)),
                (false, "a", Some(9)),
                (true, "z", Some(7)),
            ],
        );
        fail.set(failure);
        assert!(catch_unwind(AssertUnwindSafe(|| apply(&mut n, &bad))).is_err());
        fail.set(0);
        <Links as Node<SimpleScore>>::restore(&mut n, saved);
        drop(bad);
        let retry = inputs(
            &trace,
            &fail,
            &[(true, "a", Some(6)), (false, "z", Some(5))],
        );
        trace.borrow_mut().clear();
        let actual = apply(&mut n, &retry);
        let actual_values = values(&actual);
        drop(actual);
        let actual_trace = trace.borrow().clone();
        let fresh_trace = Trace::default();
        let fresh_fail = Rc::new(Cell::new(0));
        let mut fresh = node(&fresh_trace, &fresh_fail);
        let fresh_inputs = inputs(
            &fresh_trace,
            &fresh_fail,
            &[(true, "a", Some(6)), (false, "z", Some(5))],
        );
        let expected = apply(&mut fresh, &fresh_inputs);
        assert_eq!(actual_values, values(&expected));
        drop(expected);
        assert_eq!(actual_trace, *fresh_trace.borrow());
    }
}

#[test]
fn shorter_retry_clears_endpoint_positions_after_success_and_each_panic_stage() {
    for failure in [0, 1, 2, 3] {
        let trace = Trace::default();
        let fail = Rc::new(Cell::new(0));
        let mut n = node(&trace, &fail);
        let initial = inputs(
            &trace,
            &fail,
            &[
                (false, "z", Some(8)),
                (true, "z", Some(7)),
                (false, "a", Some(5)),
                (true, "a", Some(6)),
            ],
        );
        drop(apply(&mut n, &initial));
        let saved = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
        let bad = inputs(
            &trace,
            &fail,
            &[
                (true, "unknown", Some(99)),
                (false, "z", Some(8)),
                (false, "a", Some(9)),
                (true, "z", Some(7)),
            ],
        );
        fail.set(failure);
        let attempted = catch_unwind(AssertUnwindSafe(|| apply(&mut n, &bad)));
        assert_eq!(attempted.is_err(), failure != 0);
        drop(attempted);
        fail.set(0);
        <Links as Node<SimpleScore>>::restore(&mut n, saved);
        drop(bad);
        // The failed round left input position 2 for left endpoint "a". The
        // retry has one right-side event: stale lookup would panic or use its
        // payload as the unchanged left row. Check exact callbacks/ownership too.
        let retry = inputs(&trace, &fail, &[(true, "z", Some(10))]);
        trace.borrow_mut().clear();
        let output = apply(&mut n, &retry);
        assert_eq!(values(&output), [("e0".to_owned(), Some(15))]);
        assert_eq!(
            *trace.borrow(),
            [
                ("predicate", 5, 10),
                ("projection", 5, 10),
                ("clone", 10, 0),
                ("drop", 7, 0)
            ]
        );
        drop(output);
        assert_eq!(trace.borrow().last(), Some(&("drop", 15, 0)));
        assert_eq!(n.left.get(0).unwrap().0, 5);
        assert_eq!(n.left.get(1).unwrap().0, 8);
        assert_eq!(n.right.get(0).unwrap().0, 6);
        assert_eq!(n.right.get(1).unwrap().0, 10);
        // Empty and unknown-only rounds also invalidate the previous lookup.
        assert!(apply(&mut n, &[]).is_empty());
        let unknown = inputs(&trace, &fail, &[(false, "unknown", Some(42))]);
        assert!(apply(&mut n, &unknown).is_empty());
        let next = inputs(&trace, &fail, &[(false, "a", Some(4))]);
        assert_eq!(
            values(&apply(&mut n, &next)),
            [("e0".to_owned(), Some(14)), ("e2".to_owned(), Some(10))]
        );
    }
}

#[test]
fn reused_link_markers_preserve_nested_commit_rollback_and_endpoint_drops() {
    let trace = Trace::default();
    let fail = Rc::new(Cell::new(0));
    let mut n = node(&trace, &fail);
    let row = |value| Row(value, trace.clone(), fail.clone());
    let outer = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
    let address = marker_address(outer.as_ref());
    n.left.set(0, Some(row(5)));
    let inner = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
    let child_address = marker_address(inner.as_ref());
    assert_ne!(address, child_address);
    n.right.set(1, Some(row(6)));
    <Links as Node<SimpleScore>>::commit_checkpoint(&mut n, inner);
    assert!(trace.borrow().is_empty());
    let retry = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
    assert_eq!(child_address, marker_address(retry.as_ref()));
    n.left.set(0, Some(row(7)));
    <Links as Node<SimpleScore>>::restore(&mut n, retry);
    assert_eq!(*trace.borrow(), [("drop", 7, 0)]);
    <Links as Node<SimpleScore>>::restore(&mut n, outer);
    assert_eq!(
        *trace.borrow(),
        [("drop", 7, 0), ("drop", 5, 0), ("drop", 6, 0)]
    );
    assert_eq!(n.left.get(0).unwrap().0, 1);
    assert_eq!(n.right.get(1).unwrap().0, 4);
    let committed = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
    assert_eq!(address, marker_address(committed.as_ref()));
    n.right.set(0, Some(row(8)));
    <Links as Node<SimpleScore>>::commit_checkpoint(&mut n, committed);
    assert_eq!(trace.borrow().last(), Some(&("drop", 3, 0)));
    let clean = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
    assert_eq!(address, marker_address(clean.as_ref()));
    <Links as Node<SimpleScore>>::clear(&mut n);
    <Links as Node<SimpleScore>>::restore(&mut n, clean);
    assert_eq!(n.right.get(0).unwrap().0, 8);
    assert_eq!(trace.borrow().len(), 4);
}
