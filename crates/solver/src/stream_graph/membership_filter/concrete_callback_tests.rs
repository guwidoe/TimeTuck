//! Original callback-dispatch controls; no upstream-method credit.
use super::super::*;
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

thread_local! {
    static TRACE: RefCell<Vec<(&'static str, i64)>> = const { RefCell::new(Vec::new()) };
    static FAIL_CLONE: Cell<bool> = const { Cell::new(false) };
}
#[derive(Debug)]
struct Row(i64);
impl Clone for Row {
    fn clone(&self) -> Self {
        TRACE.with(|t| t.borrow_mut().push(("clone", self.0)));
        assert!(!FAIL_CLONE.with(Cell::get), "clone panic");
        Self(self.0)
    }
}
struct PredicateDrop;
impl Drop for PredicateDrop {
    fn drop(&mut self) {
        TRACE.with(|t| t.borrow_mut().push(("predicate_drop", 0)));
    }
}
fn scenario(concrete: bool) -> Vec<(&'static str, i64)> {
    TRACE.with(|t| t.borrow_mut().clear());
    let fail = Cell::new(false);
    let guard = PredicateDrop;
    let predicate = |row: &Row| {
        let _keep_guard = &guard;
        TRACE.with(|t| t.borrow_mut().push(("predicate", row.0)));
        assert!(!fail.get(), "predicate panic");
        row.0 >= 0
    };
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, Row>();
    let filtered = if concrete {
        builder.filter(source.port(), predicate).unwrap()
    } else {
        // Same acceptance-cache backend, with the original boxed callback.
        let port: StreamPort<usize, Row> = builder.push(MembershipFilter::new(
            Membership::<usize>::new().unwrap(),
            predicate,
        ));
        builder.connect(source.node_id(), port.node, false);
        port
    };
    let constraint = builder
        .score("sum", &filtered, |row| {
            TRACE.with(|t| t.borrow_mut().push(("score", row.0)));
            SimpleScore::new(row.0)
        })
        .unwrap();
    let mut graph = builder.build();
    {
        let mut tx = graph.transaction();
        for (id, value) in [(0, 2), (1, -1), (usize::MAX, 4)] {
            tx.apply(&source, TupleChange::put(id, Row(value))).unwrap();
        }
        tx.commit().unwrap();
    }
    let baseline = graph.constraint_matches(&constraint).unwrap().clone();
    let baseline_state = graph.capture_full();
    let (candidate, saved) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, Row(2))).unwrap();
        tx.apply(&source, TupleChange::put(1, Row(3))).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(9));
        let candidate = tx.graph.constraint_matches(&constraint).unwrap().clone();
        let saved = tx.capture().unwrap();
        (candidate, saved)
    };
    let restored = graph.constraint_matches(&constraint).unwrap();
    assert!(std::ptr::eq(
        restored.get(&0).unwrap(),
        baseline.get(&0).unwrap()
    ));
    assert!(!std::ptr::eq(
        restored.get(&0).unwrap(),
        candidate.get(&0).unwrap()
    ));
    graph.install(saved);
    assert!(std::ptr::eq(
        graph
            .constraint_matches(&constraint)
            .unwrap()
            .get(&0)
            .unwrap(),
        candidate.get(&0).unwrap()
    ));
    graph.install(baseline_state);
    let before_panic = graph.score();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0, Row(8))).unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(graph.score(), before_panic);
    FAIL_CLONE.with(|flag| flag.set(true));
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0, Row(8))).unwrap();
    }));
    FAIL_CLONE.with(|flag| flag.set(false));
    assert!(outcome.is_err());
    assert_eq!(graph.score(), before_panic);
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::retract(usize::MAX)).unwrap();
        tx.apply(&source, TupleChange::put(0, Row(-2))).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(0));
        tx.commit().unwrap();
    }
    drop(graph);
    drop(guard);
    TRACE.with(|t| t.borrow().clone())
}
#[test]
fn concrete_filter_callback_preserves_graph_trace_unwind_capture_and_public_owners() {
    assert_eq!(scenario(true), scenario(false));
}

#[test]
fn concrete_filter_callback_owned_closure_drops_once_when_node_drops() {
    struct Watch<'a>(&'a Cell<usize>);
    impl Drop for Watch<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let count = Cell::new(0);
    let watch = Watch(&count);
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, i64>();
    let _filtered = builder
        .filter(source.port(), move |row| {
            let _watch = &watch;
            *row >= 0
        })
        .unwrap();
    let mut graph = builder.build();
    assert_eq!(count.get(), 0);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, 1)).unwrap();
    drop(tx);
    assert_eq!(count.get(), 0);
    drop(graph);
    assert_eq!(count.get(), 1);
}
