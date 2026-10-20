//! Original dispatch controls; no upstream-method credit.
use super::*;
use crate::{Score, SimpleScore};
use std::{cell::RefCell, cmp::Ordering, ops};

type Trace = Rc<RefCell<Vec<(char, i64)>>>;
type PairSummary = ((usize, usize), crate::IntSummary);
type BuiltGraph = (
    StreamGraph<'static, SimpleScore>,
    StreamSource<usize, i64>,
    StreamConstraint<(usize, usize), PairSummary>,
    StreamConstraint<usize, i64>,
    StreamDataset<(usize, usize), PairSummary>,
);

fn built_graph(pruning: bool, legacy: bool, trace: Trace) -> BuiltGraph {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    if pruning {
        builder = builder.with_activity_pruning();
    }
    let source = builder.source::<usize, i64>();
    let accepted = builder.filter(source.port(), |value| *value >= 0).unwrap();
    let mapped = builder
        .map(&accepted, |value| ((*value as usize) % 2, *value))
        .unwrap();
    let joined = builder
        .join_equal(&mapped, &mapped, |row| row.0, |row| row.0)
        .unwrap();
    let grouped = builder
        .group_collect(&joined, |(left, right)| {
            ((left.0, right.0), left.1 + right.1)
        })
        .unwrap();
    let dataset = builder.dataset(&grouped).unwrap();
    let group_constraint = builder
        .score("groups", &grouped, {
            let trace = trace.clone();
            move |(_, summary)| {
                trace.borrow_mut().push(('g', summary.sum));
                SimpleScore::new(summary.sum)
            }
        })
        .unwrap();
    builder
        .score_only("only", &mapped, {
            let trace = trace.clone();
            move |(_, value)| {
                trace.borrow_mut().push(('o', *value));
                SimpleScore::new(*value * 2)
            }
        })
        .unwrap();
    let fixed = builder
        .score_fixed("fixed", source.port(), [0, 1, 2, 3], move |value| {
            trace.borrow_mut().push(('f', *value));
            SimpleScore::new(-*value)
        })
        .unwrap();
    let mut graph = builder.build();
    assert_eq!(
        graph
            .score_probe_nodes
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&&v| v)
            .count(),
        3
    );
    if legacy {
        graph.score_probe_nodes = None;
    }
    (graph, source, group_constraint, fixed, dataset)
}

#[test]
fn builtin_score_routing_matches_original_events_matches_datasets_and_physical_counts() {
    for pruning in [false, true] {
        let optimized_trace = Trace::default();
        let original_trace = Trace::default();
        let (mut optimized, source, grouped, fixed, dataset) =
            built_graph(pruning, false, optimized_trace.clone());
        let (mut original, old_source, old_grouped, old_fixed, old_dataset) =
            built_graph(pruning, true, original_trace.clone());
        for (round, change) in [
            TupleChange::put(0, 3),
            TupleChange::put(1, 4),
            TupleChange::put(2, -1),
            TupleChange::put(0, 7),
            TupleChange::retract(1),
            TupleChange::put(3, 0),
            TupleChange::retract(2),
            TupleChange::put(1, 6),
        ]
        .into_iter()
        .enumerate()
        {
            optimized_trace.borrow_mut().clear();
            original_trace.borrow_mut().clear();
            let before = optimized.dataset_rows(&dataset).unwrap().clone();
            let mut transaction = optimized.transaction();
            let mut old_transaction = original.transaction();
            transaction.apply(&source, change.clone()).unwrap();
            old_transaction.apply(&old_source, change).unwrap();
            assert_eq!(
                transaction.score().unwrap(),
                old_transaction.score().unwrap()
            );
            assert_eq!(
                transaction.score_evaluations(),
                old_transaction.score_evaluations()
            );
            assert_eq!(
                transaction.score_evaluations(),
                optimized_trace.borrow().len() as u128
            );
            assert_eq!(*optimized_trace.borrow(), *original_trace.borrow());
            assert_eq!(
                transaction
                    .graph()
                    .unwrap()
                    .constraint_matches(&grouped)
                    .unwrap(),
                old_transaction
                    .graph()
                    .unwrap()
                    .constraint_matches(&old_grouped)
                    .unwrap()
            );
            assert_eq!(
                transaction
                    .graph()
                    .unwrap()
                    .constraint_matches(&fixed)
                    .unwrap(),
                old_transaction
                    .graph()
                    .unwrap()
                    .constraint_matches(&old_fixed)
                    .unwrap()
            );
            assert_eq!(
                transaction.graph().unwrap().dataset_rows(&dataset).unwrap(),
                old_transaction
                    .graph()
                    .unwrap()
                    .dataset_rows(&old_dataset)
                    .unwrap()
            );
            if round % 3 == 1 {
                drop(transaction);
                drop(old_transaction);
                assert_eq!(optimized.dataset_rows(&dataset).unwrap(), &before);
            } else {
                transaction.commit().unwrap();
                old_transaction.commit().unwrap();
            }
            assert_eq!(optimized.score(), original.score());
            assert_eq!(
                optimized.source_rows(&source).unwrap(),
                original.source_rows(&old_source).unwrap()
            );
        }
        optimized_trace.borrow_mut().clear();
        original_trace.borrow_mut().clear();
        optimized.assert_consistent().unwrap();
        original.assert_consistent().unwrap();
        assert_eq!(*optimized_trace.borrow(), *original_trace.borrow());
        assert_eq!(optimized.score(), original.score());
    }
}

#[test]
fn builtin_score_routing_preserves_checked_overflow_failure_boundaries_and_rollback() {
    for legacy in [false, true] {
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, i64>();
        let mapped = builder.map(source.port(), |value| *value).unwrap();
        for (name, contribution) in [("large", i64::MAX), ("one", 1)] {
            let calls = calls.clone();
            builder
                .score(name, &mapped, move |_| {
                    calls.borrow_mut().push(name);
                    SimpleScore::new(contribution)
                })
                .unwrap();
        }
        let mut graph = builder.build();
        if legacy {
            graph.score_probe_nodes = None;
        }
        let mut transaction = graph.transaction();
        assert!(matches!(
            transaction.apply(&source, TupleChange::put(0, 1)),
            Err(GraphError::ScoreOverflow { constraint: None })
        ));
        assert_eq!(transaction.score_evaluations(), 2);
        assert_eq!(*calls.borrow(), ["large", "one"]);
        assert!(transaction.score().is_err());
        drop(transaction);
        assert_eq!(graph.score(), SimpleScore::ZERO);
        assert!(graph.source_rows(&source).unwrap().is_empty());
    }
}

fn assert_builtin_metadata<S: AdditiveScore + 'static>() {
    let mut builder = StreamGraphBuilder::<S>::new();
    let source = builder.source::<usize, i64>();
    let mapped = builder.map(source.port(), |value| *value).unwrap();
    let score = builder.score("zero", &mapped, |_| S::ZERO).unwrap();
    let graph = builder.build();
    let flags = graph
        .score_probe_nodes
        .as_ref()
        .expect("exact built-in score");
    assert!(!flags[source.0.node]);
    assert!(!flags[mapped.node]);
    assert!(flags[score.0.node]);
}
#[test]
fn routing_allowlist_is_exact_and_all_builtin_terminal_kinds_remain_scoring_nodes() {
    assert_builtin_metadata::<SimpleScore>();
    assert_builtin_metadata::<crate::HardSoftScore>();
    assert_builtin_metadata::<crate::HardMediumSoftScore>();
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let left = builder.source::<usize, i64>();
    let right = builder.source::<usize, i64>();
    let fused = builder
        .score_fixed_links_batched_shared(
            "fused",
            left.port(),
            right.port(),
            [(0usize, 0usize, 0usize)],
            |_, _, _| true,
            |_, left, right| left + right,
            |value| SimpleScore::new(*value),
        )
        .unwrap();
    let mut graph = builder.build();
    assert!(graph.score_probe_nodes.as_ref().unwrap()[fused.0.node]);
    let mut batch = crate::StreamBatch::new();
    batch.push(&left, TupleChange::put(0, 3)).unwrap();
    batch.push(&right, TupleChange::put(0, 4)).unwrap();
    let mut transaction = graph.transaction();
    transaction.apply_batch(batch).unwrap();
    assert_eq!(transaction.score_evaluations(), 1);
    assert_eq!(transaction.score().unwrap(), SimpleScore::new(7));
    transaction.commit().unwrap();
}

thread_local! {
    static HOOKS: RefCell<Vec<(&'static str, i64, i64)>> = const { RefCell::new(Vec::new()) };
    static FAIL_EQUALITY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn record(kind: &'static str, left: i64, right: i64) {
    HOOKS.with(|hooks| hooks.borrow_mut().push((kind, left, right)));
}
#[derive(Debug)]
struct ObservableScore(i64);
impl Clone for ObservableScore {
    fn clone(&self) -> Self {
        record("clone", self.0, 0);
        Self(self.0)
    }
}
impl PartialEq for ObservableScore {
    fn eq(&self, other: &Self) -> bool {
        record("eq", self.0, other.0);
        assert!(
            !FAIL_EQUALITY.with(std::cell::Cell::get),
            "observable score equality failure"
        );
        self.0 == other.0
    }
}
impl Eq for ObservableScore {}
impl PartialOrd for ObservableScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ObservableScore {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}
impl ops::Add for ObservableScore {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        record("add", self.0, other.0);
        Self(self.0 + other.0)
    }
}
impl ops::Sub for ObservableScore {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        record("sub", self.0, other.0);
        Self(self.0 - other.0)
    }
}
impl Score for ObservableScore {
    type Level = i64;
    const HARD_LEVEL_COUNT: usize = 0;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        std::iter::once(self.0)
    }
    fn structural(&self) -> i64 {
        0
    }
    fn hard_cmp(&self, _: &Self) -> Ordering {
        Ordering::Equal
    }
    fn is_feasible(&self) -> bool {
        true
    }
}
impl AdditiveScore for ObservableScore {
    const ZERO: Self = Self(0);
    // A custom score's capability opt-in must not expand the TypeId allowlist.
    const POOLED_SAME_CONTEXT_MEMO: bool = true;
    fn with_structural(self, _: i64) -> Self {
        self
    }
    fn checked_adjust<'a>(
        &self,
        changes: impl IntoIterator<Item = (&'a Self, &'a Self)>,
    ) -> Option<Self>
    where
        Self: 'a,
    {
        let changes: Vec<_> = changes.into_iter().collect();
        record("checked-adjust", self.0, changes.len() as i64);
        changes
            .into_iter()
            .try_fold(self.0, |value, (old, new)| {
                value.checked_sub(old.0)?.checked_add(new.0)
            })
            .map(Self)
    }
}
struct ZeroProbe<'a> {
    node: Box<dyn Node<ObservableScore> + 'a>,
}
impl Node<ObservableScore> for ZeroProbe<'_> {
    fn checkpoint(&self) -> Box<dyn Any> {
        self.node.checkpoint()
    }
    fn clear(&mut self) {
        self.node.clear();
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.node.restore(saved);
    }
    fn score(&self) -> ObservableScore {
        record("zero-score", 0, 0);
        ObservableScore::ZERO
    }
    fn evaluates_score(&self, _: &Event) -> bool {
        record("zero-evaluates", 0, 0);
        false
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        self.node.apply(right, event, arithmetic)
    }
}

#[test]
fn custom_no_drop_score_retains_zero_equality_clone_and_checked_hooks() {
    assert!(!std::mem::needs_drop::<ObservableScore>());
    let mut builder = StreamGraphBuilder::<ObservableScore>::new();
    let source = builder.source::<usize, i64>();
    let mapped = builder.map(source.port(), |value| *value).unwrap();
    builder
        .score("value", &mapped, |value| {
            record("callback", *value, 0);
            ObservableScore(*value)
        })
        .unwrap();
    let mut graph = builder.build();
    assert!(graph.score_probe_nodes.is_none());
    let node = graph.nodes.remove(mapped.node);
    graph
        .nodes
        .insert(mapped.node, Box::new(ZeroProbe { node }));
    let mut transaction = graph.transaction();
    HOOKS.with(|hooks| hooks.borrow_mut().clear());
    transaction.apply(&source, TupleChange::put(0, 5)).unwrap();
    let hooks = HOOKS.with(|hooks| hooks.borrow().clone());
    assert_eq!(
        hooks
            .iter()
            .filter(|&&(kind, _, _)| kind == "zero-score")
            .count(),
        2
    );
    assert_eq!(
        hooks
            .iter()
            .filter(|&&(kind, _, _)| kind == "zero-evaluates")
            .count(),
        1
    );
    assert_eq!(
        hooks
            .iter()
            .filter(|&&(kind, left, right)| kind == "eq" && left == 0 && right == 0)
            .count(),
        2
    );
    assert_eq!(
        hooks
            .iter()
            .filter(|&&(kind, _, _)| kind == "clone")
            .count(),
        4
    );
    assert_eq!(
        hooks
            .iter()
            .filter(|&&(kind, _, _)| kind == "checked-adjust")
            .count(),
        2
    );
    assert_eq!(transaction.score_evaluations(), 1);
    transaction.commit().unwrap();
    assert_eq!(graph.score().0, 5);
    let mut transaction = graph.transaction();
    FAIL_EQUALITY.with(|failure| failure.set(true));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = transaction.apply(&source, TupleChange::put(0, 9));
        }))
        .is_err()
    );
    FAIL_EQUALITY.with(|failure| failure.set(false));
    drop(transaction);
    assert_eq!(graph.score().0, 5);
    assert_eq!(graph.source_rows(&source).unwrap()[&0], 5);
}

struct BuiltinZeroProbe<'a> {
    node: Box<dyn Node<SimpleScore> + 'a>,
    scores: Rc<std::cell::Cell<usize>>,
    evaluations: Rc<std::cell::Cell<usize>>,
}
impl Node<SimpleScore> for BuiltinZeroProbe<'_> {
    fn checkpoint(&self) -> Box<dyn Any> {
        self.node.checkpoint()
    }
    fn clear(&mut self) {
        self.node.clear();
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        self.node.restore(saved);
    }
    fn score(&self) -> SimpleScore {
        self.scores.set(self.scores.get() + 1);
        SimpleScore::ZERO
    }
    fn evaluates_score(&self, _: &Event) -> bool {
        self.evaluations.set(self.evaluations.get() + 1);
        false
    }
    fn apply(
        &mut self,
        right: bool,
        event: &Event,
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        self.node.apply(right, event, arithmetic)
    }
}

#[test]
fn builtin_noncontributing_nodes_execute_without_virtual_score_or_evaluation_probes() {
    for legacy in [false, true] {
        let scores = Rc::new(std::cell::Cell::new(0));
        let evaluations = Rc::new(std::cell::Cell::new(0));
        let mut builder = StreamGraphBuilder::<SimpleScore>::new();
        let source = builder.source::<usize, i64>();
        let mapped = builder.map(source.port(), |value| *value).unwrap();
        builder
            .score("value", &mapped, |value| SimpleScore::new(*value))
            .unwrap();
        let mut graph = builder.build();
        if legacy {
            graph.score_probe_nodes = None;
        }
        let node = graph.nodes.remove(mapped.node);
        graph.nodes.insert(
            mapped.node,
            Box::new(BuiltinZeroProbe {
                node,
                scores: scores.clone(),
                evaluations: evaluations.clone(),
            }),
        );
        let mut transaction = graph.transaction();
        transaction.apply(&source, TupleChange::put(0, 7)).unwrap();
        assert_eq!(transaction.score_evaluations(), 1);
        transaction.commit().unwrap();
        assert_eq!(graph.score(), SimpleScore::new(7));
        assert_eq!(scores.get(), if legacy { 2 } else { 0 });
        assert_eq!(evaluations.get(), usize::from(legacy));
    }
}
