//! Original initialization delivery, pruning, rollback and native-director integration.
use std::cell::RefCell;
use timetuck::*;
#[test]
fn notifications_follow_producer_order_include_disabled_edges_and_do_not_score_or_replay() {
    let notifications = RefCell::new(Vec::new());
    let calls = std::cell::Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
    let l = b.source::<usize, i64>();
    let r = b.source::<usize, i64>();
    let mapped = b
        .map(l.port(), |v| {
            calls.set(calls.get() + 1);
            *v
        })
        .unwrap();
    let joined = b.join_equal(&mapped, r.port(), |_| (), |_| ()).unwrap();
    b.score_weighted("disabled", &joined, SimpleScore::ZERO, |_| 1)
        .unwrap();
    b.after_initial_facts(&joined, |can| {
        notifications.borrow_mut().push(("join", can))
    })
    .unwrap();
    b.after_initial_facts(&mapped, |can| notifications.borrow_mut().push(("map", can)))
        .unwrap();
    b.after_initial_facts(l.port(), |can| {
        notifications.borrow_mut().push(("source", can))
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, 2)).unwrap();
    tx.apply(&r, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    let before = calls.get();
    assert_eq!(g.finish_initial_facts(), 3);
    assert_eq!(
        *notifications.borrow(),
        vec![("source", true), ("map", true), ("join", true)]
    );
    assert_eq!(calls.get(), before);
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert!(!g.activity().get(&joined).unwrap().active);
    let mut tx = g.transaction();
    tx.apply(&r, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.finish_initial_facts(), 0);
    assert_eq!(notifications.borrow().len(), 3);
}
#[test]
fn rolled_back_initial_rows_never_produce_a_successful_capability_and_foreign_ports_reject() {
    let notifications = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let foreign = StreamGraphBuilder::<SimpleScore>::new().source::<usize, i64>();
    assert!(matches!(
        b.after_initial_facts(foreign.port(), |_| {}),
        Err(GraphError::ForeignPort)
    ));
    b.after_initial_facts(source.port(), |can| notifications.borrow_mut().push(can))
        .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 1)).unwrap();
    }
    assert_eq!(g.finish_initial_facts(), 1);
    assert_eq!(*notifications.borrow(), vec![false]);
    assert!(g.source_rows(&source).unwrap().is_empty());
}
#[test]
fn basic_director_finishes_initial_facts_after_all_planning_rows_are_bound() {
    let notifications = RefCell::new(Vec::new());
    let p = FiniteProblem::new(vec![Variable::new(vec![1, 2]), Variable::new(vec![1, 2])]).unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<i32>)>();
    let mapped = b.map(source.port(), |(_, v)| v.unwrap_or(0)).unwrap();
    b.score("sum", &mapped, |v| SimpleScore::new(i64::from(*v)))
        .unwrap();
    b.after_initial_facts(&mapped, |can| notifications.borrow_mut().push(can))
        .unwrap();
    let mut director =
        StreamScoreDirector::new(&p, b.build(), source, p.initial_assignment()).unwrap();
    assert_eq!(*notifications.borrow(), vec![true]);
    director
        .apply(&Move::Change {
            variable: 0,
            value: Some(2),
        })
        .unwrap()
        .commit();
    assert_eq!(notifications.borrow().len(), 1);
    director.assert_consistent().unwrap();
}

#[test]
fn list_and_mixed_directors_complete_only_after_all_sources_are_bound() {
    let p = ListProblem::new(vec![10, 20], vec![ListEntity::new(vec![10])]).unwrap();
    let notifications = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = ListStreamSources::new(&mut b);
    let joined = b
        .join_equal(
            sources.entities.port(),
            sources.values.port(),
            |_| (),
            |_| (),
        )
        .unwrap();
    b.score("pairs", &joined, |_| SimpleScore::new(1)).unwrap();
    b.after_initial_facts(&joined, |can| notifications.borrow_mut().push(can))
        .unwrap();
    let mut d =
        ListStreamScoreDirector::new(&p, b.build(), sources, p.initial_assignment()).unwrap();
    assert_eq!(*notifications.borrow(), vec![true]);
    d.apply(&ListMove::Assign {
        value: 20,
        destination: ListPosition {
            entity: 0,
            index: 1,
        },
    })
    .unwrap()
    .commit();
    d.assert_consistent().unwrap();
    assert_eq!(notifications.borrow().len(), 1);
    let basic = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
    let mixed = MixedProblem::new(basic, p);
    let notifications = RefCell::new(Vec::new());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let sources = MixedStreamSources::new(&mut b);
    let joined = b
        .join_equal(
            sources.basic.port(),
            sources.lists.values.port(),
            |_| (),
            |_| (),
        )
        .unwrap();
    b.score("pairs", &joined, |_| SimpleScore::new(1)).unwrap();
    b.after_initial_facts(&joined, |can| notifications.borrow_mut().push(can))
        .unwrap();
    let mut d =
        MixedStreamScoreDirector::new(&mixed, b.build(), sources, mixed.initial_assignment())
            .unwrap();
    assert_eq!(*notifications.borrow(), vec![true]);
    d.assert_consistent().unwrap();
}
#[test]
fn panic_consumes_the_lifecycle_without_changing_retained_facts_or_scores() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let calls = std::cell::Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    b.score("sum", source.port(), |v| SimpleScore::new(*v))
        .unwrap();
    b.after_initial_facts(source.port(), |_| {
        calls.set(calls.get() + 1);
        panic!("observer failed")
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    assert!(catch_unwind(AssertUnwindSafe(|| g.finish_initial_facts())).is_err());
    assert_eq!(g.finish_initial_facts(), 0);
    assert_eq!(calls.get(), 1);
    assert_eq!(g.score(), SimpleScore::new(3));
    assert_eq!(g.source_rows(&source).unwrap()[&0], 3);
    g.assert_consistent().unwrap();
}
#[test]
fn conservative_capabilities_ignore_predicate_and_key_outcomes_in_both_scheduling_modes() {
    for pruning in [false, true] {
        for left in [false, true] {
            for right in [false, true] {
                for passes in [false, true] {
                    for keys_match in [false, true] {
                        let notifications = RefCell::new(Vec::new());
                        let mut b = StreamGraphBuilder::<SimpleScore>::new();
                        if pruning {
                            b = b.with_activity_pruning();
                        }
                        let l = b.source::<usize, i64>();
                        let r = b.source::<usize, i64>();
                        let filtered = b.filter(l.port(), move |_| passes).unwrap();
                        let joined = b
                            .join_equal(&filtered, r.port(), |_| 0, move |_| i32::from(!keys_match))
                            .unwrap();
                        b.score("join", &joined, |_| SimpleScore::new(1)).unwrap();
                        b.after_initial_facts(&joined, |can| notifications.borrow_mut().push(can))
                            .unwrap();
                        let mut g = b.build();
                        let mut tx = g.transaction();
                        if left {
                            tx.apply(&l, TupleChange::put(0, 2)).unwrap();
                        }
                        if right {
                            tx.apply(&r, TupleChange::put(0, 3)).unwrap();
                        }
                        tx.commit().unwrap();
                        let before = g.score();
                        assert_eq!(
                            before,
                            SimpleScore::new(i64::from(left && right && passes && keys_match))
                        );
                        assert_eq!(g.finish_initial_facts(), 1);
                        assert_eq!(*notifications.borrow(), vec![left && right]);
                        assert_eq!(g.score(), before);
                        g.assert_consistent().unwrap();
                    }
                }
            }
        }
    }
}
