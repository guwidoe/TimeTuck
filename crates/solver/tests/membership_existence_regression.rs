//! Original membership existence regressions; no upstream compatibility credits.
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
    direction: usize,
    mode: ExistenceMode,
) -> BTreeMap<usize, Row> {
    left.iter()
        .filter(|(_, a)| {
            let present = right.values().any(|b| {
                let member = match direction {
                    0 => a.keys.iter().any(|k| b.keys.contains(k)),
                    1 => a.keys.contains(&b.tag),
                    _ => b.keys.contains(&a.tag),
                };
                a.group == b.group && a.time <= b.time && a.capacity >= b.capacity && member
            });
            present == (mode == ExistenceMode::Exists)
        })
        .map(|(id, row)| (*id, row.clone()))
        .collect()
}
fn score(rows: &BTreeMap<usize, Row>) -> SimpleScore {
    SimpleScore::new(rows.values().map(|a| a.value).sum())
}
#[test]
fn all_collection_directions_match_brute_force_through_replacement_retraction_and_rollback() {
    for direction in 0..3 {
        for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
            for prune in [false, true] {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                if prune {
                    b = b.with_activity_pruning();
                }
                let l = b.source::<usize, Row>();
                let r = b.source::<usize, Row>();
                let rules = rules();
                let joined = match direction {
                    0 => rules.apply_existence_containing_any_of(
                        &mut b,
                        l.port(),
                        r.port(),
                        mode,
                        |v| v.keys.clone(),
                        |v| v.keys.clone(),
                    ),
                    1 => rules.apply_existence_containing(
                        &mut b,
                        l.port(),
                        r.port(),
                        mode,
                        |v| v.keys.clone(),
                        |v| v.tag,
                    ),
                    _ => rules.apply_existence_contained_in(
                        &mut b,
                        l.port(),
                        r.port(),
                        mode,
                        |v| v.tag,
                        |v| v.keys.clone(),
                    ),
                }
                .unwrap();
                let data = b.dataset(&joined).unwrap();
                b.score("left rows", &joined, |a| SimpleScore::new(a.value))
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
                    let wanted = expected(&nl, &nr, direction, mode);
                    saw_match |= !wanted.is_empty();
                    let mut tx = g.transaction();
                    tx.apply(if n % 3 == 0 { &l } else { &r }, change).unwrap();
                    assert_eq!(tx.score().unwrap(), score(&wanted));
                    assert_eq!(
                        tx.graph()
                            .unwrap()
                            .dataset_rows(&data)
                            .unwrap()
                            .iter()
                            .map(|(id, row)| (*id, row.clone()))
                            .collect::<BTreeMap<_, _>>(),
                        wanted
                    );
                    if n % 4 != 0 {
                        tx.commit().unwrap();
                        left = nl;
                        right = nr;
                    } else {
                        drop(tx);
                    }
                    let wanted = expected(&left, &right, direction, mode);
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
}

#[test]
fn witness_counts_refresh_payloads_and_empty_right_reactivates_absence() {
    for prune in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if prune {
            b = b.with_activity_pruning();
        }
        let l = b.source::<usize, (Vec<i32>, i64)>();
        let r = b.source::<usize, (Vec<i32>, i64)>();
        let rules = JoinConditions::new([]);
        let yes = rules
            .apply_existence_containing_any_of(
                &mut b,
                l.port(),
                r.port(),
                ExistenceMode::Exists,
                |r| r.0.clone(),
                |r| r.0.clone(),
            )
            .unwrap();
        let no = rules
            .apply_existence_containing_any_of(
                &mut b,
                l.port(),
                r.port(),
                ExistenceMode::NotExists,
                |r| r.0.clone(),
                |r| r.0.clone(),
            )
            .unwrap();
        b.score("yes", &yes, |r| SimpleScore::new(r.1)).unwrap();
        b.score("no", &no, |r| SimpleScore::new(-r.1)).unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, (vec![1, 1, 2], 7)))
            .unwrap();
        tx.apply(&l, TupleChange::put(1, (vec![], 3))).unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(-10));
        for id in 0..2 {
            let mut tx = g.transaction();
            tx.apply(&r, TupleChange::put(id, (vec![1, 2, 2], 100)))
                .unwrap();
            tx.commit().unwrap();
            assert_eq!(g.score(), SimpleScore::new(4));
        }
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, (vec![2, 1], 11))).unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(8));
        for id in 0..2 {
            let mut tx = g.transaction();
            tx.apply(&r, TupleChange::retract(id)).unwrap();
            tx.commit().unwrap();
            assert_eq!(g.score(), SimpleScore::new(if id == 0 { 8 } else { -14 }));
        }
        let mut tx = g.transaction();
        tx.apply(&r, TupleChange::put(0, (vec![2], 0))).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(8));
        drop(tx);
        assert_eq!(g.score(), SimpleScore::new(-14));
        let mut tx = g.transaction();
        tx.apply(&r, TupleChange::put(0, (vec![2], 0))).unwrap();
        tx.commit().unwrap();
        assert_eq!(g.score(), SimpleScore::new(8));
        g.assert_consistent().unwrap();
    }
}

#[test]
fn self_membership_observes_final_state_on_both_edges() {
    for prune in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if prune {
            b = b.with_activity_pruning();
        }
        let l = b.source::<usize, Vec<i32>>();
        let rules = JoinConditions::new([]);
        let yes = rules
            .apply_existence_containing_any_of(
                &mut b,
                l.port(),
                l.port(),
                ExistenceMode::Exists,
                Clone::clone,
                Clone::clone,
            )
            .unwrap();
        let no = rules
            .apply_existence_containing_any_of(
                &mut b,
                l.port(),
                l.port(),
                ExistenceMode::NotExists,
                Clone::clone,
                Clone::clone,
            )
            .unwrap();
        b.score("yes", &yes, |_| SimpleScore::new(1)).unwrap();
        b.score("no", &no, |_| SimpleScore::new(-1)).unwrap();
        let mut g = b.build();
        for (id, keys, wanted) in [
            (0, vec![1, 1], 1),
            (1, vec![1, 2], 2),
            (0, vec![], 0),
            (1, vec![], -2),
            (0, vec![2], 0),
        ] {
            let mut tx = g.transaction();
            tx.apply(&l, TupleChange::put(id, keys)).unwrap();
            tx.commit().unwrap();
            assert_eq!(g.score(), SimpleScore::new(wanted));
        }
        for (id, wanted) in [(0, -1), (1, 0)] {
            let mut tx = g.transaction();
            tx.apply(&l, TupleChange::retract(id)).unwrap();
            tx.commit().unwrap();
            assert_eq!(g.score(), SimpleScore::new(wanted));
        }
        g.assert_consistent().unwrap();
    }
}

#[test]
fn partitions_and_shared_keys_limit_residual_work() {
    type V = (i32, Vec<i32>, i32);
    let calls = Cell::new(0);
    let group = JoinProjection::new(|r: &V| r.0);
    let time = JoinProjection::new(|r: &V| {
        calls.set(calls.get() + 1);
        r.2
    });
    let rules = JoinConditions::new([
        JoinCondition::new(JoinComparison::LessThan, &time, &time),
        JoinCondition::new(JoinComparison::Equal, &group, &group),
    ]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, V>();
    let r = b.source::<usize, V>();
    let yes = rules
        .apply_existence_containing_any_of(
            &mut b,
            l.port(),
            r.port(),
            ExistenceMode::Exists,
            |r| r.1.clone(),
            |r| r.1.clone(),
        )
        .unwrap();
    b.score("yes", &yes, |_| SimpleScore::new(1)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&l, TupleChange::put(0, (0, vec![1, 1, 2], 0)))
        .unwrap();
    for id in 0..40 {
        tx.apply(
            &r,
            TupleChange::put(id, (if id == 39 { 0 } else { 1 }, vec![1, 2], 1)),
        )
        .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(1));
    assert_eq!(calls.get(), 2);
    calls.set(0);
    let mut tx = g.transaction();
    tx.apply(&r, TupleChange::put(39, (0, vec![3], 1))).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::ZERO);
    assert_eq!(calls.get(), 0);
}

#[test]
fn projection_residual_and_sink_panics_roll_back_witnesses_and_indexes() {
    type V = (Vec<i32>, i32);
    for mode in [ExistenceMode::Exists, ExistenceMode::NotExists] {
        let fail = Cell::new(0);
        let time = JoinProjection::new(|r: &V| {
            assert_ne!(fail.get(), 2, "residual panic");
            r.1
        });
        let rules =
            JoinConditions::new([JoinCondition::new(JoinComparison::LessThan, &time, &time)]);
        let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
        let fs = foreign.source::<usize, V>();
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_activity_pruning();
        let l = b.source::<usize, V>();
        let r = b.source::<usize, V>();
        assert!(
            rules
                .apply_existence_containing_any_of(
                    &mut b,
                    l.port(),
                    fs.port(),
                    mode,
                    |_| panic!("foreign"),
                    |_: &V| Vec::<i32>::new()
                )
                .is_err()
        );
        let out = rules
            .apply_existence_containing_any_of(
                &mut b,
                l.port(),
                r.port(),
                mode,
                |r| {
                    assert_ne!(fail.get(), 1, "projection panic");
                    r.0.clone()
                },
                |r| r.0.clone(),
            )
            .unwrap();
        let data = b.dataset(&out).unwrap();
        b.score("output", &out, |_| {
            assert_ne!(fail.get(), 3, "sink panic");
            SimpleScore::new(1)
        })
        .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, (vec![1, 1], 0))).unwrap();
        tx.apply(&r, TupleChange::put(0, (vec![1], 1))).unwrap();
        tx.commit().unwrap();
        let before = g.dataset_rows(&data).unwrap().clone();
        let old = g.score();
        for stage in 1..=3 {
            fail.set(stage);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let mut tx = g.transaction();
                    // Both modes reach the sink: Exists refreshes a visible payload;
                    // NotExists loses its qualifying witness when the left time is 2.
                    let time = if stage == 3 && mode == ExistenceMode::NotExists {
                        2
                    } else {
                        0
                    };
                    tx.apply(&l, TupleChange::put(0, (vec![1, 2], time)))
                        .unwrap();
                    tx.commit().unwrap();
                }))
                .is_err()
            );
            assert_eq!(*g.dataset_rows(&data).unwrap(), before);
            assert_eq!(g.score(), old);
        }
        fail.set(0);
        let mut tx = g.transaction();
        tx.apply(&r, TupleChange::retract(0)).unwrap();
        tx.commit().unwrap();
        assert_eq!(
            g.score(),
            SimpleScore::new(if mode == ExistenceMode::NotExists {
                1
            } else {
                0
            })
        );
        let mut tx = g.transaction();
        tx.apply(&r, TupleChange::put(0, (vec![1], 1))).unwrap();
        tx.commit().unwrap();
        assert_eq!(*g.dataset_rows(&data).unwrap(), before);
        assert_eq!(g.score(), old);
        g.assert_consistent().unwrap();
    }
}
