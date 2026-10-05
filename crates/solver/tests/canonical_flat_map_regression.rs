use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[derive(Debug, PartialEq, Eq)]
struct Item {
    key: i32,
    data: i64,
}
fn item(key: i32, data: i64) -> Rc<Item> {
    Rc::new(Item { key, data })
}
#[test]
fn first_representatives_preserve_occurrences_and_are_local_to_each_expansion() {
    let calls = Cell::new(0);
    let mut n = FlatMapNode::new_canonical(
        |v: &Vec<Rc<Item>>| {
            calls.set(calls.get() + 1);
            v.clone()
        },
        |v| v.key,
    );
    let a = item(1, 10);
    let equal = item(1, 90);
    let b = item(2, 20);
    let rows = n.apply(TupleChange::put(
        0,
        vec![a.clone(), b.clone(), equal.clone()],
    ));
    assert_eq!(rows.len(), 3);
    assert!(Rc::ptr_eq(rows[2].value.as_ref().unwrap(), &a));
    let other = n.apply(TupleChange::put(1, vec![equal.clone(), a.clone()]));
    assert!(Rc::ptr_eq(other[1].value.as_ref().unwrap(), &equal));
    let reordered = n.apply(TupleChange::put(0, vec![equal.clone(), a]));
    assert_eq!(reordered[0], TupleChange::retract((0, 2)));
    assert!(Rc::ptr_eq(reordered[2].value.as_ref().unwrap(), &equal));
    assert_eq!(n.apply(TupleChange::retract(0)).len(), 2);
    assert!(n.apply(TupleChange::retract(0)).is_empty());
    assert_eq!(calls.get(), 3);
}
#[test]
fn key_panic_does_not_change_cached_occurrence_lengths() {
    let mut n = FlatMapNode::new_canonical(
        |v: &Vec<i32>| v.clone(),
        |v| {
            assert_ne!(*v, -1);
            *v
        },
    );
    n.apply(TupleChange::put(0, vec![1, 1, 2]));
    assert!(
        catch_unwind(AssertUnwindSafe(
            || n.apply(TupleChange::put(0, vec![3, -1]))
        ))
        .is_err()
    );
    assert_eq!(
        n.apply(TupleChange::retract(0)),
        vec![
            TupleChange::retract((0, 0)),
            TupleChange::retract((0, 1)),
            TupleChange::retract((0, 2))
        ]
    );
}
#[test]
fn incremental_graph_identity_scores_and_rollback_match_first_occurrence_model() {
    for prune in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if prune {
            b = b.with_activity_pruning();
        }
        let s = b.source::<usize, Vec<Rc<Item>>>();
        let out = b
            .flat_map_canonical(s.port(), Clone::clone, |v| v.key)
            .unwrap();
        let data = b.dataset(&out).unwrap();
        b.score("sum", &out, |v| SimpleScore::new(v.data)).unwrap();
        let mut g = b.build();
        let mut model = BTreeMap::<usize, Vec<Rc<Item>>>::new();
        let check = |g: &StreamGraph<'_, SimpleScore>, m: &BTreeMap<usize, Vec<Rc<Item>>>| {
            let rows = g.dataset_rows(&data).unwrap();
            let mut expected_score = 0;
            assert_eq!(rows.len(), m.values().map(Vec::len).sum::<usize>());
            for (&id, values) in m {
                for (pos, value) in values.iter().enumerate() {
                    let first = values.iter().find(|v| v.key == value.key).unwrap();
                    assert!(Rc::ptr_eq(&rows[&(id, pos)], first));
                    expected_score += first.data;
                }
            }
            assert_eq!(g.score(), SimpleScore::new(expected_score));
        };
        for n in 0..160 {
            let mut next = model.clone();
            let mut tx = g.transaction();
            if n % 20 == 0 {
                for id in next.keys() {
                    tx.apply(&s, TupleChange::retract(*id)).unwrap();
                }
                next.clear();
            } else {
                let id = n % 4;
                let values: Vec<_> = (0..n % 7)
                    .map(|i| item((i % 3) as i32, (n + i) as i64))
                    .collect();
                tx.apply(&s, TupleChange::put(id, values.clone())).unwrap();
                next.insert(id, values);
            }
            check(tx.graph().unwrap(), &next);
            if n % 5 == 0 {
                drop(tx);
            } else {
                tx.commit().unwrap();
                model = next;
            }
            check(&g, &model);
        }
        g.assert_consistent().unwrap();
    }
}
#[test]
fn graph_overflow_rolls_back_canonical_rows_and_source_updates() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, Vec<Rc<Item>>>();
    let out = b
        .flat_map_canonical(s.port(), Clone::clone, |v| v.key)
        .unwrap();
    let data = b.dataset(&out).unwrap();
    b.score("sum", &out, |v| SimpleScore::new(v.data)).unwrap();
    let mut g = b.build();
    let original = item(1, 3);
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, vec![original.clone()]))
        .unwrap();
    tx.commit().unwrap();
    {
        let mut tx = g.transaction();
        assert!(
            tx.apply(&s, TupleChange::put(0, vec![item(1, i64::MAX), item(1, 1)]))
                .is_err()
        );
    }
    assert_eq!(g.score(), SimpleScore::new(3));
    assert!(Rc::ptr_eq(
        &g.dataset_rows(&data).unwrap()[&(0, 0)],
        &original
    ));
    g.assert_consistent().unwrap();
}
