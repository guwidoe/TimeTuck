//! Original compound-existence integration checks; no upstream method credit.
use JoinComparison::*;
use std::collections::BTreeMap;
use timetuck::*;
type Row = (u8, i32, i64);
fn random(seed: &mut u64) -> u64 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
    *seed >> 32
}
#[test]
fn conjunctions_preserve_left_identity_through_both_modes_and_activity_transitions() {
    let mut seed = 203019;
    for pruning in [false, true] {
        for shape in 0..4 {
            let group = JoinProjection::new(|r: &Row| r.0);
            let x = JoinProjection::new(|r: &Row| r.1);
            let y = JoinProjection::new(|r: &Row| r.2);
            let clauses = match shape {
                0 => vec![],
                1 => vec![JoinCondition::new(Equal, &group, &group)],
                2 => vec![
                    JoinCondition::new(LessThanOrEqual, &x, &x),
                    JoinCondition::new(GreaterThan, &y, &y),
                ],
                _ => vec![
                    JoinCondition::new(LessThanOrEqual, &x, &x),
                    JoinCondition::new(Equal, &group, &group),
                    JoinCondition::new(GreaterThan, &y, &y),
                ],
            };
            let conditions = JoinConditions::new(clauses);
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            if pruning {
                b = b.with_activity_pruning();
            }
            let l = b.source::<usize, Row>();
            let r = b.source::<usize, Row>();
            let joined = conditions.apply(&mut b, l.port(), r.port()).unwrap();
            b.score("pairs", &joined, |_| SimpleScore::new(1)).unwrap();
            let exists = conditions
                .apply_existence(&mut b, l.port(), r.port(), ExistenceMode::Exists)
                .unwrap();
            let absent = conditions
                .apply_existence(&mut b, l.port(), r.port(), ExistenceMode::NotExists)
                .unwrap();
            let yes = b.dataset(&exists).unwrap();
            let no = b.dataset(&absent).unwrap();
            let again = conditions
                .equals_first()
                .apply_existence(&mut b, l.port(), r.port(), ExistenceMode::Exists)
                .unwrap();
            assert_eq!(yes, b.dataset(&again).unwrap());
            assert_ne!(yes, no);
            b.score("exists", &exists, |row| SimpleScore::new(row.2))
                .unwrap();
            b.score("absent", &absent, |_| SimpleScore::new(-2))
                .unwrap();
            let mut g = b.build();
            let mut left = BTreeMap::<usize, Row>::new();
            let mut right = BTreeMap::<usize, Row>::new();
            for step in 0..500 {
                let old_score = g.score();
                let old_yes = g.dataset_rows(&yes).unwrap().clone();
                let old_no = g.dataset_rows(&no).unwrap().clone();
                let id = (random(&mut seed) % 8) as usize;
                let value = (!random(&mut seed).is_multiple_of(4)).then(|| {
                    (
                        (random(&mut seed) % 3) as u8,
                        (random(&mut seed) % 7) as i32 - 3,
                        (random(&mut seed) % 7) as i64 - 3,
                    )
                });
                let mut nl = left.clone();
                let mut nr = right.clone();
                let target = if step % 2 == 0 { &mut nl } else { &mut nr };
                if let Some(row) = value {
                    target.insert(id, row);
                } else {
                    target.remove(&id);
                }
                let mut tx = g.transaction();
                tx.apply(
                    if step % 2 == 0 { &l } else { &r },
                    TupleChange { id, value },
                )
                .unwrap();
                let predicate = |a: &Row, b: &Row| {
                    (shape != 1 && shape != 3 || a.0 == b.0)
                        && (shape < 2 || a.1 <= b.1 && a.2 > b.2)
                };
                let expected_yes: BTreeMap<_, _> = nl
                    .iter()
                    .filter(|(_, a)| nr.values().any(|b| predicate(a, b)))
                    .map(|(id, row)| (*id, *row))
                    .collect();
                let expected_no: BTreeMap<_, _> = nl
                    .iter()
                    .filter(|(id, _)| !expected_yes.contains_key(*id))
                    .map(|(id, row)| (*id, *row))
                    .collect();
                let graph = tx.graph().unwrap();
                assert_eq!(
                    graph
                        .dataset_rows(&yes)
                        .unwrap()
                        .iter()
                        .map(|(id, row)| (*id, *row))
                        .collect::<BTreeMap<_, _>>(),
                    expected_yes
                );
                assert_eq!(
                    graph
                        .dataset_rows(&no)
                        .unwrap()
                        .iter()
                        .map(|(id, row)| (*id, *row))
                        .collect::<BTreeMap<_, _>>(),
                    expected_no
                );
                let pairs = nl
                    .values()
                    .map(|a| nr.values().filter(|b| predicate(a, b)).count() as i64)
                    .sum::<i64>();
                assert_eq!(
                    tx.score().unwrap(),
                    SimpleScore::new(
                        pairs + expected_yes.values().map(|r| r.2).sum::<i64>()
                            - 2 * expected_no.len() as i64
                    )
                );
                if step % 3 != 0 {
                    tx.commit().unwrap();
                    left = nl;
                    right = nr;
                } else {
                    drop(tx);
                    assert_eq!(g.score(), old_score);
                    assert!(g.dataset_rows(&yes).unwrap().iter().eq(old_yes.iter()));
                    assert!(g.dataset_rows(&no).unwrap().iter().eq(old_no.iter()));
                }
            }
            g.assert_consistent().unwrap();
        }
    }
}

#[test]
fn right_multiplicity_changes_visibility_only_at_zero_and_self_relations_are_valid() {
    for self_join in [false, true] {
        let key = JoinProjection::new(|r: &i32| *r);
        let conditions = JoinConditions::new([JoinCondition::new(Equal, &key, &key)]);
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let l = b.source::<usize, i32>();
        let r = b.source::<usize, i32>();
        let right = if self_join { l.port() } else { r.port() };
        let yes = conditions
            .apply_existence(&mut b, l.port(), right, ExistenceMode::Exists)
            .unwrap();
        let no = conditions
            .apply_existence(&mut b, l.port(), right, ExistenceMode::NotExists)
            .unwrap();
        b.score("yes", &yes, |_| SimpleScore::new(1)).unwrap();
        b.score("no", &no, |_| SimpleScore::new(-1)).unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, 7)).unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(if self_join { 1 } else { -1 }));
        let target = if self_join { &l } else { &r };
        for id in 1..=20 {
            let mut tx = g.transaction();
            tx.apply(target, TupleChange::put(id, 7)).unwrap();
            tx.commit().unwrap();
            assert_eq!(
                g.score(),
                SimpleScore::new(if self_join { id as i64 + 1 } else { 1 })
            );
        }
        for id in (1..=20).rev() {
            let mut tx = g.transaction();
            tx.apply(target, TupleChange::retract(id)).unwrap();
            tx.commit().unwrap();
            assert_eq!(
                g.score(),
                SimpleScore::new(if self_join {
                    id as i64
                } else if id == 1 {
                    -1
                } else {
                    1
                })
            );
        }
        g.assert_consistent().unwrap();
    }
}

#[test]
fn ordered_ranges_skip_residual_work_outside_the_partition_and_bounds() {
    use std::cell::Cell;
    for relation in [LessThan, LessThanOrEqual, GreaterThan, GreaterThanOrEqual] {
        for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
            let calls = Cell::new(0);
            let order_calls = Cell::new(0);
            let group = JoinProjection::new(|r: &Row| r.0);
            let order = JoinProjection::new(|r: &Row| {
                order_calls.set(order_calls.get() + 1);
                r.1
            });
            let residual = JoinProjection::new(|r: &Row| {
                calls.set(calls.get() + 1);
                r.2
            });
            let c = JoinConditions::new([
                JoinCondition::new(relation, &order, &order),
                JoinCondition::new(Equal, &group, &group),
                JoinCondition::new(GreaterThanOrEqual, &residual, &residual),
            ]);
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let l = b.source::<usize, Row>();
            let r = b.source::<usize, Row>();
            let out = c.apply_existence(&mut b, l.port(), r.port(), mode).unwrap();
            b.score("visible", &out, |_| SimpleScore::new(1)).unwrap();
            let mut g = b.build();
            let mut tx = g.transaction();
            for id in 0..40 {
                tx.apply(
                    &r,
                    TupleChange::put(
                        id,
                        (
                            if id < 20 { 1 } else { 2 },
                            (id % 20) as i32,
                            (id % 3) as i64,
                        ),
                    ),
                )
                .unwrap();
            }
            assert_eq!(calls.get(), 0);
            assert_eq!(order_calls.get(), 40);
            tx.apply(&l, TupleChange::put(0, (1, 10, 1))).unwrap();
            tx.commit().unwrap();
            let count = (0..20)
                .filter(|&v| match relation {
                    LessThan => 10 < v,
                    LessThanOrEqual => 10 <= v,
                    GreaterThan => 10 > v,
                    GreaterThanOrEqual => 10 >= v,
                    Equal => unreachable!(),
                })
                .count();
            assert_eq!(order_calls.get(), 41);
            assert_eq!(calls.get(), 2 * count); // Only indexed candidates evaluate both residual projections.
            assert_eq!(
                g.score(),
                SimpleScore::new(i64::from(mode == ExistenceMode::Exists))
            );
            let mut tx = g.transaction();
            tx.apply(&l, TupleChange::put(0, (2, 10, 1))).unwrap();
            tx.commit().unwrap();
            assert_eq!(order_calls.get(), 42);
            assert_eq!(calls.get(), 4 * count); // Old partition retractions use captured membership.
            let before = calls.get();
            let mut tx = g.transaction();
            for id in 0..40 {
                tx.apply(&r, TupleChange::retract(id)).unwrap();
            }
            tx.commit().unwrap();
            assert_eq!(calls.get(), before);
            assert_eq!(order_calls.get(), 42);
            assert_eq!(
                g.score(),
                SimpleScore::new(i64::from(mode == ExistenceMode::NotExists))
            );
            g.assert_consistent().unwrap();
        }
    }
}

#[test]
fn callback_panics_restore_ordered_indexes_links_and_both_visibility_modes() {
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    let fail = Cell::new(false);
    let group = JoinProjection::new(|r: &Row| r.0);
    let order = JoinProjection::new(|r: &Row| r.1);
    let residual = JoinProjection::new(|r: &Row| {
        assert!(!fail.get(), "injected residual failure");
        r.2
    });
    let c = JoinConditions::new([
        JoinCondition::new(LessThan, &order, &order),
        JoinCondition::new(Equal, &group, &group),
        JoinCondition::new(GreaterThanOrEqual, &residual, &residual),
    ]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, Row>();
    let r = b.source::<usize, Row>();
    let yes = c
        .apply_existence(&mut b, l.port(), r.port(), ExistenceMode::Exists)
        .unwrap();
    let no = c
        .apply_existence(&mut b, l.port(), r.port(), ExistenceMode::NotExists)
        .unwrap();
    b.score("yes", &yes, |_| SimpleScore::new(1)).unwrap();
    b.score("no", &no, |_| SimpleScore::new(-10)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, (1, 5, 0))).unwrap();
    tx.apply(&r, TupleChange::put(0, (1, 6, 0))).unwrap();
    tx.commit().unwrap();
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&r, TupleChange::put(0, (1, 7, 0))).unwrap();
            tx.commit().unwrap();
        }))
        .is_err()
    );
    fail.set(false);
    assert_eq!(g.score(), SimpleScore::new(1));
    g.assert_consistent().unwrap();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(1, (1, 6, 0))).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-9)); // Captured right key must have rolled back to6, not7.
    fail.set(true);
    let mut tx = g.transaction();
    tx.apply(&r, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-20)); // Retraction requires no residual callbacks.
    fail.set(false);
    g.assert_consistent().unwrap();
}
