//! Original scalar arena controls; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn narrow_queue_layout_and_inline_fanout_use_one_arena_cell() {
    eprintln!(
        "event={} queued={} narrow_cell={} wide_cell={}",
        size_of::<Event>(),
        size_of::<QueuedEvent>(),
        size_of::<(Edge, QueuedEvent)>(),
        size_of::<(Edge, Event)>(),
    );
    assert!(size_of::<QueuedEvent>() <= 3 * size_of::<usize>());
    assert!(size_of::<(Edge, QueuedEvent)>() <= 5 * size_of::<usize>());
    assert!(size_of::<(Edge, QueuedEvent)>() < size_of::<(Edge, Event)>());
    let edges = [
        Edge {
            node: 1,
            right: false,
        },
        Edge {
            node: 2,
            right: true,
        },
        Edge {
            node: 3,
            right: false,
        },
    ];
    let mut pending = VecDeque::new();
    let mut arena = Vec::new();
    let row = TupleChange::put(usize::MAX, (0usize, 1usize, usize::MAX));
    enqueue(
        &edges,
        Event::new(row.clone()),
        &mut pending,
        &mut arena,
        false,
    );
    assert_eq!(arena.len(), 1);
    assert_eq!(
        arena[0].downcast_ref::<TupleChange<usize, (usize, usize, usize)>>(),
        Some(&row)
    );
    for (expected, (edge, queued)) in edges.iter().zip(pending) {
        assert_eq!((edge.node, edge.right), (expected.node, expected.right));
        assert!(matches!(queued, QueuedEvent::Inline(0)));
    }
    assert!(!matches!(arena[0], Event::Shared(_)));
}

#[test]
fn shared_payloads_never_enter_arena_and_keep_fifo_release_phase() {
    struct Payload(usize, Rc<RefCell<Vec<usize>>>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    let drops = Rc::new(RefCell::new(Vec::new()));
    let edges = [
        Edge {
            node: 1,
            right: false,
        },
        Edge {
            node: 2,
            right: true,
        },
    ];
    let mut pending = VecDeque::new();
    let mut arena = Vec::new();
    for id in 0..3 {
        enqueue(
            &edges,
            Event::new(Payload(id, drops.clone())),
            &mut pending,
            &mut arena,
            false,
        );
    }
    assert!(arena.is_empty());
    let (_, first) = pending.pop_front().unwrap();
    drop(first);
    assert!(drops.borrow().is_empty());
    let (_, second) = pending.pop_front().unwrap();
    drop(second);
    assert_eq!(*drops.borrow(), [0]);
    drop(pending);
    assert_eq!(*drops.borrow(), [0, 1, 2]);
    // Whole-owner consumers promote BEFORE fanout, so both entries share one Rc.
    enqueue(
        &edges,
        Event::new(TupleChange::put(0usize, (1usize, 2usize, 3usize))),
        &mut VecDeque::new(),
        &mut arena,
        true,
    );
    assert!(arena.is_empty());
}

fn empty_and_capacity(g: &StreamGraph<'_, SimpleScore>) -> (usize, usize, usize) {
    let workspace = &g.propagation_workspace;
    assert!(workspace.pending.is_empty());
    assert!(workspace.arena_pending.is_empty());
    assert!(workspace.arena.is_empty());
    assert!(workspace.changes.is_empty());
    (
        workspace.arena_pending.capacity(),
        workspace.arena.capacity(),
        workspace.changes.capacity(),
    )
}

#[test]
fn scalar_arena_reuses_capacity_and_cleans_after_errors_panics_and_nested_replay() {
    for failure in 0..5 {
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, (usize, usize, usize)>();
        let mapped = b
            .map(source.port(), |row| {
                assert!(!(fail.get() && failure == 1), "mapping panic");
                (row.0, row.1, row.2 + 1)
            })
            .unwrap();
        b.score("mapped", &mapped, |row| {
            assert!(!(fail.get() && failure == 2), "score panic");
            SimpleScore::new(row.2 as i64)
        })
        .unwrap();
        b.score("direct", source.port(), |row| {
            if fail.get() && failure == 3 {
                return SimpleScore::new(0).with_structural(1);
            }
            if fail.get() && failure == 4 {
                return SimpleScore::new(i64::MAX);
            }
            SimpleScore::new(row.2 as i64)
        })
        .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 3usize)))
            .unwrap();
        tx.commit().unwrap();
        let capacity = empty_and_capacity(&g);
        assert!(capacity.0 > 0 && capacity.1 > 0);
        let pointer = g.propagation_workspace.arena.as_ptr();
        for _ in 0..2 {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 5usize)))
                .unwrap();
            assert_eq!(empty_and_capacity(tx.graph), capacity);
            assert_eq!(tx.graph.propagation_workspace.arena.as_ptr(), pointer);
            tx.assert_consistent().unwrap();
            assert_eq!(empty_and_capacity(tx.graph), capacity);
            drop(tx);
            assert_eq!(g.score(), SimpleScore::new(7));
        }
        fail.set(failure > 0);
        let mut tx = g.transaction();
        let result = catch_unwind(AssertUnwindSafe(|| {
            tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 5usize)))
        }));
        match failure {
            1 | 2 => assert!(result.is_err()),
            3 => assert!(matches!(
                result.unwrap(),
                Err(GraphError::StructuralContribution(_))
            )),
            4 => assert!(matches!(
                result.unwrap(),
                Err(GraphError::ScoreOverflow { .. })
            )),
            _ => assert!(result.unwrap().is_ok()),
        }
        empty_and_capacity(tx.graph);
        drop(tx);
        assert_eq!(g.score(), SimpleScore::new(7));
        assert_eq!(g.source_rows(&source).unwrap()[&0], (0, 0, 3));
        fail.set(false);
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 5usize)))
            .unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(11));
        empty_and_capacity(&g);
        g.assert_consistent().unwrap();
    }
}
