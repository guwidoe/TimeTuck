//! Original consuming-event forwarding regressions; no upstream method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
type Pair = (usize, usize);
type Quad = (usize, usize, usize, usize);
type Id = (Pair, Pair);
type Row = (Quad, Quad);
type Change = TupleChange<Id, Row>;
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
#[test]
fn shared_forward_member_actions_keep_full_ids_fifo_fanout_and_private_owner_lifetimes() {
    if usize::BITS == 64 {
        assert_eq!(std::mem::size_of::<Event>(), 72);
        assert_eq!(std::mem::size_of::<event_queue::QueuedEvent>(), 24);
    }
    let changes: Vec<Change> = (0usize..256)
        .map(|n| {
            let id = ((usize::MAX, n % 7), (n % 5, usize::MAX));
            if n.is_multiple_of(7) {
                Change::retract(id)
            } else {
                Change::put(id, ((usize::MAX, n % 2, n, 3), (4, usize::MAX, n, 6)))
            }
        })
        .collect();
    for receivers in [0usize, 1, 3] {
        let mut fast: Box<dyn Node<SimpleScore>> = Box::new(MembershipFilter::new_inline(
            Membership::<Id>::new().unwrap(),
            |r: &Row| r.0.1 == 0,
        ));
        let mut old: Box<dyn Node<SimpleScore>> = Box::new(MembershipFilter::new_inline(
            Membership::<Id>::new().unwrap(),
            |r: &Row| r.0.1 == 0,
        ));
        assert!(fast.has_inline_forward());
        let saved = fast.checkpoint();
        let old_saved = old.checkpoint();
        for change in changes.iter().cloned() {
            let input = Event::new(change);
            let weak = Rc::downgrade(input.shared_ref());
            let action = fast.apply_inline_forward(&input);
            let expected: Vec<_> = old
                .apply(false, &input, arithmetic())
                .unwrap()
                .into_iter()
                .collect();
            assert!(expected.len() <= 1);
            let edges: Vec<_> = (0..receivers)
                .map(|node| Edge {
                    node,
                    right: node % 2 != 0,
                })
                .collect();
            let mut pending = VecDeque::new();
            let prefix: Rc<dyn Any> =
                Rc::new(Change::retract(((usize::MAX, usize::MAX), (0, usize::MAX))));
            pending.push_back((
                Edge {
                    node: usize::MAX,
                    right: true,
                },
                event_queue::QueuedEvent::Shared(prefix.clone()),
            ));
            match action {
                InlineForward::None => assert!(expected.is_empty()),
                InlineForward::Same => {
                    event_queue::enqueue_existing(&edges, &input, None, &mut pending)
                }
                InlineForward::Retract => event_queue::enqueue(
                    &edges,
                    input.forward_retraction(),
                    &mut pending,
                    &mut Vec::new(),
                    false,
                ),
            }
            let (edge, event_queue::QueuedEvent::Shared(owner)) = pending.pop_front().unwrap()
            else {
                panic!("prefix")
            };
            assert_eq!((edge.node, edge.right), (usize::MAX, true));
            assert!(Rc::ptr_eq(&owner, &prefix));
            assert_eq!(pending.len(), receivers * expected.len());
            let mut retained: Option<Rc<dyn Any>> = None;
            if let Some(expected) = expected.first() {
                for edge in edges {
                    let (actual, event_queue::QueuedEvent::Shared(owner)) =
                        pending.pop_front().unwrap()
                    else {
                        panic!("full member owner")
                    };
                    assert_eq!((actual.node, actual.right), (edge.node, edge.right));
                    assert_eq!(
                        owner.downcast_ref::<Change>().unwrap(),
                        typed::<Id, Row>(expected)
                    );
                    assert_eq!(
                        Rc::ptr_eq(input.shared_ref(), &owner),
                        action == InlineForward::Same
                    );
                    retained = Some(owner);
                }
            }
            assert!(pending.is_empty());
            drop(input);
            assert_eq!(
                weak.upgrade().is_some(),
                action == InlineForward::Same && receivers > 0
            );
            drop(retained);
            assert!(weak.upgrade().is_none());
        }
        fast.restore(saved);
        old.restore(old_saved);
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
    fail: &'a Cell<usize>,
) -> (
    StreamGraph<'a, SimpleScore>,
    StreamSource<Pair, Quad>,
    Vec<usize>,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<Pair, Quad>();
    let joined = b
        .join_equal(source.port(), source.port(), |r| (r.0, r.1), |r| (r.0, r.1))
        .unwrap();
    let first = b
        .filter(&joined, |r| {
            trace.borrow_mut().push(("first", r.0.2, r.1.2));
            assert_ne!(fail.get(), 1, "predicate panic");
            r.0.2 < r.1.2
        })
        .unwrap();
    let second = b
        .filter(&first, |r| {
            trace.borrow_mut().push(("second", r.0.2, r.1.2));
            r.0.3 > 0
        })
        .unwrap();
    b.score("direct", &first, |r| {
        trace.borrow_mut().push(("direct", r.0.2, r.1.2));
        SimpleScore::new(1)
    })
    .unwrap();
    b.score("second", &second, |r| {
        trace.borrow_mut().push(("score", r.0.2, r.1.2));
        assert_ne!(fail.get(), 2, "score panic");
        match fail.get() {
            3 => SimpleScore::new(0).with_structural(1),
            4 => SimpleScore::new(i64::MAX),
            _ => SimpleScore::new((r.0.3 + r.1.3) as i64),
        }
    })
    .unwrap();
    b.score("raw", &joined, |r| {
        trace.borrow_mut().push(("raw", r.0.2, r.1.2));
        SimpleScore::new(1)
    })
    .unwrap();
    (b.build(), source, vec![first.node, second.node])
}
#[test]
fn shared_forward_consumed_member_graph_preserves_callbacks_captures_errors_overflow_and_retry() {
    fn run(enabled: bool, wide: bool, force_shared: bool, phase: usize) -> Run {
        event_queue::with_wide(wide, || {
            event::with_forced_shared(force_shared, || {
                let trace = Trace::default();
                let failure = Cell::new(0);
                let (mut g, source, filters) = graph(&trace, &failure);
                assert!(g.try_enable_consuming_search(source.node_id(), 6));
                assert!(
                    filters
                        .iter()
                        .all(|&id| g.inline_forward_nodes[id] && !g.shared_event_producers[id])
                );
                if !enabled {
                    g.inline_forward_nodes.fill(false);
                }
                {
                    let mut tx = g.transaction();
                    for id in 0usize..6 {
                        tx.apply(&source, TupleChange::put((0, id), (0, 0, id, id + 1)))
                            .unwrap();
                    }
                    tx.commit().unwrap();
                }
                let full = g.capture_full();
                let base = g.score();
                let mut scores = vec![base];
                let mut work = Vec::new();
                trace.borrow_mut().clear();
                let captured = {
                    let mut outer = g.transaction();
                    outer
                        .apply(&source, TupleChange::put((0, 0), (0, 0, 100, 7)))
                        .unwrap();
                    {
                        let mut child = outer.graph.transaction();
                        child.apply(&source, TupleChange::retract((0, 1))).unwrap();
                        child
                            .apply(&source, TupleChange::put((0, 1), (0, 0, 101, 0)))
                            .unwrap();
                        child.commit().unwrap();
                    }
                    scores.push(outer.score().unwrap());
                    work.push(outer.score_evaluations());
                    outer.capture().unwrap()
                };
                assert_eq!(g.score(), base);
                let callbacks = trace.borrow().len();
                g.install(captured);
                assert_eq!(trace.borrow().len(), callbacks);
                scores.push(g.score());
                g.install(full);
                assert_eq!(g.score(), base);
                failure.set(phase);
                let outcome = {
                    let mut tx = g.transaction();
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
                assert_eq!(g.score(), base);
                failure.set(0);
                g.assert_consistent().unwrap();
                assert!(g.propagation_workspace.arena_pending.is_empty());
                {
                    let mut tx = g.transaction();
                    tx.apply(&source, TupleChange::put((0, 0), (0, 0, 200, 9)))
                        .unwrap();
                    scores.push(tx.score().unwrap());
                }
                assert_eq!(g.score(), base);
                drop(g);
                (trace.into_inner(), scores, work, outcome)
            })
        })
    }
    for phase in 0..=4 {
        let original = run(false, false, false, phase);
        assert_eq!(run(true, false, false, phase), original);
        assert_eq!(run(true, true, false, phase), original);
        assert_eq!(run(true, false, true, phase), original);
    }
}
struct Recorder<'a>(&'a RefCell<Vec<Rc<dyn Any>>>);
impl Node<SimpleScore> for Recorder<'_> {
    fn clear(&mut self) {}
    fn retains_input_event(&self) -> bool {
        true
    }
    fn is_constraint(&self) -> bool {
        true
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(())
    }
    fn restore(&mut self, _: Box<dyn Any>) {}
    fn apply(&mut self, _: bool, input: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        self.0.borrow_mut().push(input.shared_ref().clone());
        Ok(Outputs::new())
    }
}
#[test]
fn shared_forward_public_unknown_and_retaining_endpoints_keep_distinct_promoted_owners() {
    for consuming in [false, true] {
        let raw = RefCell::new(Vec::new());
        let seen = RefCell::new(Vec::new());
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<Id, Row>();
        let filter = b.filter(source.port(), |_| true).unwrap();
        let first: StreamPort<Id, Row> = b.push(Recorder(&seen));
        let second: StreamPort<Id, Row> = b.push(Recorder(&seen));
        let direct: StreamPort<Id, Row> = b.push(Recorder(&raw));
        b.connect(filter.node, first.node, false);
        b.connect(filter.node, second.node, false);
        b.connect(source.node_id(), direct.node, false);
        let mut g = b.build();
        assert!(g.inline_forward_nodes[filter.node]);
        assert!(g.shared_event_producers[filter.node]);
        assert!(
            !g.try_enable_consuming_search(source.node_id(), 1),
            "unknown retaining nodes cannot acquire consuming permission"
        );
        if consuming {
            // Instrument the inner ownership guard separately. Production never
            // grants this synthetic observer the whole-graph permission above.
            g.consuming_eligible = true;
            assert!(g.try_enable_consuming_search(source.node_id(), 1));
        }
        {
            let mut tx = g.transaction();
            tx.apply(
                &source,
                Change::put(
                    ((usize::MAX, 7), (9, usize::MAX)),
                    ((1, 2, 3, 4), (5, 6, 7, 8)),
                ),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(raw.borrow().len(), 1);
        assert_eq!(seen.borrow().len(), 2);
        let raw = raw.borrow();
        let seen = seen.borrow();
        assert!(Rc::ptr_eq(&seen[0], &seen[1]));
        assert!(!Rc::ptr_eq(&raw[0], &seen[0]));
        assert_eq!(
            raw[0].downcast_ref::<Change>(),
            seen[0].downcast_ref::<Change>()
        );
    }
}
