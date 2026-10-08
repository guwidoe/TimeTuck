// Adapted from Timefold Community UnindexedJoinBiNodeTest. Apache-2.0.
// Copyright Timefold BV and contributors. See compat/provenance.json.
use std::cell::Cell;
use timetuck::*;
fn update(left: bool) {
    for timing in [JoinEvaluation::Eager, JoinEvaluation::Deferred] {
        let mut n = BatchedJoinNode::new(timing, |_: &&str| (), |_: &&str| (), |_, _| true);
        n.enqueue_left(TupleChange::put(0, "L1"));
        n.enqueue_right(TupleChange::put(0, "R1"));
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Insert {
                id: (0, 0),
                value: ("L1", "R1")
            }]
        );
        if left {
            n.enqueue_left(TupleChange::put(0, "L2"));
        } else {
            n.enqueue_right(TupleChange::put(0, "R2"));
        }
        assert_eq!(
            n.propagate(),
            vec![TupleLifecycleEvent::Update {
                id: (0, 0),
                value: if left { ("L2", "R1") } else { ("L1", "R2") }
            }]
        );
    }
}
#[test]
fn update_left_refreshes_left_fact() {
    update(true);
}
#[test]
fn update_right_refreshes_right_fact() {
    update(false);
}
type Node<'a> = BatchedJoinNode<'a, usize, usize, String, String, ()>;
fn counting(calls: &Cell<usize>) -> Node<'_> {
    Node::new(
        JoinEvaluation::Deferred,
        |_| (),
        |_| (),
        |_, _| {
            calls.set(calls.get() + 1);
            true
        },
    )
}
fn load(
    n: &mut Node<'_>,
    offset: usize,
    left: usize,
    right: usize,
    left_id: usize,
    right_id: usize,
) {
    for i in 0..left {
        n.enqueue_left(TupleChange::put(left_id + i, format!("L{}", offset + i)));
    }
    for i in 0..right {
        n.enqueue_right(TupleChange::put(right_id + i, format!("R{}", offset + i)));
    }
    n.propagate();
}
#[test]
fn preload_reads_eagerly_then_defers_again() {
    let preloaded = Cell::new(0);
    let control = Cell::new(0);
    let mut p = counting(&preloaded);
    p.begin_preload();
    load(&mut p, 0, 2, 3, 0, 0);
    assert_eq!(preloaded.get(), 2 * 3);
    p.end_preload();
    let mut c = counting(&control);
    load(&mut c, 0, 2, 3, 0, 0);
    preloaded.set(0);
    control.set(0);
    // Fact R2 occurs twice but each input tuple has its own identity.
    load(&mut p, 2, 1, 1, 2, 3);
    load(&mut c, 2, 1, 1, 2, 3);
    assert_eq!(preloaded.get(), control.get());
    assert_eq!(p.rows().count(), 12);
    assert_eq!(c.rows().count(), 12);
}
