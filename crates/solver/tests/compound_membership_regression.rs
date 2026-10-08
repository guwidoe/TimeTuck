//! Original compound collection join semantics, indexing and rollback tests.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
struct Row {
    group: i32,
    time: i32,
    capacity: i32,
    tag: Option<i32>,
    keys: Vec<Option<i32>>,
    value: i64,
}
fn rules() -> JoinConditions<'static, Row, Row> {
    let g = JoinProjection::new(|r: &Row| r.group);
    let t = JoinProjection::new(|r: &Row| r.time);
    let c = JoinProjection::new(|r: &Row| r.capacity);
    JoinConditions::new([
        JoinCondition::new(JoinComparison::LessThanOrEqual, &t, &t),
        JoinCondition::new(JoinComparison::Equal, &g, &g),
        JoinCondition::new(JoinComparison::GreaterThanOrEqual, &c, &c),
    ])
}
fn expected(
    left: &BTreeMap<usize, Row>,
    right: &BTreeMap<usize, Row>,
    mode: usize,
) -> BTreeMap<(usize, usize), (Row, Row)> {
    let mut out = BTreeMap::new();
    for (&i, a) in left {
        for (&j, b) in right {
            let member = match mode {
                0 => a.keys.iter().any(|k| b.keys.contains(k)),
                1 => a.keys.contains(&b.tag),
                _ => b.keys.contains(&a.tag),
            };
            if a.group == b.group && a.time <= b.time && a.capacity >= b.capacity && member {
                out.insert((i, j), (a.clone(), b.clone()));
            }
        }
    }
    out
}
fn score(rows: &BTreeMap<(usize, usize), (Row, Row)>) -> SimpleScore {
    SimpleScore::new(rows.values().map(|(a, b)| a.value + b.value).sum())
}
#[test]
fn all_collection_directions_match_brute_force_through_replacement_retraction_and_rollback() {
    for mode in 0..3 {
        for prune in [false, true] {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if prune {
                b = b.with_activity_pruning();
            }
            let l = b.source::<usize, Row>();
            let r = b.source::<usize, Row>();
            let rules = rules();
            let joined = match mode {
                0 => rules.apply_containing_any_of(
                    &mut b,
                    l.port(),
                    r.port(),
                    |v| v.keys.clone(),
                    |v| v.keys.clone(),
                ),
                1 => rules.apply_containing(
                    &mut b,
                    l.port(),
                    r.port(),
                    |v| v.keys.clone(),
                    |v| v.tag,
                ),
                _ => rules.apply_contained_in(
                    &mut b,
                    l.port(),
                    r.port(),
                    |v| v.tag,
                    |v| v.keys.clone(),
                ),
            }
            .unwrap();
            let data = b.dataset(&joined).unwrap();
            b.score("pairs", &joined, |(a, b)| {
                SimpleScore::new(a.value + b.value)
            })
            .unwrap();
            let mut g = b.build();
            let mut left = BTreeMap::new();
            let mut right = BTreeMap::new();
            let mut saw_match = false;
            for n in 0..90usize {
                let mut nl = left.clone();
                let mut nr = right.clone();
                let id = n % 5;
                let tag = (n / 4 % 3 != 0).then_some((n / 7 % 4) as i32);
                let row = Row {
                    group: (n % 2) as i32,
                    time: (n % 6) as i32,
                    capacity: (n % 7) as i32,
                    tag,
                    keys: if n % 8 == 0 {
                        vec![]
                    } else {
                        vec![tag, None, tag]
                    },
                    value: n as i64,
                };
                let side = if n % 3 == 0 { &mut nl } else { &mut nr };
                let change = if n % 7 == 0 {
                    side.remove(&id);
                    TupleChange::retract(id)
                } else {
                    side.insert(id, row.clone());
                    TupleChange::put(id, row)
                };
                let wanted = expected(&nl, &nr, mode);
                saw_match |= !wanted.is_empty();
                let mut tx = g.transaction();
                tx.apply(if n % 3 == 0 { &l } else { &r }, change).unwrap();
                assert_eq!(tx.score().unwrap(), score(&wanted));
                if n % 4 != 0 {
                    tx.commit().unwrap();
                    left = nl;
                    right = nr;
                } else {
                    drop(tx);
                }
                let wanted = expected(&left, &right, mode);
                assert_eq!(
                    g.dataset_rows(&data)
                        .unwrap()
                        .iter()
                        .map(|(id, pair)| (*id, pair.clone()))
                        .collect::<BTreeMap<_, _>>(),
                    wanted
                );
                assert_eq!(g.score(), score(&wanted));
            }
            assert!(saw_match, "fixture must exercise actual joins");
        }
    }
}
#[test]
fn equalities_and_shared_keys_prune_residual_comparison_work() {
    type V = (i32, Vec<i32>, i32);
    let residual_calls = Cell::new(0);
    let group = JoinProjection::new(|r: &V| r.0);
    let time = JoinProjection::new(|r: &V| {
        residual_calls.set(residual_calls.get() + 1);
        r.2
    });
    let rules = JoinConditions::new([
        JoinCondition::new(JoinComparison::LessThan, &time, &time),
        JoinCondition::new(JoinComparison::Equal, &group, &group),
    ]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, V>();
    let r = b.source::<usize, V>();
    let pairs = rules
        .apply_containing_any_of(&mut b, l.port(), r.port(), |v| v.1.clone(), |v| v.1.clone())
        .unwrap();
    b.score("pairs", &pairs, |_| SimpleScore::new(1)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, (0, vec![1, 1, 2], 0)))
        .unwrap();
    for i in 0..40 {
        tx.apply(
            &r,
            TupleChange::put(i, (if i == 39 { 0 } else { 1 }, vec![1, 2, 2], 1)),
        )
        .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(1));
    assert_eq!(residual_calls.get(), 2);
    residual_calls.set(0);
    let mut tx = g.transaction();
    tx.apply(&r, TupleChange::put(39, (0, vec![3], 1))).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert_eq!(residual_calls.get(), 0);
}
#[test]
fn callback_panics_restore_graph_and_foreign_ports_are_rejected_before_mapping() {
    type V = (i32, Vec<i32>);
    let fail = Cell::new(false);
    let calls = Cell::new(0);
    let group = JoinProjection::new(|v: &V| v.0);
    let rules = JoinConditions::new([JoinCondition::new(JoinComparison::Equal, &group, &group)]);
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let fs = foreign.source::<usize, V>();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, V>();
    let r = b.source::<usize, V>();
    assert!(
        rules
            .apply_containing_any_of(
                &mut b,
                l.port(),
                fs.port(),
                |_| panic!("not invoked"),
                |_: &V| Vec::<i32>::new()
            )
            .is_err()
    );
    let pairs = rules
        .apply_containing_any_of(
            &mut b,
            l.port(),
            r.port(),
            |v| {
                calls.set(calls.get() + 1);
                assert!(!fail.get());
                v.1.clone()
            },
            |v| v.1.clone(),
        )
        .unwrap();
    let data = b.dataset(&pairs).unwrap();
    b.score("pairs", &pairs, |_| SimpleScore::new(1)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, (0, vec![1]))).unwrap();
    tx.apply(&r, TupleChange::put(0, (0, vec![1]))).unwrap();
    tx.commit().unwrap();
    let before = g.dataset_rows(&data).unwrap().clone();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&l, TupleChange::put(0, (1, vec![2]))).unwrap();
            tx.commit().unwrap();
        }))
        .is_err()
    );
    assert_eq!(*g.dataset_rows(&data).unwrap(), before);
    assert_eq!(g.score(), SimpleScore::new(1));
    let count = calls.get();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(calls.get(), count);
    assert_eq!(g.score(), SimpleScore::ZERO);
    fail.set(false);
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, (0, vec![1]))).unwrap();
    tx.commit().unwrap();
    assert_eq!(*g.dataset_rows(&data).unwrap(), before);
}
