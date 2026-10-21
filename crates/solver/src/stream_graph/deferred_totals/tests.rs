//! Original totals-telescoping controls; no upstream method credit.
use super::super::*;
use crate::{HardSoftScore, SimpleScore};
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn deferred_totals_preserve_every_callback_work_capture_failure_and_retry() {
    type Trace = Vec<(&'static str, usize, usize)>;
    type ResultTrace = (Trace, Vec<HardSoftScore>, Vec<u128>, String);
    fn run(defer: bool, wide: bool, phase: usize) -> ResultTrace {
        event_queue::with_wide(wide, || {
            let trace = RefCell::new(Trace::new());
            let failure = Cell::new(0usize);
            let mut b = StreamGraphBuilder::<HardSoftScore>::new();
            let source = b.source::<usize, (usize, usize, usize)>();
            let pairs = b
                .join_equal(source.port(), source.port(), |r| r.0, |r| r.0)
                .unwrap();
            let accepted = b.filter(&pairs, |(a, b)| a.1 < b.1).unwrap();
            let grouped = b
                .group_collect(&accepted, |(a, b)| ((a.1, b.1), 1))
                .unwrap();
            b.score("group", &grouped, |(key, summary)| {
                trace.borrow_mut().push(("group", key.0, summary.count));
                assert_ne!(failure.get(), 1, "group callback");
                match failure.get() {
                    2 => HardSoftScore::new(0, 0).with_structural(1),
                    3 => HardSoftScore::new(i64::MAX, 0),
                    _ => HardSoftScore::new(0, summary.sum),
                }
            })
            .unwrap();
            b.score("parallel", &grouped, |(key, summary)| {
                trace.borrow_mut().push(("parallel", key.1, summary.count));
                HardSoftScore::new(0, -summary.sum)
            })
            .unwrap();
            b.score("direct", &accepted, |(a, b)| {
                trace.borrow_mut().push(("direct", a.1, b.1));
                HardSoftScore::new(0, (a.2 + b.2) as i64)
            })
            .unwrap();
            let mut g = b.build();
            assert!(g.try_enable_consuming_search(source.node_id(), 6));
            // Clearing the existing optional probe metadata selects the
            // original per-event path without granting any new permission.
            if !defer {
                g.score_probe_nodes = None;
            }
            {
                let mut tx = g.transaction();
                for n in 0usize..6 {
                    tx.apply(&source, TupleChange::put(n, (0, n, n + 1)))
                        .unwrap();
                }
                tx.commit().unwrap();
            }
            let base = g.score();
            let full = g.capture_full();
            let mut scores = vec![base];
            let mut work = Vec::new();
            trace.borrow_mut().clear();
            let saved = {
                let mut tx = g.transaction();
                tx.apply(&source, TupleChange::put(0, (0, 7, 10))).unwrap();
                {
                    let mut child = tx.graph.transaction();
                    child.apply(&source, TupleChange::retract(1)).unwrap();
                    child
                        .apply(&source, TupleChange::put(1, (0, 8, 11)))
                        .unwrap();
                    child.commit().unwrap();
                }
                scores.push(tx.score().unwrap());
                work.push(tx.score_evaluations());
                tx.capture().unwrap()
            };
            assert_eq!(g.score(), base);
            let callbacks = trace.borrow().len();
            g.install(saved);
            assert_eq!(trace.borrow().len(), callbacks);
            scores.push(g.score());
            g.install(full);
            failure.set(phase);
            let outcome = {
                let mut tx = g.transaction();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    tx.apply(&source, TupleChange::put(0, (0, 9, 12)))
                }));
                work.push(tx.score_evaluations());
                match result {
                    Err(_) => "panic".to_owned(),
                    Ok(Err(e)) => format!("{e:?}"),
                    Ok(Ok(())) => "ok".to_owned(),
                }
            };
            assert_eq!(g.score(), base);
            failure.set(0);
            g.assert_consistent().unwrap();
            assert!(g.propagation_workspace.deferred_totals.old.is_empty());
            assert!(
                g.propagation_workspace
                    .deferred_totals
                    .seen
                    .iter()
                    .all(|v| !v)
            );
            {
                let mut tx = g.transaction();
                tx.apply(&source, TupleChange::put(0, (0, 9, 12))).unwrap();
                scores.push(tx.score().unwrap());
                work.push(tx.score_evaluations());
            }
            assert_eq!(g.score(), base);
            drop(g);
            (trace.into_inner(), scores, work, outcome)
        })
    }
    for phase in 0..=3 {
        let original = run(false, false, phase);
        assert_eq!(run(true, false, phase), original);
        assert_eq!(run(true, true, phase), original);
        match phase {
            0 => assert_eq!(original.3, "ok"),
            1 => assert_eq!(original.3, "panic"),
            _ => {
                assert_ne!(original.3, "ok");
                assert_ne!(original.3, "panic");
            }
        }
    }
}

#[test]
fn deferred_totals_preserve_final_graph_overflow_after_all_terminal_callbacks() {
    fn run(
        defer: bool,
        wide: bool,
        wrapping: bool,
        cancel: bool,
    ) -> (Vec<&'static str>, bool, SimpleScore, u128) {
        event_queue::with_wide(wide, || {
            let trace = RefCell::new(Vec::new());
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if wrapping {
                b = b.with_arithmetic(crate::ScoringArithmetic::Wrapping);
            }
            let s = b.source::<usize, (usize, usize, usize)>();
            b.score("first", s.port(), |_| {
                trace.borrow_mut().push("first");
                SimpleScore::new(i64::MAX)
            })
            .unwrap();
            b.score("second", s.port(), |_| {
                trace.borrow_mut().push("second");
                SimpleScore::new(if cancel { -i64::MAX } else { 1 })
            })
            .unwrap();
            let mut g = b.build();
            assert!(g.try_enable_consuming_search(s.node_id(), 1));
            if !defer {
                g.score_probe_nodes = None;
            }
            let (ok, score, work) = {
                let mut tx = g.transaction();
                let result = tx.apply(&s, TupleChange::put(0, (0, 0, 0)));
                let ok = result.is_ok();
                if !ok {
                    assert!(matches!(
                        result,
                        Err(GraphError::ScoreOverflow { constraint: None })
                    ));
                }
                (ok, tx.graph.score(), tx.score_evaluations())
            };
            assert_eq!(g.score(), SimpleScore::ZERO);
            assert!(g.propagation_workspace.deferred_totals.old.is_empty());
            drop(g);
            (trace.into_inner(), ok, score, work)
        })
    }
    for wrapping in [false, true] {
        for cancel in [false, true] {
            let original = run(false, false, wrapping, cancel);
            assert_eq!(original.0, vec!["first", "second"]);
            assert_eq!(original.1, wrapping || cancel);
            assert_eq!(run(true, false, wrapping, cancel), original);
            assert_eq!(run(true, true, wrapping, cancel), original);
        }
    }
}
