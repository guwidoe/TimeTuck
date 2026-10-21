//! Original consumed numeric adapter controls; no upstream method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Trace = Vec<(&'static str, usize, usize)>;
struct Rows<'a> {
    n: usize,
    p: usize,
    trace: &'a RefCell<Trace>,
    fail: &'a Cell<usize>,
}
impl Iterator for Rows<'_> {
    type Item = (usize, usize, usize, usize);
    fn next(&mut self) -> Option<Self::Item> {
        self.trace.borrow_mut().push(("next", self.n, self.p));
        assert!(!(self.fail.get() == 2 && self.p == 1), "iterator panic");
        if self.p == self.n {
            return None;
        }
        let p = self.p;
        self.p += 1;
        Some((usize::MAX, self.n, p, usize::MAX - 1))
    }
}
impl Drop for Rows<'_> {
    fn drop(&mut self) {
        self.trace.borrow_mut().push(("iter-drop", self.n, self.p));
    }
}
#[test]
fn direct_flat_map_consumed_graph_callback_iterator_order_captures_panic_error_retry_match_original()
 {
    fn run(mode: usize, phase: usize) -> (Trace, Vec<SimpleScore>, Vec<u128>, String) {
        event_queue::with_wide(mode == 3, || {
            event::with_forced_shared(mode == 4, || {
                let trace = RefCell::new(Trace::new());
                let fail = Cell::new(0usize);
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = b.source::<usize, usize>();
                let expanded = b
                    .flat_map(source.port(), |n: &usize| {
                        trace.borrow_mut().push(("map", *n, 0));
                        assert_ne!(fail.get(), 1, "mapping panic");
                        Rows {
                            n: *n,
                            p: 0,
                            trace: &trace,
                            fail: &fail,
                        }
                    })
                    .unwrap();
                b.score("rows", &expanded, |row| {
                    trace.borrow_mut().push(("score", row.1, row.2));
                    assert_eq!(row.0, usize::MAX);
                    assert_eq!(row.3, usize::MAX - 1);
                    assert_ne!(fail.get(), 3, "score panic");
                    match fail.get() {
                        4 => SimpleScore::new(0).with_structural(1),
                        5 => SimpleScore::new(i64::MAX),
                        _ => SimpleScore::new((row.2 + 1) as i64),
                    }
                })
                .unwrap();
                let mut g = b.build();
                if mode != 0 {
                    assert!(g.try_enable_consuming_search(source.node_id(), 3));
                }
                if mode == 2 {
                    g.nodes[expanded.node].disable_direct_flatmap();
                }
                assert_eq!(
                    g.nodes[expanded.node].direct_flatmap_enabled(),
                    matches!(mode, 1 | 3 | 4)
                );
                let mut scores = Vec::new();
                let mut work = Vec::new();
                for (id, n) in [
                    (0, 0),
                    (0, 0),
                    (0, 3),
                    (0, 3),
                    (0, 1),
                    (1, 2),
                    (0, 0),
                    (0, 0),
                ] {
                    let mut tx = g.transaction();
                    tx.apply(&source, TupleChange::put(id, n)).unwrap();
                    work.push(tx.score_evaluations());
                    tx.commit().unwrap();
                    scores.push(g.score());
                }
                let base = g.score();
                let full = g.capture_full();
                let saved = {
                    let mut outer = g.transaction();
                    outer.apply(&source, TupleChange::put(0, 4)).unwrap();
                    {
                        let mut child = outer.graph.transaction();
                        child.apply(&source, TupleChange::retract(1)).unwrap();
                        child.apply(&source, TupleChange::put(1, 0)).unwrap();
                        child.commit().unwrap();
                    }
                    scores.push(outer.score().unwrap());
                    work.push(outer.score_evaluations());
                    outer.capture().unwrap()
                };
                assert_eq!(g.score(), base);
                let before = trace.borrow().len();
                g.install(saved);
                assert_eq!(trace.borrow().len(), before);
                scores.push(g.score());
                g.install(full);
                fail.set(phase);
                let outcome = {
                    let mut tx = g.transaction();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        tx.apply(&source, TupleChange::put(0, 4))
                    }));
                    work.push(tx.score_evaluations());
                    match result {
                        Err(_) => "panic".to_owned(),
                        Ok(Err(e)) => format!("{e:?}"),
                        Ok(Ok(())) => "ok".to_owned(),
                    }
                };
                assert_eq!(g.score(), base);
                fail.set(0);
                g.assert_consistent().unwrap();
                {
                    let mut tx = g.transaction();
                    tx.apply(&source, TupleChange::put(0, 4)).unwrap();
                    scores.push(tx.score().unwrap());
                }
                assert_eq!(g.score(), base);
                drop(g);
                (trace.into_inner(), scores, work, outcome)
            })
        })
    }
    for phase in 0..=5 {
        let original = run(0, phase);
        for mode in 1..=4 {
            assert_eq!(run(mode, phase), original, "mode={mode} phase={phase}");
        }
        match phase {
            0 => assert_eq!(original.3, "ok"),
            1..=3 => assert_eq!(original.3, "panic"),
            _ => assert!(original.3.contains("Overflow") || original.3.contains("Structural")),
        }
    }
}
