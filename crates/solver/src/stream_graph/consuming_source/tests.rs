//! Original consuming-source controls; no upstream source-method credit.
use super::*;
use crate::SimpleScore;
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
type Planning = (usize, Option<usize>);
fn arithmetic() -> NodeArithmetic {
    NodeArithmetic {
        impact: crate::ScoringArithmetic::Checked,
        totals: crate::ScoringArithmetic::Checked,
    }
}
fn event(id: usize, value: Option<Planning>) -> Event {
    Event::new(TupleChange { id, value })
}
fn contents(node: &dyn Node<SimpleScore>) -> Vec<(usize, Planning)> {
    node.source_events()
        .iter()
        .map(|e| {
            let r = typed::<usize, Planning>(e);
            (r.id, r.value.unwrap())
        })
        .collect()
}
fn apply(
    node: &mut dyn Node<SimpleScore>,
    id: usize,
    value: Option<Planning>,
) -> Vec<(usize, Option<Planning>)> {
    node.apply(false, &event(id, value), arithmetic())
        .unwrap()
        .into_iter()
        .map(|e| {
            let r = typed::<usize, Planning>(&e);
            (r.id, r.value)
        })
        .collect()
}
fn copied(count: usize) -> Box<dyn Node<SimpleScore>> {
    owned_source::<usize, Planning, SimpleScore>(&SnapshotMap::new(), count).unwrap()
}

#[test]
fn consuming_source_strict_factory_rejects_custom_clone_drop_ids_and_populated_rows_without_hooks()
{
    thread_local! {static HOOKS:Cell<usize>=const{Cell::new(0)};}
    struct Custom;
    impl Clone for Custom {
        fn clone(&self) -> Self {
            HOOKS.with(|v| v.set(v.get() + 1));
            panic!("custom Clone must not run")
        }
    }
    struct Dropped;
    impl Clone for Dropped {
        fn clone(&self) -> Self {
            panic!("custom clone")
        }
    }
    impl Drop for Dropped {
        fn drop(&mut self) {
            HOOKS.with(|v| v.set(v.get() + 1));
        }
    }
    #[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
    struct Id(usize);
    assert!(!std::mem::needs_drop::<Custom>());
    assert!(owned_source::<usize, Custom, SimpleScore>(&SnapshotMap::new(), 573).is_none());
    assert!(owned_source::<usize, Dropped, SimpleScore>(&SnapshotMap::new(), 573).is_none());
    assert!(owned_source::<Id, Planning, SimpleScore>(&SnapshotMap::new(), 573).is_none());
    assert!(owned_source::<u32, Planning, SimpleScore>(&SnapshotMap::new(), 573).is_none());
    let rows = SnapshotMap::from_sorted([(0usize, (0usize, Some(1usize)))].into_iter());
    let held = rows.shared_entries();
    assert!(owned_source::<usize, Planning, SimpleScore>(&rows, 573).is_none());
    assert!(Arc::ptr_eq(&held[0].1, &rows.shared_entries()[0].1));
    assert_eq!(HOOKS.with(Cell::get), 0);
    assert!(copier::<Planning>().is_some());
    assert!(copier::<((usize, usize, usize), (usize, usize, usize))>().is_some());
}
#[test]
fn consuming_source_573_rows_nested_current_full_and_outer_installs_match_original_source() {
    let mut fast = copied(573);
    let mut old: Box<dyn Node<SimpleScore>> =
        Box::new(SourceNode::<usize, Planning>(SnapshotMap::new()));
    for id in 0..573 {
        assert_eq!(
            apply(fast.as_mut(), id, Some((id, Some(id % 11)))),
            apply(old.as_mut(), id, Some((id, Some(id % 11))))
        );
    }
    assert_eq!(contents(fast.as_ref()), contents(old.as_ref()));
    let full = fast.checkpoint();
    let old_full = old.checkpoint();
    let outer = fast.begin_checkpoint();
    let old_outer = old.begin_checkpoint();
    let child = fast.begin_checkpoint();
    let old_child = old.begin_checkpoint();
    for (id, v) in [
        (0, Some((0, None))),
        (572, None),
        (65, Some((65, Some(usize::MAX)))),
    ] {
        assert_eq!(apply(fast.as_mut(), id, v), apply(old.as_mut(), id, v));
    }
    let current = fast.capture_checkpoint(outer.as_ref());
    let old_current = old.capture_checkpoint(old_outer.as_ref());
    fast.commit_checkpoint(child);
    old.commit_checkpoint(old_child);
    let wanted = contents(fast.as_ref());
    assert_eq!(wanted, contents(old.as_ref()));
    fast.restore(outer);
    old.restore(old_outer);
    let install = fast.begin_checkpoint();
    let old_install = old.begin_checkpoint();
    fast.restore(current);
    old.restore(old_current);
    assert_eq!(contents(fast.as_ref()), wanted);
    fast.clear();
    old.clear();
    fast.restore(full);
    old.restore(old_full);
    assert_eq!(contents(fast.as_ref()), contents(old.as_ref()));
    fast.restore(install);
    old.restore(old_install);
    assert_eq!(contents(fast.as_ref()), contents(old.as_ref()));
    assert_eq!(fast.source_count(), Some(573));
    let empty = copied(0);
    assert_eq!(empty.source_count(), Some(0));
    assert!(empty.source_events().is_empty());
}
#[test]
fn consuming_source_snapshots_and_current_captures_never_materialize_or_promote_rows() {
    type N = Source<Planning>;
    let mut n = N::new(573, copier::<Planning>().unwrap());
    n.update(&event(572, Some((572, Some(usize::MAX)))))
        .unwrap();
    let outer = <N as Node<SimpleScore>>::begin_checkpoint(&mut n);
    n.update(&event(572, Some((572, None)))).unwrap();
    let full = <N as Node<SimpleScore>>::checkpoint(&n);
    let saved = <N as Node<SimpleScore>>::capture_checkpoint(&n, outer.as_ref());
    assert!(
        n.rows.get().is_none(),
        "copy-only captures must not promote a row owner"
    );
    <N as Node<SimpleScore>>::restore(&mut n, outer);
    assert_eq!(n.values[572], Some((572, Some(usize::MAX))));
    let outer = <N as Node<SimpleScore>>::begin_checkpoint(&mut n);
    <N as Node<SimpleScore>>::restore(&mut n, saved);
    assert_eq!(n.values[572], Some((572, None)));
    <N as Node<SimpleScore>>::clear(&mut n);
    <N as Node<SimpleScore>>::restore(&mut n, full);
    assert!(n.rows.get().is_none());
    assert_eq!(n.values[572], Some((572, None)));
    <N as Node<SimpleScore>>::restore(&mut n, outer);
    assert_eq!(n.values[572], Some((572, Some(usize::MAX))));
    let held = n.rows().clone();
    n.update(&event(572, None)).unwrap();
    assert!(n.rows.get().is_none());
    assert_eq!(held[&572], (572, Some(usize::MAX)));
}
#[test]
fn consuming_source_sorted_initialization_and_last_write_batch_order_match_original() {
    let mut fast = copied(573);
    let mut old: Box<dyn Node<SimpleScore>> =
        Box::new(SourceNode::<usize, Planning>(SnapshotMap::new()));
    let events: Vec<_> = (0..573)
        .map(|id| (0, event(id, Some((id, Some(id % 11))))))
        .collect();
    let outer = fast.begin_checkpoint();
    let old_outer = old.begin_checkpoint();
    assert!(fast.initialize_sorted_source(&events));
    assert!(old.initialize_sorted_source(&events));
    let inputs = vec![
        (false, event(65, Some((65, None)))),
        (false, event(0, None)),
        (false, event(65, Some((65, Some(4))))),
        (false, event(572, Some((572, None)))),
    ];
    fn changes(rows: BatchOutputs) -> Vec<(usize, Option<Planning>)> {
        rows.into_iter()
            .map(|e| {
                let r = typed::<usize, Planning>(&e);
                (r.id, r.value)
            })
            .collect()
    }
    assert_eq!(
        changes(fast.apply_batch(&inputs, arithmetic()).unwrap()),
        changes(old.apply_batch(&inputs, arithmetic()).unwrap())
    );
    assert_eq!(contents(fast.as_ref()), contents(old.as_ref()));
    fast.restore(outer);
    old.restore(old_outer);
    assert!(contents(fast.as_ref()).is_empty());
    assert_eq!(contents(fast.as_ref()), contents(old.as_ref()));
    assert!(!fast.initialize_sorted_source(&[
        (0, event(1, Some((1, None)))),
        (0, event(0, Some((0, None))))
    ]));
    assert_eq!(fast.source_count(), Some(0));
}
#[test]
fn consuming_source_bound_failure_strict_nesting_and_recovery_preserve_current_rows() {
    let mut fast = copied(3);
    let mut fixed: Box<dyn Node<SimpleScore>> = Box::new(fixed_source::FixedSourceNode::new(
        (0..3)
            .map(|id| (Arc::new(id), Arc::new((id, Some(id)))))
            .collect(),
    ));
    for id in 0..3 {
        apply(fast.as_mut(), id, Some((id, Some(id))));
    }
    let outer = fast.begin_checkpoint();
    let fixed_outer = fixed.begin_checkpoint();
    let bad = event(usize::MAX, Some((0, None)));
    assert_eq!(fast.apply_source(&bad), fixed.apply_source(&bad));
    fast.restore(outer);
    fixed.restore(fixed_outer);
    assert_eq!(contents(fast.as_ref()), contents(fixed.as_ref()));
    let mut n = Source::new(3, copier::<Planning>().unwrap());
    let outer = n.frames.len();
    n.frames.push(Vec::new());
    n.frames.push(Vec::new());
    assert!(catch_unwind(AssertUnwindSafe(|| n.restore_frame(outer))).is_err());
    n.restore_frame(1);
    n.restore_frame(0);
    assert_eq!(n.len, 0);
}

type Trace = Vec<(&'static str, usize)>;
type Observation = (Trace, Vec<u128>, SimpleScore, Vec<(usize, Planning)>);
fn graph_trace(copy: bool) -> Observation {
    let trace = RefCell::new(Vec::new());
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Planning>();
    let present = b
        .filter(source.port(), |row| {
            trace.borrow_mut().push(("filter", row.0));
            row.1.is_some()
        })
        .unwrap();
    let constraint = b
        .score("sum", &present, |row| {
            trace.borrow_mut().push(("score", row.0));
            assert!(!fail.get(), "score panic");
            SimpleScore::new(row.1.unwrap() as i64)
        })
        .unwrap();
    if copy {
        b.nodes[source.0.node] = copied(573);
    }
    let mut graph = b.build();
    {
        let mut tx = graph.transaction();
        for id in 0..573 {
            tx.apply(&source, TupleChange::put(id, (id, Some(id % 11))))
                .unwrap();
        }
        tx.commit().unwrap();
    }
    graph.bind_fixed_planning_source(&source, 573).unwrap();
    trace.borrow_mut().clear();
    let base = graph.capture_full();
    let base_rows = graph.source_rows(&source).unwrap().clone();
    let baseline = graph.constraint_matches(&constraint).unwrap().clone();
    let before = baseline.shared_entries();
    let mut counts = Vec::new();
    let (candidate, saved) = {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(572, (572, Some(0))))
            .unwrap();
        counts.push(tx.score_evaluations());
        (
            tx.graph.constraint_matches(&constraint).unwrap().clone(),
            tx.capture().unwrap(),
        )
    };
    assert_eq!(graph.source_rows(&source).unwrap(), &base_rows);
    assert!(Arc::ptr_eq(
        &before[572].1,
        &graph
            .constraint_matches(&constraint)
            .unwrap()
            .shared_entries()[572]
            .1
    ));
    assert!(!Arc::ptr_eq(
        &before[572].1,
        &candidate.shared_entries()[572].1
    ));
    let calls = trace.borrow().len();
    graph.install(saved);
    assert_eq!(trace.borrow().len(), calls);
    assert!(Arc::ptr_eq(
        &candidate.shared_entries()[572].1,
        &graph
            .constraint_matches(&constraint)
            .unwrap()
            .shared_entries()[572]
            .1
    ));
    graph.install(base);
    fail.set(true);
    {
        let mut tx = graph.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&source, TupleChange::put(572, (572, Some(1))))
            ))
            .is_err()
        );
        counts.push(tx.score_evaluations());
    }
    fail.set(false);
    assert_eq!(graph.source_rows(&source).unwrap(), &base_rows);
    graph.assert_consistent().unwrap();
    let score = graph.score();
    let rows = graph
        .source_rows(&source)
        .unwrap()
        .iter()
        .map(|(id, v)| (*id, *v))
        .collect();
    drop(graph);
    (trace.into_inner(), counts, score, rows)
}
#[test]
fn consuming_source_graph_573_callback_work_replay_panic_and_downstream_public_owners_match_original()
 {
    assert_eq!(graph_trace(false), graph_trace(true));
}
