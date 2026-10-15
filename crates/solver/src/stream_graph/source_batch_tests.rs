//! Original source batching controls; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Trace = Rc<RefCell<Vec<(&'static str, i64)>>>;
struct Row(i64, Trace, Rc<Cell<bool>>);
impl Clone for Row {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push(("row clone", self.0));
        assert!(!(self.0 == 9 && self.2.get()), "row clone failure");
        Self(self.0, self.1.clone(), self.2.clone())
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        self.1.borrow_mut().push(("row drop", self.0));
    }
}
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
// Exercise the supported single-threaded payload case behind shared row owners.
#[allow(clippy::arc_with_non_send_sync)]
fn fixed(trace: &Trace, fail: &Rc<Cell<bool>>) -> FixedSourceNode<Row> {
    FixedSourceNode::new(
        (0..3)
            .map(|id| {
                (
                    Arc::new(id),
                    Arc::new(Row(id as i64, trace.clone(), fail.clone())),
                )
            })
            .collect(),
    )
}
fn batch(
    trace: &Trace,
    fail: &Rc<Cell<bool>>,
    values: &[(usize, Option<i64>)],
) -> Vec<(bool, Event)> {
    values
        .iter()
        .map(|&(id, value)| {
            (
                false,
                Rc::new(TupleChange {
                    id,
                    value: value.map(|v| Row(v, trace.clone(), fail.clone())),
                }) as Event,
            )
        })
        .collect()
}
fn apply(
    node: &mut FixedSourceNode<Row>,
    inputs: &[(bool, Event)],
) -> Result<BatchOutputs, GraphError> {
    <FixedSourceNode<Row> as Node<SimpleScore>>::apply_batch(node, inputs, arithmetic())
}
fn output_values(output: &[Event]) -> Vec<(usize, Option<i64>)> {
    output
        .iter()
        .map(|event| {
            let c = typed::<usize, Row>(event);
            (c.id, c.value.as_ref().map(|v| v.0))
        })
        .collect()
}
#[test]
fn fixed_batches_sort_last_winners_retract_and_reuse_only_ordinal_scratch() {
    let trace = Trace::default();
    let fail = Rc::new(Cell::new(false));
    let mut node = fixed(&trace, &fail);
    let inputs = batch(
        &trace,
        &fail,
        &[
            (2, Some(8)),
            (0, Some(4)),
            (2, Some(5)),
            (1, None),
            (0, Some(7)),
        ],
    );
    let output = apply(&mut node, &inputs).unwrap();
    assert_eq!(
        output_values(&output),
        [(0, Some(7)), (1, None), (2, Some(5))]
    );
    assert!(Rc::ptr_eq(&output[0], &inputs[4].1));
    assert!(Rc::ptr_eq(&output[1], &inputs[3].1));
    assert!(Rc::ptr_eq(&output[2], &inputs[2].1));
    assert_eq!(
        trace
            .borrow()
            .iter()
            .filter(|e| e.0 == "row clone")
            .map(|e| e.1)
            .collect::<Vec<_>>(),
        [7, 5]
    );
    let scratch = node.latest.as_ptr();
    let capacity = node.latest.capacity();
    assert!(apply(&mut node, &[]).unwrap().is_empty());
    let smaller = batch(&trace, &fail, &[(2, Some(6))]);
    assert_eq!(
        output_values(&apply(&mut node, &smaller).unwrap()),
        [(2, Some(6))]
    );
    assert_eq!(
        (node.latest.as_ptr(), node.latest.capacity()),
        (scratch, capacity)
    );
    assert_eq!(
        node.rows()
            .iter()
            .map(|(id, v)| (*id, v.0))
            .collect::<Vec<_>>(),
        [(0, 7), (2, 6)]
    );
}
#[test]
fn later_invalid_id_and_clone_panic_restore_checkpoint_and_retry_with_held_rows() {
    for panic in [false, true] {
        let trace = Trace::default();
        let fail = Rc::new(Cell::new(false));
        let mut node = fixed(&trace, &fail);
        let held = node.rows().clone();
        let owners = held.shared_entries();
        let saved = <FixedSourceNode<Row> as Node<SimpleScore>>::begin_checkpoint(&mut node);
        let bad = batch(
            &trace,
            &fail,
            &[(0, Some(7)), (if panic { 1 } else { 99 }, Some(9))],
        );
        fail.set(panic);
        let result = catch_unwind(AssertUnwindSafe(|| apply(&mut node, &bad)));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err(GraphError::Dependency(_)))));
        }
        fail.set(false);
        <FixedSourceNode<Row> as Node<SimpleScore>>::restore(&mut node, saved);
        assert_eq!(
            node.rows()
                .iter()
                .map(|(id, v)| (*id, v.0))
                .collect::<Vec<_>>(),
            [(0, 0), (1, 1), (2, 2)]
        );
        let restored = node.rows().shared_entries();
        for (old, new) in owners.iter().zip(&restored) {
            assert!(Arc::ptr_eq(&old.0, &new.0));
            assert!(Arc::ptr_eq(&old.1, &new.1));
        }
        trace.borrow_mut().clear();
        let retry = batch(&trace, &fail, &[(2, Some(6)), (0, Some(8))]);
        assert_eq!(
            output_values(&apply(&mut node, &retry).unwrap()),
            [(0, Some(8)), (2, Some(6))]
        );
        assert_eq!(
            trace
                .borrow()
                .iter()
                .filter(|e| e.0 == "row clone")
                .map(|e| e.1)
                .collect::<Vec<_>>(),
            [8, 6]
        );
        assert_eq!(held[&0].0, 0);
        assert_eq!(node.rows()[&0].0, 8);
    }
}
struct Id(i64, i64, Trace);
impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for Id {}
impl PartialOrd for Id {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Id {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
impl Clone for Id {
    fn clone(&self) -> Self {
        self.2.borrow_mut().push(("ID clone", self.1));
        Self(self.0, self.1, self.2.clone())
    }
}
impl Drop for Id {
    fn drop(&mut self) {
        self.2.borrow_mut().push(("ID drop", self.1));
    }
}
#[test]
fn generic_source_retains_event_and_stored_representatives_and_clone_drop_order() {
    let trace = Trace::default();
    let fail = Rc::new(Cell::new(false));
    let mut source = SourceNode::<Id, Row>(SnapshotMap::new());
    let inputs: Vec<_> = [(1, 10, 1), (7, 20, 2), (1, 11, 3)]
        .into_iter()
        .map(|(key, tag, v)| {
            (
                false,
                Rc::new(TupleChange {
                    id: Id(key, tag, trace.clone()),
                    value: Some(Row(v, trace.clone(), fail.clone())),
                }) as Event,
            )
        })
        .collect();
    let output =
        <SourceNode<Id, Row> as Node<SimpleScore>>::apply_batch(&mut source, &inputs, arithmetic())
            .unwrap();
    assert!(Rc::ptr_eq(&output[0], &inputs[2].1));
    assert!(Rc::ptr_eq(&output[1], &inputs[1].1));
    assert_eq!(
        *trace.borrow(),
        [
            ("ID clone", 10),
            ("ID clone", 20),
            ("ID clone", 11),
            ("ID drop", 11),
            ("ID drop", 10),
            ("ID clone", 11),
            ("row clone", 3),
            ("ID drop", 20),
            ("ID clone", 20),
            ("row clone", 2)
        ]
    );
    assert_eq!(
        source
            .0
            .iter()
            .map(|(id, v)| (id.0, id.1, v.0))
            .collect::<Vec<_>>(),
        [(1, 11, 3), (7, 20, 2)]
    );
    let replacement: Vec<_> = vec![(
        false,
        Rc::new(TupleChange {
            id: Id(1, 12, trace.clone()),
            value: Some(Row(4, trace.clone(), fail.clone())),
        }) as Event,
    )];
    trace.borrow_mut().clear();
    let next = <SourceNode<Id, Row> as Node<SimpleScore>>::apply_batch(
        &mut source,
        &replacement,
        arithmetic(),
    )
    .unwrap();
    assert_eq!(typed::<Id, Row>(&next[0]).id.1, 12);
    assert_eq!(source.0.iter().next().unwrap().0.1, 11);
    assert_eq!(
        *trace.borrow(),
        [
            ("ID clone", 12),
            ("ID drop", 12),
            ("ID clone", 12),
            ("row clone", 4),
            ("ID drop", 12),
            ("row drop", 3)
        ]
    );
}
