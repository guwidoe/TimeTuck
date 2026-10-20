//! Original concrete-callback controls; no upstream source-method credit.
use super::super::*;
use crate::{Score, SimpleScore};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Trace = Vec<(&'static str, i64)>;
thread_local! {static TRACE:RefCell<Trace>=const{RefCell::new(Vec::new())};}
fn record(kind: &'static str, value: i64) {
    TRACE.with(|trace| trace.borrow_mut().push((kind, value)));
}
#[derive(Debug)]
struct Row(i64);
thread_local! {static CLONE_FAIL:Cell<bool>=const{Cell::new(false)};}
impl Clone for Row {
    fn clone(&self) -> Self {
        record("row clone", self.0);
        assert!(
            !CLONE_FAIL.with(|fail| fail.replace(false)),
            "row clone panic"
        );
        Self(self.0)
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        record("row drop", self.0);
    }
}
struct Environment;
impl Environment {
    fn touch(&self) {
        record("environment call", 0);
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        record("environment drop", 0);
    }
}
#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct HookScore(SimpleScore);
impl Clone for HookScore {
    fn clone(&self) -> Self {
        record("score clone", self.0.score());
        Self(self.0)
    }
}
impl Drop for HookScore {
    fn drop(&mut self) {
        record("score drop", self.0.score());
    }
}
impl std::ops::Add for HookScore {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        record("score add", self.0.score());
        Self(self.0 + rhs.0)
    }
}
impl std::ops::Sub for HookScore {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        record("score sub", self.0.score());
        Self(self.0 - rhs.0)
    }
}
impl Score for HookScore {
    type Level = i64;
    const HARD_LEVEL_COUNT: usize = 0;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        self.0.levels().into_iter()
    }
    fn structural(&self) -> i64 {
        self.0.structural()
    }
    fn hard_cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.hard_cmp(&other.0)
    }
    fn is_feasible(&self) -> bool {
        self.0.is_feasible()
    }
}
impl AdditiveScore for HookScore {
    const ZERO: Self = Self(SimpleScore::ZERO);
    fn with_structural(self, structural: i64) -> Self {
        Self(self.0.with_structural(structural))
    }
}
fn score<'a, I: Ord + Clone + 'static, T: Clone + 'static, S: AdditiveScore + 'static>(
    builder: &mut StreamGraphBuilder<'a, S>,
    input: &StreamPort<I, T>,
    callback: impl Fn(&T) -> S + 'a,
    concrete: bool,
) -> StreamConstraint<I, T> {
    if concrete {
        return builder.score("sum", input, callback).unwrap();
    }
    let live =
        live_commit_key::<I>() && !std::mem::needs_drop::<T>() && live_commit_callback(&callback);
    let out = builder.push(Terminal::<I, T, S> {
        evaluate: TerminalEvaluation::Direct(Box::new(callback)),
        total: S::ZERO,
        name: "sum".into(),
        matches: terminal_rows::<I, T, S>(),
        consumed: None,
        checkpoint_box: CheckpointBox::new(),
    });
    builder.live_commit_nodes[out.node] = live;
    builder.constraints.insert("sum".into(), out.node);
    builder.connect(input.node, out.node, false);
    StreamConstraint(out)
}
fn graph_trace<S: AdditiveScore<Level = i64> + 'static>(
    concrete: bool,
    make: fn(i64) -> S,
) -> (Trace, Vec<u128>) {
    TRACE.with(|trace| trace.borrow_mut().clear());
    let callback_fail = Cell::new(false);
    let structural = Cell::new(false);
    let environment = Environment;
    let mut builder = StreamGraphBuilder::<S>::new();
    let source = builder.source::<usize, Row>();
    let callback_fail_ref = &callback_fail;
    let structural_ref = &structural;
    let constraint = score(
        &mut builder,
        source.port(),
        move |row| {
            environment.touch();
            record("score callback", row.0);
            assert!(!callback_fail_ref.get(), "score panic");
            make(row.0).with_structural(if structural_ref.get() { -1 } else { 0 })
        },
        concrete,
    );
    let mut graph = builder.build();
    let mut counts = Vec::new();
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, Row(7))).unwrap();
        counts.push(tx.score_evaluations());
        tx.commit().unwrap();
    }
    // First observe an equal candidate after its write, then retain its owner
    // through rollback, an alternate trial, and captured-state installation.
    let (captured, candidate) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, Row(7))).unwrap();
        counts.push(tx.score_evaluations());
        let candidate = tx.graph.constraint_matches(&constraint).unwrap().clone();
        (tx.capture().unwrap(), candidate)
    };
    let baseline = graph.constraint_matches(&constraint).unwrap().clone();
    assert!(!std::ptr::eq(
        candidate.get(&0).unwrap(),
        baseline.get(&0).unwrap()
    ));
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(0usize, Row(9))).unwrap();
        counts.push(tx.score_evaluations());
    }
    graph.install(captured);
    assert!(std::ptr::eq(
        candidate.get(&0).unwrap(),
        graph
            .constraint_matches(&constraint)
            .unwrap()
            .get(&0)
            .unwrap()
    ));
    graph.assert_consistent().unwrap();
    callback_fail.set(true);
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(0usize, Row(8)))
            ))
            .is_err()
        );
        counts.push(tx.score_evaluations());
    }
    callback_fail.set(false);
    CLONE_FAIL.with(|fail| fail.set(true));
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(0usize, Row(8)))
            ))
            .is_err()
        );
        counts.push(tx.score_evaluations());
    }
    structural.set(true);
    {
        let mut tx = graph.transaction();
        assert!(matches!(
            tx.apply(&source, TupleChange::put(0usize, Row(8))),
            Err(GraphError::StructuralContribution(_))
        ));
        counts.push(tx.score_evaluations());
    }
    structural.set(false);
    assert_eq!(graph.score().ordinary_levels().collect::<Vec<_>>(), vec![7]);
    drop(candidate);
    drop(baseline);
    drop(graph);
    (TRACE.with(|trace| trace.borrow().clone()), counts)
}
#[test]
fn concrete_terminal_graph_retains_boxed_trace_panics_counts_owners_and_custom_score_hooks() {
    assert_eq!(
        graph_trace(true, SimpleScore::new),
        graph_trace(false, SimpleScore::new)
    );
    assert_eq!(
        graph_trace(true, |value| HookScore(SimpleScore::new(value))),
        graph_trace(false, |value| HookScore(SimpleScore::new(value)))
    );
}
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn terminal(concrete: bool) -> Box<dyn Node<SimpleScore>> {
    let callback = |value: &i64| SimpleScore::new(*value);
    if concrete {
        Box::new(TerminalState::<(usize, usize), i64, SimpleScore, _> {
            evaluate: DirectTerminal(callback),
            total: SimpleScore::ZERO,
            name: "sum".into(),
            matches: terminal_rows::<(usize, usize), i64, SimpleScore>(),
            consumed: None,
            checkpoint_box: CheckpointBox::new(),
        })
    } else {
        Box::new(Terminal::<(usize, usize), i64, SimpleScore> {
            evaluate: TerminalEvaluation::Direct(Box::new(callback)),
            total: SimpleScore::ZERO,
            name: "sum".into(),
            matches: terminal_rows::<(usize, usize), i64, SimpleScore>(),
            consumed: None,
            checkpoint_box: CheckpointBox::new(),
        })
    }
}
fn put(
    node: &mut dyn Node<SimpleScore>,
    id: (usize, usize),
    value: Option<i64>,
) -> Result<(), GraphError> {
    node.apply(false, &Event::new(TupleChange { id, value }), arithmetic())
        .map(|_| ())
}
fn matches(node: &dyn Node<SimpleScore>) -> &SnapshotMap<(usize, usize), (i64, SimpleScore)> {
    node.matches().unwrap().downcast_ref().unwrap()
}
#[test]
fn concrete_terminal_shared_full_nested_current_state_and_checked_errors_stay_canonical() {
    for concrete in [false, true] {
        let mut node = terminal(concrete);
        let id = (usize::MAX, usize::MAX);
        put(node.as_mut(), id, Some(7)).unwrap();
        let full = node.checkpoint();
        let baseline = matches(node.as_ref()).clone();
        let outer = node.begin_checkpoint();
        put(node.as_mut(), id, Some(7)).unwrap();
        let candidate = matches(node.as_ref()).clone();
        assert!(!std::ptr::eq(
            candidate.get(&id).unwrap(),
            baseline.get(&id).unwrap()
        ));
        let child = node.begin_checkpoint();
        put(node.as_mut(), (0, 0), Some(3)).unwrap();
        node.commit_checkpoint(child);
        let current = node.capture_checkpoint(outer.as_ref());
        node.restore(outer);
        assert!(std::ptr::eq(
            matches(node.as_ref()).get(&id).unwrap(),
            baseline.get(&id).unwrap()
        ));
        let outer = node.begin_checkpoint();
        node.restore(current);
        assert!(std::ptr::eq(
            matches(node.as_ref()).get(&id).unwrap(),
            candidate.get(&id).unwrap()
        ));
        assert_eq!(node.score(), SimpleScore::new(10));
        node.restore(outer);
        node.restore(full);
        let outer = node.begin_checkpoint();
        node.clear();
        put(node.as_mut(), id, Some(i64::MAX)).unwrap();
        assert!(matches!(
            put(node.as_mut(), (0, 0), Some(1)),
            Err(GraphError::ScoreOverflow { .. })
        ));
        assert_eq!(node.score(), SimpleScore::new(i64::MAX));
        node.restore(outer);
        assert!(std::ptr::eq(
            matches(node.as_ref()).get(&id).unwrap(),
            baseline.get(&id).unwrap()
        ));
    }
}
#[test]
fn concrete_terminal_preserves_live_commit_callback_certification_and_borrowed_lifetimes() {
    for concrete in [false, true] {
        let calls = Cell::new(0);
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, usize>();
        score(
            &mut builder,
            source.port(),
            |value| {
                calls.set(calls.get() + 1);
                SimpleScore::new(*value as i64)
            },
            concrete,
        );
        let mut graph = builder.build();
        assert!(graph.supports_live_commit());
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0usize, 2usize)).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(calls.get(), 1);
        drop(graph);
        let environment = Rc::new(Cell::new(0));
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, usize>();
        score(
            &mut builder,
            source.port(),
            {
                let environment = environment.clone();
                move |value| {
                    environment.set(environment.get() + 1);
                    SimpleScore::new(*value as i64)
                }
            },
            concrete,
        );
        assert!(!builder.build().supports_live_commit());
    }
}
#[test]
fn concrete_terminal_weighted_metadata_and_default_alias_remain_original() {
    let mut node = Terminal::<usize, i64, SimpleScore> {
        evaluate: TerminalEvaluation::Weighted {
            weight: SimpleScore::new(2),
            impact: Box::new(|value, weight, arithmetic| Ok(arithmetic.scale(weight, *value)?)),
        },
        total: SimpleScore::ZERO,
        name: "weighted".into(),
        matches: terminal_rows::<usize, i64, SimpleScore>(),
        consumed: None,
        checkpoint_box: CheckpointBox::new(),
    };
    assert_eq!(node.constraint_weight(), Some(&SimpleScore::new(2)));
    node.replace_weight(SimpleScore::new(3));
    node.apply(
        false,
        &Event::new(TupleChange::put(0usize, 4i64)),
        arithmetic(),
    )
    .unwrap();
    assert_eq!(node.score(), SimpleScore::new(12));
}

#[test]
fn concrete_terminal_panicking_callback_destructor_keeps_remaining_field_drop_phases() {
    struct Bomb;
    impl Bomb {
        fn touch(&self) {}
    }
    impl Drop for Bomb {
        fn drop(&mut self) {
            record("callback destructor panic", 0);
            panic!("callback destructor panic");
        }
    }
    fn trace(concrete: bool) -> Trace {
        TRACE.with(|trace| trace.borrow_mut().clear());
        let mut builder = StreamGraphBuilder::<HookScore>::new();
        let source = builder.source::<usize, Row>();
        let bomb = Bomb;
        score(
            &mut builder,
            source.port(),
            move |row| {
                bomb.touch();
                HookScore(SimpleScore::new(row.0))
            },
            concrete,
        );
        let mut graph = builder.build();
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0usize, Row(7))).unwrap();
            tx.commit().unwrap();
        }
        assert!(catch_unwind(AssertUnwindSafe(|| drop(graph))).is_err());
        TRACE.with(|trace| trace.borrow().clone())
    }
    assert_eq!(trace(true), trace(false));
}
