//! Graph controls for the private equal-cardinality shortcut; no upstream credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Trace = Rc<RefCell<Vec<(&'static str, i64)>>>;
struct Row {
    value: i64,
    trace: Trace,
    fail: Rc<Cell<bool>>,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        self.trace.borrow_mut().push(("clone", self.value));
        assert!(!self.fail.replace(false), "output clone panic");
        Self {
            value: self.value,
            trace: self.trace.clone(),
            fail: self.fail.clone(),
        }
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(("drop", self.value));
    }
}
type Run = (Vec<(&'static str, i64)>, Vec<u128>, Vec<i64>);
fn run(optimized: bool) -> Run {
    let trace = Trace::default();
    let fail = Rc::new(Cell::new(false));
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (usize, i64)>();
    let mapping = {
        let trace = trace.clone();
        let fail = fail.clone();
        move |&(count, base): &(usize, i64)| {
            trace.borrow_mut().push(("mapping", base));
            (0..count)
                .map(|p| Row {
                    value: base + p as i64,
                    trace: trace.clone(),
                    fail: fail.clone(),
                })
                .collect::<Vec<_>>()
        }
    };
    let expanded = if optimized {
        builder.flat_map(source.port(), mapping).unwrap()
    } else {
        let mut node = FlatMapNode::<usize, (usize, i64), Row>::new(mapping);
        assert!(node.bind_primitive_inputs());
        let out = builder.push(FlatMap(node));
        builder.connect(source.0.node, out.node, false);
        out
    };
    let constraint = builder
        .score("rows", &expanded, {
            let trace = trace.clone();
            move |row| {
                trace.borrow_mut().push(("score", row.value));
                SimpleScore::new(row.value)
            }
        })
        .unwrap();
    let mut graph = builder.build();
    let mut counts = Vec::new();
    let mut scores = Vec::new();
    for (count, base) in [
        (0usize, 0i64),
        (0, 1),
        (3, 7),
        (3, 9),
        (1, 2),
        (0, 0),
        (2, 4),
    ] {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, (count, base)))
            .unwrap();
        counts.push(tx.score_evaluations());
        tx.commit().unwrap();
        scores.push(graph.score().score());
    }
    graph.bind_fixed_planning_source(&source, 1).unwrap();
    let before = trace.borrow().len();
    {
        let mut tx = graph.transaction();
        assert!(
            tx.apply(&source, TupleChange::put(1usize, (2usize, 4i64)))
                .is_err()
        );
        assert_eq!(tx.score_evaluations(), 0);
    }
    assert_eq!(
        trace.borrow().len(),
        before,
        "source bounds fail before mapping"
    );
    // Observe only AFTER the equal write. Length-cache omission must never
    // collapse the publicly retained downstream candidate/baseline versions.
    let (saved, candidate) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, (2usize, 4i64)))
            .unwrap();
        counts.push(tx.score_evaluations());
        let candidate = tx.graph.constraint_matches(&constraint).unwrap().clone();
        (tx.capture().unwrap(), candidate)
    };
    let baseline = graph.constraint_matches(&constraint).unwrap().clone();
    let key = (0usize, 0usize);
    assert_ne!(
        candidate.get(&key).unwrap() as *const _,
        baseline.get(&key).unwrap() as *const _
    );
    graph.install(saved);
    assert!(std::ptr::eq(
        candidate.get(&key).unwrap(),
        graph
            .constraint_matches(&constraint)
            .unwrap()
            .get(&key)
            .unwrap()
    ));
    graph.assert_consistent().unwrap();
    fail.set(true);
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(0usize, (2usize, 4i64)))
            ))
            .is_err()
        );
        counts.push(tx.score_evaluations());
    }
    assert_eq!(graph.score(), SimpleScore::new(9));
    assert_eq!(graph.source_rows(&source).unwrap().get(&0), Some(&(2, 4)));
    drop(candidate);
    drop(baseline);
    drop(graph);
    let result = trace.borrow().clone();
    (result, counts, scores)
}
#[test]
fn equal_lengths_graph_matches_legacy_callbacks_clones_drops_bounds_counts_and_public_versions() {
    assert_eq!(run(true), run(false));
}
