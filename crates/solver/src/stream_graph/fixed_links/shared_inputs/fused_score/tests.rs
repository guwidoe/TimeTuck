//! Original native regressions. Source-only definitions; no upstream-method credit.
use super::*;
use crate::{SimpleScore, StreamBatch};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
};
fn s(value: i64) -> SimpleScore {
    SimpleScore::new(value)
}
type Id = ((i64, String), char);
type Payload = (i64, i64, i64);
type Matches = BTreeMap<Id, (Payload, SimpleScore)>;
const TOPOLOGY: [(i64, &str, char); 5] = [
    (19, "z", 'q'),
    (3, "a", 'q'),
    (11, "a", 'x'),
    (7, "z", 'x'),
    (5, "a", 'q'),
];
fn independent(left: &BTreeMap<String, i64>, right: &BTreeMap<char, i64>) -> Matches {
    TOPOLOGY
        .iter()
        .filter_map(|&(edge, i, j)| {
            let (a, b) = (left.get(i)?, right.get(&j)?);
            ((edge + a + b) % 3 != 0).then(|| {
                (
                    ((edge, i.to_owned()), j),
                    ((edge, *a, *b), s(edge + a * 10 - b)),
                )
            })
        })
        .collect()
}
fn actual(g: &StreamGraph<'_, SimpleScore>, c: &StreamConstraint<Id, Payload>) -> Matches {
    g.constraint_matches(c)
        .unwrap()
        .iter()
        .map(|(id, row)| (id.clone(), *row))
        .collect()
}
#[test]
fn sparse_generic_final_events_and_every_match_equal_independent_source_model() {
    for pruning in [false, true] {
        let calls = Rc::new(Cell::new(0_u128));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let left = b.source::<String, i64>();
        let right = b.source::<char, i64>();
        let counter = calls.clone();
        let c = b
            .score_fixed_links_batched_shared(
                "model",
                left.port(),
                right.port(),
                TOPOLOGY.map(|(e, i, j)| (e, i.to_owned(), j)),
                |e, a, b| (e + a + b) % 3 != 0,
                |e, a, b| (*e, *a, *b),
                move |(e, a, b)| {
                    counter.set(counter.get() + 1);
                    s(e + a * 10 - b)
                },
            )
            .unwrap();
        let mut g = b.build();
        let lifetime = g.score_evaluation_counters[c.0.node]
            .as_ref()
            .unwrap()
            .clone();
        let mut l = BTreeMap::new();
        let mut r = BTreeMap::new();
        for round in 0..37_i64 {
            let mut batch = StreamBatch::new();
            // Repeated IDs and put/retract pairs must publish only final rows.
            for (id, value) in [
                ("a", Some(round + 100)),
                ("z", Some(2 - round)),
                ("unknown", Some(91)),
                ("a", (round % 4 != 0).then_some(round)),
            ] {
                batch
                    .push(
                        &left,
                        TupleChange {
                            id: id.to_owned(),
                            value,
                        },
                    )
                    .unwrap();
                if let Some(value) = value {
                    l.insert(id.to_owned(), value);
                } else {
                    l.remove(id);
                }
            }
            for (id, value) in [
                ('x', Some(999)),
                ('q', Some(round % 5)),
                ('x', (round % 3 != 0).then_some(7 - round)),
                ('?', Some(1000)),
            ] {
                batch.push(&right, TupleChange { id, value }).unwrap();
                if let Some(value) = value {
                    r.insert(id, value);
                } else {
                    r.remove(&id);
                }
            }
            let before = calls.get();
            let mut tx = g.transaction();
            tx.apply_batch(batch).unwrap();
            assert_eq!(tx.score_evaluations(), calls.get() - before);
            let expected = independent(&l, &r);
            assert_eq!(actual(tx.graph().unwrap(), &c), expected);
            assert_eq!(
                tx.score().unwrap(),
                s(expected.values().map(|(_, v)| v.score()).sum())
            );
            assert_eq!(
                tx.graph().unwrap().nodes[c.0.node].match_count(),
                expected.len()
            );
            tx.commit().unwrap();
            assert_eq!(
                g.source_rows(&left).unwrap().iter().collect::<Vec<_>>(),
                l.iter().collect::<Vec<_>>()
            );
            assert_eq!(
                g.source_rows(&right).unwrap().iter().collect::<Vec<_>>(),
                r.iter().collect::<Vec<_>>()
            );
            assert_eq!(lifetime.get(), calls.get());
        }
        let held = g.constraint_matches(&c).unwrap().clone();
        let saved = g.capture_full();
        let before = calls.get();
        let mut tx = g.transaction();
        tx.apply_batch(StreamBatch::new()).unwrap();
        assert_eq!(tx.score_evaluations(), calls.get() - before);
        assert_eq!(tx.score_evaluations(), 0);
        tx.commit().unwrap();
        g.assert_consistent().unwrap();
        g.install(saved);
        assert_eq!(g.constraint_matches(&c).unwrap(), &held);
        assert_eq!(lifetime.get(), calls.get());
    }
}

#[test]
fn interleaved_trace_counts_actual_scores_including_zero_and_old_api_is_unchanged() {
    let trace = Rc::new(RefCell::new(Vec::new()));
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let p = trace.clone();
    let q = trace.clone();
    let e = trace.clone();
    let fused = b
        .score_fixed_links_batched_shared(
            "fused",
            source.port(),
            source.port(),
            [(8, 0, 1), (1, 0, 0), (5, 1, 0)],
            move |edge, _, _| {
                p.borrow_mut().push(("p", *edge));
                *edge != 5
            },
            move |edge, _, _| {
                q.borrow_mut().push(("q", *edge));
                *edge
            },
            move |edge| {
                e.borrow_mut().push(("s", *edge));
                s(0)
            },
        )
        .unwrap();
    let old_calls = Cell::new(0_u128);
    b.score("old", source.port(), |_| {
        old_calls.set(old_calls.get() + 1);
        s(0)
    })
    .unwrap();
    let mut g = b.build();
    let mut batch = StreamBatch::new();
    batch.push(&source, TupleChange::put(0, 100)).unwrap();
    batch.push(&source, TupleChange::put(0, 2)).unwrap();
    batch.push(&source, TupleChange::put(1, 3)).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(batch).unwrap();
    assert_eq!(
        *trace.borrow(),
        [
            ("p", 1),
            ("q", 1),
            ("s", 1),
            ("p", 5),
            ("p", 8),
            ("q", 8),
            ("s", 8)
        ]
    );
    let real = trace
        .borrow()
        .iter()
        .filter(|(kind, _)| *kind == "s")
        .count() as u128;
    assert_eq!(tx.score_evaluations(), real + old_calls.get());
    assert_eq!(old_calls.get(), 2); // Legacy terminal scores both source-coalesced puts.
    tx.commit().unwrap();
    assert_eq!(g.constraint_matches(&fused).unwrap().len(), 2);
    assert_eq!(g.nodes[fused.0.node].match_count(), 2);
    let before = trace.borrow().len();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    assert_eq!(tx.score_evaluations(), 0);
    tx.commit().unwrap();
    assert_eq!(trace.borrow().len(), before); // Captured retractions need no old predicate.
    assert!(g.constraint_matches(&fused).unwrap().is_empty());
}

#[test]
fn each_callback_panic_counts_real_attempts_poison_rolls_back_and_short_retry_recovers() {
    for phase in 1..=3 {
        for batching in [false, true] {
            let failure = Rc::new(Cell::new(0));
            let score_calls = Rc::new(Cell::new(0_u128));
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, i64>();
            let p = failure.clone();
            let q = failure.clone();
            let e = failure.clone();
            let calls = score_calls.clone();
            let c = b
                .score_fixed_links_batched_shared(
                    "panic",
                    source.port(),
                    source.port(),
                    [(1, 0, 0), (4, 0, 1), (7, 1, 0)],
                    move |edge, _, _| {
                        assert!(!(p.get() == 1 && *edge == 4), "predicate");
                        true
                    },
                    move |edge, a, z| {
                        assert!(!(q.get() == 2 && *edge == 4), "projection");
                        (*edge, a + z)
                    },
                    move |(edge, value)| {
                        calls.set(calls.get() + 1);
                        assert!(!(e.get() == 3 && *edge == 4), "score");
                        s(*value)
                    },
                )
                .unwrap();
            let mut g = b.build();
            let lifetime = g.score_evaluation_counters[c.0.node]
                .as_ref()
                .unwrap()
                .clone();
            let mut initial = StreamBatch::new();
            initial.push(&source, TupleChange::put(0, 2)).unwrap();
            initial.push(&source, TupleChange::put(1, 3)).unwrap();
            let mut tx = g.transaction();
            tx.apply_batch(initial).unwrap();
            tx.commit().unwrap();
            let held = g.constraint_matches(&c).unwrap().clone();
            let original = g.score();
            failure.set(phase);
            let before = score_calls.get();
            let mut tx = g.transaction();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    if batching {
                        let mut changes = StreamBatch::new();
                        changes.push(&source, TupleChange::put(0, 99)).unwrap();
                        changes.push(&source, TupleChange::put(1, 88)).unwrap();
                        tx.apply_batch(changes).unwrap();
                    } else {
                        tx.apply(&source, TupleChange::put(0, 99)).unwrap();
                    }
                }))
                .is_err()
            );
            assert_eq!(tx.score_evaluations(), score_calls.get() - before);
            assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
            assert!(matches!(tx.graph(), Err(GraphError::FailedTransaction)));
            assert!(matches!(tx.capture(), Err(GraphError::FailedTransaction)));
            assert!(matches!(tx.commit(), Err(GraphError::FailedTransaction)));
            failure.set(0);
            assert_eq!(g.score(), original);
            assert_eq!(g.constraint_matches(&c).unwrap(), &held);
            assert_eq!(lifetime.get(), score_calls.get());
            let before = score_calls.get();
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(1, 6)).unwrap();
            assert_eq!(tx.score_evaluations(), score_calls.get() - before);
            tx.commit().unwrap();
            g.assert_consistent().unwrap();
            assert_eq!(lifetime.get(), score_calls.get());
        }
    }
}

#[test]
fn saved_winner_and_diagnostic_nested_replay_keep_transaction_counts_local() {
    let calls = Cell::new(0_u128);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let c = b
        .score_fixed_links_batched_shared(
            "saved",
            source.port(),
            source.port(),
            [(0, 0, 0), (9, 0, 1)],
            |_, _, _| true,
            |_, a, b| a + b,
            |v| {
                calls.set(calls.get() + 1);
                s(*v)
            },
        )
        .unwrap();
    let mut g = b.build();
    let counter = g.score_evaluation_counters[c.0.node]
        .as_ref()
        .unwrap()
        .clone();
    let mut initial = StreamBatch::new();
    initial.push(&source, TupleChange::put(0, 2)).unwrap();
    initial.push(&source, TupleChange::put(1, 3)).unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(initial).unwrap();
    tx.commit().unwrap();
    let held = g.constraint_matches(&c).unwrap().clone();
    let full = g.capture_full();
    let before = calls.get();
    let winner = {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 8)).unwrap();
        let own = tx.score_evaluations();
        assert_eq!(own, calls.get() - before);
        let diagnostic_start = calls.get();
        tx.assert_consistent().unwrap();
        assert!(calls.get() > diagnostic_start);
        assert_eq!(tx.score_evaluations(), own);
        let saved = tx.capture().unwrap();
        assert_eq!(
            tx.graph().unwrap().constraint_matches(&c).unwrap()[&((0, 0), 0)].0,
            16
        );
        saved
    };
    assert_eq!(g.constraint_matches(&c).unwrap(), &held);
    let before_install = calls.get();
    g.install(winner);
    assert_eq!(calls.get(), before_install);
    assert_eq!(g.score(), s(27));
    assert_eq!(held[&((0, 0), 0)].0, 4);
    g.install(full);
    assert_eq!(g.constraint_matches(&c).unwrap(), &held);
    assert_eq!(counter.get(), calls.get());
}

#[test]
fn checked_impact_terminal_total_graph_total_and_structural_failures_count_before_rollback() {
    for mode in 0..4 {
        let calls = Cell::new(0_u128);
        let mut b = StreamGraphBuilder::<SimpleScore>::new()
            .with_arithmetic(crate::ScoringArithmetic::Checked);
        let source = b.source::<usize, i64>();
        let c = if mode == 0 {
            b.score_fixed_links_batched_shared_weighted(
                "failure",
                source.port(),
                source.port(),
                [(0, 0, 0)],
                |_, _, _| true,
                |_, a, _| *a,
                s(2),
                |v| {
                    calls.set(calls.get() + 1);
                    *v
                },
            )
            .unwrap()
        } else {
            b.score_fixed_links_batched_shared(
                "failure",
                source.port(),
                source.port(),
                if mode == 1 {
                    vec![(0, 0, 0), (1, 0, 0)]
                } else {
                    vec![(0, 0, 0)]
                },
                |_, _, _| true,
                |_, a, _| *a,
                |v| {
                    calls.set(calls.get() + 1);
                    s(*v).with_structural(i64::from(mode == 3))
                },
            )
            .unwrap()
        };
        if mode == 2 {
            b.score("other", source.port(), |_| s(i64::MAX)).unwrap();
        }
        let mut g = b.build();
        let counter = g.score_evaluation_counters[c.0.node]
            .as_ref()
            .unwrap()
            .clone();
        let mut batch = StreamBatch::new();
        batch.push(&source, TupleChange::put(0, i64::MAX)).unwrap();
        let mut tx = g.transaction();
        let result = tx.apply_batch(batch);
        if mode == 3 {
            assert!(matches!(result, Err(GraphError::StructuralContribution(_))));
        } else {
            assert!(matches!(result, Err(GraphError::ScoreOverflow { .. })));
        }
        assert_eq!(tx.score_evaluations(), calls.get() + u128::from(mode == 2));
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        drop(tx);
        assert_eq!(g.score(), s(0));
        assert!(g.source_rows(&source).unwrap().is_empty());
        assert!(g.constraint_matches(&c).unwrap().is_empty());
        assert_eq!(counter.get(), calls.get());
    }
}

#[test]
fn wrapping_keeps_real_scores_and_zero_membership_while_retracts_do_no_work() {
    let calls = Cell::new(0_u128);
    let mut b = StreamGraphBuilder::<SimpleScore>::new()
        .with_arithmetic(crate::ScoringArithmetic::Wrapping);
    let source = b.source::<usize, i64>();
    let c = b
        .score_fixed_links_batched_shared(
            "wrapping",
            source.port(),
            source.port(),
            [(0, 0, 0), (2, 0, 0)],
            |_, _, _| true,
            |_, a, _| *a,
            |v| {
                calls.set(calls.get() + 1);
                s(*v)
            },
        )
        .unwrap();
    let mut g = b.build();
    for value in [i64::MAX, i64::MIN, 0, 7] {
        let before = calls.get();
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, value)).unwrap();
        assert_eq!(tx.score_evaluations(), calls.get() - before);
        assert_eq!(tx.score().unwrap(), s(value.wrapping_mul(2)));
        tx.commit().unwrap();
        assert_eq!(g.constraint_matches(&c).unwrap().len(), 2);
    }
    let before = calls.get();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    assert_eq!(tx.score_evaluations(), 0);
    tx.commit().unwrap();
    assert_eq!(calls.get(), before);
    assert!(g.constraint_matches(&c).unwrap().is_empty());
}

fn weights(value: i64) -> crate::ConstraintWeightOverrides<SimpleScore> {
    [("weighted".to_owned(), s(value))].into_iter().collect()
}
#[test]
fn disabled_updates_reactivation_failed_weight_replay_and_activity_retain_complete_facts() {
    for pruning in [false, true] {
        let calls = Cell::new(0_u128);
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new()
            .with_arithmetic(crate::ScoringArithmetic::Checked);
        if pruning {
            b = b.with_activity_pruning();
        }
        let left = b.source::<usize, i64>();
        let right = b.source::<usize, i64>();
        let c = b
            .score_fixed_links_batched_shared_weighted(
                "weighted",
                left.port(),
                right.port(),
                [(1, 0, 0), (4, 0, 1)],
                |_, _, _| true,
                |_, a, b| a + b,
                s(0),
                |v| {
                    calls.set(calls.get() + 1);
                    assert!(!fail.get(), "weight replay callback");
                    *v
                },
            )
            .unwrap();
        let mut g = b.build();
        let counter = g.score_evaluation_counters[c.0.node]
            .as_ref()
            .unwrap()
            .clone();
        let mut batch = StreamBatch::new();
        batch.push(&left, TupleChange::put(0, 2)).unwrap();
        batch.push(&left, TupleChange::put(100, 99)).unwrap();
        batch.push(&right, TupleChange::put(0, 3)).unwrap();
        batch.push(&right, TupleChange::put(1, 4)).unwrap();
        let mut tx = g.transaction();
        tx.apply_batch(batch).unwrap();
        assert_eq!(tx.score_evaluations(), 0);
        tx.commit().unwrap();
        let before = calls.get();
        let work = g.apply_weight_overrides(&weights(2)).unwrap();
        assert_eq!(work, calls.get() - before);
        assert_eq!(g.score(), s(22));
        let held = g.constraint_matches(&c).unwrap().clone();
        fail.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| g.apply_weight_overrides(&weights(3)))).is_err());
        fail.set(false);
        assert_eq!(g.score(), s(22));
        assert_eq!(g.constraint_matches(&c).unwrap(), &held);
        assert_eq!(g.constraint_weights().next().unwrap().1, &s(2));
        g.apply_weight_overrides(&weights(0)).unwrap();
        let mut tx = g.transaction();
        tx.apply(&left, TupleChange::put(0, 7)).unwrap();
        tx.apply(&right, TupleChange::retract(1)).unwrap();
        assert_eq!(tx.score_evaluations(), 0);
        tx.commit().unwrap();
        let before = calls.get();
        let work = g.apply_weight_overrides(&weights(1)).unwrap();
        assert_eq!(work, calls.get() - before);
        assert_eq!(g.score(), s(10));
        assert_eq!(g.source_rows(&left).unwrap()[&100], 99);
        assert_eq!(g.nodes[c.0.node].match_count(), 1);
        let held = g.constraint_matches(&c).unwrap().clone();
        let mut tx = g.transaction();
        tx.apply(&right, TupleChange::retract(0)).unwrap();
        tx.commit().unwrap();
        assert!(g.constraint_matches(&c).unwrap().is_empty());
        let before = calls.get();
        let mut tx = g.transaction();
        tx.apply(&right, TupleChange::put(0, 8)).unwrap();
        assert_eq!(tx.score_evaluations(), calls.get() - before);
        tx.commit().unwrap();
        assert_eq!(g.score(), s(15));
        assert_eq!(held[&((1, 0), 0)].0, 10);
        g.assert_consistent().unwrap();
        assert_eq!(counter.get(), calls.get());
    }
}

struct Endpoint(i64); // No Clone requirement at this boundary.
#[derive(Debug)]
struct Output {
    value: i64,
    clones: Rc<Cell<usize>>,
    panic_clone: Rc<Cell<bool>>,
}
impl Clone for Output {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        assert!(!self.panic_clone.get(), "inspection clone");
        Self {
            value: self.value,
            clones: self.clones.clone(),
            panic_clone: self.panic_clone.clone(),
        }
    }
}
type PrivateNode =
    FusedFixedLinkConstraint<'static, usize, usize, usize, Endpoint, Endpoint, Output, SimpleScore>;
fn private_node(clones: &Rc<Cell<usize>>, panic_clone: &Rc<Cell<bool>>) -> PrivateNode {
    let clones = clones.clone();
    let panic_clone = panic_clone.clone();
    FusedFixedLinkConstraint {
        inputs: SharedFixedLinks {
            links: vec![((2, 0), 0), ((9, 0), 0)],
            endpoints: vec![(0, 0), (0, 0)],
            by_left: BTreeMap::from([(0, 0)]),
            by_right: BTreeMap::from([(0, 0)]),
            direct_left: true,
            direct_right: true,
            left_links: vec![vec![0, 1]],
            right_links: vec![vec![0, 1]],
            left: Slots::new(1),
            right: Slots::new(1),
            checkpoint_box: CheckpointBox::new(),
            left_changes: Vec::new(),
            right_changes: Vec::new(),
            affected: Vec::new(),
            left_inputs: vec![usize::MAX],
            right_inputs: vec![usize::MAX],
            inputs_indexed: false,
            predicate: Box::new(|_, _, _| true),
            projection: Box::new(move |edge, a, b| Output {
                value: *edge as i64 + a.0 + b.0,
                clones: clones.clone(),
                panic_clone: panic_clone.clone(),
            }),
        },
        rows: Slots::new(2),
        materialized: OnceCell::new(),
        evaluate: TerminalEvaluation::Direct(Box::new(|v: &Output| s(v.value))),
        total: s(0),
        count: 0,
        evaluations: Rc::new(Cell::new(0)),
        checkpoint_box: CheckpointBox::new(),
        name: "private".to_owned(),
    }
}
fn endpoint(id: usize, value: Option<i64>) -> Event {
    Event::new(TupleChange {
        id,
        value: value.map(Endpoint),
    })
}
fn checked() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
#[test]
fn shared_whole_event_owners_move_only_outputs_lazy_inspection_and_nested_full_journals() {
    let clones = Rc::new(Cell::new(0));
    let panic_clone = Rc::new(Cell::new(false));
    let mut n = private_node(&clones, &panic_clone);
    let left = endpoint(0, Some(3));
    let right = endpoint(0, Some(4));
    assert!(
        n.apply_batch(&[(false, left.clone()), (true, right.clone())], checked())
            .unwrap()
            .is_empty()
    );
    assert_eq!(clones.get(), 0); // No payload duplicate during physical scoring.
    let typed_left = left
        .clone()
        .downcast::<TupleChange<usize, Endpoint>>()
        .unwrap();
    let typed_right = right
        .clone()
        .downcast::<TupleChange<usize, Endpoint>>()
        .unwrap();
    assert!(Rc::ptr_eq(
        n.inputs.left.snapshot()[0].as_ref().unwrap(),
        &typed_left
    ));
    assert!(Rc::ptr_eq(
        n.inputs.right.snapshot()[0].as_ref().unwrap(),
        &typed_right
    ));
    let counter = n.evaluations.clone();
    assert_eq!(counter.get(), 2);
    panic_clone.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| n.matches())).is_err());
    panic_clone.set(false);
    let prior_clones = clones.get();
    let held = n
        .matches()
        .unwrap()
        .downcast_ref::<SnapshotMap<((usize, usize), usize), (Output, SimpleScore)>>()
        .unwrap()
        .clone();
    assert_eq!(clones.get() - prior_clones, 2);
    n.matches();
    assert_eq!(clones.get() - prior_clones, 2); // Cached inspection.
    let full = n.checkpoint();
    let outer = n.begin_checkpoint();
    n.apply(false, &endpoint(0, Some(8)), checked()).unwrap();
    let inner = n.begin_checkpoint();
    n.apply(true, &endpoint(0, Some(10)), checked()).unwrap();
    n.commit_checkpoint(inner);
    assert!(!n.scores_match(outer.as_ref()));
    let candidate = n.checkpoint();
    n.restore(full);
    assert!(n.scores_match(outer.as_ref()));
    n.restore(outer);
    assert_eq!(n.total, s(25));
    assert_eq!(n.count, 2);
    let before_install = counter.get();
    n.restore(candidate);
    assert_eq!(n.total, s(47));
    assert_eq!(counter.get(), before_install);
    assert_eq!(held[&((2, 0), 0)].0.value, 9);
    let old_owner = Rc::downgrade(&typed_left);
    drop(typed_left);
    drop(left);
    assert!(old_owner.upgrade().is_none()); // Old owner has no retained snapshot now.
    let saved = n.checkpoint();
    let current_owner = Rc::downgrade(n.inputs.left.snapshot()[0].as_ref().unwrap());
    n.clear();
    assert_eq!(n.count, 0);
    assert_eq!(n.total, s(0));
    assert!(current_owner.upgrade().is_some());
    n.restore(saved);
    assert_eq!(n.inputs.left.get(0).unwrap().id, 0);
    assert_eq!(n.inputs.left.get(0).unwrap().value.as_ref().unwrap().0, 8);
    n.clear();
    assert!(current_owner.upgrade().is_none());
    assert_eq!(counter.get(), before_install);
}

#[test]
fn public_non_clone_endpoint_mapping_and_real_basic_planning_trials_keep_full_matches() {
    use crate::{FiniteProblem, Move, StreamScoreDirector, Variable};
    let calls = Cell::new(0_u128);
    let problem =
        FiniteProblem::new(vec![Variable::new(vec![1_i64, 2, 3]).with_initial(1); 3]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i64>)>();
    let mapped = b
        .filter_map_batched(source.port(), |(_, v)| v.map(Endpoint))
        .unwrap();
    let c = b
        .score_fixed_links_batched_shared(
            "planning",
            &mapped,
            &mapped,
            [(0, 0, 0), (4, 0, 1), (9, 2, 0)],
            |_, _, _| true,
            |edge, a, b| (*edge, a.0, b.0),
            |(edge, a, b)| {
                calls.set(calls.get() + 1);
                s(*edge + a * 10 - b)
            },
        )
        .unwrap();
    let mut d = StreamScoreDirector::new(&problem, b.build(), source, problem.initial_assignment())
        .unwrap();
    let held = d.graph().constraint_matches(&c).unwrap().clone();
    for commit in [false, true] {
        let before_assignment = d.assignment().clone();
        let before_score = d.score();
        let before = calls.get();
        let trial = d
            .apply(&Move::Composite(vec![
                Move::Change {
                    variable: 0,
                    value: Some(3),
                },
                Move::Change {
                    variable: 2,
                    value: Some(2),
                },
            ]))
            .unwrap();
        assert_eq!(trial.constraint_evaluations(), calls.get() - before);
        let values = trial.assignment().values();
        let expected: BTreeMap<_, _> = [(0, 0, 0), (4, 0, 1), (9, 2, 0)]
            .into_iter()
            .map(|(edge, i, j)| {
                let (a, b) = (values[i].unwrap(), values[j].unwrap());
                (((edge, i), j), ((edge, a, b), s(edge + a * 10 - b)))
            })
            .collect();
        let actual: BTreeMap<_, _> = trial
            .graph()
            .constraint_matches(&c)
            .unwrap()
            .iter()
            .map(|(id, row)| (*id, *row))
            .collect();
        assert_eq!(actual, expected);
        if commit {
            trial.commit();
        } else {
            drop(trial);
            assert_eq!(d.assignment(), &before_assignment);
            assert_eq!(d.score(), before_score);
        }
    }
    assert_eq!(held[&((0, 0), 0)].0, (0, 1, 1));
    d.assert_consistent().unwrap();
}

#[test]
fn decimal_factors_actual_counts_and_failed_scale_restore_matches_and_weights() {
    use crate::{Decimal, DecimalArithmeticError, SimpleDecimalScore};
    let calls = Cell::new(0_u128);
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let source = b.source::<usize, Decimal>();
    let c = b
        .score_fixed_links_batched_shared_weighted_decimal(
            "decimal",
            source.port(),
            source.port(),
            [(0, 0, 0)],
            |_, _, _| true,
            |_, a, _| a.clone(),
            "0.1".parse().unwrap(),
            |v| {
                calls.set(calls.get() + 1);
                v.clone()
            },
        )
        .unwrap();
    let mut g = b.build();
    let counter = g.score_evaluation_counters[c.0.node]
        .as_ref()
        .unwrap()
        .clone();
    let mut batch = StreamBatch::new();
    batch
        .push(&source, TupleChange::put(0, "0.025".parse().unwrap()))
        .unwrap();
    let mut tx = g.transaction();
    tx.apply_batch(batch).unwrap();
    assert_eq!(tx.score_evaluations(), calls.get());
    tx.commit().unwrap();
    let held = g.constraint_matches(&c).unwrap().clone();
    assert_eq!(g.score(), "0.0025".parse().unwrap());
    let before = calls.get();
    let mut tx = g.transaction();
    assert_eq!(
        tx.apply(
            &source,
            TupleChange::put(0, Decimal::new(1.into(), i32::MAX))
        ),
        Err(GraphError::DecimalWeightArithmetic {
            constraint: "decimal".to_owned(),
            error: DecimalArithmeticError::ScaleOverflow
        })
    );
    assert_eq!(tx.score_evaluations(), calls.get() - before);
    drop(tx);
    assert_eq!(g.constraint_matches(&c).unwrap(), &held);
    let overrides = [("decimal".to_owned(), "2.0".parse().unwrap())]
        .into_iter()
        .collect();
    let before = calls.get();
    assert_eq!(
        g.apply_weight_overrides(&overrides).unwrap(),
        calls.get() - before
    );
    assert_eq!(g.score(), "0.0500".parse().unwrap());
    assert_eq!(counter.get(), calls.get());
}

#[test]
fn invalid_topology_foreign_ports_names_structural_weights_and_empty_declarations() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let foreign = StreamGraphBuilder::<SimpleScore>::new().source::<usize, i64>();
    assert!(matches!(
        b.score_fixed_links_batched_shared(
            "foreign",
            source.port(),
            foreign.port(),
            [(0, 0, 0)],
            |_, _, _| true,
            |_, a, _| *a,
            |v| s(*v)
        ),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.score_fixed_links_batched_shared(
            "duplicate",
            source.port(),
            source.port(),
            [(1, 0, 0), (1, 1, 1)],
            |_, _, _| true,
            |_, a, _| *a,
            |v| s(*v)
        ),
        Err(GraphError::Dependency(_))
    ));
    assert!(matches!(
        b.score_fixed_links_batched_shared_weighted(
            "weight",
            source.port(),
            source.port(),
            [(0, 0, 0)],
            |_, _, _| true,
            |_, a, _| *a,
            s(0).with_structural(1),
            |v| *v
        ),
        Err(GraphError::StructuralConstraintWeight(_))
    ));
    let c = b
        .score_fixed_links_batched_shared(
            "empty",
            source.port(),
            source.port(),
            std::iter::empty::<(usize, usize, usize)>(),
            |_, _, _| panic!("empty predicate"),
            |_, _, _| -> i64 { panic!("empty projection") },
            |_| panic!("empty score"),
        )
        .unwrap();
    assert!(matches!(
        b.score_fixed_links_batched_shared(
            "empty",
            source.port(),
            source.port(),
            [(0, 0, 0)],
            |_, _, _| true,
            |_, a, _| *a,
            |v| s(*v)
        ),
        Err(GraphError::DuplicateConstraint(_))
    ));
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(usize::MAX, 5)).unwrap();
    assert_eq!(tx.score_evaluations(), 0);
    tx.commit().unwrap();
    assert_eq!(g.score(), s(0));
    assert!(g.constraint_matches(&c).unwrap().is_empty());
    assert_eq!(g.source_rows(&source).unwrap()[&usize::MAX], 5);
    g.assert_consistent().unwrap();
}

#[test]
fn checked_replay_allows_valid_prefix_cancellation_and_rejects_final_overflow() {
    for overflow in [false, true] {
        let calls = Cell::new(0_u128);
        let mut b = StreamGraphBuilder::<SimpleScore>::new()
            .with_arithmetic(crate::ScoringArithmetic::Checked);
        let source = b.source::<usize, i64>();
        let c = b
            .score_fixed_links_batched_shared_weighted(
                "weighted",
                source.port(),
                source.port(),
                [(0, 0, 0), (1, 0, 0), (2, 0, 0)],
                |_, _, _| true,
                |edge, _, _| *edge,
                s(0),
                |edge| {
                    calls.set(calls.get() + 1);
                    match edge {
                        0 => i64::MAX,
                        1 => 1,
                        _ => {
                            if overflow {
                                0
                            } else {
                                -1
                            }
                        }
                    }
                },
            )
            .unwrap();
        let mut g = b.build();
        let counter = g.score_evaluation_counters[c.0.node]
            .as_ref()
            .unwrap()
            .clone();
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 4)).unwrap();
        tx.commit().unwrap();
        let before = calls.get();
        let result = g.apply_weight_overrides(&weights(1));
        assert_eq!(counter.get() - before, calls.get() - before);
        assert_eq!(calls.get() - before, 3); // All actual factors ran, even final-validation failure.
        if overflow {
            assert!(matches!(
                result,
                Err(GraphError::ScoreOverflow {
                    constraint: Some(_)
                })
            ));
            assert_eq!(g.score(), s(0));
            assert!(g.constraint_matches(&c).unwrap().is_empty());
            assert_eq!(g.constraint_weights().next().unwrap().1, &s(0));
        } else {
            assert_eq!(result.unwrap(), calls.get() - before);
            assert_eq!(g.score(), s(i64::MAX));
            assert_eq!(g.constraint_matches(&c).unwrap().len(), 3);
            g.assert_consistent().unwrap();
        }
        assert_eq!(g.source_rows(&source).unwrap()[&0], 4);
        assert_eq!(counter.get(), calls.get());
    }
}
