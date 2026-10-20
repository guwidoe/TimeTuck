//! Original unary batching controls; no upstream source-method credit.
use super::*;
use crate::{SimpleScore, StreamBatch};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

type Trace = Rc<RefCell<Vec<(&'static str, i64)>>>;
type Observation = (bool, Vec<(usize, Option<i64>)>, Vec<(&'static str, i64)>);
struct Id(usize, Trace, Rc<Cell<bool>>);
impl Clone for Id {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push(("id clone", self.0 as i64));
        assert!(!(self.0 == 1 && self.2.get()), "ID clone failure");
        Self(self.0, self.1.clone(), self.2.clone())
    }
}
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
struct Row(i64, Trace);
impl Clone for Row {
    fn clone(&self) -> Self {
        self.1.borrow_mut().push(("row clone", self.0));
        Self(self.0, self.1.clone())
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        self.1.borrow_mut().push(("row drop", self.0));
    }
}

// The reference deliberately invokes singleton apply and extends its returned
// outputs, retaining the old trait-default algorithm independently of the override.
fn observe(batch: bool, filter: bool, failure: u8) -> Observation {
    let trace: Trace = Rc::default();
    let id_failure = Rc::new(Cell::new(failure == 2));
    let calls = trace.clone();
    let mut node: Box<dyn Node<SimpleScore>> = if filter {
        Box::new(Filter(FilterNode::<Id, Row>::new(move |row: &Row| {
            calls.borrow_mut().push(("predicate", row.0));
            assert!(!(failure == 1 && row.0 == 2), "predicate failure");
            row.0 >= 0
        })))
    } else {
        Box::new(Map::<Id, Row, Row> {
            mapping: Box::new(move |row| {
                calls.borrow_mut().push(("mapping", row.0));
                assert!(!(failure == 1 && row.0 == 2), "mapping failure");
                Row(row.0 * 10, row.1.clone())
            }),
            marker: PhantomData,
        })
    };
    let inputs: Vec<_> = [
        (0, Some(1)),
        (1, Some(2)),
        (0, None),
        (1, Some(-1)),
        (0, Some(3)),
    ]
    .into_iter()
    .map(|(id, value)| {
        (
            false,
            Event::new(TupleChange {
                id: Id(id, trace.clone(), id_failure.clone()),
                value: value.map(|v| Row(v, trace.clone())),
            }),
        )
    })
    .collect();
    let arithmetic = NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    };
    let saved = node.begin_checkpoint();
    let result = catch_unwind(AssertUnwindSafe(|| {
        if batch {
            node.apply_batch(&inputs, arithmetic).unwrap()
        } else {
            let mut output = Vec::new();
            for (side, event) in &inputs {
                output.extend(node.apply(*side, event, arithmetic).unwrap());
            }
            output
        }
    }));
    let panicked = result.is_err();
    let outputs = match result {
        Ok(outputs) => {
            let values = outputs
                .iter()
                .map(|event| {
                    let row = typed::<Id, Row>(event);
                    (row.id.0, row.value.as_ref().map(|v| v.0))
                })
                .collect();
            drop(outputs);
            values
        }
        Err(_) => Vec::new(),
    };
    node.restore(saved);
    drop(inputs);
    drop(node);
    let events = trace.borrow().clone();
    (panicked, outputs, events)
}

#[test]
fn unary_batches_preserve_repeated_ids_retractions_callbacks_clones_and_outputs() {
    for filter in [false, true] {
        let actual = observe(true, filter, 0);
        assert_eq!(actual, observe(false, filter, 0));
        assert!(!actual.0);
        assert_eq!(
            actual.1,
            if filter {
                vec![
                    (0, Some(1)),
                    (1, Some(2)),
                    (0, None),
                    (1, None),
                    (0, Some(3)),
                ]
            } else {
                vec![
                    (0, Some(10)),
                    (1, Some(20)),
                    (0, None),
                    (1, Some(-10)),
                    (0, Some(30)),
                ]
            }
        );
    }
}

#[test]
fn later_callback_and_id_clone_panics_preserve_prior_output_cleanup() {
    for filter in [false, true] {
        for failure in [1, 2] {
            let actual = observe(true, filter, failure);
            assert!(actual.0);
            assert_eq!(actual, observe(false, filter, failure));
        }
    }
}

#[test]
fn later_unary_callback_panics_roll_back_graph_and_allow_fresh_retry() {
    for fail_filter in [false, true] {
        let fail = Cell::new(false);
        let calls = RefCell::new(Vec::new());
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, i64>();
        let filtered = builder
            .filter(source.port(), |v| {
                calls.borrow_mut().push(("filter", *v));
                assert!(!(fail_filter && fail.get() && *v == 9), "later predicate");
                true
            })
            .unwrap();
        let mapped = builder
            .map(&filtered, |v| {
                calls.borrow_mut().push(("map", *v));
                assert!(fail_filter || !fail.get() || *v != 9, "later mapping");
                v * 10
            })
            .unwrap();
        let dataset = builder.dataset(&mapped).unwrap();
        builder
            .score("sum", &mapped, |v| SimpleScore::new(*v))
            .unwrap();
        let mut graph = builder.build();
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, 1)).unwrap();
            tx.apply(&source, TupleChange::put(1, 2)).unwrap();
            tx.commit().unwrap();
        }
        let original = graph.source_rows(&source).unwrap().clone();
        let held = graph.dataset_rows(&dataset).unwrap().clone();
        calls.borrow_mut().clear();
        fail.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let mut changes = StreamBatch::new();
                changes.push(&source, TupleChange::put(0, 7)).unwrap();
                changes.push(&source, TupleChange::put(1, 9)).unwrap();
                let mut tx = graph.transaction();
                tx.apply_batch(changes).unwrap();
            }))
            .is_err()
        );
        assert!(
            calls
                .borrow()
                .contains(&(if fail_filter { "filter" } else { "map" }, 7))
        );
        assert_eq!(graph.source_rows(&source).unwrap(), &original);
        assert_eq!(graph.dataset_rows(&dataset).unwrap(), &held);
        assert_eq!(graph.score(), SimpleScore::new(30));
        fail.set(false);
        {
            let mut changes = StreamBatch::new();
            changes.push(&source, TupleChange::put(0, 7)).unwrap();
            changes.push(&source, TupleChange::put(1, 9)).unwrap();
            let mut tx = graph.transaction();
            tx.apply_batch(changes).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(graph.score(), SimpleScore::new(160));
        assert_eq!(graph.dataset_rows(&dataset).unwrap()[&0], 70);
        assert_eq!(held[&0], 10);
        graph.assert_consistent().unwrap();
    }
}
