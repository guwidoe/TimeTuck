//! Original incremental and lifetime checks; no upstream method credit.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
type Row = (i32, i64);
type Pairs = BTreeMap<(usize, usize), (Row, Row)>;
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
fn matches(c: JoinComparison, a: i32, b: i32) -> bool {
    match c {
        JoinComparison::Equal => a == b,
        JoinComparison::LessThan => a < b,
        JoinComparison::LessThanOrEqual => a <= b,
        JoinComparison::GreaterThan => a > b,
        JoinComparison::GreaterThanOrEqual => a >= b,
    }
}
#[test]
fn shared_ordered_joins_match_full_enumeration_with_single_projection_work() {
    for comparison in [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ] {
        for filtered in [false, true] {
            let lc = Cell::new(0);
            let rc = Cell::new(0);
            let pc = Cell::new(0);
            let definition = SharedJoin::comparing(
                comparison,
                |r: &Row| {
                    lc.set(lc.get() + 1);
                    r.0
                },
                |r: &Row| {
                    rc.set(rc.get() + 1);
                    r.0
                },
            );
            let definition = if filtered {
                definition.with_filter(|a, b| {
                    pc.set(pc.get() + 1);
                    a.1 <= b.1
                })
            } else {
                definition
            };
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let l = b.source::<usize, Row>();
            let r = b.source::<usize, Row>();
            let out = definition.apply(&mut b, l.port(), r.port()).unwrap();
            let again = definition
                .clone()
                .apply(&mut b, l.port(), r.port())
                .unwrap();
            let dataset = b.dataset(&out).unwrap();
            assert_eq!(dataset, b.dataset(&again).unwrap());
            b.score("weighted", &out, |(a, b)| SimpleScore::new(a.1 + b.1))
                .unwrap();
            b.score("count", &again, |_| SimpleScore::new(1)).unwrap();
            let mut g = b.build();
            let mut left = BTreeMap::new();
            let mut right = BTreeMap::new();
            let mut seed = 20261201;
            for step in 0..200 {
                let id = (random(&mut seed) % 8) as usize;
                let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                    (
                        (random(&mut seed) % 4) as i32,
                        (random(&mut seed) % 11) as i64 - 5,
                    )
                });
                let mut next_left = left.clone();
                let mut next_right = right.clone();
                let before = (lc.get(), rc.get(), pc.get());
                let target = if step % 2 == 0 {
                    &mut next_left
                } else {
                    &mut next_right
                };
                if let Some(v) = value {
                    target.insert(id, v);
                } else {
                    target.remove(&id);
                }
                let mut tx = g.transaction();
                tx.apply(
                    if step % 2 == 0 { &l } else { &r },
                    TupleChange { id, value },
                )
                .unwrap();
                assert_eq!(
                    lc.get() - before.0,
                    usize::from(step % 2 == 0 && value.is_some())
                );
                assert_eq!(
                    rc.get() - before.1,
                    usize::from(step % 2 != 0 && value.is_some())
                );
                let predicate_calls = value.map_or(0, |v| {
                    if step % 2 == 0 {
                        right
                            .values()
                            .filter(|r: &&Row| matches(comparison, v.0, r.0))
                            .count()
                    } else {
                        left.values()
                            .filter(|l: &&Row| matches(comparison, l.0, v.0))
                            .count()
                    }
                });
                assert_eq!(
                    pc.get() - before.2,
                    if filtered { predicate_calls } else { 0 }
                );
                let mut expected = Pairs::new();
                for (&i, &a) in &next_left {
                    for (&j, &b) in &next_right {
                        if matches(comparison, a.0, b.0) && (!filtered || a.1 <= b.1) {
                            expected.insert((i, j), (a, b));
                        }
                    }
                }
                let actual: Pairs = tx
                    .graph()
                    .unwrap()
                    .dataset_rows(&dataset)
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (*k, *v))
                    .collect();
                assert_eq!(actual, expected);
                assert_eq!(
                    tx.score().unwrap(),
                    SimpleScore::new(expected.values().map(|(a, b)| a.1 + b.1 + 1).sum())
                );
                if step % 3 == 0 {
                    tx.commit().unwrap();
                    left = next_left;
                    right = next_right;
                } else {
                    drop(tx);
                }
            }
            g.assert_consistent().unwrap();
        }
    }
}
#[test]
fn shared_existence_and_absence_match_full_rebuilds_with_and_without_predicates() {
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        for filtered in [false, true] {
            let lc = Cell::new(0);
            let rc = Cell::new(0);
            let definition = SharedExistence::new(
                mode,
                |a: &Row| {
                    lc.set(lc.get() + 1);
                    a.0
                },
                |b: &Row| {
                    rc.set(rc.get() + 1);
                    b.0
                },
            );
            let definition = if filtered {
                definition.with_filter(|a, b| a.1 < b.1)
            } else {
                definition
            };
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let l = b.source::<usize, Row>();
            let r = b.source::<usize, Row>();
            let a = definition.apply(&mut b, l.port(), r.port()).unwrap();
            let c = definition
                .clone()
                .apply(&mut b, l.port(), r.port())
                .unwrap();
            let dataset = b.dataset(&a).unwrap();
            assert_eq!(dataset, b.dataset(&c).unwrap());
            for (name, p) in [("first", a), ("second", c)] {
                b.score(name, &p, |r| SimpleScore::new(r.1)).unwrap();
            }
            let mut g = b.build();
            let mut left = BTreeMap::<usize, Row>::new();
            let mut right = BTreeMap::<usize, Row>::new();
            let mut seed = 20261202;
            for step in 0..200 {
                let id = (random(&mut seed) % 8) as usize;
                let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                    (
                        (random(&mut seed) % 4) as i32,
                        (random(&mut seed) % 11) as i64 - 5,
                    )
                });
                let mut nl = left.clone();
                let mut nr = right.clone();
                let target = if step % 2 == 0 { &mut nl } else { &mut nr };
                if let Some(v) = value {
                    target.insert(id, v);
                } else {
                    target.remove(&id);
                }
                let before = (lc.get(), rc.get());
                let mut tx = g.transaction();
                tx.apply(
                    if step % 2 == 0 { &l } else { &r },
                    TupleChange { id, value },
                )
                .unwrap();
                assert_eq!(
                    lc.get() - before.0,
                    usize::from(step % 2 == 0 && value.is_some())
                );
                assert_eq!(
                    rc.get() - before.1,
                    usize::from(step % 2 != 0 && value.is_some())
                );
                let expected: BTreeMap<_, _> = nl
                    .iter()
                    .filter(|(_, a)| {
                        nr.values().any(|b| a.0 == b.0 && (!filtered || a.1 < b.1))
                            == (mode == ExistenceMode::Exists)
                    })
                    .map(|(&i, &a)| (i, a))
                    .collect();
                let actual: BTreeMap<_, _> = tx
                    .graph()
                    .unwrap()
                    .dataset_rows(&dataset)
                    .unwrap()
                    .iter()
                    .map(|(i, r)| (*i, *r))
                    .collect();
                assert_eq!(actual, expected);
                assert_eq!(
                    tx.score().unwrap(),
                    SimpleScore::new(2 * expected.values().map(|r| r.1).sum::<i64>())
                );
                if step % 3 == 0 {
                    tx.commit().unwrap();
                    left = nl;
                    right = nr;
                } else {
                    drop(tx);
                }
            }
            g.assert_consistent().unwrap();
        }
    }
}
#[test]
fn changing_filters_keeps_old_clones_and_nodes_and_parent_order_is_significant() {
    let relation = SharedJoin::new(|a: &i32| *a, |b: &i32| *b);
    let retained = relation.clone();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let a = b.source::<usize, i32>();
    let c = b.source::<usize, i32>();
    let old = relation.apply(&mut b, a.port(), c.port()).unwrap();
    let changed = relation.with_filter(|_, _| false);
    let new = changed.apply(&mut b, a.port(), c.port()).unwrap();
    let repeated = retained.apply(&mut b, a.port(), c.port()).unwrap();
    let reversed = retained.apply(&mut b, c.port(), a.port()).unwrap();
    assert_eq!(b.dataset(&old).unwrap(), b.dataset(&repeated).unwrap());
    assert_ne!(b.dataset(&old).unwrap(), b.dataset(&new).unwrap());
    assert_ne!(b.dataset(&old).unwrap(), b.dataset(&reversed).unwrap());
    let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
    let f = foreign.source::<usize, i32>();
    assert!(matches!(
        retained.apply(&mut b, a.port(), f.port()),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        retained.apply(&mut b, f.port(), c.port()),
        Err(GraphError::ForeignPort)
    ));
    b.score("original", &old, |_| SimpleScore::new(1)).unwrap();
    b.score("changed", &new, |_| SimpleScore::new(100)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&a, TupleChange::put(0, 1)).unwrap();
    tx.apply(&c, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(1));
}
#[test]
fn predicate_panics_roll_back_shared_join_indexes_and_consumers() {
    let fail = Cell::new(false);
    let relation = SharedJoin::new(|a: &i32| *a, |b: &i32| *b).with_filter(|_, _| {
        assert!(!fail.get(), "injected predicate failure");
        true
    });
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, i32>();
    let r = b.source::<usize, i32>();
    for name in ["one", "two"] {
        let p = relation.apply(&mut b, l.port(), r.port()).unwrap();
        b.score(name, &p, |_| SimpleScore::new(1)).unwrap();
    }
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, 1)).unwrap();
    tx.apply(&r, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&l, TupleChange::put(1, 1)).unwrap();
        }))
        .is_err()
    );
    assert_eq!(g.score(), SimpleScore::new(2));
    assert_eq!(g.source_rows(&l).unwrap().len(), 1);
    fail.set(false);
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(1, 1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(4));
    g.assert_consistent().unwrap();
}
#[test]
fn self_exclusion_tracks_other_entities_once_and_keeps_absence_mode_distinct() {
    let exists = SharedExistence::new(ExistenceMode::Exists, |_: &usize| (), |_: &usize| ())
        .with_filter(|a, b| a != b);
    let absent = SharedExistence::new(ExistenceMode::NotExists, |_: &usize| (), |_: &usize| ())
        .with_filter(|a, b| a != b);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, usize>();
    for name in ["exists1", "exists2"] {
        let p = exists.apply(&mut b, s.port(), s.port()).unwrap();
        b.score(name, &p, |_| SimpleScore::new(10)).unwrap();
    }
    for name in ["absent1", "absent2"] {
        let p = absent.apply(&mut b, s.port(), s.port()).unwrap();
        b.score(name, &p, |_| SimpleScore::new(1)).unwrap();
    }
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, 0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(2));
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(1, 1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(40));
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(2, 2)).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(60));
    }
    assert_eq!(g.score(), SimpleScore::new(40));
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(2));
    g.assert_consistent().unwrap();
}
