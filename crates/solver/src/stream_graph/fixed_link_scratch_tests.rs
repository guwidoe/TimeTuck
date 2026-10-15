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
        left_changes: Vec::new(),
        right_changes: Vec::new(),
        affected: Vec::new(),
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
fn apply(node: &mut Links, rows: &[(bool, Event)]) -> Outputs {
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
fn values(outputs: &Outputs) -> Vec<(String, Option<i64>)> {
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
        let saved = <Links as Node<SimpleScore>>::begin_checkpoint(&mut n);
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
