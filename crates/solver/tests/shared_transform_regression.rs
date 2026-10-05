//! Original runtime checks for explicit transform sharing.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn shared_callbacks_run_once_per_change_across_multiple_constraints() {
    let filters = Cell::new(0);
    let maps = Cell::new(0);
    let flats = Cell::new(0);
    let filter = SharedFilter::new(|v: &i64| {
        filters.set(filters.get() + 1);
        *v > 0
    });
    let map = SharedMap::new(|v: &i64| {
        maps.set(maps.get() + 1);
        *v * 2
    });
    let flat = SharedFlatMap::new(|v: &i64| {
        flats.set(flats.get() + 1);
        [*v, *v]
    });
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    for name in ["first", "second"] {
        let f = filter.clone().apply(&mut b, source.port()).unwrap();
        let m = map.clone().apply(&mut b, &f).unwrap();
        let out = flat.clone().apply(&mut b, &m).unwrap();
        b.score(name, &out, |v| SimpleScore::new(*v)).unwrap();
    }
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(24));
    assert_eq!((filters.get(), maps.get(), flats.get()), (1, 1, 1));
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, -1)).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::ZERO);
    }
    assert_eq!(g.score(), SimpleScore::new(24));
    assert_eq!((filters.get(), maps.get(), flats.get()), (2, 1, 1));
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert_eq!((filters.get(), maps.get(), flats.get()), (2, 1, 1));
}
#[test]
fn fresh_definitions_and_parents_do_not_share_despite_equal_callback_results() {
    let calls = Cell::new(0);
    let callback = |v: &i32| {
        calls.set(calls.get() + 1);
        *v
    };
    let first = SharedMap::new(callback);
    let second = SharedMap::new(callback);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i32>();
    let filtered = b.filter(s.port(), |_| true).unwrap();
    let a = first.apply(&mut b, s.port()).unwrap();
    let repeated = first.clone().apply(&mut b, s.port()).unwrap();
    let different_definition = second.apply(&mut b, s.port()).unwrap();
    let different_parent = first.apply(&mut b, &filtered).unwrap();
    assert_eq!(b.dataset(&a).unwrap(), b.dataset(&repeated).unwrap());
    assert_ne!(
        b.dataset(&a).unwrap(),
        b.dataset(&different_definition).unwrap()
    );
    assert_ne!(
        b.dataset(&a).unwrap(),
        b.dataset(&different_parent).unwrap()
    );
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 3);
}
#[test]
fn definitions_span_builders_without_cross_graph_aliases_or_retaining_graph_nodes() {
    let token = Rc::new(());
    let weak = Rc::downgrade(&token);
    let shared = SharedMap::new(move |v: &i32| {
        let _ = &token;
        *v
    });
    let mut a = StreamGraphBuilder::<SimpleScore>::new();
    let sa = a.source::<usize, i32>();
    let pa = shared.apply(&mut a, sa.port()).unwrap();
    a.score("a", &pa, |v| SimpleScore::new(i64::from(*v)))
        .unwrap();
    let mut b = StreamGraphBuilder::<HardSoftScore>::new();
    let sb = b.source::<usize, i32>();
    assert!(matches!(
        shared.apply(&mut b, sa.port()),
        Err(GraphError::ForeignPort)
    ));
    let pb = shared.apply(&mut b, sb.port()).unwrap();
    b.score("b", &pb, |v| HardSoftScore::new(0, i64::from(*v)))
        .unwrap();
    let mut ga = a.build();
    let mut gb = b.build();
    drop(shared);
    let mut tx = ga.transaction();
    tx.apply(&sa, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    let mut tx = gb.transaction();
    tx.apply(&sb, TupleChange::put(0, 7)).unwrap();
    tx.commit().unwrap();
    assert_eq!(ga.score(), SimpleScore::new(3));
    assert_eq!(gb.score(), HardSoftScore::new(0, 7));
    assert!(weak.upgrade().is_some());
    drop(ga);
    drop(gb);
    assert!(weak.upgrade().is_none());
}
#[test]
fn failure_in_one_consumer_rolls_back_all_shared_outputs() {
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let shared = SharedMap::new(|v: &i64| {
        calls.set(calls.get() + 1);
        *v * 2
    });
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let a = shared.apply(&mut b, s.port()).unwrap();
    let c = shared.apply(&mut b, s.port()).unwrap();
    b.score("first", &a, |v| SimpleScore::new(*v)).unwrap();
    b.score("second", &c, |v| {
        assert!(!fail.get(), "injected downstream panic");
        SimpleScore::new(*v)
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 3)).unwrap();
    tx.commit().unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::put(0, 5)).unwrap();
        }))
        .is_err()
    );
    assert_eq!(g.score(), SimpleScore::new(12));
    assert_eq!(calls.get(), 2);
    fail.set(false);
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 7)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(28));
    assert_eq!(calls.get(), 3);
    g.assert_consistent().unwrap();
    assert_eq!(calls.get(), 4);
}
#[test]
fn shared_expansion_keeps_duplicate_occurrences_and_restores_shrinking_trials() {
    let calls = Cell::new(0);
    let shared = SharedFlatMap::new(|v: &Vec<i64>| {
        calls.set(calls.get() + 1);
        v.clone()
    });
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Vec<i64>>();
    let a = shared.apply(&mut b, s.port()).unwrap();
    let c = shared.clone().apply(&mut b, s.port()).unwrap();
    let dataset = b.dataset(&a).unwrap();
    b.score("first", &a, |v| SimpleScore::new(*v)).unwrap();
    b.score("second", &c, |v| SimpleScore::new(*v)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, vec![1, 1, 2])).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.dataset_rows(&dataset).unwrap().len(), 3);
    assert_eq!(g.score(), SimpleScore::new(8));
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(0, vec![9])).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(18));
        assert_eq!(tx.graph().unwrap().dataset_rows(&dataset).unwrap().len(), 1);
    }
    assert_eq!(g.score(), SimpleScore::new(8));
    assert_eq!(g.dataset_rows(&dataset).unwrap().len(), 3);
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(g.score(), SimpleScore::ZERO);
}
