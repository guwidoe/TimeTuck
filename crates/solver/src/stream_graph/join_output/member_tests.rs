use super::*;
type Id = Pair;
type Change = MemberChange;
type Output<'a> = JoinOutput<'a>;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn shared_member_carrier_keeps_complete_rows_fifo_orientation_and_owner_lifetimes() {
    assert_eq!(
        std::mem::size_of::<JoinOutput<'_>>(),
        2 * std::mem::size_of::<usize>()
    );
    assert!(std::mem::size_of::<Output<'_>>() <= 3 * std::mem::size_of::<usize>());
    assert!(supports_member::<Id, Id, Quad, Quad>());
    assert!(!supports_member::<usize, usize, Quad, Quad>());
    for right in [false, true] {
        for receivers in [0, 1, 3] {
            let original: Vec<Change> = vec![
                TupleChange::put(
                    ((usize::MAX, 7), (11, usize::MAX)),
                    ((0, 1, 2, usize::MAX), (usize::MAX, 4, 5, 6)),
                ),
                TupleChange::retract(((1, 2), (3, 4))),
            ];
            let mut rows = original.clone();
            let capacity = rows.capacity();
            let edges: Vec<_> = (0..receivers)
                .map(|node| Edge {
                    node,
                    right: node % 2 != 0,
                })
                .collect();
            let mut pending = VecDeque::new();
            Output::new_member(&mut rows, right).publish(
                &edges,
                &mut pending,
                &mut Vec::new(),
                false,
            );
            assert!(rows.is_empty());
            assert_eq!(rows.capacity(), capacity);
            let expected: Vec<_> = original
                .into_iter()
                .map(|row| {
                    if right {
                        TupleChange {
                            id: (row.id.1, row.id.0),
                            value: row.value.map(|(r, l)| (l, r)),
                        }
                    } else {
                        row
                    }
                })
                .collect();
            for expected in expected {
                let mut previous: Option<Rc<dyn Any>> = None;
                for edge in &edges {
                    let (actual, queued) = pending.pop_front().unwrap();
                    assert_eq!((actual.node, actual.right), (edge.node, edge.right));
                    let event_queue::QueuedEvent::Shared(owner) = queued else {
                        panic!("shared member owner")
                    };
                    assert_eq!(owner.downcast_ref::<Change>().unwrap(), &expected);
                    if let Some(previous) = previous.as_ref() {
                        assert!(Rc::ptr_eq(previous, &owner));
                    }
                    previous = Some(owner);
                }
                if let Some(owner) = previous {
                    let weak = Rc::downgrade(&owner);
                    drop(owner);
                    assert!(weak.upgrade().is_none());
                }
            }
            assert!(pending.is_empty());
        }
    }
}

type Trace = RefCell<Vec<(&'static str, usize, usize)>>;
type Run = (
    Vec<(&'static str, usize, usize)>,
    Vec<SimpleScore>,
    Vec<u128>,
    String,
);
fn graph<'a>(
    trace: &'a Trace,
    failure: &'a Cell<usize>,
) -> (StreamGraph<'a, SimpleScore>, StreamSource<Id, Quad>, usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<Id, Quad>();
    let joined = b
        .join_equal(
            source.port(),
            source.port(),
            |row| {
                trace.borrow_mut().push(("left", row.2, row.3));
                assert_ne!(failure.get(), 1, "projection panic");
                (row.0, row.1)
            },
            |row| {
                trace.borrow_mut().push(("right", row.2, row.3));
                (row.0, row.1)
            },
        )
        .unwrap();
    let mapped = b
        .map(&joined, |pair| {
            trace.borrow_mut().push(("map", pair.0.2, pair.1.2));
            assert_ne!(failure.get(), 2, "map panic");
            *pair
        })
        .unwrap();
    b.score("mapped", &mapped, |pair| {
        trace.borrow_mut().push(("score", pair.0.2, pair.1.2));
        assert_ne!(failure.get(), 3, "score panic");
        match failure.get() {
            4 => SimpleScore::new(0).with_structural(1),
            5 => SimpleScore::new(i64::MAX),
            _ => SimpleScore::new((pair.0.3 + pair.1.3) as i64),
        }
    })
    .unwrap();
    b.score("direct", &joined, |pair| {
        trace.borrow_mut().push(("direct", pair.0.2, pair.1.2));
        SimpleScore::new(1)
    })
    .unwrap();
    (b.build(), source, joined.node)
}
#[test]
fn shared_member_carrier_consumed_graph_preserves_callbacks_captures_failures_and_retry() {
    fn run(carrier: bool, wide: bool, phase: usize) -> Run {
        event_queue::with_wide(wide, || {
            let trace = Trace::default();
            let failure = Cell::new(0);
            let (mut graph, source, joined) = graph(&trace, &failure);
            assert!(graph.try_enable_consuming_search(source.node_id(), 24));
            assert!(graph.join_output_nodes[joined]);
            graph.join_output_nodes[joined] = carrier;
            {
                let mut tx = graph.transaction();
                for id in 0..24 {
                    tx.apply(&source, TupleChange::put((0, id), (0, 0, id, id % 3)))
                        .unwrap();
                }
                tx.commit().unwrap();
            }
            let full = graph.capture_full();
            let base = graph.score();
            let mut scores = vec![base];
            let mut work = Vec::new();
            trace.borrow_mut().clear();
            let captured = {
                let mut outer = graph.transaction();
                outer
                    .apply(&source, TupleChange::put((0, 0), (0, 0, 100, 7)))
                    .unwrap();
                {
                    let mut child = outer.graph.transaction();
                    child.apply(&source, TupleChange::retract((0, 1))).unwrap();
                    child
                        .apply(&source, TupleChange::put((0, 1), (0, 0, 101, 8)))
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
            failure.set(phase);
            let result = {
                let mut tx = graph.transaction();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    tx.apply(&source, TupleChange::put((0, 0), (0, 0, 200, 9)))
                }));
                work.push(tx.score_evaluations());
                match result {
                    Err(_) => "panic".into(),
                    Ok(Err(e)) => format!("{e:?}"),
                    Ok(Ok(())) => "ok".into(),
                }
            };
            assert_eq!(graph.score(), base);
            failure.set(0);
            graph.assert_consistent().unwrap();
            scores.push(graph.score());
            assert!(
                graph.propagation_workspace.arena.is_empty()
                    && graph.propagation_workspace.arena_pending.is_empty()
            );
            drop(graph);
            (trace.into_inner(), scores, work, result)
        })
    }
    for phase in 0..=5 {
        let old = run(false, false, phase);
        assert_eq!(run(true, false, phase), old);
        assert_eq!(run(true, true, phase), old);
    }
}
