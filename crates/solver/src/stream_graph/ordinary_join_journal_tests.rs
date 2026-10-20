//! Original ordinary-graph journal controls; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Id = (usize, usize);
type Trace = Vec<(u8, usize, i64)>;
thread_local! { static TRACE: RefCell<Trace> = const { RefCell::new(Vec::new()) }; }
#[derive(Debug, PartialEq, Eq)]
struct Row(usize, i64);
impl Clone for Row {
    fn clone(&self) -> Self {
        TRACE.with(|trace| trace.borrow_mut().push((0, self.0, self.1)));
        Self(self.0, self.1)
    }
}
fn key(row: &Row, side: u8) -> usize {
    TRACE.with(|trace| trace.borrow_mut().push((side, row.0, row.1)));
    assert_ne!(row.1, 998, "projection panic");
    row.0
}
type J = JoinNode<'static, Id, Id, Row, Row, usize>;
type Match = ((Id, Id), (usize, i64, usize, i64), i64);
fn contents(
    graph: &StreamGraph<'_, SimpleScore>,
    c: &StreamConstraint<(Id, Id), (Row, Row)>,
) -> Vec<Match> {
    graph
        .constraint_matches(c)
        .unwrap()
        .iter()
        .map(|(id, ((l, r), score))| (*id, (l.0, l.1, r.0, r.1), score.score()))
        .collect()
}

fn exercise(enabled: bool, comparison: JoinComparison) -> (Trace, Vec<Vec<Match>>) {
    TRACE.with(|trace| trace.borrow_mut().clear());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let left = b.source::<Id, Row>();
    let right = b.source::<Id, Row>();
    let joined = if enabled {
        if comparison == JoinComparison::Equal {
            b.join_equal(left.port(), right.port(), |r| key(r, 1), |r| key(r, 2))
                .unwrap()
        } else {
            b.join_compare(
                left.port(),
                right.port(),
                comparison,
                |r| key(r, 1),
                |r| key(r, 2),
            )
            .unwrap()
        }
    } else {
        // Same ordinary graph wrapper, with its original full-checkpoint route.
        let out = b.push(Join::new(J::comparing(
            comparison,
            |r| key(r, 1),
            |r| key(r, 2),
        )));
        b.connect(left.0.node, out.node, false);
        b.connect(right.0.node, out.node, true);
        out
    };
    let constraint = b
        .score("pairs", &joined, |(l, r)| {
            TRACE.with(|trace| trace.borrow_mut().push((3, l.0, r.1)));
            assert_ne!(l.1, 999, "downstream panic");
            SimpleScore::new(l.1)
        })
        .unwrap();
    let mut graph = b.build();
    {
        let mut tx = graph.transaction();
        for (id, k) in [
            ((0usize, 0usize), 1usize),
            ((usize::MAX, 4), 2),
            ((1, usize::MAX), 1),
        ] {
            tx.apply(&left, TupleChange::put(id, Row(k, 2))).unwrap();
            tx.apply(&right, TupleChange::put(id, Row(k, 3))).unwrap();
        }
        tx.commit().unwrap();
    }
    let base = contents(&graph, &constraint);
    // No retained full root: this actually starts the value-replacement journal.
    {
        let mut tx = graph.transaction();
        tx.apply(&left, TupleChange::put((0usize, 0usize), Row(1, 5)))
            .unwrap();
        let saved = tx
            .journal
            .iter()
            .find(|(id, _)| *id == joined.node)
            .unwrap();
        assert_eq!(J::is_ordinary_journal_for_test(saved.1.as_ref()), enabled);
        // Both source sides participate in the enclosing transaction before
        // a nested scope commits edits to them.
        tx.apply(&right, TupleChange::put((0usize, 0usize), Row(1, 3)))
            .unwrap();
        // Actual nested graph transaction; membership edits promote map frames.
        let mut inner = tx.graph.transaction();
        inner
            .apply(
                &right,
                TupleChange::put((usize::MAX, usize::MAX), Row(3, 8)),
            )
            .unwrap();
        inner
            .apply(&left, TupleChange::put((usize::MAX, 4usize), Row(0, 4)))
            .unwrap();
        inner
            .apply(&right, TupleChange::retract((1usize, usize::MAX)))
            .unwrap();
        inner.commit().unwrap();
    }
    assert_eq!(contents(&graph, &constraint), base);
    // Full retained roots force the journal's shared-root fallback. Saved current
    // captures must survive other trials from the same base and install silently.
    let full = graph.capture_full();
    let (candidate, wanted) = {
        let mut tx = graph.transaction();
        tx.apply(&left, TupleChange::put((0usize, 0usize), Row(2, 7)))
            .unwrap();
        tx.apply(&right, TupleChange::retract((usize::MAX, 4usize)))
            .unwrap();
        tx.apply(&right, TupleChange::put((50usize, 70usize), Row(2, 9)))
            .unwrap();
        (tx.capture().unwrap(), contents(tx.graph, &constraint))
    };
    {
        let mut tx = graph.transaction();
        tx.apply(&left, TupleChange::retract((0usize, 0usize)))
            .unwrap();
        tx.apply(&right, TupleChange::put((6usize, 5usize), Row(0, 1)))
            .unwrap();
    }
    assert_eq!(contents(&graph, &constraint), base);
    let before = TRACE.with(|trace| trace.borrow().len());
    graph.install(candidate);
    assert_eq!(contents(&graph, &constraint), wanted);
    graph.install(full);
    assert_eq!(contents(&graph, &constraint), base);
    assert_eq!(TRACE.with(|trace| trace.borrow().len()), before);
    // Both staging and downstream failures poison only the trial; drop restores
    // row and index journals without replaying callbacks.
    let failure_key = match comparison {
        JoinComparison::GreaterThan | JoinComparison::GreaterThanOrEqual => 3usize,
        _ => 1usize,
    };
    for bad in [998i64, 999] {
        let mut tx = graph.transaction();
        let result = catch_unwind(AssertUnwindSafe(|| {
            tx.apply(
                &left,
                TupleChange::put((0usize, 0usize), Row(failure_key, bad)),
            )
            .unwrap();
        }));
        assert!(result.is_err());
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        drop(tx);
        assert_eq!(contents(&graph, &constraint), base);
    }
    // Checked arithmetic failure after multiple emitted matches also rolls back.
    {
        let mut tx = graph.transaction();
        let result = tx.apply(
            &left,
            TupleChange::put((0usize, 0usize), Row(failure_key, i64::MAX)),
        );
        assert!(matches!(result, Err(GraphError::ScoreOverflow { .. })));
    }
    assert_eq!(contents(&graph, &constraint), base);
    (
        TRACE.with(|trace| trace.borrow().clone()),
        vec![base, wanted],
    )
}

#[test]
fn ordinary_join_journals_match_full_route_for_nested_membership_ids_and_failures() {
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        assert_eq!(
            exercise(true, comparison),
            exercise(false, comparison),
            "{comparison:?}"
        );
    }
}

#[test]
fn ordinary_join_journals_retain_custom_ownership_and_fixed_routes() {
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
    struct Custom(usize);
    let mut custom_id = JoinNode::<Custom, Id, usize, usize, usize>::new(|v| *v, |v| *v);
    assert!(!custom_id.bind_primitive_graph_journals());
    assert!(custom_id.begin_bound_checkpoint().is_none());
    let mut custom_key =
        JoinNode::<Id, Id, usize, usize, Custom>::new(|v| Custom(*v), |v| Custom(*v));
    assert!(!custom_key.bind_primitive_graph_journals());
    assert!(custom_key.begin_bound_checkpoint().is_none());
    let mut owned = JoinNode::<Id, Id, Rc<usize>, usize, usize>::new(|v| **v, |v| *v);
    assert!(!owned.bind_primitive_graph_journals());
    let marker = Rc::new(2usize);
    let weak = Rc::downgrade(&marker);
    owned.apply_left(TupleChange::put((0usize, 0usize), marker));
    let saved = owned.checkpoint();
    owned.apply_left(TupleChange::retract((0usize, 0usize)));
    assert!(weak.upgrade().is_some());
    owned.restore_bound_checkpoint(saved);
    drop(owned);
    assert!(weak.upgrade().is_none());
    // Standalone remains full; graph opt-in never sends FixedRows through maps.
    let mut fixed = JoinNode::<usize, usize, usize, usize, usize>::new(|v| *v, |v| *v);
    assert!(fixed.begin_bound_checkpoint().is_none());
    assert!(fixed.bind_primitive_graph_journals());
    fixed.apply_left(TupleChange::put(0usize, 2usize));
    assert!(fixed.bind_fixed_input(false, 1));
    let saved = fixed.begin_bound_checkpoint().unwrap();
    assert!(
        !JoinNode::<usize, usize, usize, usize, usize>::is_ordinary_journal_for_test(
            saved.as_ref()
        )
    );
    fixed.apply_left(TupleChange::put(0usize, 4usize));
    fixed.restore_bound_checkpoint(saved);
    fixed.apply_right(TupleChange::put(0usize, 2usize));
    assert_eq!(fixed.matches().count(), 1);
}
