//! Original graph integration regressions; no upstream method credit.
use JoinComparison::*;
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
const RELATIONS: [JoinComparison; 5] = [
    Equal,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
];
type Row = (u8, bool, i32, i64);
fn accepts<T: Ord>(relation: JoinComparison, a: T, b: T) -> bool {
    match relation {
        Equal => a == b,
        LessThan => a < b,
        LessThanOrEqual => a <= b,
        GreaterThan => a > b,
        GreaterThanOrEqual => a >= b,
    }
}
fn random(s: &mut u64) -> u64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
    *s >> 32
}
#[test]
fn heterogeneous_conjunctions_match_full_pairs_across_updates_and_rollback() {
    let mut seed = 71234;
    for equality_count in 0..=2 {
        for first in RELATIONS {
            for second in RELATIONS {
                let group = JoinProjection::new(|r: &Row| r.0);
                let flag = JoinProjection::new(|r: &Row| r.1);
                let x = JoinProjection::new(|r: &Row| r.2);
                let y = JoinProjection::new(|r: &Row| r.3);
                let mut clauses = vec![JoinCondition::new(first, &x, &x)];
                if equality_count > 0 {
                    clauses.push(JoinCondition::new(Equal, &group, &group));
                }
                clauses.push(JoinCondition::new(second, &y, &y));
                if equality_count > 1 {
                    clauses.push(JoinCondition::new(Equal, &flag, &flag));
                }
                let definition = JoinConditions::new(clauses);
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let l = b.source::<usize, Row>();
                let r = b.source::<usize, Row>();
                let joined = definition.apply(&mut b, l.port(), r.port()).unwrap();
                let again = definition
                    .equals_first()
                    .apply(&mut b, l.port(), r.port())
                    .unwrap();
                let data = b.dataset(&joined).unwrap();
                assert_eq!(data, b.dataset(&again).unwrap());
                b.score("count", &joined, |_| SimpleScore::new(1)).unwrap();
                b.score("fanout", &again, |(a, b)| SimpleScore::new(a.3 - b.3))
                    .unwrap();
                let mut g = b.build();
                let mut left = BTreeMap::<usize, Row>::new();
                let mut right = BTreeMap::<usize, Row>::new();
                for step in 0..120 {
                    let before = g
                        .dataset_rows(&data)
                        .unwrap()
                        .iter()
                        .map(|(id, row)| (*id, *row))
                        .collect::<BTreeMap<_, _>>();
                    let before_score = g.score();
                    let id = (random(&mut seed) % 9) as usize;
                    let value = (!random(&mut seed).is_multiple_of(5)).then(|| {
                        (
                            (random(&mut seed) % 3) as u8,
                            random(&mut seed).is_multiple_of(2),
                            (random(&mut seed) % 7) as i32 - 3,
                            (random(&mut seed) % 7) as i64 - 3,
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
                    let mut tx = g.transaction();
                    tx.apply(
                        if step % 2 == 0 { &l } else { &r },
                        TupleChange { id, value },
                    )
                    .unwrap();
                    let mut expected = BTreeMap::new();
                    for (&i, &a) in &nl {
                        for (&j, &b) in &nr {
                            let matches = (equality_count == 0 || a.0 == b.0)
                                && (equality_count < 2 || a.1 == b.1)
                                && accepts(first, a.2, b.2)
                                && accepts(second, a.3, b.3);
                            assert_eq!(definition.matches(&a, &b), matches);
                            if matches {
                                expected.insert((i, j), (a, b));
                            }
                        }
                    }
                    assert_eq!(
                        tx.graph()
                            .unwrap()
                            .dataset_rows(&data)
                            .unwrap()
                            .iter()
                            .map(|(id, row)| (*id, *row))
                            .collect::<BTreeMap<_, _>>(),
                        expected
                    );
                    assert_eq!(
                        tx.score().unwrap(),
                        SimpleScore::new(expected.values().map(|(a, b)| 1 + a.3 - b.3).sum())
                    );
                    if step % 3 == 0 {
                        tx.commit().unwrap();
                        left = nl;
                        right = nr;
                    } else {
                        drop(tx);
                        assert_eq!(
                            g.dataset_rows(&data)
                                .unwrap()
                                .iter()
                                .map(|(id, row)| (*id, *row))
                                .collect::<BTreeMap<_, _>>(),
                            before
                        );
                        assert_eq!(g.score(), before_score);
                    }
                }
                g.assert_consistent().unwrap();
            }
        }
    }
}
#[test]
fn empty_and_equality_only_self_joins_keep_occurrences() {
    for equality in [false, true] {
        let projection = JoinProjection::new(|r: &i32| *r);
        let clauses = JoinConditions::new(if equality {
            vec![JoinCondition::new(Equal, &projection, &projection)]
        } else {
            vec![]
        });
        assert!(clauses.same_instance(&clauses.equals_first()));
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, i32>();
        let pairs = clauses.apply(&mut b, source.port(), source.port()).unwrap();
        let data = b.dataset(&pairs).unwrap();
        b.score("count", &pairs, |_| SimpleScore::new(1)).unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        for (id, v) in [2, 2, 3].into_iter().enumerate() {
            tx.apply(&source, TupleChange::put(id, v)).unwrap();
        }
        tx.commit().unwrap();
        assert_eq!(
            g.dataset_rows(&data).unwrap().len(),
            if equality { 5 } else { 9 }
        );
        assert_eq!(g.score(), SimpleScore::new(if equality { 5 } else { 9 }));
        let old = g
            .dataset_rows(&data)
            .unwrap()
            .iter()
            .map(|(id, row)| (*id, *row))
            .collect::<BTreeMap<_, _>>();
        {
            let mut tx = g.transaction();
            tx.apply(&source, TupleChange::retract(0)).unwrap();
        }
        assert_eq!(
            g.dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(id, row)| (*id, *row))
                .collect::<BTreeMap<_, _>>(),
            old
        );
        g.assert_consistent().unwrap();
    }
}
#[test]
fn sharing_uses_callback_identity_and_ordered_parents_without_extra_projection_work() {
    #[derive(Eq, PartialEq, Ord, PartialOrd)]
    struct NotClone(i32);
    let calls = Cell::new(0);
    let projection = JoinProjection::new(|v: &i32| {
        calls.set(calls.get() + 1);
        NotClone(*v)
    });
    let cloned = projection.clone();
    let a = JoinConditions::new([JoinCondition::new(Equal, &projection, &projection)]);
    let equivalent = JoinConditions::new([JoinCondition::new(Equal, &cloned, &projection)]);
    let fresh = JoinProjection::new(|v: &i32| NotClone(*v));
    let distinct = JoinConditions::new([JoinCondition::new(Equal, &fresh, &fresh)]);
    for _ in 0..3 {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let l = b.source::<usize, i32>();
        let r = b.source::<usize, i32>();
        let p = a.apply(&mut b, l.port(), r.port()).unwrap();
        let same = equivalent.apply(&mut b, l.port(), r.port()).unwrap();
        assert_eq!(b.dataset(&p).unwrap(), b.dataset(&same).unwrap());
        let reverse = a.apply(&mut b, r.port(), l.port()).unwrap();
        assert_ne!(b.dataset(&p).unwrap(), b.dataset(&reverse).unwrap());
        let other = distinct.apply(&mut b, l.port(), r.port()).unwrap();
        assert_ne!(b.dataset(&p).unwrap(), b.dataset(&other).unwrap());
        b.score("a", &p, |_| SimpleScore::new(1)).unwrap();
        b.score("b", &same, |_| SimpleScore::new(2)).unwrap();
        let mut foreign = StreamGraphBuilder::<SimpleScore>::new();
        let f = foreign.source::<usize, i32>();
        assert!(matches!(
            a.apply(&mut b, l.port(), f.port()),
            Err(GraphError::ForeignPort)
        ));
        calls.set(0);
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, 7)).unwrap();
        tx.apply(&r, TupleChange::put(0, 7)).unwrap();
        tx.commit().unwrap();
        assert_eq!(calls.get(), 4); // One evaluation per side, for each of two distinct parent orders.
        assert_eq!(g.score(), SimpleScore::new(3));
    }
}
#[test]
fn residual_comparison_panics_restore_both_shared_consumers() {
    let panic_on = Cell::new(false);
    let group = JoinProjection::new(|r: &(i32, i32)| r.0);
    let residual = JoinProjection::new(|r: &(i32, i32)| {
        assert!(!panic_on.get());
        r.1
    });
    let definition = JoinConditions::new([
        JoinCondition::new(LessThanOrEqual, &group, &group),
        JoinCondition::new(GreaterThanOrEqual, &residual, &residual),
    ]);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let l = b.source::<usize, (i32, i32)>();
    let r = b.source::<usize, (i32, i32)>();
    let p = definition.apply(&mut b, l.port(), r.port()).unwrap();
    let same = definition
        .clone()
        .apply(&mut b, l.port(), r.port())
        .unwrap();
    let data = b.dataset(&p).unwrap();
    b.score("a", &p, |_| SimpleScore::new(1)).unwrap();
    b.score("b", &same, |_| SimpleScore::new(2)).unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&l, TupleChange::put(0, (0, 5))).unwrap();
        tx.apply(&r, TupleChange::put(0, (1, 4))).unwrap();
        tx.commit().unwrap();
    }
    let before = g
        .dataset_rows(&data)
        .unwrap()
        .iter()
        .map(|(id, row)| (*id, *row))
        .collect::<BTreeMap<_, _>>();
    panic_on.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut tx = g.transaction();
            tx.apply(&r, TupleChange::put(0, (2, 3))).unwrap();
            tx.commit().unwrap();
        }))
        .is_err()
    );
    panic_on.set(false);
    assert_eq!(
        g.dataset_rows(&data)
            .unwrap()
            .iter()
            .map(|(id, row)| (*id, *row))
            .collect::<BTreeMap<_, _>>(),
        before
    );
    assert_eq!(g.score(), SimpleScore::new(3));
    g.assert_consistent().unwrap();
    let mut tx = g.transaction();
    tx.apply(&r, TupleChange::put(0, (-1, 3))).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(0));
}

#[test]
fn distinct_row_types_and_different_side_projections_keep_relation_orientation() {
    #[derive(Clone)]
    struct Demand {
        group: String,
        minimum: i32,
        maximum: i64,
    }
    type Supply = (String, i32, i64);
    let dl = JoinProjection::new(|d: &Demand| d.group.clone());
    let sr = JoinProjection::new(|s: &Supply| s.0.clone());
    let minimum = JoinProjection::new(|d: &Demand| d.minimum);
    let capacity = JoinProjection::new(|s: &Supply| s.1);
    let maximum = JoinProjection::new(|d: &Demand| d.maximum);
    let price = JoinProjection::new(|s: &Supply| s.2);
    let conditions = JoinConditions::new([
        JoinCondition::new(LessThanOrEqual, &minimum, &capacity),
        JoinCondition::new(GreaterThanOrEqual, &maximum, &price),
        JoinCondition::new(Equal, &dl, &sr),
    ]);
    let normalized = conditions.equals_first();
    assert!(normalized.conditions()[0].left_projection_is(&dl));
    assert!(normalized.conditions()[0].right_projection_is(&sr));
    assert_eq!(normalized.conditions()[1].comparison(), LessThanOrEqual);
    assert_eq!(normalized.conditions()[2].comparison(), GreaterThanOrEqual);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let demand = b.source::<usize, Demand>();
    let supply = b.source::<String, Supply>();
    let pairs = conditions
        .apply(&mut b, demand.port(), supply.port())
        .unwrap();
    let terminal = b
        .score("acceptable supplies", &pairs, |_| SimpleScore::new(1))
        .unwrap();
    drop(conditions);
    drop(normalized);
    drop(dl);
    drop(sr);
    drop(minimum);
    drop(capacity);
    drop(maximum);
    drop(price);
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(
        &demand,
        TupleChange::put(
            0,
            Demand {
                group: "a".into(),
                minimum: 5,
                maximum: 10,
            },
        ),
    )
    .unwrap();
    for (id, group, capacity, price) in [
        ("exact", "a", 5, 10),
        ("better", "a", 9, 3),
        ("expensive", "a", 9, 11),
        ("small", "a", 4, 3),
        ("foreign", "b", 9, 3),
    ] {
        tx.apply(
            &supply,
            TupleChange::put(id.to_string(), (group.to_string(), capacity, price)),
        )
        .unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(
        g.constraint_matches(&terminal)
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec![(0, "better".to_string()), (0, "exact".to_string())]
    );
    assert_eq!(g.score(), SimpleScore::new(2));
    g.assert_consistent().unwrap();
}
