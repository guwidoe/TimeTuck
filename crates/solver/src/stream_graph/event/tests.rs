//! Original inline-erasure controls; no upstream source-method credit.
use super::*;
use crate::{HardSoftScore, IntSummary, StreamBatch, StreamGraphBuilder};
use std::{
    cell::{Cell, RefCell},
    mem::{align_of, size_of},
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn exact_inline_shapes_round_trip_put_retract_extremes_and_clone() {
    type PairChange = TupleChange<(usize, usize), (Triple, Triple)>;
    let triples = [0, 1, usize::MAX].map(|n| (n, usize::MAX - n, n));
    for value in [None, Some((triples[0], triples[2]))] {
        let change = PairChange {
            id: (usize::MAX, 0),
            value,
        };
        let event = Event::new(change.clone());
        assert!(matches!(event, Event::Pair(_)));
        assert_eq!(event.downcast_ref::<PairChange>(), Some(&change));
        assert_eq!(event.clone().downcast_ref::<PairChange>(), Some(&change));
        assert!(event.downcast_ref::<TupleChange<usize, Triple>>().is_none());
    }
    for value in [None, Some(triples[1])] {
        let change = TupleChange {
            id: usize::MAX,
            value,
        };
        let event = Event::new(change.clone());
        assert!(matches!(event, Event::Triple(_)));
        assert_eq!(
            event.clone().downcast_ref::<TupleChange<usize, Triple>>(),
            Some(&change)
        );
    }
    for value in [None, Some((usize::MAX, None)), Some((0, Some(usize::MAX)))] {
        let change = TupleChange {
            id: usize::MAX,
            value,
        };
        let event = Event::new(change.clone());
        assert!(matches!(event, Event::Planning(_)));
        assert_eq!(
            event.clone().downcast_ref::<TupleChange<usize, Planning>>(),
            Some(&change)
        );
    }
}

#[test]
fn layout_budget_includes_queue_outputs_and_wide_fallback() {
    type PairChange = TupleChange<(usize, usize), (Triple, Triple)>;
    type Wide = TupleChange<(usize, usize), ((usize, usize), IntSummary)>;
    // Layout is a real cost: no claim that the wider routing cells are free.
    eprintln!(
        "event={} shared={} pair={} wide={} outputs={} queued={} align={}",
        size_of::<Event>(),
        size_of::<Rc<dyn Any>>(),
        size_of::<PairChange>(),
        size_of::<Wide>(),
        size_of::<super::super::outputs::Outputs>(),
        size_of::<(super::super::Edge, Event)>(),
        align_of::<Event>()
    );
    assert!(size_of::<Event>() <= size_of::<PairChange>() + 2 * size_of::<usize>());
    #[cfg(target_pointer_width = "64")]
    {
        assert_eq!(size_of::<Rc<dyn Any>>(), 16);
        assert_eq!(size_of::<PairChange>(), 72);
        assert!(size_of::<Event>() <= 80);
        assert!(size_of::<(super::super::Edge, Event)>() <= 96);
    }
    let wide = Event::new(Wide::put((usize::MAX, 0), ((1, 2), IntSummary::default())));
    assert!(matches!(wide, Event::Shared(_)));
    let nested = Event::new(TupleChange::put(
        ((1usize, 2usize), 3usize),
        (4usize, 5usize, 6usize, 7usize),
    ));
    assert!(matches!(nested, Event::Shared(_)));
}

#[test]
fn custom_clone_drop_and_shared_aliases_are_unchanged() {
    struct Row(Rc<Cell<usize>>, Rc<RefCell<Vec<&'static str>>>);
    impl Clone for Row {
        fn clone(&self) -> Self {
            panic!("event routing must never call custom Clone")
        }
    }
    impl Drop for Row {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            self.1.borrow_mut().push("drop");
        }
    }
    let drops = Rc::new(Cell::new(0));
    let trace = Rc::new(RefCell::new(Vec::new()));
    let event = Event::new(TupleChange::put(0usize, Row(drops.clone(), trace.clone())));
    assert!(matches!(event, Event::Shared(_)));
    let weak = Rc::downgrade(event.shared_ref());
    let alias = event.clone();
    assert!(Rc::ptr_eq(alias.shared_ref(), event.shared_ref()));
    assert_eq!(Rc::strong_count(event.shared_ref()), 2);
    drop(event);
    assert_eq!(drops.get(), 0);
    drop(alias);
    assert!(weak.upgrade().is_none());
    assert_eq!((drops.get(), trace.borrow().clone()), (1, vec!["drop"]));
    #[derive(PartialEq, Eq, PartialOrd, Ord)]
    struct Id(usize);
    impl Clone for Id {
        fn clone(&self) -> Self {
            panic!("custom ID clone")
        }
    }
    let custom = Event::new(TupleChange::put(Id(usize::MAX), (0usize, 1usize, 2usize)));
    assert!(matches!(custom, Event::Shared(_)));
    assert!(
        custom
            .clone()
            .downcast_ref::<TupleChange<Id, Triple>>()
            .is_some()
    );
}

type Trace = Rc<RefCell<Vec<(&'static str, usize)>>>;
type Observation = (Vec<HardSoftScore>, Vec<(&'static str, usize)>, u128);
fn graph_trial(force_shared: bool, batch: bool, failure: u8) -> Observation {
    with_forced_shared(force_shared, || {
        let trace = Trace::default();
        let fail = Cell::new(false);
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let source = b.source::<usize, Planning>();
        let calls = trace.clone();
        let present = b
            .filter(source.port(), move |row| {
                calls.borrow_mut().push(("present", row.0));
                row.1.is_some()
            })
            .unwrap();
        let calls = trace.clone();
        let failure_flag = &fail;
        let assigned = b
            .map(&present, move |row| {
                calls.borrow_mut().push(("map", row.0));
                assert!(!(failure_flag.get() && failure == 1), "mapping panic");
                (row.0 % 2, row.0, row.1.unwrap())
            })
            .unwrap();
        let joined = b
            .join_equal(
                &assigned,
                &assigned,
                |row| (row.0, row.2),
                |row| (row.0, row.2),
            )
            .unwrap();
        let calls = trace.clone();
        let ordered = b
            .filter(&joined, move |(l, r)| {
                calls.borrow_mut().push(("pair", l.1 * 8 + r.1));
                l.1 < r.1
            })
            .unwrap();
        let groups = b.group_collect(&ordered, |(l, r)| ((l.1, r.1), 1)).unwrap();
        let calls = trace.clone();
        let failure_flag = &fail;
        b.score("pairs", &groups, move |(key, summary)| {
            calls.borrow_mut().push(("score", key.0 * 8 + key.1));
            assert!(!(failure_flag.get() && failure == 2), "score panic");
            HardSoftScore::new(0, summary.sum)
        })
        .unwrap();
        let mut graph = b.build();
        let mut initial = graph.transaction();
        for id in 0usize..4 {
            initial
                .apply(&source, TupleChange::put(id, (id, Some(0))))
                .unwrap();
        }
        initial.commit().unwrap();
        graph.assert_consistent().unwrap();
        trace.borrow_mut().clear();
        let original = graph.score();
        let mut scores = vec![original];
        let mut physical = 0;
        for commit in [false, true] {
            fail.set(failure > 0);
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                let mut tx = graph.transaction();
                if batch {
                    let mut changes = StreamBatch::new();
                    changes
                        .push(&source, TupleChange::put(0usize, (0, Some(1))))
                        .unwrap();
                    changes
                        .push(&source, TupleChange::put(2usize, (2, Some(1))))
                        .unwrap();
                    tx.apply_batch(changes).unwrap();
                } else {
                    tx.apply(&source, TupleChange::put(0usize, (0, Some(1))))
                        .unwrap();
                    tx.apply(&source, TupleChange::put(2usize, (2, Some(1))))
                        .unwrap();
                }
                physical += tx.score_evaluations();
                if commit {
                    tx.commit().unwrap();
                }
            }));
            assert_eq!(outcome.is_err(), failure > 0);
            fail.set(false);
            if !commit || failure > 0 {
                assert_eq!(graph.score(), original);
            }
            scores.push(graph.score());
            graph.assert_consistent().unwrap();
        }
        let result_trace = trace.borrow().clone();
        (scores, result_trace, physical)
    })
}

#[test]
fn inline_and_shared_routes_preserve_callback_trace_batch_and_panic_rollback() {
    for batch in [false, true] {
        for failure in 0..3 {
            assert_eq!(
                graph_trial(false, batch, failure),
                graph_trial(true, batch, failure)
            );
        }
    }
}

#[test]
fn moving_output_owner_keeps_custom_fanout_fifo_aliases_and_drop_point() {
    use super::super::{Edge, enqueue_output};
    use std::collections::VecDeque;
    struct Payload(Rc<RefCell<Vec<usize>>>, usize);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.borrow_mut().push(self.1);
        }
    }
    for fanout in [0, 1, 3] {
        let trace = Rc::new(RefCell::new(Vec::new()));
        let event = Event::new(Payload(trace.clone(), fanout));
        let alias = event.clone();
        let edges = (0..fanout)
            .map(|node| Edge {
                node,
                right: node % 2 == 1,
            })
            .collect::<Vec<_>>();
        let mut pending = VecDeque::new();
        enqueue_output(&edges, event, &mut pending, false);
        assert_eq!(pending.len(), fanout);
        assert_eq!(Rc::strong_count(alias.shared_ref()), fanout + 1);
        for node in 0..fanout {
            let (edge, event) = pending.pop_front().unwrap();
            assert_eq!((edge.node, edge.right), (node, node % 2 == 1));
            assert!(Rc::ptr_eq(event.shared_ref(), alias.shared_ref()));
            drop(event);
            assert!(trace.borrow().is_empty());
        }
        drop(alias);
        assert_eq!(*trace.borrow(), [fanout]);
    }
}

#[test]
fn numeric_shared_owner_promotion_happens_before_passthrough_fanout() {
    use super::super::{GraphError, Node, NodeArithmetic, Outputs};
    type Owners = Rc<RefCell<Vec<Rc<TupleChange<usize, Triple>>>>>;
    struct Pass;
    struct Retain(Owners);
    impl Node<HardSoftScore> for Pass {
        fn clear(&mut self) {}
        fn checkpoint(&self) -> Box<dyn Any> {
            Box::new(())
        }
        fn restore(&mut self, _: Box<dyn Any>) {}
        fn apply(
            &mut self,
            _: bool,
            event: &Event,
            _: NodeArithmetic,
        ) -> Result<Outputs, GraphError> {
            Ok(Outputs::one(event.clone()))
        }
    }
    impl Node<HardSoftScore> for Retain {
        fn retains_input_event(&self) -> bool {
            true
        }
        fn clear(&mut self) {}
        fn checkpoint(&self) -> Box<dyn Any> {
            Box::new(())
        }
        fn restore(&mut self, _: Box<dyn Any>) {}
        fn apply(
            &mut self,
            _: bool,
            event: &Event,
            _: NodeArithmetic,
        ) -> Result<Outputs, GraphError> {
            self.0.borrow_mut().push(event.clone().downcast().unwrap());
            Ok(Outputs::new())
        }
    }
    for batch in [false, true] {
        let owners = Owners::default();
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let source = b.source::<usize, Triple>();
        let a = b.push::<usize, Triple>(Pass);
        let c = b.push::<usize, Triple>(Pass);
        let x = b.push::<usize, Triple>(Retain(owners.clone()));
        let y = b.push::<usize, Triple>(Retain(owners.clone()));
        b.connect(source.0.node, a.node, false);
        b.connect(source.0.node, c.node, false);
        b.connect(a.node, x.node, false);
        b.connect(c.node, y.node, false);
        let mut graph = b.build();
        assert!(graph.shared_event_producers[source.0.node]);
        let mut tx = graph.transaction();
        let change = TupleChange::put(usize::MAX, (1usize, 2usize, usize::MAX));
        if batch {
            let mut changes = StreamBatch::new();
            changes.push(&source, change).unwrap();
            tx.apply_batch(changes).unwrap();
        } else {
            tx.apply(&source, change).unwrap();
        }
        tx.commit().unwrap();
        let rows = owners.borrow();
        assert_eq!(rows.len(), 2);
        assert!(Rc::ptr_eq(&rows[0], &rows[1]));
        assert_eq!(rows[0].value, Some((1, 2, usize::MAX)));
    }
}

#[test]
fn numeric_shared_and_fused_fixed_link_adapters_accept_promoted_producer_events() {
    for batch in [false, true] {
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let source = b.source::<usize, Triple>();
        let links = [(0usize, 0usize, 1usize)];
        let joined = b
            .join_fixed_links_batched_shared(
                source.port(),
                source.port(),
                links,
                |_, _, _| true,
                |_, left, right| left.0 + right.0,
            )
            .unwrap();
        b.score("ordinary", &joined, |v| HardSoftScore::new(0, *v as i64))
            .unwrap();
        b.score_fixed_links_batched_shared(
            "fused",
            source.port(),
            source.port(),
            links,
            |_, _, _| true,
            |_, left, right| left.0 + right.0,
            |v| HardSoftScore::new(0, *v as i64),
        )
        .unwrap();
        let mut graph = b.build();
        assert!(graph.shared_event_producers[source.0.node]);
        let mut tx = graph.transaction();
        if batch {
            let mut changes = StreamBatch::new();
            changes
                .push(&source, TupleChange::put(0usize, (4usize, 0usize, 0usize)))
                .unwrap();
            changes
                .push(&source, TupleChange::put(1usize, (6usize, 0usize, 0usize)))
                .unwrap();
            tx.apply_batch(changes).unwrap();
        } else {
            tx.apply(&source, TupleChange::put(0usize, (4usize, 0usize, 0usize)))
                .unwrap();
            tx.apply(&source, TupleChange::put(1usize, (6usize, 0usize, 0usize)))
                .unwrap();
        }
        tx.commit().unwrap();
        assert_eq!(graph.score(), HardSoftScore::new(0, 20));
        graph.assert_consistent().unwrap();
    }
}

#[test]
fn arena_wide_inline_and_shared_scalar_routes_have_identical_observables() {
    for failure in 0..3 {
        let arena = graph_trial(false, false, failure);
        let wide =
            super::super::event_queue::with_wide(true, || graph_trial(false, false, failure));
        let shared = graph_trial(true, false, failure);
        assert_eq!(arena, wide);
        assert_eq!(arena, shared);
    }
}
