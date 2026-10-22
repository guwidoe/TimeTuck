//! Original deferred-consumer controls; no upstream-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Trace = Vec<(&'static str, Vec<usize>, IntSummary)>;
type Run = (Trace, Vec<SimpleScore>, Vec<u128>, String);
fn run(enabled: bool, failed: usize) -> Run {
    let trace = RefCell::new(Trace::new());
    let phase = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i64)>();
    let pair = b
        .group_collect(source.port(), |&(key, value)| {
            assert_ne!(phase.get(), 5, "mapping panic");
            ((key, key % 3), value)
        })
        .unwrap();
    let quad = b
        .group_collect(source.port(), |&(key, value)| {
            ((key, key % 3, usize::MAX, key % 5), value)
        })
        .unwrap();
    let other = b
        .group_collect(source.port(), |&(key, value)| {
            ((key % 2, usize::MAX), value.wrapping_mul(3))
        })
        .unwrap();
    b.score("pair", &pair, |r| {
        trace.borrow_mut().push(("pair", vec![r.0.0, r.0.1], r.1));
        assert_ne!(phase.get(), 1, "pair callback panic");
        match phase.get() {
            3 => SimpleScore::new(0).with_structural(1),
            4 => SimpleScore::new(i64::MAX),
            _ => SimpleScore::new(r.1.sum),
        }
    })
    .unwrap();
    b.score("quad", &quad, |r| {
        trace
            .borrow_mut()
            .push(("quad", vec![r.0.0, r.0.1, r.0.2, r.0.3], r.1));
        assert_ne!(phase.get(), 2, "quad callback panic");
        SimpleScore::new(r.1.sum)
    })
    .unwrap();
    b.score("other", &other, |r| {
        trace.borrow_mut().push(("other", vec![r.0.0, r.0.1], r.1));
        SimpleScore::new(r.1.sum)
    })
    .unwrap();
    let mut g = b.build();
    assert!(g.try_enable_consuming_search(source.node_id(), 6));
    assert!(g.propagation_workspace.summary_arena.seed_enabled::<Pair>());
    assert!(g.propagation_workspace.summary_arena.seed_enabled::<Quad>());
    if !enabled {
        g.group_summary_lanes.fill(None);
    }
    {
        let mut tx = g.transaction();
        for id in 0usize..6 {
            tx.apply(&source, TupleChange::put(id, (id % 3, id as i64 + 1)))
                .unwrap();
        }
        tx.commit().unwrap();
    }
    let base = g.score();
    let full = g.capture_full();
    let mut scores = vec![base];
    let mut work = Vec::new();
    trace.borrow_mut().clear();
    let captured = {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, (usize::MAX, 7i64)))
            .unwrap();
        {
            let mut child = tx.graph.transaction();
            child.apply(&source, TupleChange::put(1, (0, -7))).unwrap();
            child.apply(&source, TupleChange::retract(2)).unwrap();
            child.commit().unwrap();
        }
        scores.push(tx.score().unwrap());
        work.push(tx.score_evaluations());
        tx.capture().unwrap()
    };
    assert_eq!(g.score(), base);
    let callbacks = trace.borrow().len();
    g.install(captured);
    assert_eq!(trace.borrow().len(), callbacks);
    scores.push(g.score());
    g.install(full);
    assert_eq!(g.score(), base);
    phase.set(failed);
    let outcome = {
        let mut tx = g.transaction();
        let result = catch_unwind(AssertUnwindSafe(|| {
            tx.apply(&source, TupleChange::put(0, (1, 9)))
        }));
        work.push(tx.score_evaluations());
        match result {
            Err(_) => "panic".into(),
            Ok(Err(e)) => format!("{e:?}"),
            Ok(Ok(())) => "ok".into(),
        }
    };
    assert_eq!(g.score(), base);
    phase.set(0);
    g.assert_consistent().unwrap();
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, (1, 9))).unwrap();
        scores.push(tx.score().unwrap());
        work.push(tx.score_evaluations());
    }
    assert_eq!(g.score(), base);
    drop(g);
    (trace.into_inner(), scores, work, outcome)
}
#[test]
fn deferred_summary_consumer_preserves_full_callback_fields_work_failures_and_recovery() {
    for phase in 0..=5 {
        assert_eq!(run(true, phase), run(false, phase));
    }
}
#[test]
fn seed_lane_round_trips_all_extrema_options_and_reuses_complete_snapshots() {
    let mut lane = Lanes::default();
    lane.enable_seeds(true, true);
    let edges = [Edge {
        node: 0,
        right: false,
    }];
    let mut pending = VecDeque::new();
    let values = [None, Some(i64::MIN), Some(0), Some(i64::MAX)];
    for min in values {
        for max in values {
            let summary = IntSummary {
                count: usize::MAX,
                distinct_count: 7,
                sum: i64::MIN,
                min,
                max,
            };
            lane.publish(
                TupleChange::put((usize::MAX, 7usize), summary),
                &edges,
                &mut pending,
            );
            let (_, QueuedEvent::PairSummary(index)) = pending.pop_front().unwrap() else {
                panic!("pair seed")
            };
            let SummaryRef::PairSeed(row) = lane.pair(index) else {
                panic!("configured seed")
            };
            let (key, seed) = row.value.as_ref().unwrap();
            assert_eq!(*key, (usize::MAX, 7usize));
            assert_eq!(seed.materialize(), summary);
        }
    }
    let summary = IntSummary {
        count: 1,
        distinct_count: 1,
        sum: -7,
        min: Some(-7),
        max: Some(-7),
    };
    lane.publish(
        TupleChange::put((1usize, 2usize, 3usize, usize::MAX), summary),
        &edges,
        &mut pending,
    );
    let (_, QueuedEvent::QuadSummary(index)) = pending.pop_front().unwrap() else {
        panic!("quad seed")
    };
    let SummaryRef::QuadSeed(row) = lane.quad(index) else {
        panic!("configured seed")
    };
    assert_eq!(row.value.as_ref().unwrap().1.materialize(), summary);
    lane.clear();
    assert_eq!(lane.seeds.as_ref().unwrap().pairs.as_ref().unwrap().len, 0);
    lane.publish(
        TupleChange::retract((usize::MAX, 7usize)),
        &edges,
        &mut pending,
    );
    let (_, QueuedEvent::PairSummary(index)) = pending.pop_front().unwrap() else {
        panic!("pair seed")
    };
    assert!(!lane.pair(index).is_put());
}
#[test]
fn original_fanout_metadata_keeps_shared_rows_after_temporary_edge_pruning() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i64)>();
    let group = b
        .group_collect(source.port(), |&(k, v)| ((k, k), v))
        .unwrap();
    let seen = Cell::new(0usize);
    b.score("first", &group, |r| {
        seen.set(r as *const (Pair, IntSummary) as usize);
        SimpleScore::new(r.1.sum)
    })
    .unwrap();
    b.score("second", &group, |r| {
        assert_eq!(seen.get(), r as *const (Pair, IntSummary) as usize);
        SimpleScore::new(r.1.sum)
    })
    .unwrap();
    let mut g = b.build();
    let removed = g.edges[group.node].pop().unwrap();
    assert!(g.try_enable_consuming_search(source.node_id(), 1));
    assert!(!g.propagation_workspace.summary_arena.seed_enabled::<Pair>());
    g.edges[group.node].push(removed);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, (0, 7))).unwrap();
    assert_eq!(tx.score().unwrap(), SimpleScore::new(14));
}
