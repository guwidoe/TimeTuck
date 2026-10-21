//! Original carrier/routing regressions, no upstream-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};

fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn fixed<'a>(
    comparison: JoinComparison,
    left: impl Fn(&Triple) -> usize + 'a,
    right: impl Fn(&Triple) -> usize + 'a,
) -> Join<'a, usize, usize, Triple, Triple, usize> {
    let mut node = JoinNode::comparing(comparison, left, right);
    assert!(node.bind_primitive_graph_journals());
    for id in 0usize..4 {
        node.apply_left(TupleChange::put(id, (id, id % 2, usize::MAX)));
        node.apply_right(TupleChange::put(id, (id + 4, id % 2, usize::MAX)));
    }
    assert!(node.bind_fixed_input(false, 4));
    assert!(node.bind_fixed_input(true, 4));
    Join::new(node)
}
fn collect(output: JoinOutput<'_>) -> Vec<Change> {
    let mut changes = Vec::new();
    output
        .for_each(|event| changes.push(typed::<(usize, usize), (Triple, Triple)>(&event).clone()));
    changes
}
fn legacy(output: Outputs) -> Vec<Change> {
    output
        .into_iter()
        .map(|event| typed::<(usize, usize), (Triple, Triple)>(&event).clone())
        .collect()
}

#[test]
fn join_output_carrier_fixed_both_sides_order_and_current_nested_full_restore_match_original() {
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        let mut old = fixed(comparison, |r| r.1, |r| r.1);
        let mut new = fixed(comparison, |r| r.1, |r| r.1);
        assert!(<Join<'_, _, _, _, _, _> as Node<SimpleScore>>::has_join_output(&new));
        let full_old = <_ as Node<SimpleScore>>::checkpoint(&old);
        let full_new = <_ as Node<SimpleScore>>::checkpoint(&new);
        let outer_old = <_ as Node<SimpleScore>>::begin_checkpoint(&mut old);
        let outer_new = <_ as Node<SimpleScore>>::begin_checkpoint(&mut new);
        for (right, change) in [
            (
                false,
                TupleChange::put(0usize, (77usize, 1usize, usize::MAX)),
            ),
            (true, TupleChange::retract(1usize)),
            (
                true,
                TupleChange::put(1usize, (88usize, 0usize, usize::MAX)),
            ),
            (false, TupleChange::retract(3usize)),
        ] {
            let event = Event::new(change);
            assert_eq!(
                collect(
                    <_ as Node<SimpleScore>>::apply_join_output(&mut new, right, &event).unwrap()
                ),
                legacy(
                    <_ as Node<SimpleScore>>::apply(&mut old, right, &event, arithmetic()).unwrap()
                ),
            );
            assert!(new.1.is_empty() && new.2.is_empty());
        }
        let captured_old = <_ as Node<SimpleScore>>::capture_checkpoint(&old, outer_old.as_ref());
        let captured_new = <_ as Node<SimpleScore>>::capture_checkpoint(&new, outer_new.as_ref());
        let child_old = <_ as Node<SimpleScore>>::begin_checkpoint(&mut old);
        let child_new = <_ as Node<SimpleScore>>::begin_checkpoint(&mut new);
        let event = Event::new(TupleChange::put(2usize, (99usize, 1usize, 0usize)));
        assert_eq!(
            collect(<_ as Node<SimpleScore>>::apply_join_output(&mut new, true, &event).unwrap()),
            legacy(<_ as Node<SimpleScore>>::apply(&mut old, true, &event, arithmetic()).unwrap()),
        );
        <_ as Node<SimpleScore>>::commit_checkpoint(&mut old, child_old);
        <_ as Node<SimpleScore>>::commit_checkpoint(&mut new, child_new);
        <_ as Node<SimpleScore>>::restore(&mut old, outer_old);
        <_ as Node<SimpleScore>>::restore(&mut new, outer_new);
        <_ as Node<SimpleScore>>::restore(&mut old, captured_old);
        <_ as Node<SimpleScore>>::restore(&mut new, captured_new);
        assert_eq!(
            old.0.matches().collect::<Vec<_>>(),
            new.0.matches().collect::<Vec<_>>()
        );
        <_ as Node<SimpleScore>>::restore(&mut old, full_old);
        <_ as Node<SimpleScore>>::restore(&mut new, full_new);
        assert_eq!(
            old.0.matches().collect::<Vec<_>>(),
            new.0.matches().collect::<Vec<_>>()
        );
    }
}

#[test]
fn join_output_carrier_end_input_borrow_arena_growth_promotion_and_panic_release() {
    let mut node = fixed(JoinComparison::Equal, |_| 0, |_| 0);
    let mut arena = vec![Event::new(TupleChange::put(
        0usize,
        (10usize, 0usize, 0usize),
    ))];
    arena.shrink_to_fit();
    let previous_capacity = arena.capacity();
    // The input lives in the SAME arena publication grows; the hook lifetime
    // permits this without cloning or retaining that input reference.
    let output = <_ as Node<SimpleScore>>::apply_join_output(&mut node, false, &arena[0]).unwrap();
    output.for_each(|event| arena.push(event));
    assert_eq!(arena.len(), 5);
    assert!(arena.capacity() > previous_capacity);
    let pointer = node.1.as_ptr();
    let capacity = node.1.capacity();
    for _ in 0..2 {
        let input = Event::new(TupleChange::put(0usize, (11usize, 0usize, 0usize)));
        let output = <_ as Node<SimpleScore>>::apply_join_output(&mut node, false, &input).unwrap();
        output.for_each(|event| {
            let owner = event.promote();
            let peer = owner.clone();
            assert!(Rc::ptr_eq(owner.shared_ref(), peer.shared_ref()));
            assert_eq!(Rc::strong_count(owner.shared_ref()), 2);
            let weak = Rc::downgrade(owner.shared_ref());
            drop(owner);
            assert!(weak.upgrade().is_some());
            drop(peer);
            assert!(weak.upgrade().is_none());
        });
        assert_eq!(node.1.as_ptr(), pointer);
        assert_eq!(node.1.capacity(), capacity);
    }
    let input = Event::new(TupleChange::put(0usize, (12usize, 0usize, 0usize)));
    let seen = Cell::new(0usize);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let output =
                <_ as Node<SimpleScore>>::apply_join_output(&mut node, false, &input).unwrap();
            output.for_each(|_| {
                seen.set(seen.get() + 1);
                panic!("publication test");
            });
        }))
        .is_err()
    );
    assert_eq!(seen.get(), 1);
    assert!(node.1.is_empty());
    assert_eq!(node.1.capacity(), 0);
    let mut oversized: Vec<Change> =
        Vec::with_capacity(JOIN_BUFFER_BYTES / std::mem::size_of::<Change>() + 1);
    JoinOutput::new(&mut oversized, false).for_each(|_| unreachable!());
    assert_eq!(oversized.capacity(), 0);
    assert!(std::mem::size_of::<JoinOutput<'_>>() <= 3 * std::mem::size_of::<usize>());
}

type Run = (
    Vec<(&'static str, usize, usize)>,
    Vec<SimpleScore>,
    Vec<u128>,
    String,
);
type Trace = RefCell<Vec<(&'static str, usize, usize)>>;
fn graph<'a>(
    trace: &'a Trace,
    failure: &'a Cell<usize>,
) -> (
    StreamGraph<'a, SimpleScore>,
    StreamSource<usize, Triple>,
    usize,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Triple>();
    let joined = b
        .join_equal(
            source.port(),
            source.port(),
            |row| {
                trace.borrow_mut().push(("left", row.0, row.1));
                assert_ne!(failure.get(), 1, "projection panic");
                row.1
            },
            |row| {
                trace.borrow_mut().push(("right", row.0, row.1));
                row.1
            },
        )
        .unwrap();
    let mapped = b
        .map(&joined, |pair| {
            trace.borrow_mut().push(("map", pair.0.0, pair.1.0));
            assert_ne!(failure.get(), 2, "mapping panic");
            *pair
        })
        .unwrap();
    b.score("mapped", &mapped, |pair| {
        trace
            .borrow_mut()
            .push(("mapped score", pair.0.0, pair.1.0));
        assert_ne!(failure.get(), 3, "score panic");
        match failure.get() {
            4 => SimpleScore::new(0).with_structural(1),
            5 => SimpleScore::new(i64::MAX),
            _ => SimpleScore::new((pair.0.2 + pair.1.2) as i64),
        }
    })
    .unwrap();
    b.score("direct", &joined, |pair| {
        trace
            .borrow_mut()
            .push(("direct score", pair.0.0, pair.1.0));
        SimpleScore::new(1)
    })
    .unwrap();
    (b.build(), source, joined.node)
}

#[test]
fn join_output_carrier_actual_consumed_graph_bfs_replay_captures_errors_and_panics_match_cold_and_wide()
 {
    fn run(carrier: bool, wide: bool, phase: usize) -> Run {
        event_queue::with_wide(wide, || {
            let trace = Trace::default();
            let failure = Cell::new(0usize);
            let (mut graph, source, joined) = graph(&trace, &failure);
            assert!(graph.try_enable_consuming_search(source.node_id(), 24));
            {
                let mut tx = graph.transaction();
                for id in 0usize..24 {
                    tx.apply(&source, TupleChange::put(id, (id, 0usize, id % 3)))
                        .unwrap();
                }
                tx.commit().unwrap();
            }
            graph.bind_fixed_planning_source(&source, 24).unwrap();
            assert!(graph.join_output_nodes[joined]);
            graph.join_output_nodes[joined] = carrier;
            let full = graph.capture_full();
            let base = graph.score();
            let mut scores = vec![base];
            let mut work = Vec::new();
            trace.borrow_mut().clear();
            let captured = {
                let mut outer = graph.transaction();
                outer
                    .apply(
                        &source,
                        TupleChange::put(0usize, (100usize, 0usize, 7usize)),
                    )
                    .unwrap();
                {
                    let mut child = outer.graph.transaction();
                    child.apply(&source, TupleChange::retract(1usize)).unwrap();
                    child
                        .apply(
                            &source,
                            TupleChange::put(1usize, (101usize, 0usize, 8usize)),
                        )
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
                    tx.apply(
                        &source,
                        TupleChange::put(0usize, (200usize, 0usize, 9usize)),
                    )
                }));
                work.push(tx.score_evaluations());
                match result {
                    Err(_) => "panic".to_owned(),
                    Ok(Err(error)) => format!("{error:?}"),
                    Ok(Ok(())) => "ok".to_owned(),
                }
            };
            assert_eq!(graph.score(), base);
            failure.set(0);
            graph.assert_consistent().unwrap();
            scores.push(graph.score());
            assert!(graph.propagation_workspace.arena.is_empty());
            assert!(graph.propagation_workspace.arena_pending.is_empty());
            drop(graph);
            (trace.into_inner(), scores, work, result)
        })
    }
    for phase in 0..=5 {
        let original = run(false, false, phase);
        assert_eq!(run(true, false, phase), original);
        assert_eq!(run(true, true, phase), original);
    }
}

thread_local! {
    static CLONES: Cell<usize> = const { Cell::new(0) };
    static PANIC_CLONE: Cell<bool> = const { Cell::new(false) };
}
#[derive(Eq, PartialEq)]
struct Custom(Triple);
impl Clone for Custom {
    fn clone(&self) -> Self {
        CLONES.with(|n| n.set(n.get() + 1));
        assert!(!PANIC_CLONE.with(Cell::get), "custom clone panic");
        Self(self.0)
    }
}
#[test]
fn join_output_carrier_scope_rejects_unbounded_partial_custom_clone_and_public_graph() {
    assert!(supports::<usize, usize, Triple, Triple>());
    assert!(!std::mem::needs_drop::<Custom>());
    assert!(!supports::<usize, usize, Custom, Custom>());
    assert!(!supports::<(usize, usize), (usize, usize), Triple, Triple>());
    assert!(!supports::<usize, usize, String, String>());
    let mut node = JoinNode::<usize, usize, Triple, Triple, usize>::new(|_| 0, |_| 0);
    node.bind_primitive_graph_journals();
    node.apply_left(TupleChange::put(0usize, (0usize, 0usize, 0usize)));
    node.apply_right(TupleChange::put(0usize, (1usize, 0usize, 0usize)));
    let mut wrapper = Join::new(node);
    assert!(!<_ as Node<SimpleScore>>::has_join_output(&wrapper));
    <_ as Node<SimpleScore>>::bind_fixed_input(&mut wrapper, false, 1);
    assert!(!<_ as Node<SimpleScore>>::has_join_output(&wrapper));
    <_ as Node<SimpleScore>>::bind_fixed_input(&mut wrapper, true, 1);
    assert!(<_ as Node<SimpleScore>>::has_join_output(&wrapper));
    let mut custom = Join::new(JoinNode::<usize, usize, Custom, Custom, usize>::new(
        |_| 0,
        |_| 0,
    ));
    CLONES.with(|n| n.set(0));
    let event = Event::new(TupleChange::put(0usize, Custom((0, 0, 0))));
    <_ as Node<SimpleScore>>::apply(&mut custom, false, &event, arithmetic()).unwrap();
    assert!(CLONES.with(Cell::get) > 0);
    assert!(!<_ as Node<SimpleScore>>::has_join_output(&custom));
    let saved = <_ as Node<SimpleScore>>::checkpoint(&custom);
    let next = Event::new(TupleChange::put(1usize, Custom((1, 0, 0))));
    PANIC_CLONE.with(|flag| flag.set(true));
    let failed = catch_unwind(AssertUnwindSafe(|| {
        <_ as Node<SimpleScore>>::apply(&mut custom, false, &next, arithmetic())
    }));
    PANIC_CLONE.with(|flag| flag.set(false));
    assert!(failed.is_err());
    <_ as Node<SimpleScore>>::restore(&mut custom, saved);
    <_ as Node<SimpleScore>>::apply(&mut custom, false, &next, arithmetic()).unwrap();
    let trace = Trace::default();
    let failure = Cell::new(0usize);
    let (mut retained, source, joined) = graph(&trace, &failure);
    {
        let mut tx = retained.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 0usize)))
            .unwrap();
        tx.commit().unwrap();
    }
    retained.bind_fixed_planning_source(&source, 1).unwrap();
    assert!(retained.join_output_nodes[joined]);
    assert!(!retained.consuming_search);
    let score_id = retained.constraints["direct"];
    let baseline = retained.nodes[score_id]
        .matches()
        .unwrap()
        .downcast_ref::<SnapshotMap<(usize, usize), ((Triple, Triple), SimpleScore)>>()
        .unwrap()
        .clone();
    let owners = baseline.shared_entries();
    let weak = std::sync::Arc::downgrade(&owners[0].1);
    {
        let mut tx = retained.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 0usize)))
            .unwrap();
        let current = tx.graph.nodes[score_id]
            .matches()
            .unwrap()
            .downcast_ref::<SnapshotMap<(usize, usize), ((Triple, Triple), SimpleScore)>>()
            .unwrap();
        assert!(!std::sync::Arc::ptr_eq(
            &owners[0].1,
            &current.shared_entries()[0].1
        ));
    }
    assert!(weak.upgrade().is_some());
    let restored = retained.nodes[score_id]
        .matches()
        .unwrap()
        .downcast_ref::<SnapshotMap<(usize, usize), ((Triple, Triple), SimpleScore)>>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &owners[0].1,
        &restored.shared_entries()[0].1
    ));
}
