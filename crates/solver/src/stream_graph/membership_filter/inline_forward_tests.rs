//! Original private forwarding controls; no upstream method credit.
use super::*;
use crate::SimpleScore;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
type Triple = (usize, usize, usize);
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn compare<I, T>(rows: Vec<TupleChange<I, T>>, predicate: fn(&T) -> bool, pair: bool)
where
    I: Ord + Clone + std::fmt::Debug + 'static,
    T: Clone + PartialEq + std::fmt::Debug + 'static,
{
    let mut fast: Box<dyn Node<SimpleScore>> = Box::new(MembershipFilter::new_inline(
        Membership::<I>::new().unwrap(),
        predicate,
    ));
    let mut old: Box<dyn Node<SimpleScore>> = Box::new(MembershipFilter::new_inline(
        Membership::<I>::new().unwrap(),
        predicate,
    ));
    assert!(fast.has_inline_forward());
    for node in [&mut fast, &mut old] {
        if pair {
            node.bind_fixed_pair_input(false, 32, 32);
        } else {
            node.bind_fixed_input(false, 32);
        }
    }
    assert!(fast.has_inline_forward());
    let saved = fast.checkpoint();
    let old_saved = old.checkpoint();
    for change in rows {
        let input = Event::new(change);
        let action = fast.apply_inline_forward(&input);
        let actual = match action {
            InlineForward::None => None,
            InlineForward::Same => Some(input.clone()),
            InlineForward::Retract => Some(input.inline_retraction()),
        };
        let expected: Vec<_> = old
            .apply(false, &input, arithmetic())
            .unwrap()
            .into_iter()
            .collect();
        assert_eq!(
            actual.as_ref().map(typed::<I, T>),
            expected.first().map(typed::<I, T>)
        );
        assert!(expected.len() <= 1);
    }
    fast.restore(saved);
    old.restore(old_saved);
    assert!(fast.has_inline_forward());
}
#[test]
fn inline_forward_actions_match_original_all_copy_shapes_acceptance_transitions_and_full_range_ids()
{
    let mut triples = Vec::new();
    let mut pairs = Vec::new();
    let mut planning = Vec::new();
    for n in 0usize..512 {
        let id = (n * 13) % 32;
        triples.push(if n.is_multiple_of(7) {
            TupleChange::retract(id)
        } else {
            TupleChange::put(id, (usize::MAX, n % 5, n))
        });
        pairs.push(if n.is_multiple_of(7) {
            TupleChange::retract((id, id))
        } else {
            TupleChange::put((id, id), ((usize::MAX, n % 5, n), (n, usize::MAX, 0)))
        });
        planning.push(if n.is_multiple_of(7) {
            TupleChange::retract(id)
        } else {
            TupleChange::put(
                id,
                (usize::MAX, (!n.is_multiple_of(3)).then_some(usize::MAX)),
            )
        });
    }
    compare(triples, |v: &Triple| v.1.is_multiple_of(2), false);
    compare(pairs, |v: &(Triple, Triple)| v.0.1.is_multiple_of(2), true);
    compare(planning, |v: &(usize, Option<usize>)| v.1.is_some(), false);
    let max = Event::new(TupleChange::<(usize, usize), (Triple, Triple)>::put(
        (usize::MAX, usize::MAX),
        ((usize::MAX, 0, 1), (2, 3, usize::MAX)),
    ));
    let retract = max.inline_retraction();
    assert_eq!(
        typed::<(usize, usize), (Triple, Triple)>(&retract),
        &TupleChange::retract((usize::MAX, usize::MAX))
    );
    assert_eq!(
        typed::<(usize, usize), (Triple, Triple)>(&max)
            .value
            .unwrap()
            .0
            .0,
        usize::MAX
    );
    #[derive(Clone)]
    struct Custom(Triple);
    assert!(!inline_shape::<usize, Custom>());
    assert_eq!(Custom((0, 1, 2)).0.2, 2);
    assert!(!inline_shape::<u64, Triple>());
    assert!(!inline_shape::<usize, String>());
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
    StreamSource<usize, Triple>,
    Vec<usize>,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Triple>();
    let first = b
        .filter(source.port(), |r| {
            trace.borrow_mut().push(("first", r.0, r.1));
            assert_ne!(fail.get(), 1, "predicate panic");
            r.1.is_multiple_of(2)
        })
        .unwrap();
    let second = b
        .filter(&first, |r| {
            trace.borrow_mut().push(("second", r.0, r.1));
            r.2 > 0
        })
        .unwrap();
    b.score("direct", &first, |r| {
        trace.borrow_mut().push(("direct", r.0, r.1));
        SimpleScore::new(r.2 as i64)
    })
    .unwrap();
    b.score("second", &second, |r| {
        trace.borrow_mut().push(("score", r.0, r.1));
        assert_ne!(fail.get(), 2, "score panic");
        match fail.get() {
            3 => SimpleScore::new(0).with_structural(1),
            4 => SimpleScore::new(i64::MAX),
            _ => SimpleScore::new(r.2 as i64),
        }
    })
    .unwrap();
    let joined = b
        .join_equal(&first, source.port(), |r| r.1, |r| r.1)
        .unwrap();
    let pair = b
        .filter(&joined, |r| {
            trace.borrow_mut().push(("pair", r.0.0, r.1.0));
            r.0.0 != r.1.0
        })
        .unwrap();
    b.score("pair", &pair, |r| {
        trace.borrow_mut().push(("pair score", r.0.0, r.1.0));
        SimpleScore::new(1)
    })
    .unwrap();
    (b.build(), source, vec![first.node, second.node, pair.node])
}
#[test]
fn inline_forward_consumed_graph_fifo_fanout_captures_nested_rollback_errors_and_shared_fallback_match_original()
 {
    fn run(enabled: bool, wide: bool, shared: bool, phase: usize) -> Run {
        event_queue::with_wide(wide, || {
            event::with_forced_shared(shared, || {
                let trace = Trace::default();
                let failure = Cell::new(0);
                let (mut g, source, filters) = graph(&trace, &failure);
                assert!(g.try_enable_consuming_search(source.node_id(), 4));
                {
                    let mut tx = g.transaction();
                    for id in 0usize..4 {
                        tx.apply(&source, TupleChange::put(id, (id, id % 2, id + 1)))
                            .unwrap();
                    }
                    tx.commit().unwrap();
                }
                g.bind_fixed_planning_source(&source, 4).unwrap();
                assert!(filters.iter().all(|&id| g.inline_forward_nodes[id]));
                if !enabled {
                    g.inline_forward_nodes.fill(false);
                }
                let full = g.capture_full();
                let base = g.score();
                let mut scores = vec![base];
                let mut work = Vec::new();
                trace.borrow_mut().clear();
                let captured = {
                    let mut outer = g.transaction();
                    outer
                        .apply(&source, TupleChange::put(0usize, (0usize, 1usize, 7usize)))
                        .unwrap();
                    {
                        let mut child = outer.graph.transaction();
                        child.apply(&source, TupleChange::retract(2usize)).unwrap();
                        child
                            .apply(&source, TupleChange::put(1usize, (1usize, 0usize, 9usize)))
                            .unwrap();
                        child.commit().unwrap();
                    }
                    scores.push(outer.score().unwrap());
                    work.push(outer.score_evaluations());
                    outer.capture().unwrap()
                };
                assert_eq!(g.score(), base);
                g.install(captured);
                scores.push(g.score());
                g.install(full);
                assert_eq!(g.score(), base);
                failure.set(phase);
                let outcome = {
                    let mut tx = g.transaction();
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        tx.apply(&source, TupleChange::put(0usize, (10usize, 0usize, 8usize)))
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
                assert!(g.propagation_workspace.arena_pending.is_empty());
                {
                    let mut tx = g.transaction();
                    tx.apply(&source, TupleChange::put(0usize, (10usize, 0usize, 8usize)))
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
#[test]
fn inline_forward_public_graph_keeps_retained_canonical_match_owners() {
    let trace = Trace::default();
    let failure = Cell::new(0);
    let (mut g, source, filters) = graph(&trace, &failure);
    {
        let mut tx = g.transaction();
        for id in 0usize..4 {
            tx.apply(&source, TupleChange::put(id, (id, 0usize, id + 1)))
                .unwrap();
        }
        tx.commit().unwrap();
    }
    g.bind_fixed_planning_source(&source, 4).unwrap();
    assert!(filters.iter().all(|&id| g.inline_forward_nodes[id]));
    assert!(!g.consuming_search);
    let terminal = g.constraints["direct"];
    type Matches = SnapshotMap<usize, (Triple, SimpleScore)>;
    let baseline = g.nodes[terminal]
        .matches()
        .unwrap()
        .downcast_ref::<Matches>()
        .unwrap()
        .clone();
    let owners = baseline.shared_entries();
    let before = owners.iter().find(|(k, _)| **k == 0).unwrap().1.clone();
    let weak = Arc::downgrade(&before);
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 1usize)))
            .unwrap();
        let now = tx.graph.nodes[terminal]
            .matches()
            .unwrap()
            .downcast_ref::<Matches>()
            .unwrap();
        let current = now.shared_entries();
        assert!(!Arc::ptr_eq(
            &before,
            &current.iter().find(|(k, _)| **k == 0).unwrap().1
        ));
    }
    let now = g.nodes[terminal]
        .matches()
        .unwrap()
        .downcast_ref::<Matches>()
        .unwrap();
    assert!(Arc::ptr_eq(
        &before,
        &now.shared_entries()
            .iter()
            .find(|(k, _)| **k == 0)
            .unwrap()
            .1
    ));
    assert!(weak.upgrade().is_some());
}
