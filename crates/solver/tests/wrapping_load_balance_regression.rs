//! ORIGINAL native policy regressions; no upstream-method coverage credit.
use std::panic::{AssertUnwindSafe, catch_unwind};
use timetuck::{
    Decimal, LoadBalanceCollector, ReversibleCollector, SimpleScore, StreamGraphBuilder,
    TupleChange, WrappingLoadBalanceCollector, WrappingLoadBalanceError,
};
type C = WrappingLoadBalanceCollector<i32>;
fn decimal(coefficient: i64, scale: i32) -> Decimal {
    Decimal::new(coefficient.into(), scale)
}
fn pair(a: i64, b: i64) -> C {
    let mut c = C::default();
    c.insert(&(0, a, 0));
    c.insert(&(1, b, 0));
    c
}
#[test]
fn empty_zero_positive_and_negative_radicands_have_typed_results() {
    let empty = C::default().finish().unwrap();
    assert!(empty.loads().is_empty());
    assert_eq!(empty.unfairness(), &Decimal::ZERO);
    assert_eq!(pair(2, 2).finish().unwrap().unfairness(), &Decimal::ZERO);
    assert_eq!(
        pair(2, 1).try_finish().unwrap().unfairness(),
        &decimal(707107, 6)
    );
    // 3037000500 squared exceeds i64::MAX. Q=T is negative, n=2;
    // widened n*Q-T therefore remains negative, without guessing NaN semantics.
    let mut c = pair(3037000500, 0);
    assert_eq!(c.finish(), Err(WrappingLoadBalanceError::NegativeRadicand));
    assert_eq!(
        c.try_finish(),
        Err(WrappingLoadBalanceError::NegativeRadicand)
    );
    assert!(c.remove(&(0, 3037000500, 0)));
    assert_eq!(c.finish().unwrap().unfairness(), &Decimal::ZERO);
}
#[test]
fn recorded_maximum_and_adjacent_maximum_shapes_have_modular_moments() {
    let mut c = pair(i64::MAX, 0);
    assert_eq!(c.finish().unwrap().unfairness(), &decimal(707107, 6));
    c.insert(&(0, i64::MAX, 0));
    let out = c.finish().unwrap();
    assert_eq!(out.loads()[&0], -2);
    assert_eq!(out.unfairness(), &decimal(141421, 5));
    let mut c = pair(i64::MAX, i64::MAX - 1);
    assert_eq!(c.finish().unwrap().unfairness(), &decimal(707107, 6));
    c.insert(&(0, i64::MAX, 0));
    assert_eq!(c.finish().unwrap().unfairness(), &Decimal::ZERO);
    assert!(c.remove(&(0, i64::MAX, 0)));
    assert_eq!(c.finish().unwrap().unfairness(), &decimal(707107, 6));
    assert!(c.replace(&(0, i64::MAX, 0), &(1, i64::MAX - 1, 0)));
    let out = c.finish().unwrap();
    assert_eq!(out.loads().len(), 1);
    assert_eq!(out.loads()[&1], -4);
    assert_eq!(out.unfairness(), &Decimal::ZERO);
}
#[test]
fn recorded_initial_overflow_shape_preserves_signed_wrapped_loads() {
    let mut c = C::default();
    let a = (0, i64::MAX, i64::MAX);
    let b = (1, i64::MIN, i64::MIN);
    c.insert(&a);
    c.insert(&b);
    let saved = c.finish().unwrap();
    assert_eq!(saved.loads()[&0], -2);
    assert_eq!(saved.loads()[&1], 0);
    assert_eq!(saved.unfairness(), &decimal(141421, 5));
    c.insert(&a);
    let out = c.finish().unwrap();
    assert_eq!(out.loads()[&0], i64::MAX - 2);
    assert_eq!(out.unfairness(), &decimal(212132, 5));
    c.remove(&a);
    c.remove(&a);
    let out = c.finish().unwrap();
    assert_eq!(out.loads().len(), 1);
    assert_eq!(out.loads()[&1], 0);
    assert_eq!(saved.loads()[&0], -2);
    c.insert(&b);
    assert_eq!(c.finish().unwrap().loads()[&1], i64::MIN);
    assert_eq!(c.finish().unwrap().unfairness(), &Decimal::ZERO);
}
#[test]
fn duplicate_captures_missing_triples_replacement_failures_and_snapshots_are_exact() {
    let mut c = C::default();
    let a = (0, 3, 7);
    let b = (1, -2, 0);
    c.insert(&a);
    c.insert(&a);
    c.insert(&b);
    let saved = c.finish().unwrap();
    let mut branch = c.clone();
    assert!(!branch.remove(&(0, 3, 8)));
    assert!(!branch.replace(&(9, 0, 0), &b));
    assert_eq!(branch.finish().unwrap(), saved);
    assert!(catch_unwind(AssertUnwindSafe(|| branch.replace(&a, &(0, 4, 8)))).is_err());
    assert_eq!(branch.finish().unwrap(), saved);
    assert!(branch.remove(&a));
    assert_eq!(branch.finish().unwrap().loads()[&0], 10);
    assert!(branch.replace(&a, &(0, 4, 8)));
    assert_eq!(branch.finish().unwrap().loads()[&0], 12);
    assert_eq!(saved.loads()[&0], 13);
    assert_eq!(c.finish().unwrap(), saved);
    assert!(branch.remove(&(0, 4, 8)));
    assert!(branch.remove(&b));
    assert!(!branch.remove(&a));
    assert!(branch.finish().unwrap().loads().is_empty());
}
#[test]
fn modular_rebuild_and_inverse_updates_depend_only_on_active_multiset() {
    let rows = [
        (0, i64::MAX, 0),
        (1, i64::MAX - 1, 0),
        (0, i64::MAX, 0),
        (2, 0, 0),
    ];
    let mut a = C::default();
    let mut b = C::default();
    for row in rows {
        a.insert(&row);
    }
    for row in rows.into_iter().rev() {
        b.insert(&row);
    }
    let saved = a.finish();
    assert_eq!(saved, b.finish());
    let old = rows[0];
    let replacement = (2, 12, 0);
    assert!(a.replace(&old, &replacement));
    assert!(a.replace(&replacement, &old));
    assert_eq!(a.finish(), saved);
    for row in rows {
        assert!(a.remove(&row));
        assert!(b.remove(&row));
    }
    assert!(a.finish().unwrap().loads().is_empty());
    assert_eq!(a.finish(), b.finish());
}
#[test]
fn exact_default_and_large_offset_non_cancellation_are_preserved() {
    let c = pair(1_000_000_000, 1_000_000_001);
    assert_eq!(c.finish().unwrap().unfairness(), &decimal(707107, 6));
    let mut exact = LoadBalanceCollector::<i32>::default();
    exact.insert(&(0, i64::MAX, 0));
    exact.insert(&(0, i64::MAX, 0));
    exact.insert(&(1, 0, 0));
    assert_eq!(
        exact.finish().loads()[&0],
        timetuck::BigInt::from(i64::MAX) * 2
    );
}
#[test]
fn graph_trial_rollback_and_committed_error_recovery_preserve_captures() {
    let mut builder = StreamGraphBuilder::<SimpleScore>::new();
    let source = builder.source::<usize, (i32, i64, i64)>();
    let out = builder
        .collect::<_, _, _, C>(source.port(), |row| *row)
        .unwrap();
    builder
        .score("policy", &out, |result| {
            SimpleScore::new(if result.is_err() { -100 } else { 0 })
        })
        .unwrap();
    let mut graph = builder.build();
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(0, (0, 3037000500, 0)))
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(0));
    {
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(1, (1, 0, 0))).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(-100));
    }
    assert_eq!(graph.score(), SimpleScore::new(0));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(1, (1, 0, 0))).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-100));
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert_eq!(graph.score(), SimpleScore::new(0));
    graph.assert_consistent().unwrap();
}
#[test]
fn exact_policy_root_reuse_retains_six_digit_half_even_ties() {
    for (root, coefficient, scale) in [
        (1234565, 123456, -1),
        (1234575, 123458, -1),
        (9999995, 1, -7),
    ] {
        let mut c = C::default();
        for (key, load) in [(0, 0), (1, root), (2, root), (3, 0)] {
            c.insert(&(key, load, 0));
        }
        assert_eq!(
            c.finish().unwrap().unfairness(),
            &decimal(coefficient, scale)
        );
    }
}
