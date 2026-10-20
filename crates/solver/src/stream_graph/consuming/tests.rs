//! Ownership-consuming search controls; no upstream-method credit.
use super::*;
use crate::{
    Assignment, FiniteProblem, HardSoftScore, LocalSearchConfig, Move, SimpleScore,
    StreamScoreDirector, Variable,
};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
type Trace = RefCell<Vec<&'static str>>;
type Planning = StreamSource<usize, (usize, Option<usize>)>;
fn shape<'a>(sailing: bool, trace: &'a Trace) -> (StreamGraph<'a, HardSoftScore>, Planning) {
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let present = b
        .filter(source.port(), move |r| {
            trace.borrow_mut().push("present");
            r.1.is_some()
        })
        .unwrap();
    let assigned = b
        .map(&present, move |&(i, v)| {
            trace.borrow_mut().push("assigned");
            (i / 3, i % 3, v.unwrap())
        })
        .unwrap();
    let loads = b
        .group_collect(&assigned, move |&(s, _, g)| {
            trace.borrow_mut().push("load");
            ((s, g), 1)
        })
        .unwrap();
    b.score("load", &loads, move |(_, sum)| {
        trace.borrow_mut().push("load score");
        HardSoftScore::new(-sum.sum * sum.sum, 0)
    })
    .unwrap();
    let pairs = b
        .join_equal(
            &assigned,
            &assigned,
            move |r| {
                trace.borrow_mut().push("left");
                (r.0, r.2)
            },
            move |r| {
                trace.borrow_mut().push("right");
                (r.0, r.2)
            },
        )
        .unwrap();
    let pairs = b
        .filter(&pairs, move |(a, b)| {
            trace.borrow_mut().push("pair");
            a.1 < b.1
        })
        .unwrap();
    let contacts = b
        .group_collect(&pairs, move |(a, b)| {
            trace.borrow_mut().push("contact");
            ((a.1, b.1), 1)
        })
        .unwrap();
    b.score("contacts", &contacts, move |(_, s)| {
        trace.borrow_mut().push("contact score");
        HardSoftScore::new(0, -s.sum * s.sum)
    })
    .unwrap();
    if sailing {
        let members = b
            .flat_map(&assigned, move |&(s, p, g)| {
                trace.borrow_mut().push("member");
                [(0usize, s, p, g), (1usize, s, p, g)]
            })
            .unwrap();
        let joined = b
            .join_equal(
                &members,
                &members,
                move |r| {
                    trace.borrow_mut().push("member left");
                    (r.0, r.1)
                },
                move |r| {
                    trace.borrow_mut().push("member right");
                    (r.0, r.1)
                },
            )
            .unwrap();
        let joined = b
            .filter(&joined, move |(a, b)| {
                trace.borrow_mut().push("member pair");
                a.2 < b.2
            })
            .unwrap();
        b.score("members", &joined, move |(a, b)| {
            trace.borrow_mut().push("member score");
            HardSoftScore::new(-i64::from(a.3 != b.3), 0)
        })
        .unwrap();
        let attributes = b
            .flat_map(&assigned, move |&(s, p, g)| {
                trace.borrow_mut().push("attribute");
                [(0usize, s, g, 0usize, i64::from(p == 0))]
            })
            .unwrap();
        let balance = b
            .group_collect(&attributes, move |&(a, s, g, t, v)| {
                trace.borrow_mut().push("balance");
                ((a, s, g, t), v)
            })
            .unwrap();
        b.score("balance", &balance, move |(_, s)| {
            trace.borrow_mut().push("balance score");
            HardSoftScore::new(0, -s.sum.abs())
        })
        .unwrap();
    }
    (b.build(), source)
}
#[test]
fn consuming_search_both_numeric_pipeline_shapes_activate_every_terminal_and_source_without_live_commit_changes()
 {
    let p = FiniteProblem::new(
        (0..6)
            .map(|_| Variable::new(vec![0usize, 1]).with_initial(0))
            .collect(),
    )
    .unwrap();
    for sailing in [false, true] {
        let trace = Trace::default();
        let (graph, source) = shape(sailing, &trace);
        assert!(
            graph.consuming_eligible,
            "known numeric graph must not silently fall back"
        );
        let live = graph.supports_live_commit();
        let mut director =
            StreamScoreDirector::new_consuming(&p, graph, source, p.initial_assignment()).unwrap();
        assert!(director.graph().consuming_search);
        assert_eq!(director.graph().supports_live_commit(), live);
        assert_eq!(
            director
                .graph()
                .nodes
                .iter()
                .filter(|n| n.is_constraint())
                .count(),
            if sailing { 4 } else { 2 }
        );
        assert!(
            director
                .graph()
                .nodes
                .iter()
                .filter(|n| n.is_constraint())
                .all(|n| n.matches().is_none() && n.match_count() > 0)
        );
        director.assert_consistent().unwrap();
    }
}
#[test]
fn consuming_search_actual_graph_fifo_callbacks_work_nested_full_current_restore_match_retained_legacy()
 {
    fn run(private: bool, sailing: bool) -> (Vec<&'static str>, Vec<HardSoftScore>, Vec<u128>) {
        let trace = Trace::default();
        let (mut graph, source) = shape(sailing, &trace);
        if private {
            assert!(graph.try_enable_consuming_search(source.node_id(), 6));
        }
        let mut scores = Vec::new();
        let mut work = Vec::new();
        {
            let mut tx = graph.transaction();
            for id in 0usize..6 {
                tx.apply(&source, TupleChange::put(id, (id, Some(id % 2))))
                    .unwrap();
            }
            work.push(tx.score_evaluations());
            tx.commit().unwrap();
        }
        let base = graph.score();
        let full = graph.capture_full();
        let saved = {
            let mut outer = graph.transaction();
            outer
                .apply(&source, TupleChange::put(0usize, (0usize, Some(0usize))))
                .unwrap();
            outer
                .apply(&source, TupleChange::put(0usize, (0usize, Some(1usize))))
                .unwrap();
            {
                let mut child = outer.graph.transaction();
                child
                    .apply(&source, TupleChange::put(1usize, (1usize, None)))
                    .unwrap();
                child
                    .apply(&source, TupleChange::put(1usize, (1usize, Some(0usize))))
                    .unwrap();
                child.commit().unwrap();
            }
            scores.push(outer.score().unwrap());
            work.push(outer.score_evaluations());
            outer.capture().unwrap()
        };
        assert_eq!(graph.score(), base);
        let calls = trace.borrow().len();
        graph.install(saved);
        assert_eq!(calls, trace.borrow().len());
        scores.push(graph.score());
        graph.install(full);
        assert_eq!(graph.score(), base);
        graph.assert_consistent().unwrap();
        scores.push(graph.score());
        drop(graph);
        (trace.into_inner(), scores, work)
    }
    for sailing in [false, true] {
        assert_eq!(run(true, sailing), run(false, sailing));
    }
}
#[test]
fn consuming_search_whole_graph_unknown_dataset_score_only_custom_and_populated_fallback_is_atomic()
{
    // Dataset/score-only/custom projections are unknown to this permission;
    // supporting neighboring ordinary nodes must never partially activate them.
    for unsupported in 0..4 {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        b.score("ordinary", source.port(), |r| {
            SimpleScore::new(r.1.unwrap_or(0) as i64)
        })
        .unwrap();
        match unsupported {
            0 => {
                b.dataset(source.port()).unwrap();
            }
            1 => {
                b.score_only("only", source.port(), |_| SimpleScore::new(0))
                    .unwrap();
            }
            2 => {
                b.map_cached(source.port(), (), |_, r| *r).unwrap();
            }
            _ => {
                let extra = b.source::<usize, String>();
                b.score("string", extra.port(), |_| SimpleScore::new(0))
                    .unwrap();
            }
        }
        let mut graph = b.build();
        assert!(!graph.consuming_eligible);
        assert!(!graph.try_enable_consuming_search(source.node_id(), 1));
        assert!(!graph.consuming_search);
        assert!(
            graph.nodes[graph.constraints["ordinary"]]
                .matches()
                .is_some()
        );
    }
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let planning = b.source::<usize, (usize, Option<usize>)>();
    b.score("ordinary", planning.port(), |_| SimpleScore::new(0))
        .unwrap();
    let facts = b.source::<usize, i64>();
    let scored = b
        .score("facts", facts.port(), |v| SimpleScore::new(*v))
        .unwrap();
    let mut graph = b.build();
    assert!(graph.consuming_eligible);
    {
        let mut tx = graph.transaction();
        tx.apply(&facts, TupleChange::put(usize::MAX, 7i64))
            .unwrap();
        tx.commit().unwrap();
    }
    let held = graph.constraint_matches(&scored).unwrap().clone();
    let owners = held.shared_entries();
    let weak = Arc::downgrade(&owners[0].1);
    assert!(!graph.try_enable_consuming_search(planning.node_id(), 1));
    assert!(
        graph
            .nodes
            .iter()
            .filter(|n| n.is_constraint())
            .all(|n| n.matches().is_some())
    );
    assert!(Arc::ptr_eq(
        &owners[0].1,
        &graph.constraint_matches(&scored).unwrap().shared_entries()[0].1
    ));
    assert!(weak.upgrade().is_some());
}
#[test]
fn consuming_search_terminal_sparse_ids_panic_structural_overflow_and_diagnostic_corruption_match_legacy()
 {
    fn run(private: bool, phase: usize) -> (Vec<&'static str>, String, SimpleScore) {
        let trace = Trace::default();
        let mode = Cell::new(0usize);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i64>();
        let filtered = b
            .filter(source.port(), |_| {
                trace.borrow_mut().push("predicate");
                assert_ne!(mode.get(), 1);
                true
            })
            .unwrap();
        let mapped = b
            .map(&filtered, |v| {
                trace.borrow_mut().push("mapping");
                assert_ne!(mode.get(), 2);
                *v
            })
            .unwrap();
        b.score("sum", &mapped, |v| {
            trace.borrow_mut().push("score");
            assert_ne!(mode.get(), 3);
            SimpleScore::new(if mode.get() == 5 { i64::MAX } else { *v })
                .with_structural(i64::from(mode.get() == 4))
        })
        .unwrap();
        let mut graph = b.build();
        if private {
            assert!(graph.try_enable_consuming_search(source.node_id(), 2));
        }
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0usize, 1i64)).unwrap();
            tx.apply(&source, TupleChange::put(1usize, 1i64)).unwrap();
            tx.commit().unwrap();
        }
        trace.borrow_mut().clear();
        mode.set(phase);
        let outcome = {
            let mut tx = graph.transaction();
            catch_unwind(AssertUnwindSafe(|| {
                tx.apply(&source, TupleChange::put(0usize, 3i64))
            }))
        };
        let kind = match outcome {
            Err(_) => "panic".to_owned(),
            Ok(Err(error)) => format!("{error:?}"),
            Ok(Ok(())) => panic!("expected failure"),
        };
        mode.set(0);
        assert_eq!(graph.score(), SimpleScore::new(2));
        graph.assert_consistent().unwrap();
        let score = graph.score();
        drop(graph);
        (trace.into_inner(), kind, score)
    }
    for phase in 1..=5 {
        assert_eq!(run(true, phase), run(false, phase));
    }
    // Corruption reports contain only score/node evidence, never payload owners.
    fn report(private: bool) -> crate::StreamScoreCorruption {
        let multiplier = Cell::new(1i64);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i64>();
        b.score("sum", source.port(), |v| {
            SimpleScore::new(*v * multiplier.get())
        })
        .unwrap();
        let mut graph = b.build();
        if private {
            assert!(graph.try_enable_consuming_search(source.node_id(), 1));
        }
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0usize, 3i64)).unwrap();
            tx.commit().unwrap();
        }
        multiplier.set(2);
        let report = graph.consistency_report().unwrap();
        assert_eq!(graph.score(), SimpleScore::new(3));
        report.diagnostic()
    }
    assert_eq!(report(true), report(false));
}

#[test]
fn consuming_search_public_provider_best_candidates_diagnostics_counters_and_callback_drop_match_legacy()
 {
    struct Watch<'a>(&'a Trace);
    impl Drop for Watch<'_> {
        fn drop(&mut self) {
            self.0.borrow_mut().push("callback drop");
        }
    }
    struct Provider<'a>(&'a Trace);
    impl crate::MoveProvider<FiniteProblem<usize>, Assignment<usize>, Move<usize>> for Provider<'_> {
        fn event(&mut self, event: crate::MoveProviderEvent) {
            use crate::MoveProviderEvent::*;
            self.0.borrow_mut().push(match event {
                PhaseStarted => "phase start",
                PhaseEnded => "phase end",
                StepStarted => "step start",
                StepEnded => "step end",
                _ => "solve",
            });
        }
        fn moves<'a>(
            &'a mut self,
            _: &'a FiniteProblem<usize>,
            a: &'a Assignment<usize>,
            _: u64,
        ) -> Box<dyn Iterator<Item = Move<usize>> + 'a>
        where
            Move<usize>: 'a,
        {
            self.0.borrow_mut().push("moves");
            Box::new(
                (0..2)
                    .filter(move |&id| a.values()[id] == Some(0))
                    .map(|variable| Move::Change {
                        variable,
                        value: Some(1usize),
                    }),
            )
        }
    }
    fn run(
        private: bool,
        diagnostics: bool,
    ) -> (
        Vec<&'static str>,
        crate::LocalSearchResult<usize, SimpleScore>,
    ) {
        let trace = Trace::default();
        let p = FiniteProblem::new(
            (0..2)
                .map(|_| Variable::new(vec![0usize, 1]).with_initial(0))
                .collect(),
        )
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        let watch = Watch(&trace);
        b.score("value", source.port(), move |(_, value)| {
            let _keep = &watch;
            trace_score(watch.0);
            SimpleScore::new(value.unwrap_or(0) as i64)
        })
        .unwrap();
        fn trace_score(trace: &Trace) {
            trace.borrow_mut().push("score");
        }
        let graph = b.build();
        assert!(graph.consuming_eligible);
        let config = LocalSearchConfig {
            max_evaluations: 8,
            max_steps: 3,
            selection: crate::MoveSelection::BestAccepted,
            assert_scores: diagnostics,
            ..Default::default()
        };
        let mut provider = Provider(&trace);
        let result = if private {
            crate::stream_local_search_from_with_provider(
                &p,
                graph,
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        } else {
            crate::provider_search::basic_stream(
                &p,
                graph,
                source,
                p.initial_assignment(),
                config,
                crate::provider_search::ProviderContext {
                    provider: crate::MoveProviderBinding::Infallible(&mut provider),
                    control: None,
                    observer: None,
                },
            )
        }
        .unwrap();
        assert_eq!(
            trace
                .borrow()
                .iter()
                .filter(|&&event| event == "callback drop")
                .count(),
            1
        );
        (trace.into_inner(), result)
    }
    for diagnostics in [false, true] {
        assert_eq!(run(true, diagnostics), run(false, diagnostics));
    }
}

thread_local! {static CUSTOM_HOOKS:RefCell<Vec<&'static str>>=const{RefCell::new(Vec::new())};}
#[derive(Debug, Eq, PartialEq)]
struct Value(usize);
impl Clone for Value {
    fn clone(&self) -> Self {
        CUSTOM_HOOKS.with(|v| v.borrow_mut().push("clone"));
        Self(self.0)
    }
}
#[test]
fn consuming_search_custom_no_drop_clone_fallback_and_public_director_keep_observable_owners() {
    fn custom(request: bool) -> Vec<&'static str> {
        CUSTOM_HOOKS.with(|v| v.borrow_mut().clear());
        let p = FiniteProblem::new(vec![
            Variable::new(vec![Value(0), Value(1)]).with_initial(Value(0)),
        ])
        .unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<Value>)>();
        let scored = b
            .score("value", source.port(), |(_, value)| {
                CUSTOM_HOOKS.with(|v| v.borrow_mut().push("score"));
                SimpleScore::new(value.as_ref().unwrap().0 as i64)
            })
            .unwrap();
        let graph = b.build();
        assert!(!graph.consuming_eligible);
        let mut director = if request {
            StreamScoreDirector::new_consuming(&p, graph, source, p.initial_assignment())
        } else {
            StreamScoreDirector::new(&p, graph, source, p.initial_assignment())
        }
        .unwrap();
        assert!(!director.graph().consuming_search);
        let held = director
            .graph()
            .constraint_matches(&scored)
            .unwrap()
            .clone();
        {
            drop(
                director
                    .apply(&Move::Change {
                        variable: 0,
                        value: Some(Value(1)),
                    })
                    .unwrap(),
            );
        }
        assert!(std::ptr::eq(
            &held[&0],
            &director.graph().constraint_matches(&scored).unwrap()[&0]
        ));
        drop(director);
        CUSTOM_HOOKS.with(|v| v.borrow().clone())
    }
    assert!(!std::mem::needs_drop::<Value>());
    assert_eq!(custom(true), custom(false));
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]).with_initial(0)]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let scored = b
        .score("value", source.port(), |(_, v)| {
            SimpleScore::new(v.unwrap_or(0) as i64)
        })
        .unwrap();
    let graph = b.build();
    assert!(graph.consuming_eligible);
    let mut director = StreamScoreDirector::new(&p, graph, source, p.initial_assignment()).unwrap();
    assert!(!director.graph().consuming_search);
    let held = director
        .graph()
        .constraint_matches(&scored)
        .unwrap()
        .clone();
    {
        drop(
            director
                .apply(&Move::Change {
                    variable: 0,
                    value: Some(1usize),
                })
                .unwrap(),
        );
    }
    assert!(std::ptr::eq(
        &held[&0],
        &director.graph().constraint_matches(&scored).unwrap()[&0]
    ));
}

#[test]
fn consuming_search_private_terminal_current_and_full_captures_keep_checked_weights_and_max_ids() {
    fn run(private: bool) -> (Vec<SimpleScore>, Vec<u128>) {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, usize>();
        let grouped = b
            .group_collect(source.port(), |&v| ((usize::MAX, v), 1))
            .unwrap();
        b.score_weighted("weighted", &grouped, SimpleScore::new(2), |(_, s)| s.sum)
            .unwrap();
        let mut graph = b.build();
        assert!(graph.consuming_eligible);
        if private {
            assert!(graph.try_enable_consuming_search(source.node_id(), 2));
        }
        let mut scores = Vec::new();
        let mut work = Vec::new();
        {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::put(0usize, usize::MAX))
                .unwrap();
            tx.apply(&source, TupleChange::put(1usize, usize::MAX - 1))
                .unwrap();
            work.push(tx.score_evaluations());
            tx.commit().unwrap();
        }
        graph.assert_consistent().unwrap();
        let base = graph.capture_full();
        let saved = {
            let mut tx = graph.transaction();
            tx.apply(&source, TupleChange::retract(0usize)).unwrap();
            tx.apply(&source, TupleChange::put(1usize, usize::MAX))
                .unwrap();
            scores.push(tx.score().unwrap());
            work.push(tx.score_evaluations());
            tx.capture().unwrap()
        };
        graph.install(saved);
        scores.push(graph.score());
        graph.assert_consistent().unwrap();
        graph.install(base);
        scores.push(graph.score());
        (scores, work)
    }
    assert_eq!(run(true), run(false));
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd)]
struct HookScore(SimpleScore);
impl Clone for HookScore {
    fn clone(&self) -> Self {
        CUSTOM_HOOKS.with(|v| v.borrow_mut().push("score clone"));
        Self(self.0)
    }
}
impl crate::Score for HookScore {
    type Level = i64;
    const HARD_LEVEL_COUNT: usize = 0;
    fn ordinary_levels(&self) -> impl Iterator<Item = i64> {
        self.0.ordinary_levels()
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
impl std::ops::Add for HookScore {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}
impl std::ops::Sub for HookScore {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}
impl crate::AdditiveScore for HookScore {
    const ZERO: Self = Self(SimpleScore::ZERO);
    fn with_structural(self, s: i64) -> Self {
        Self(self.0.with_structural(s))
    }
}
#[test]
fn consuming_search_custom_score_hooks_and_activity_capability_fall_back_before_any_activation() {
    fn run(request: bool) -> Vec<&'static str> {
        CUSTOM_HOOKS.with(|v| v.borrow_mut().clear());
        let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]).with_initial(0)]).unwrap();
        let mut b = StreamGraphBuilder::<HookScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        b.score("score", source.port(), |(_, v)| {
            HookScore(SimpleScore::new(v.unwrap_or(0) as i64))
        })
        .unwrap();
        let graph = b.build();
        assert!(graph.consuming_eligible);
        let mut d = if request {
            StreamScoreDirector::new_consuming(&p, graph, source, p.initial_assignment())
        } else {
            StreamScoreDirector::new(&p, graph, source, p.initial_assignment())
        }
        .unwrap();
        assert!(!d.graph().consuming_search);
        assert!(
            d.graph()
                .nodes
                .iter()
                .filter(|n| n.is_constraint())
                .all(|n| n.matches().is_some())
        );
        drop(
            d.apply(&Move::Change {
                variable: 0,
                value: Some(1usize),
            })
            .unwrap(),
        );
        drop(d);
        CUSTOM_HOOKS.with(|v| v.borrow().clone())
    }
    assert!(!std::mem::needs_drop::<HookScore>());
    assert_eq!(run(true), run(false));
    for capability in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if capability {
            b = b.with_activity_pruning();
        }
        let source = b.source::<usize, (usize, Option<usize>)>();
        b.score("value", source.port(), |_| SimpleScore::new(0))
            .unwrap();
        if !capability {
            b.after_initial_facts(source.port(), |_| {}).unwrap();
        }
        let mut graph = b.build();
        assert!(!graph.consuming_eligible);
        assert!(!graph.try_enable_consuming_search(source.node_id(), 1));
        assert!(
            graph
                .nodes
                .iter()
                .filter(|n| n.is_constraint())
                .all(|n| n.matches().is_some())
        );
    }
}

#[test]
fn consuming_search_public_fallible_provider_error_preserves_detached_error_and_callback_counts() {
    fn run(private: bool, error: crate::MoveProviderError) -> (usize, String) {
        let calls = Cell::new(0usize);
        let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]).with_initial(0)]).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        b.score("value", source.port(), |(_, v)| {
            calls.set(calls.get() + 1);
            SimpleScore::new(v.unwrap_or(0) as i64)
        })
        .unwrap();
        let mut provider = |_: &FiniteProblem<usize>,
                            _: &Assignment<usize>,
                            _: u64|
         -> Result<Vec<Move<usize>>, crate::MoveProviderError> {
            Err(error.clone())
        };
        let graph = b.build();
        let config = LocalSearchConfig {
            max_evaluations: 4,
            ..Default::default()
        };
        let result = if private {
            crate::stream_local_search_from_with_fallible_provider(
                &p,
                graph,
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        } else {
            crate::provider_search::basic_stream(
                &p,
                graph,
                source,
                p.initial_assignment(),
                config,
                crate::provider_search::ProviderContext {
                    provider: crate::MoveProviderBinding::Fallible(&mut provider),
                    control: None,
                    observer: None,
                },
            )
        };
        (calls.get(), format!("{:?}", result.unwrap_err()))
    }
    let error = crate::MoveProviderError::new(std::io::Error::other("provider stopped"));
    assert_eq!(run(true, error.clone()), run(false, error));
}

#[test]
fn consuming_search_cancel_during_scoring_restores_live_and_saved_candidate_paths() {
    fn run(
        private: bool,
        first: bool,
    ) -> (Vec<usize>, crate::LocalSearchResult<usize, SimpleScore>) {
        let token = crate::CancellationToken::new();
        let calls = RefCell::new(Vec::new());
        let p =
            FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]).with_initial(0)]).unwrap();
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, Option<usize>)>();
        b.score("value", source.port(), |(_, v)| {
            let value = v.unwrap_or(0);
            calls.borrow_mut().push(value);
            if value == 1 {
                token.cancel();
            }
            SimpleScore::new(value as i64)
        })
        .unwrap();
        let mut provider = |_: &FiniteProblem<usize>, _: &Assignment<usize>, _: u64| {
            vec![
                Move::Change {
                    variable: 0,
                    value: Some(1usize),
                },
                Move::Change {
                    variable: 0,
                    value: Some(2usize),
                },
            ]
        };
        let config = LocalSearchConfig {
            max_evaluations: 4,
            cancellation: Some(&token),
            selection: if first {
                crate::MoveSelection::FirstAccepted
            } else {
                crate::MoveSelection::BestAccepted
            },
            ..Default::default()
        };
        let graph = b.build();
        assert!(graph.supports_live_commit());
        let result = if private {
            crate::stream_local_search_from_with_provider(
                &p,
                graph,
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
        } else {
            crate::provider_search::basic_stream(
                &p,
                graph,
                source,
                p.initial_assignment(),
                config,
                crate::provider_search::ProviderContext {
                    provider: crate::MoveProviderBinding::Infallible(&mut provider),
                    control: None,
                    observer: None,
                },
            )
        }
        .unwrap();
        assert_eq!(result.termination, crate::LocalSearchTermination::Cancelled);
        (calls.into_inner(), result)
    }
    for first in [false, true] {
        assert_eq!(run(true, first), run(false, first));
    }
}
