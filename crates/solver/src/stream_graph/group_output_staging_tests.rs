//! Original final-output staging controls; no upstream method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Key = (usize, usize);
type Row = (usize, i64);
type Mapping<'a> = dyn Fn(&Row) -> (Key, i64) + 'a;
struct Legacy<'a> {
    node: GroupNode<usize, Key>,
    mapping: Box<Mapping<'a>>,
}
impl Node<SimpleScore> for Legacy<'_> {
    fn clear(&mut self) {
        self.node.clear();
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        self.node.checkpoint()
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        self.node.begin_journal_checkpoint()
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        self.node.commit_journal_checkpoint(saved);
    }
    fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any> {
        self.node.capture_checkpoint(original)
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.node.restore_checkpoint(saved);
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        let change = typed::<usize, Row>(event);
        let mapped = TupleChange {
            id: change.id,
            value: change.value.as_ref().map(|v| (self.mapping)(v)),
        };
        Ok(erase(
            self.node
                .apply_inline(mapped)
                .into_iter()
                .flatten()
                .map(|change| {
                    let key = change.id;
                    change.map(|summary| (key, summary))
                }),
        ))
    }
}
fn graph_trace(legacy: bool, batch: bool, failure: u8) -> (Vec<(u8, i64)>, Vec<u128>) {
    let trace = RefCell::new(Vec::new());
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Row>();
    let mapping = |row: &Row| {
        trace.borrow_mut().push((0, row.1));
        assert!(!(fail.get() && failure == 1), "mapping panic");
        ((0usize, row.0), row.1)
    };
    let grouped = if legacy {
        let mut node = GroupNode::new();
        node.bind_primitive_group_keys();
        node.bind_primitive_inputs();
        node.enable_equal_input_reuse();
        node.enable_owned_inputs();
        let out = b.push::<Key, (Key, IntSummary)>(Legacy {
            node,
            mapping: Box::new(mapping),
        });
        b.connect(source.node_id(), out.node, false);
        out
    } else {
        b.group_collect(source.port(), mapping).unwrap()
    };
    let kept = b
        .filter(&grouped, |(_, summary)| {
            trace.borrow_mut().push((1, summary.sum));
            true
        })
        .unwrap();
    let constraint = b
        .score("sum", &kept, |(_, summary)| {
            trace.borrow_mut().push((2, summary.sum));
            assert!(!(fail.get() && failure == 2), "score panic");
            SimpleScore::new(summary.sum)
        })
        .unwrap();
    b.score("other", &grouped, |(_, summary)| {
        SimpleScore::new(summary.count as i64)
    })
    .unwrap();
    let mut graph = b.build();
    let mut counts = Vec::new();
    {
        let mut tx = graph.transaction();
        if batch {
            let mut changes = crate::StreamBatch::new();
            for (id, row) in [(0usize, (0usize, 1i64)), (1, (0, 2)), (2, (1, 3))] {
                changes.push(&source, TupleChange::put(id, row)).unwrap();
            }
            tx.apply_batch(changes).unwrap();
        } else {
            for (id, row) in [(0usize, (0usize, 1i64)), (1, (0, 2)), (2, (1, 3))] {
                tx.apply(&source, TupleChange::put(id, row)).unwrap();
            }
        }
        counts.push(tx.score_evaluations());
        tx.commit().unwrap();
    }
    let full = graph.capture_full();
    let saved = {
        let mut tx = graph.transaction();
        for (id, row) in [(0usize, (1usize, 4i64)), (1, (0, 2)), (2, (0, 5))] {
            tx.apply(&source, TupleChange::put(id, row)).unwrap();
        }
        tx.assert_consistent().unwrap();
        counts.push(tx.score_evaluations());
        tx.capture().unwrap()
    };
    graph.install(saved);
    graph.assert_consistent().unwrap();
    let observed = graph.constraint_matches(&constraint).unwrap().clone();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(1usize, (0usize, 2i64)))
            .unwrap();
        let candidate = tx.graph.constraint_matches(&constraint).unwrap().clone();
        assert!(!std::ptr::eq(
            observed.get(&(0, 0)).unwrap(),
            candidate.get(&(0, 0)).unwrap()
        ));
    }
    graph.install(full);
    let before = graph.score();
    fail.set(true);
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(0usize, (1usize, 6i64)))
            ))
            .is_err()
        );
        counts.push(tx.score_evaluations());
    }
    fail.set(false);
    assert_eq!(graph.score(), before);
    graph.assert_consistent().unwrap();
    {
        let mut tx = graph.transaction();
        for id in [0usize, 1, 2] {
            tx.apply(&source, TupleChange::retract(id)).unwrap();
        }
        counts.push(tx.score_evaluations());
        tx.commit().unwrap();
    }
    assert_eq!(graph.score(), SimpleScore::ZERO);
    drop(graph);
    (trace.into_inner(), counts)
}
#[test]
fn group_output_staging_actual_graph_fifo_callbacks_snapshots_capture_diagnostics_and_panics_match_array()
 {
    assert!(live_commit_key::<usize>() && live_commit_key::<Key>());
    for batch in [false, true] {
        for failure in [1u8, 2] {
            assert_eq!(
                graph_trace(false, batch, failure),
                graph_trace(true, batch, failure)
            );
        }
    }
}

#[test]
fn group_output_staging_payload_key_clone_event_owner_cleanup_and_custom_gate_match_array() {
    thread_local! {static TRACE:RefCell<Vec<(u8,usize)>>=const{RefCell::new(Vec::new())};static CLONES:Cell<usize>=const{Cell::new(0)};static FAIL:Cell<usize>=const{Cell::new(0)};}
    #[derive(Debug)]
    struct Custom(usize);
    impl Clone for Custom {
        fn clone(&self) -> Self {
            TRACE.with(|t| t.borrow_mut().push((0, self.0)));
            let n = CLONES.with(|n| {
                n.set(n.get() + 1);
                n.get()
            });
            assert_ne!(n, FAIL.with(Cell::get), "key clone panic");
            Self(self.0)
        }
    }
    impl Drop for Custom {
        fn drop(&mut self) {
            TRACE.with(|t| t.borrow_mut().push((1, self.0)));
        }
    }
    assert!(!live_commit_key::<Custom>());
    fn run(direct: bool, fail: usize) -> Vec<(u8, usize)> {
        TRACE.with(|t| t.borrow_mut().clear());
        CLONES.with(|n| n.set(0));
        FAIL.with(|n| n.set(fail));
        let old = Some(TupleChange::put(Custom(1), IntSummary::default()));
        let new = Some(TupleChange::retract(Custom(2)));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let outputs = if direct {
                group_outputs(old, new)
            } else {
                erase([old, new].into_iter().flatten().map(|change| {
                    let key = change.id.clone();
                    change.map(|summary| (key, summary))
                }))
            };
            assert_eq!(outputs.len(), 2);
            drop(outputs);
        }));
        assert_eq!(result.is_err(), fail != 0);
        TRACE.with(|t| t.borrow().clone())
    }
    for fail in [0usize, 1, 2] {
        assert_eq!(run(true, fail), run(false, fail));
    }
}
