//! Original concrete Map/Group controls; no upstream source-method credit.
use super::*;
use crate::{Score, SimpleScore};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Trace = Vec<(&'static str, i64)>;
thread_local! {static TRACE:RefCell<Trace>=const{RefCell::new(Vec::new())};static FAIL:Cell<u8>=const{Cell::new(0)};}
fn record(kind: &'static str, value: i64) {
    TRACE.with(|trace| trace.borrow_mut().push((kind, value)));
}
fn fault(code: u8) {
    assert!(
        !FAIL.with(|fail| {
            if fail.get() == code {
                fail.set(0);
                true
            } else {
                false
            }
        }),
        "requested callback/identity failure"
    );
}
#[derive(Debug, Eq, PartialEq)]
struct Id(usize);
impl Clone for Id {
    fn clone(&self) -> Self {
        record("id clone", self.0 as i64);
        fault(1);
        Self(self.0)
    }
}
impl PartialOrd for Id {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for Id {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        record("id compare", self.0 as i64);
        fault(6);
        self.0.cmp(&rhs.0)
    }
}
#[derive(Debug, Eq, PartialEq)]
struct Key(i64);
impl Clone for Key {
    fn clone(&self) -> Self {
        record("key clone", self.0);
        fault(3);
        Self(self.0)
    }
}
impl PartialOrd for Key {
    fn partial_cmp(&self, rhs: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}
impl Ord for Key {
    fn cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        record("key compare", self.0);
        fault(4);
        self.0.cmp(&rhs.0)
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        record("key drop", self.0);
    }
}
#[derive(Debug)]
struct Row(i64);
impl Clone for Row {
    fn clone(&self) -> Self {
        record("row clone", self.0);
        fault(5);
        Self(self.0)
    }
}
impl Drop for Row {
    fn drop(&mut self) {
        record("row drop", self.0);
    }
}
struct Environment(&'static str);
impl Environment {
    fn touch(&self) {
        record(self.0, 0);
    }
}
impl Drop for Environment {
    fn drop(&mut self) {
        record(self.0, -1);
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
    fn hard_cmp(&self, rhs: &Self) -> std::cmp::Ordering {
        self.0.hard_cmp(&rhs.0)
    }
    fn is_feasible(&self) -> bool {
        self.0.is_feasible()
    }
}
impl AdditiveScore for HookScore {
    const ZERO: Self = Self(SimpleScore::ZERO);
    fn with_structural(self, n: i64) -> Self {
        Self(self.0.with_structural(n))
    }
}
fn map<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    U: Clone + 'static,
    S: AdditiveScore + 'static,
>(
    b: &mut StreamGraphBuilder<'a, S>,
    input: &StreamPort<I, T>,
    mapping: impl Fn(&T) -> U + 'a,
    concrete: bool,
) -> StreamPort<I, U> {
    if concrete {
        return b.map(input, mapping).unwrap();
    }
    let live = live_commit_key::<I>()
        && !std::mem::needs_drop::<T>()
        && !std::mem::needs_drop::<U>()
        && live_commit_callback(&mapping);
    let out = b.push(Map::<I, T, U> {
        mapping: Box::new(mapping),
        marker: PhantomData,
    });
    b.live_commit_nodes[out.node] = live;
    b.connect(input.node, out.node, false);
    out
}
fn group<
    'a,
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    K: Ord + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    b: &mut StreamGraphBuilder<'a, S>,
    input: &StreamPort<I, T>,
    mapping: impl Fn(&T) -> (K, i64) + 'a,
    concrete: bool,
) -> StreamPort<K, (K, IntSummary)> {
    if concrete {
        return b.group_collect(input, mapping).unwrap();
    }
    let live = live_commit_key::<I>()
        && live_commit_key::<K>()
        && !std::mem::needs_drop::<T>()
        && live_commit_callback(&mapping);
    let mut node = GroupNode::<I, K>::new();
    node.bind_primitive_group_keys();
    node.bind_primitive_inputs();
    node.enable_equal_input_reuse();
    let out = b.push(Group::<I, T, K> {
        node,
        mapping: Box::new(mapping),
        marker: PhantomData,
    });
    b.live_commit_nodes[out.node] = live;
    b.connect(input.node, out.node, false);
    out
}
fn graph_trace<S: AdditiveScore<Level = i64> + 'static>(
    concrete: bool,
    make: fn(i64) -> S,
) -> (Trace, Vec<u128>) {
    TRACE.with(|trace| trace.borrow_mut().clear());
    FAIL.with(|fail| fail.set(0));
    let mut b = StreamGraphBuilder::<S>::new();
    let source = b.source::<Id, Row>();
    let map_environment = Environment("map environment");
    let group_environment = Environment("group environment");
    let mapped = map(
        &mut b,
        source.port(),
        move |row| {
            map_environment.touch();
            record("map", row.0);
            fault(2);
            Row(row.0)
        },
        concrete,
    );
    let grouped = group(
        &mut b,
        &mapped,
        move |row| {
            group_environment.touch();
            record("group map", row.0);
            fault(7);
            (Key(row.0 % 2), row.0)
        },
        concrete,
    );
    assert!(!b.nodes[grouped.node].prefers_batch());
    let constraint = b
        .score("sum", &grouped, move |(_, summary)| {
            record("evaluate", summary.sum);
            make(summary.sum)
        })
        .unwrap();
    let mut graph = b.build();
    let mut counts = Vec::new();
    for (id, value) in [(0usize, 2i64), (1, 4)] {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(Id(id), Row(value)))
            .unwrap();
        counts.push(tx.score_evaluations());
        tx.commit().unwrap();
    }
    let (saved, candidate) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(Id(0), Row(2))).unwrap();
        counts.push(tx.score_evaluations());
        let candidate = tx.graph.constraint_matches(&constraint).unwrap().clone();
        (tx.capture().unwrap(), candidate)
    };
    let key = Key(0);
    let baseline = graph.constraint_matches(&constraint).unwrap().clone();
    assert!(!std::ptr::eq(
        candidate.get(&key).unwrap(),
        baseline.get(&key).unwrap()
    ));
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(Id(0), Row(9))).unwrap();
        counts.push(tx.score_evaluations());
    }
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
    for failure in 1..=7 {
        FAIL.with(|fail| fail.set(failure));
        {
            let mut tx = graph.transaction();
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || tx.apply(&source, TupleChange::put(Id(0), Row(8)))
                ))
                .is_err(),
                "failure {failure}"
            );
            counts.push(tx.score_evaluations());
        }
        assert_eq!(FAIL.with(Cell::get), 0);
        assert_eq!(graph.score().ordinary_levels().collect::<Vec<_>>(), vec![6]);
    }
    {
        let mut tx = graph.transaction();
        let mut batch = crate::StreamBatch::new();
        batch
            .push(&source, TupleChange::put(Id(0), Row(6)))
            .unwrap();
        batch
            .push(&source, TupleChange::put(Id(0), Row(2)))
            .unwrap();
        batch.push(&source, TupleChange::retract(Id(1))).unwrap();
        tx.apply_batch(batch).unwrap();
        counts.push(tx.score_evaluations());
    }
    assert_eq!(graph.score().ordinary_levels().collect::<Vec<_>>(), vec![6]);
    drop(candidate);
    drop(baseline);
    drop(graph);
    drop(key);
    (TRACE.with(|trace| trace.borrow().clone()), counts)
}
#[test]
fn concrete_mapping_graph_preserves_id_key_clone_ord_drop_score_hooks_panics_and_late_owners() {
    assert_eq!(
        graph_trace(true, SimpleScore::new),
        graph_trace(false, SimpleScore::new)
    );
    assert_eq!(
        graph_trace(true, |n| HookScore(SimpleScore::new(n))),
        graph_trace(false, |n| HookScore(SimpleScore::new(n)))
    );
}
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn map_batch_trace(concrete: bool, batch: bool) -> (Trace, Vec<(usize, Option<i64>)>) {
    TRACE.with(|trace| trace.borrow_mut().clear());
    FAIL.with(|fail| fail.set(0));
    let mapping = |row: &Row| {
        record("map", row.0);
        Row(row.0 * 10)
    };
    let mut node: Box<dyn Node<SimpleScore>> = if concrete {
        Box::new(MapState::<Id, Row, Row, _> {
            mapping,
            marker: PhantomData,
        })
    } else {
        Box::new(Map::<Id, Row, Row> {
            mapping: Box::new(mapping),
            marker: PhantomData,
        })
    };
    let inputs = [(0usize, Some(1i64)), (1, Some(2)), (0, None), (1, Some(3))]
        .into_iter()
        .map(|(id, value)| {
            (
                false,
                Event::new(TupleChange {
                    id: Id(id),
                    value: value.map(Row),
                }),
            )
        })
        .collect::<Vec<_>>();
    let output = if batch {
        node.apply_batch(&inputs, arithmetic()).unwrap()
    } else {
        let mut output = Vec::new();
        for (_, event) in &inputs {
            output.extend(node.apply(false, event, arithmetic()).unwrap());
        }
        output
    };
    let observed = output
        .iter()
        .map(|event| {
            let change = typed::<Id, Row>(event);
            (change.id.0, change.value.as_ref().map(|row| row.0))
        })
        .collect();
    drop(output);
    drop(inputs);
    drop(node);
    (TRACE.with(|trace| trace.borrow().clone()), observed)
}
#[test]
fn concrete_map_batch_and_scalar_outputs_keep_per_event_projection_and_cleanup() {
    for batch in [false, true] {
        assert_eq!(map_batch_trace(true, batch), map_batch_trace(false, batch));
    }
}
#[test]
fn concrete_mapping_borrowed_callbacks_keep_live_commit_and_explicit_batched_group_boxed() {
    for concrete in [false, true] {
        let calls = Cell::new((0usize, 0usize));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i64>();
        let mapped = map(
            &mut b,
            source.port(),
            |value| {
                let (m, g) = calls.get();
                calls.set((m + 1, g));
                value * 2
            },
            concrete,
        );
        let grouped = group(
            &mut b,
            &mapped,
            |value| {
                let (m, g) = calls.get();
                calls.set((m, g + 1));
                (0usize, *value)
            },
            concrete,
        );
        b.score("sum", &grouped, |(_, summary)| {
            SimpleScore::new(summary.sum)
        })
        .unwrap();
        let mut graph = b.build();
        assert!(graph.supports_live_commit());
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0usize, 7i64)).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(calls.get(), (1, 1));
        assert_eq!(graph.score(), SimpleScore::new(14));
    }
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let grouped = b
        .group_collect_batched(source.port(), |value| {
            calls.set(calls.get() + 1);
            (0usize, *value)
        })
        .unwrap();
    assert!(b.nodes[grouped.node].prefers_batch());
    let constraint = b
        .score("sum", &grouped, |(_, summary)| {
            SimpleScore::new(summary.sum)
        })
        .unwrap();
    let mut graph = b.build();
    assert!(!graph.supports_live_commit());
    let mut batch = crate::StreamBatch::new();
    for (id, value) in [(0usize, 2i64), (0, 4), (1, 6)] {
        batch.push(&source, TupleChange::put(id, value)).unwrap();
    }
    {
        let mut tx = graph.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(tx.score_evaluations(), 1);
        tx.commit().unwrap();
    }
    assert_eq!(calls.get(), 2);
    assert_eq!(
        graph
            .constraint_matches(&constraint)
            .unwrap()
            .get(&0)
            .unwrap()
            .0
            .1
            .sum,
        10
    );
}
