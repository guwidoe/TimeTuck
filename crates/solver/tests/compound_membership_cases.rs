// Adapted from Timefold Community JoinerTypeTest. Apache-2.0.
// Copyright Timefold BV and contributors. See compat/provenance.json.
use timetuck::*;
#[test]
fn contain() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, Vec<i32>>();
    let r = b.source::<usize, i32>();
    let out = JoinConditions::new([])
        .apply_containing(&mut b, l.port(), r.port(), Clone::clone, |v| *v)
        .unwrap();
    let data = b.dataset(&out).unwrap();
    let mut g = b.build();
    for (value, expected) in [(1, true), (2, false)] {
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, vec![1, 3])).unwrap();
        tx.apply(&r, TupleChange::put(0, value)).unwrap();
        tx.commit().unwrap();
        assert_eq!(!g.dataset_rows(&data).unwrap().is_empty(), expected);
    }
}
#[test]
fn contained_in() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, i32>();
    let r = b.source::<usize, Vec<i32>>();
    let out = JoinConditions::new([])
        .apply_contained_in(&mut b, l.port(), r.port(), |v| *v, Clone::clone)
        .unwrap();
    let data = b.dataset(&out).unwrap();
    let mut g = b.build();
    for (value, expected) in [(1, true), (2, false)] {
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, value)).unwrap();
        tx.apply(&r, TupleChange::put(0, vec![1, 3])).unwrap();
        tx.commit().unwrap();
        assert_eq!(!g.dataset_rows(&data).unwrap().is_empty(), expected);
    }
}
#[test]
fn contains_any() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, Vec<i32>>();
    let r = b.source::<usize, Vec<i32>>();
    let out = JoinConditions::new([])
        .apply_containing_any_of(&mut b, l.port(), r.port(), Clone::clone, Clone::clone)
        .unwrap();
    let data = b.dataset(&out).unwrap();
    let mut g = b.build();
    for (left, right, expected) in [
        (vec![1, 2, 3], vec![2], true),
        (vec![1, 2, 3], vec![6], false),
        (vec![1, 2, 3], vec![3, 4, 5], true),
        (vec![3, 4, 5], vec![1, 2, 3], true),
        (vec![1, 2, 3], vec![4, 5, 6], false),
        (vec![1, 2, 3], vec![], false),
        (vec![], vec![1], false),
        (vec![], vec![], false),
    ] {
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, left)).unwrap();
        tx.apply(&r, TupleChange::put(0, right)).unwrap();
        tx.commit().unwrap();
        assert_eq!(!g.dataset_rows(&data).unwrap().is_empty(), expected);
    }
}
