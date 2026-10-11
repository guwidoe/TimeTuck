//! Original live graph-weight regressions; no upstream-method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn weights(items: &[(&str, i64)]) -> ConstraintWeightOverrides<SimpleScore> {
    items
        .iter()
        .map(|(name, w)| (name.to_string(), s(*w)))
        .collect()
}
struct Fixture {
    graph: StreamGraph<'static, SimpleScore>,
    jobs: StreamSource<usize, (i64, i64)>,
    rates: StreamSource<usize, (i64, i64)>,
    grouped: StreamConstraint<i64, (i64, IntSummary)>,
    direct: StreamConstraint<usize, (i64, i64)>,
    data: Option<StreamDataset<i64, (i64, IntSummary)>>,
}
fn fixture(
    activity: bool,
    dataset: bool,
    initial: &ConstraintWeightOverrides<SimpleScore>,
) -> Fixture {
    let mut b = StreamGraphBuilder::new();
    if activity {
        b = b.with_activity_pruning();
    }
    let jobs = b.source::<usize, (i64, i64)>();
    let rates = b.source::<usize, (i64, i64)>();
    let pairs = b
        .join_equal(jobs.port(), rates.port(), |v| v.0, |v| v.0)
        .unwrap();
    let grouped = b
        .group_collect(&pairs, |((k, v), (_, rate))| (*k, v * rate))
        .unwrap();
    let data = dataset.then(|| b.dataset(&grouped).unwrap());
    let grouped = b
        .score_weighted("group", &grouped, s(0), |(_, summary)| summary.sum)
        .unwrap();
    let direct = b
        .score_weighted("direct", jobs.port(), s(2), |v| v.1)
        .unwrap();
    Fixture {
        graph: b.build_with_weights(initial).unwrap(),
        jobs,
        rates,
        grouped,
        direct,
        data,
    }
}
fn populate(f: &mut Fixture, jobs: &[(usize, (i64, i64))], rates: &[(usize, (i64, i64))]) {
    let mut batch = StreamBatch::new();
    for (id, v) in jobs {
        batch.push(&f.jobs, TupleChange::put(*id, *v)).unwrap();
    }
    for (id, v) in rates {
        batch.push(&f.rates, TupleChange::put(*id, *v)).unwrap();
    }
    let mut tx = f.graph.transaction();
    tx.apply_batch(batch).unwrap();
    tx.commit().unwrap();
}
#[test]
fn live_join_collection_rebuild_matches_fresh_configuration_and_keeps_sources_and_handles() {
    for activity in [false, true] {
        for dataset in [false, true] {
            let mut f = fixture(activity, dataset, &weights(&[]));
            let jobs = vec![(3, (1, 7)), (5, (1, 9)), (8, (2, 4))];
            let mut rates = vec![(0, (1, 2)), (1, (2, 3))];
            populate(&mut f, &jobs, &rates);
            let mut effective = weights(&[("group", 0), ("direct", 2)]);
            for (group, direct) in [(3, 2), (0, 0), (-2, 0), (0, 5), (4, -1)] {
                // Update both a row and cardinality while the group may be inactive.
                rates[0].1.1 += 1;
                rates.push((rates.len(), (1, 2)));
                populate(&mut f, &jobs, &rates);
                effective.insert("group", s(group));
                effective.insert("direct", s(direct));
                f.graph.apply_weight_overrides(&effective).unwrap();
                let mut fresh = fixture(activity, dataset, &effective);
                populate(&mut fresh, &jobs, &rates);
                assert_eq!(f.graph.score(), fresh.graph.score());
                assert_eq!(
                    f.graph.constraint_weights().collect::<Vec<_>>(),
                    fresh.graph.constraint_weights().collect::<Vec<_>>()
                );
                assert_eq!(
                    f.graph.constraint_matches(&f.grouped).unwrap(),
                    fresh.graph.constraint_matches(&fresh.grouped).unwrap()
                );
                assert_eq!(
                    f.graph.constraint_matches(&f.direct).unwrap(),
                    fresh.graph.constraint_matches(&fresh.direct).unwrap()
                );
                assert_eq!(
                    f.graph.source_rows(&f.jobs).unwrap(),
                    fresh.graph.source_rows(&fresh.jobs).unwrap()
                );
                assert_eq!(
                    f.graph.source_rows(&f.rates).unwrap(),
                    fresh.graph.source_rows(&fresh.rates).unwrap()
                );
                assert_eq!(
                    f.graph
                        .explain_constraint_default(&f.grouped, |v| vec![v.0])
                        .unwrap(),
                    fresh
                        .graph
                        .explain_constraint_default(&fresh.grouped, |v| vec![v.0])
                        .unwrap()
                );
                if let (Some(a), Some(b)) = (&f.data, &fresh.data) {
                    assert_eq!(
                        f.graph.dataset_rows(a).unwrap(),
                        fresh.graph.dataset_rows(b).unwrap()
                    );
                }
                assert_eq!(f.graph.apply_weight_overrides(&effective).unwrap(), 0);
                f.graph.assert_consistent().unwrap();
                let before = f.graph.score();
                {
                    let mut tx = f.graph.transaction();
                    tx.apply(&f.jobs, TupleChange::put(3, (1, 999))).unwrap();
                }
                assert_eq!(f.graph.score(), before);
                f.graph.assert_consistent().unwrap();
            }
        }
    }
}
#[test]
fn disabling_skips_callbacks_and_reactivation_restores_both_self_join_edges() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, i64>();
    let pairs = b
        .join_equal(source.port(), source.port(), |_| (), |_| ())
        .unwrap();
    let terminal = b
        .score_weighted("pairs", &pairs, s(0), |&(a, b)| {
            calls.set(calls.get() + 1);
            a * b
        })
        .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 2)).unwrap();
        tx.apply(&source, TupleChange::put(1, 3)).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(calls.get(), 0);
    let work = g.apply_weight_overrides(&weights(&[("pairs", 2)])).unwrap();
    assert_eq!(work, calls.get());
    assert_eq!(g.score(), s(50));
    assert_eq!(g.constraint_matches(&terminal).unwrap().len(), 4);
    let old = calls.get();
    assert_eq!(
        g.apply_weight_overrides(&weights(&[("pairs", 0)])).unwrap(),
        0
    );
    assert_eq!(calls.get(), old);
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::retract(0)).unwrap();
        tx.apply(&source, TupleChange::put(1, 7)).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(calls.get(), old);
    assert!(g.constraint_matches(&terminal).unwrap().is_empty());
    g.apply_weight_overrides(&weights(&[("pairs", -3)]))
        .unwrap();
    assert_eq!(g.score(), s(-147));
    g.assert_consistent().unwrap();
}
#[test]
fn invalid_batches_callback_failures_and_overflow_restore_configuration_and_caches() {
    for activity in [false, true] {
        let fail = Cell::new(false);
        let calls = Cell::new(0);
        let mut b = StreamGraphBuilder::new();
        if activity {
            b = b.with_activity_pruning();
        }
        let src = b.source::<usize, i64>();
        let mapped = b
            .map(src.port(), |v| {
                assert!(!fail.get(), "reactivation mapper panic");
                *v
            })
            .unwrap();
        let terminal = b
            .score_weighted("disabled", &mapped, s(0), |v| {
                calls.set(calls.get() + 1);
                *v
            })
            .unwrap();
        b.score("fixed", src.port(), |_| s(5)).unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&src, TupleChange::put(0, 2)).unwrap();
            tx.commit().unwrap();
        }
        for override_ in [
            weights(&[("missing", 1)]),
            weights(&[("fixed", 1)]),
            [("disabled".to_string(), s(3).with_structural(-1))]
                .into_iter()
                .collect(),
        ] {
            assert!(g.apply_weight_overrides(&override_).is_err());
            assert_eq!(calls.get(), 0);
            assert_eq!(g.score(), s(5));
        }
        fail.set(true);
        assert!(
            catch_unwind(AssertUnwindSafe(
                || g.apply_weight_overrides(&weights(&[("disabled", 2)]))
            ))
            .is_err()
        );
        fail.set(false);
        assert_eq!(
            g.constraint_weights().collect::<Vec<_>>(),
            vec![("disabled", &s(0))]
        );
        assert_eq!(g.score(), s(5));
        assert!(g.constraint_matches(&terminal).unwrap().is_empty());
        assert!(matches!(
            g.apply_weight_overrides(&weights(&[("disabled", i64::MAX)])),
            Err(GraphError::ScoreOverflow {
                constraint: Some(_)
            })
        ));
        assert_eq!(g.score(), s(5));
        assert_eq!(g.source_rows(&src).unwrap()[&0], 2);
        g.assert_consistent().unwrap();
        g.apply_weight_overrides(&weights(&[("disabled", 3)]))
            .unwrap();
        assert_eq!(g.score(), s(11));
        g.assert_consistent().unwrap();
    }
}
#[test]
fn checked_rebuild_accepts_prefix_cancellation_but_rejects_terminal_and_total_overflow() {
    let mut b = StreamGraphBuilder::new();
    let src = b.source::<usize, i64>();
    b.score_weighted("a", src.port(), s(0), |v| *v).unwrap();
    b.score_weighted("b", src.port(), s(0), |_| 1).unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        for (id, v) in [i64::MAX, i64::MAX, -i64::MAX].into_iter().enumerate() {
            tx.apply(&src, TupleChange::put(id, v)).unwrap();
        }
        tx.commit().unwrap();
    }
    g.apply_weight_overrides(&weights(&[("a", 1)])).unwrap();
    assert_eq!(g.score(), s(i64::MAX));
    assert_eq!(
        g.apply_weight_overrides(&weights(&[("b", 1)])),
        Err(GraphError::ScoreOverflow { constraint: None })
    );
    assert_eq!(g.score(), s(i64::MAX));
    g.apply_weight_overrides(&weights(&[("a", 0)])).unwrap();
    {
        let mut tx = g.transaction();
        tx.apply(&src, TupleChange::retract(2)).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(
        g.apply_weight_overrides(&weights(&[("a", 1)])),
        Err(GraphError::ScoreOverflow {
            constraint: Some("a".into())
        })
    );
    assert_eq!(g.score(), s(0));
    g.assert_consistent().unwrap();
}

#[derive(Debug)]
struct Payload {
    value: i64,
    clones: Rc<Cell<usize>>,
    fail: Rc<Cell<usize>>,
}
impl Clone for Payload {
    fn clone(&self) -> Self {
        self.clones.set(self.clones.get() + 1);
        assert_ne!(self.clones.get(), self.fail.get(), "payload clone panic");
        Self {
            value: self.value,
            clones: self.clones.clone(),
            fail: self.fail.clone(),
        }
    }
}
#[test]
fn every_payload_clone_panic_across_checkpoint_and_replay_restores_live_graph() {
    let clones = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(usize::MAX));
    let make = || {
        let mut b = StreamGraphBuilder::new().with_activity_pruning();
        let src = b.source::<usize, Payload>();
        let map = b.map(src.port(), |v| v.value).unwrap();
        let terminal = b.score_weighted("new", &map, s(0), |v| *v).unwrap();
        b.score_weighted("old", src.port(), s(1), |v| v.value)
            .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        for id in 0..3 {
            tx.apply(
                &src,
                TupleChange::put(
                    id,
                    Payload {
                        value: id as i64 + 1,
                        clones: clones.clone(),
                        fail: fail.clone(),
                    },
                ),
            )
            .unwrap();
        }
        tx.commit().unwrap();
        (g, src, terminal)
    };
    let updates = weights(&[("new", 3), ("old", -2)]);
    let (mut g, _, _) = make();
    clones.set(0);
    g.apply_weight_overrides(&updates).unwrap();
    let count = clones.get();
    assert!(count >= 3);
    for at in 1..=count {
        let (mut g, src, terminal) = make();
        clones.set(0);
        fail.set(at);
        assert!(
            catch_unwind(AssertUnwindSafe(|| g.apply_weight_overrides(&updates))).is_err(),
            "clone {at}"
        );
        fail.set(usize::MAX);
        assert_eq!(g.score(), s(6));
        assert_eq!(
            g.constraint_weights()
                .map(|(n, w)| (n, *w))
                .collect::<Vec<_>>(),
            vec![("new", s(0)), ("old", s(1))]
        );
        assert!(g.constraint_matches(&terminal).unwrap().is_empty());
        assert_eq!(g.source_rows(&src).unwrap().len(), 3);
        g.assert_consistent().unwrap();
        g.apply_weight_overrides(&updates).unwrap();
        assert_eq!(g.score(), s(6));
        g.assert_consistent().unwrap();
    }
}

#[test]
fn retained_directors_continue_search_and_exact_after_weight_reactivation() {
    fn basic_graph(w: i64) -> (StreamGraph<'static, SimpleScore>, PlanningStreamSource<i64>) {
        let mut b = StreamGraphBuilder::new();
        let src = b.source::<usize, (usize, Option<i64>)>();
        b.score_weighted("value", src.port(), s(w), |(_, v)| v.unwrap_or(0))
            .unwrap();
        (b.build(), src)
    }
    fn list_graph(w: i64) -> (StreamGraph<'static, SimpleScore>, ListStreamSources<i64>) {
        let mut b = StreamGraphBuilder::new().with_activity_pruning();
        let src = ListStreamSources::new(&mut b);
        b.score_weighted("value", src.values.port(), s(w), |v| {
            v.position.map_or(0, |p| p.entity as i64 + 1)
        })
        .unwrap();
        (b.build(), src)
    }
    fn mixed_graph(
        w: i64,
    ) -> (
        StreamGraph<'static, SimpleScore>,
        MixedStreamSources<i64, i64>,
    ) {
        let mut b = StreamGraphBuilder::new();
        let src = MixedStreamSources::new(&mut b);
        let joined = b
            .join_equal(src.basic.port(), src.lists.values.port(), |_| (), |_| ())
            .unwrap();
        b.score_weighted("value", &joined, s(w), |((_, b), v)| {
            b.unwrap_or(0) + v.position.map_or(0, |p| p.entity as i64 + 1)
        })
        .unwrap();
        (b.build(), src)
    }
    let basic = FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap();
    let lists = ListProblem::new_indexed(
        vec![7],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
    )
    .unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    for exact in [false, true] {
        let config = || SolverSequence {
            phases: vec![if exact {
                SolverPhase::Exact(ExactPhaseConfig::default())
            } else {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 3,
                    assert_scores: true,
                    ..Default::default()
                })
            }],
            ..Default::default()
        };
        let mixed_config = || MixedSolverSequence {
            phases: vec![if exact {
                MixedSolverPhase::Exact(ExactPhaseConfig::default())
            } else {
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search: LocalSearchConfig {
                        max_steps: 3,
                        assert_scores: true,
                        ..Default::default()
                    },
                    ..Default::default()
                })
            }],
            ..Default::default()
        };
        macro_rules! check {
            ($problem:expr,$make:ident,$director:ident,$config:expr) => {{
                let (g, src) = $make(0);
                let mut d =
                    $director::new($problem, g, src, $problem.initial_assignment()).unwrap();
                for w in [3, 0, -2, 5] {
                    let before = d.assignment().clone();
                    d.apply_weight_overrides(&weights(&[("value", w)])).unwrap();
                    assert_eq!(d.assignment(), &before);
                    let (g, src) = $make(w);
                    let mut fresh = $director::new($problem, g, src, before).unwrap();
                    assert_eq!(d.score(), fresh.score());
                    assert_eq!(
                        d.run_solver($config).unwrap(),
                        fresh.run_solver($config).unwrap()
                    );
                    assert_eq!(d.assignment(), fresh.assignment());
                    d.assert_consistent().unwrap();
                }
            }};
        }
        check!(&basic, basic_graph, StreamScoreDirector, config());
        check!(&lists, list_graph, ListStreamScoreDirector, config());
        check!(
            &mixed,
            mixed_graph,
            MixedStreamScoreDirector,
            mixed_config()
        );
    }
}
