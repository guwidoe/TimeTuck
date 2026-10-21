//! Original exact row storage controls; no upstream-method credit.
use super::super::*;
use super::*;
use crate::SimpleScore;
use std::cell::RefCell;
#[test]
fn inline_tallies_full_numeric_fields_event_width_clone_and_promoted_fanout_owners() {
    assert_eq!(std::mem::size_of::<Event>(), 72);
    assert_eq!(std::mem::size_of::<event_queue::QueuedEvent>(), 24);
    for value in [
        None,
        Some((usize::MAX, 0, usize::MAX, 17, i64::MIN)),
        Some((0, 1, 2, 3, i64::MAX)),
    ] {
        let row = TupleChange::<(usize, usize), Tally> {
            id: (usize::MAX, usize::MAX),
            value,
        };
        let event = Event::new(row.clone());
        assert!(matches!(event, Event::Tally(_)));
        assert_eq!(event.downcast_ref(), Some(&row));
        assert_eq!(event.clone().downcast_ref(), Some(&row));
        let promoted = event.promote();
        let clone = promoted.clone();
        let owner = promoted
            .downcast::<TupleChange<(usize, usize), Tally>>()
            .unwrap();
        let second = clone
            .downcast::<TupleChange<(usize, usize), Tally>>()
            .unwrap();
        assert!(Rc::ptr_eq(&owner, &second));
        assert_eq!(*owner, row);
    }
    let quad = TupleChange::<(usize, usize), Quad>::put((usize::MAX, 0), (0, 1, 2, usize::MAX));
    let event = Event::new(quad.clone());
    assert!(matches!(event, Event::Quad(_)));
    assert_eq!(event.clone().downcast_ref(), Some(&quad));
    let promoted = event.promote();
    assert_eq!(
        *promoted
            .downcast::<TupleChange<(usize, usize), Quad>>()
            .unwrap(),
        quad
    );
    #[derive(Clone)]
    struct Custom(Tally);
    let custom = Custom((0, 1, 2, 3, 4));
    assert_eq!(custom.0.4, 4);
    assert!(matches!(
        Event::new(TupleChange::put((0usize, 0usize), custom)),
        Event::Shared(_)
    ));
}
type Trace = RefCell<Vec<(&'static str, usize, i64)>>;
type Run = (Vec<(&'static str, usize, i64)>, Vec<SimpleScore>, Vec<u128>);
fn run(shared: bool, wide: bool) -> Run {
    with_forced_shared(shared, || {
        event_queue::with_wide(wide, || {
            let trace = Trace::default();
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, Triple>();
            let expanded = b
                .flat_map(source.port(), |&(id, key, n)| {
                    trace.borrow_mut().push(("expand", id, n as i64));
                    vec![
                        (key, 0usize, usize::MAX, 0usize, n as i64),
                        (key, 0, usize::MAX, 1, -(n as i64)),
                    ]
                })
                .unwrap();
            let groups = b
                .group_collect(&expanded, |&(a, b, c, d, n)| {
                    trace.borrow_mut().push(("group", a, n));
                    ((a, b, c, d), n)
                })
                .unwrap();
            b.score("count", &groups, |(key, sum)| {
                trace.borrow_mut().push(("score", key.0, sum.sum));
                SimpleScore::new(sum.sum)
            })
            .unwrap();
            let mut g = b.build();
            assert!(g.try_enable_consuming_search(source.node_id(), 2));
            {
                let mut tx = g.transaction();
                tx.apply(&source, TupleChange::put(0usize, (0usize, 0usize, 2usize)))
                    .unwrap();
                tx.apply(&source, TupleChange::put(1usize, (1usize, 1usize, 3usize)))
                    .unwrap();
                tx.commit().unwrap();
            }
            let full = g.capture_full();
            let base = g.score();
            let mut scores = vec![base];
            let mut work = Vec::new();
            trace.borrow_mut().clear();
            let saved = {
                let mut outer = g.transaction();
                outer
                    .apply(&source, TupleChange::put(0usize, (0usize, 1usize, 5usize)))
                    .unwrap();
                {
                    let mut child = outer.graph.transaction();
                    child.apply(&source, TupleChange::retract(1usize)).unwrap();
                    child.commit().unwrap();
                }
                scores.push(outer.score().unwrap());
                work.push(outer.score_evaluations());
                outer.capture().unwrap()
            };
            assert_eq!(g.score(), base);
            g.install(saved);
            scores.push(g.score());
            g.install(full);
            assert_eq!(g.score(), base);
            g.assert_consistent().unwrap();
            drop(g);
            (trace.into_inner(), scores, work)
        })
    })
}
#[test]
fn inline_tallies_actual_flatmap_group_callbacks_captures_and_rollback_match_shared_and_wide() {
    let original = run(true, false);
    assert_eq!(run(false, false), original);
    assert_eq!(run(false, true), original);
}
