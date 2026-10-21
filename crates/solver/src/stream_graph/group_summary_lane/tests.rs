//! Original private summary-lane controls; no upstream-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
fn summary(value: i64) -> IntSummary {
    IntSummary {
        count: 1,
        distinct_count: 1,
        sum: value,
        min: Some(value),
        max: Some(value),
    }
}
#[test]
fn group_summary_lane_layout_full_rows_max_coordinates_and_stable_chunks() {
    #[cfg(target_pointer_width = "64")]
    {
        eprintln!(
            "event={} queue={} cell={} pair={} quad={} view={} carrier={}",
            size_of::<Event>(),
            size_of::<QueuedEvent>(),
            size_of::<(Edge, QueuedEvent)>(),
            size_of::<PairChange>(),
            size_of::<QuadChange>(),
            size_of::<SummaryRef<'_>>(),
            size_of::<GroupOutput<'_>>()
        );
        assert_eq!(size_of::<Event>(), 72);
        assert_eq!(size_of::<QueuedEvent>(), 24);
        assert_eq!(size_of::<(Edge, QueuedEvent)>(), 40);
        assert_eq!(size_of::<PairChange>(), 88);
        assert_eq!(size_of::<QuadChange>(), 120);
    }
    let mut lane = Lanes::default();
    let key = (usize::MAX, usize::MAX - 1);
    let mut malformed = summary(i64::MIN);
    malformed.count = 0;
    malformed.distinct_count = usize::MAX;
    malformed.min = None;
    malformed.max = Some(i64::MAX);
    let row = TupleChange::put(key, (key, malformed));
    let index = lane
        .pairs
        .push(row.clone(), || TupleChange::retract((0, 0)));
    let pointer = lane.pairs.get(index) as *const PairChange as usize;
    for _ in 0..CHUNK * 4 {
        lane.pairs
            .push(TupleChange::retract((usize::MAX, usize::MAX)), || {
                TupleChange::retract((0, 0))
            });
        lane.quads.push(
            TupleChange::put(
                (usize::MAX, 1, 2, 3),
                ((usize::MAX, 1, 2, 3), summary(i64::MAX)),
            ),
            || TupleChange::retract((0, 0, 0, 0)),
        );
    }
    assert_eq!(lane.pairs.get(index) as *const PairChange as usize, pointer);
    assert_eq!(lane.pair(index).typed::<Pair, (Pair, IntSummary)>(), &row);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let view = lane.pair(index);
            let _ = view.typed::<Quad, (Quad, IntSummary)>();
        }))
        .is_err()
    );
    let chunks = lane.pairs.chunks.len();
    lane.clear();
    assert!(catch_unwind(AssertUnwindSafe(|| lane.pair(index))).is_err());
    lane.pairs
        .push(TupleChange::retract(key), || TupleChange::retract((0, 0)));
    assert_eq!(lane.pairs.chunks.len(), chunks);
    assert_eq!(lane.pairs.get(0) as *const PairChange as usize, pointer);
    assert!(!lane.pair(0).is_put());
}

#[test]
fn group_summary_lane_stages_both_outputs_before_fifo_fanout_and_cleans_abandoned_carrier() {
    let old_key = (usize::MAX, 7usize);
    let new_key = (5usize, usize::MAX);
    let mut raw = [
        Some(TupleChange::retract(old_key)),
        Some(TupleChange::put(new_key, summary(i64::MIN))),
    ];
    let edges = [
        Edge {
            node: 3,
            right: false,
        },
        Edge {
            node: 4,
            right: false,
        },
    ];
    let mut pending = VecDeque::new();
    let mut lane = Lanes::default();
    let staged = GroupOutput::new(&mut raw);
    assert_eq!(staged.kind(), Kind::Pair);
    staged.publish(&edges, &mut pending, &mut lane);
    assert!(raw.iter().all(Option::is_none));
    for (node, index) in [(3, 0), (4, 0), (3, 1), (4, 1)] {
        let (edge, queued) = pending.pop_front().unwrap();
        assert_eq!(edge.node, node);
        assert!(matches!(queued,QueuedEvent::PairSummary(i) if i==index));
    }
    assert!(pending.is_empty());
    assert_eq!(lane.pairs.get(0), &TupleChange::retract(old_key));
    assert_eq!(
        lane.pairs.get(1),
        &TupleChange::put(new_key, (new_key, summary(i64::MIN)))
    );
    let mut raw = [Some(TupleChange::put(old_key, summary(3))), None];
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _staged = GroupOutput::new(&mut raw);
            panic!("after staging");
        }))
        .is_err()
    );
    assert!(raw.iter().all(Option::is_none));
}

type TestGraph<'a> = (
    StreamGraph<'a, SimpleScore>,
    StreamSource<usize, (usize, i64)>,
    usize,
    usize,
);
type Run = (
    Vec<(&'static str, usize, i64)>,
    Vec<SimpleScore>,
    Vec<u128>,
    String,
);
type Trace = RefCell<Vec<(&'static str, usize, i64)>>;
struct MappingWatch<'a>(&'a Trace);
impl Drop for MappingWatch<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().push(("mapping drop", usize::MAX, 0));
    }
}

fn graph<'a>(
    trace: &'a Trace,
    phase: &'a Cell<usize>,
    pair_pointer: &'a Cell<usize>,
    quad_pointer: &'a Cell<usize>,
    unsupported: bool,
) -> TestGraph<'a> {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, i64)>();
    let watch = MappingWatch(trace);
    let pair = b
        .group_collect(source.port(), move |&(key, value)| {
            let _keep = &watch;
            watch.0.borrow_mut().push(("pair mapping", key, value));
            assert_ne!(phase.get(), 1, "group mapping panic");
            ((key, key % 3), value)
        })
        .unwrap();
    let quad = b
        .group_collect(source.port(), |&(key, value)| {
            trace.borrow_mut().push(("quad mapping", key, value));
            ((key, key % 3, usize::MAX, key % 5), value)
        })
        .unwrap();
    b.score("pair first", &pair, |row| {
        pair_pointer.set(row as *const (Pair, IntSummary) as usize);
        trace.borrow_mut().push(("pair first", row.0.0, row.1.sum));
        assert_ne!(phase.get(), 2, "pair score panic");
        match phase.get() {
            4 => SimpleScore::new(0).with_structural(1),
            5 => SimpleScore::new(i64::MAX),
            _ => SimpleScore::new(row.1.sum),
        }
    })
    .unwrap();
    b.score("pair second", &pair, |row| {
        assert_eq!(
            pair_pointer.get(),
            row as *const (Pair, IntSummary) as usize,
            "same producer row for fanout"
        );
        trace.borrow_mut().push(("pair second", row.0.0, row.1.sum));
        SimpleScore::new(1)
    })
    .unwrap();
    b.score("quad first", &quad, |row| {
        quad_pointer.set(row as *const (Quad, IntSummary) as usize);
        trace.borrow_mut().push(("quad first", row.0.0, row.1.sum));
        assert_ne!(phase.get(), 3, "quad score panic");
        SimpleScore::new(row.1.sum)
    })
    .unwrap();
    b.score("quad second", &quad, |row| {
        assert_eq!(
            quad_pointer.get(),
            row as *const (Quad, IntSummary) as usize,
            "same producer row for fanout"
        );
        trace.borrow_mut().push(("quad second", row.0.0, row.1.sum));
        SimpleScore::new(1)
    })
    .unwrap();
    if unsupported {
        let forwarded = b.map(&pair, |row| *row).unwrap();
        b.score("mapped", &forwarded, |row| SimpleScore::new(row.1.sum))
            .unwrap();
    }
    (b.build(), source, pair.node, quad.node)
}
#[test]
fn group_summary_lane_actual_graph_callbacks_counts_captures_nested_rollback_errors_and_unwind_match_original()
 {
    fn run(enabled: bool, wide: bool, failure: usize) -> Run {
        event_queue::with_wide(wide, || {
            let trace = Trace::default();
            let phase = Cell::new(0);
            let pair_pointer = Cell::new(0);
            let quad_pointer = Cell::new(0);
            let (mut graph, source, pair, quad) =
                graph(&trace, &phase, &pair_pointer, &quad_pointer, false);
            assert_eq!(graph.group_summary_candidates[pair], Some(Kind::Pair));
            assert_eq!(graph.group_summary_candidates[quad], Some(Kind::Quad));
            assert!(graph.group_summary_lanes.iter().all(Option::is_none));
            assert!(graph.try_enable_consuming_search(source.node_id(), 6));
            assert_eq!(graph.group_summary_lanes[pair], Some(Kind::Pair));
            assert_eq!(graph.group_summary_lanes[quad], Some(Kind::Quad));
            if !enabled {
                graph.group_summary_lanes.fill(None);
            }
            {
                let mut tx = graph.transaction();
                for id in 0usize..6 {
                    tx.apply(&source, TupleChange::put(id, (id % 3, id as i64 + 1)))
                        .unwrap();
                }
                tx.commit().unwrap();
            }
            let base = graph.score();
            let full = graph.capture_full();
            let mut scores = vec![base];
            let mut work = Vec::new();
            trace.borrow_mut().clear();
            let captured = {
                let mut outer = graph.transaction();
                outer
                    .apply(&source, TupleChange::put(0usize, (usize::MAX, 7i64)))
                    .unwrap();
                outer
                    .apply(&source, TupleChange::put(0usize, (usize::MAX, 7i64)))
                    .unwrap();
                {
                    let mut child = outer.graph.transaction();
                    child.apply(&source, TupleChange::retract(1usize)).unwrap();
                    child
                        .apply(&source, TupleChange::put(1usize, (1usize, 0i64)))
                        .unwrap();
                    child.commit().unwrap();
                }
                scores.push(outer.score().unwrap());
                work.push(outer.score_evaluations());
                outer.capture().unwrap()
            };
            assert_eq!(graph.score(), base);
            let callbacks = trace.borrow().len();
            graph.install(captured);
            assert_eq!(trace.borrow().len(), callbacks);
            scores.push(graph.score());
            graph.install(full);
            assert_eq!(graph.score(), base);
            phase.set(failure);
            let outcome = {
                let mut tx = graph.transaction();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    tx.apply(&source, TupleChange::put(0usize, (usize::MAX, 9i64)))
                }));
                work.push(tx.score_evaluations());
                match result {
                    Err(_) => "panic".to_owned(),
                    Ok(Err(e)) => format!("{e:?}"),
                    Ok(Ok(())) => "ok".to_owned(),
                }
            };
            assert_eq!(graph.score(), base);
            phase.set(0);
            {
                let mut retry = graph.transaction();
                retry
                    .apply(&source, TupleChange::put(0usize, (usize::MAX, 11i64)))
                    .unwrap();
                scores.push(retry.score().unwrap());
                work.push(retry.score_evaluations());
            }
            assert_eq!(graph.score(), base);
            graph.assert_consistent().unwrap();
            scores.push(graph.score());
            assert_eq!(graph.propagation_workspace.summary_arena.pairs.len, 0);
            assert_eq!(graph.propagation_workspace.summary_arena.quads.len, 0);
            drop(graph);
            (trace.into_inner(), scores, work, outcome)
        })
    }
    for failure in 0..=5 {
        let original = run(false, false, failure);
        assert_eq!(run(true, false, failure), original);
        assert_eq!(run(true, true, failure), original);
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Custom(Pair);
#[test]
fn group_summary_lane_whole_cohort_custom_and_retained_public_owner_fallback() {
    let custom = Custom((usize::MAX, usize::MAX));
    assert_eq!(custom.0, (usize::MAX, usize::MAX));
    assert!(kind::<Custom>().is_none());
    assert!(!std::mem::needs_drop::<Custom>());
    assert!(kind::<String>().is_none());
    assert!(!accepts::<Pair, (Quad, IntSummary)>(Kind::Pair));
    let trace = Trace::default();
    let phase = Cell::new(0);
    let pair_pointer = Cell::new(0);
    let quad_pointer = Cell::new(0);
    let (mut unsupported, source, pair, quad) =
        graph(&trace, &phase, &pair_pointer, &quad_pointer, true);
    assert!(unsupported.group_summary_candidates[pair].is_none());
    assert!(unsupported.try_enable_consuming_search(source.node_id(), 1));
    assert!(unsupported.group_summary_lanes[pair].is_none());
    assert_eq!(unsupported.group_summary_lanes[quad], Some(Kind::Quad));
    let mut tx = unsupported.transaction();
    tx.apply(&source, TupleChange::put(0usize, (0usize, 1i64)))
        .unwrap();
    tx.commit().unwrap();
    drop(unsupported);
    let (mut retained, source, pair, _) =
        graph(&trace, &phase, &pair_pointer, &quad_pointer, false);
    assert!(retained.group_summary_candidates[pair].is_some());
    assert!(!retained.consuming_search);
    let id = retained.constraints["pair first"];
    {
        let mut tx = retained.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 1i64)))
            .unwrap();
        tx.commit().unwrap();
    }
    type Matches = SnapshotMap<Pair, ((Pair, IntSummary), SimpleScore)>;
    let held = retained.nodes[id]
        .matches()
        .unwrap()
        .downcast_ref::<Matches>()
        .unwrap()
        .clone();
    let owners = held.shared_entries();
    let weak = Arc::downgrade(&owners[0].1);
    {
        let mut tx = retained.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 1i64)))
            .unwrap();
        let now = tx.graph.nodes[id]
            .matches()
            .unwrap()
            .downcast_ref::<Matches>()
            .unwrap();
        assert!(!Arc::ptr_eq(&owners[0].1, &now.shared_entries()[0].1));
    }
    let now = retained.nodes[id]
        .matches()
        .unwrap()
        .downcast_ref::<Matches>()
        .unwrap();
    assert!(Arc::ptr_eq(&owners[0].1, &now.shared_entries()[0].1));
    assert!(weak.upgrade().is_some());
}

#[test]
fn direct_group_publication_matches_staged_reference_full_fields_sparse_outputs_and_fifo() {
    fn compare<K: Clone + 'static>(rows: [Option<TupleChange<K, IntSummary>>; 2], kind: Kind) {
        for fanout in [0usize, 1, 3] {
            let edges: Vec<_> = (0..fanout)
                .map(|node| Edge {
                    node,
                    right: node % 2 == 1,
                })
                .collect();
            let mut old_rows = rows.clone();
            let mut original = Lanes::default();
            let mut direct = Lanes::default();
            let mut old_queue = VecDeque::new();
            let mut new_queue = VecDeque::new();
            GroupOutput::new(&mut old_rows).publish(&edges, &mut old_queue, &mut original);
            for row in rows.iter().flatten().cloned() {
                direct.publish(row, &edges, &mut new_queue);
            }
            assert_eq!(old_queue.len(), new_queue.len());
            while let Some((edge, old)) = old_queue.pop_front() {
                let (got, new) = new_queue.pop_front().unwrap();
                assert_eq!((edge.node, edge.right), (got.node, got.right));
                match (kind, old, new) {
                    (Kind::Pair, QueuedEvent::PairSummary(old), QueuedEvent::PairSummary(new)) => {
                        assert_eq!(
                            original.pair(old).typed::<Pair, (Pair, IntSummary)>(),
                            direct.pair(new).typed::<Pair, (Pair, IntSummary)>()
                        )
                    }
                    (Kind::Quad, QueuedEvent::QuadSummary(old), QueuedEvent::QuadSummary(new)) => {
                        assert_eq!(
                            original.quad(old).typed::<Quad, (Quad, IntSummary)>(),
                            direct.quad(new).typed::<Quad, (Quad, IntSummary)>()
                        )
                    }
                    _ => panic!("same original queue tags and full rows"),
                }
            }
            assert!(new_queue.is_empty());
            assert_eq!(original.pairs.len, direct.pairs.len);
            assert_eq!(original.quads.len, direct.quads.len);
        }
    }
    let mut malformed = summary(i64::MIN);
    malformed.count = 0;
    malformed.distinct_count = usize::MAX;
    malformed.min = None;
    malformed.max = Some(i64::MAX);
    compare::<Pair>([None, None], Kind::Pair);
    compare::<Pair>(
        [Some(TupleChange::put((usize::MAX, 0), malformed)), None],
        Kind::Pair,
    );
    compare::<Pair>(
        [None, Some(TupleChange::retract((1, usize::MAX)))],
        Kind::Pair,
    );
    compare::<Pair>(
        [
            Some(TupleChange::retract((usize::MAX, 1))),
            Some(TupleChange::put((2, usize::MAX), malformed)),
        ],
        Kind::Pair,
    );
    compare::<Quad>([None, None], Kind::Quad);
    compare::<Quad>(
        [
            Some(TupleChange::put((usize::MAX, 0, 1, 2), malformed)),
            None,
        ],
        Kind::Quad,
    );
    compare::<Quad>(
        [
            Some(TupleChange::retract((usize::MAX, 1, 2, 3))),
            Some(TupleChange::put((2, usize::MAX, 4, 5), malformed)),
        ],
        Kind::Quad,
    );
}
