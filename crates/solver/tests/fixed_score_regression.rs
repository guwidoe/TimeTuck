//! Original fixed-universe scoring, inspection and arithmetic regressions.
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
#[test]
fn dense_and_tree_scoring_agree_on_trials_rebuild_and_retained_matches() {
    for pruning in [false, true] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        if pruning {
            b = b.with_activity_pruning();
        }
        let s = b.source::<i64, i64>();
        let ids = [-11, 3, 42, 999];
        let fixed = b
            .score_fixed("dense", s.port(), ids, |v| SimpleScore::new(v * v - 7))
            .unwrap();
        let normal = b
            .score("tree", s.port(), |v| SimpleScore::new(v * v - 7))
            .unwrap();
        let mut g = b.build();
        let mut expected = BTreeMap::new();
        for round in 0..160 {
            let before = g.constraint_matches(&fixed).unwrap().clone();
            let mut next = expected.clone();
            let mut batch = StreamBatch::new();
            for n in 0..7 {
                let id = ids[(round + n * 3) % ids.len()];
                let value = (round as i64 + n as i64) % 21 - 10;
                let change = if (round + n) % 3 == 0 {
                    next.remove(&id);
                    TupleChange::retract(id)
                } else {
                    next.insert(id, value);
                    TupleChange::put(id, value)
                };
                batch.push(&s, change).unwrap();
            }
            let mut tx = g.transaction();
            tx.apply_batch(batch).unwrap();
            assert_eq!(
                tx.score().unwrap(),
                SimpleScore::new(next.values().map(|v| 2 * (v * v - 7)).sum())
            );
            let graph = tx.graph().unwrap();
            assert_eq!(
                graph.constraint_matches(&fixed).unwrap(),
                graph.constraint_matches(&normal).unwrap()
            );
            let actual: BTreeMap<_, _> = graph
                .constraint_matches(&fixed)
                .unwrap()
                .iter()
                .map(|(&i, &(v, _))| (i, v))
                .collect();
            assert_eq!(actual, next);
            if round % 19 == 0 {
                tx.assert_consistent().unwrap();
            }
            if round % 3 == 0 {
                drop(tx);
                assert_eq!(g.constraint_matches(&fixed).unwrap(), &before);
            } else {
                tx.commit().unwrap();
                expected = next;
            }
        }
        g.assert_consistent().unwrap();
    }
}
#[test]
fn rejected_ids_and_callbacks_restore_earlier_changes_and_allow_retry() {
    let fail = Cell::new(false);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, i64>();
    let foreign = StreamGraphBuilder::<SimpleScore>::new().source::<usize, i64>();
    assert!(matches!(
        b.score_fixed("x", foreign.port(), [0], |_| SimpleScore::ZERO),
        Err(GraphError::ForeignPort)
    ));
    assert!(matches!(
        b.score_fixed("x", s.port(), [0, 0], |_| SimpleScore::ZERO),
        Err(GraphError::Dependency(_))
    ));
    assert!(matches!(
        b.score_fixed_weighted(
            "x",
            s.port(),
            [0],
            SimpleScore::ZERO.with_structural(-1),
            |_| 1
        ),
        Err(GraphError::StructuralConstraintWeight(_))
    ));
    let c = b
        .score_fixed("x", s.port(), [0, 1], |&v| {
            assert!(!fail.get());
            SimpleScore::new(v)
        })
        .unwrap();
    assert!(matches!(
        b.score_fixed("x", s.port(), [0], |_| SimpleScore::ZERO),
        Err(GraphError::DuplicateConstraint(_))
    ));
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(0, 4)).unwrap();
        tx.commit().unwrap();
    }
    let held = g.constraint_matches(&c).unwrap().clone();
    for change in [TupleChange::put(99, 100), TupleChange::retract(99)] {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(0, 8)).unwrap();
        assert!(matches!(
            tx.apply(&s, change),
            Err(GraphError::Dependency(_))
        ));
        assert!(tx.score().is_err());
        drop(tx);
        assert_eq!(g.constraint_matches(&c).unwrap(), &held);
    }
    fail.set(true);
    {
        let mut tx = g.transaction();
        assert!(catch_unwind(AssertUnwindSafe(|| tx.apply(&s, TupleChange::put(1, 2)))).is_err());
    }
    fail.set(false);
    assert_eq!(g.score(), SimpleScore::new(4));
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(1, 2)).unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(g.score(), SimpleScore::new(6));
    g.assert_consistent().unwrap();
}
#[derive(Debug)]
struct Row {
    value: i64,
    clones: Rc<Cell<usize>>,
    fail: Rc<Cell<bool>>,
}
impl Clone for Row {
    fn clone(&self) -> Self {
        assert!(!self.fail.get(), "payload clone");
        self.clones.set(self.clones.get() + 1);
        Self {
            value: self.value,
            clones: self.clones.clone(),
            fail: self.fail.clone(),
        }
    }
}
#[test]
fn match_materialization_is_lazy_cached_and_recoverable_after_clone_panic() {
    let clones = Rc::new(Cell::new(0));
    let fail = Rc::new(Cell::new(false));
    let arm = Cell::new(false);
    let mut b = StreamGraphBuilder::new().with_activity_pruning();
    let s = b.source::<usize, Row>();
    let c = b
        .score_fixed("value", s.port(), 0..3, |r| {
            if arm.get() {
                fail.set(true);
            }
            SimpleScore::new(r.value)
        })
        .unwrap();
    let mut g = b.build();
    let make = |value| Row {
        value,
        clones: clones.clone(),
        fail: fail.clone(),
    };
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(1, make(7))).unwrap();
        tx.commit().unwrap();
    }
    let before = clones.get();
    fail.set(true);
    g.activity(); // Structural inspection must not materialize matches or clone rows.
    assert!(catch_unwind(AssertUnwindSafe(|| g.constraint_matches(&c))).is_err());
    assert_eq!(g.score(), SimpleScore::new(7));
    fail.set(false);
    let held = g.constraint_matches(&c).unwrap().clone();
    assert_eq!(clones.get(), before + 1);
    assert_eq!(g.constraint_matches(&c).unwrap()[&1].0.value, 7);
    assert_eq!(clones.get(), before + 1);
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(1, make(9))).unwrap();
        assert_eq!(
            tx.graph().unwrap().constraint_matches(&c).unwrap()[&1]
                .0
                .value,
            9
        );
    }
    assert_eq!(g.constraint_matches(&c).unwrap()[&1].0.value, 7);
    assert_eq!(held[&1].0.value, 7);
    arm.set(true);
    {
        let mut tx = g.transaction();
        assert!(
            catch_unwind(AssertUnwindSafe(
                || tx.apply(&s, TupleChange::put(1, make(12)))
            ))
            .is_err()
        );
        assert!(tx.score().is_err());
    }
    arm.set(false);
    fail.set(false);
    assert_eq!(g.score(), SimpleScore::new(7));
    g.assert_consistent().unwrap();
}
#[test]
fn fixed_weighted_overrides_zero_pruning_and_decimal_products() {
    for weight in [0, -3, 2] {
        let calls = Cell::new(0);
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let s = b.source::<usize, i64>();
        let c = b
            .score_fixed_weighted("w", s.port(), [0, 1], SimpleScore::new(1), |v| {
                calls.set(calls.get() + 1);
                *v
            })
            .unwrap();
        let config = [("w".to_owned(), SimpleScore::new(weight))]
            .into_iter()
            .collect();
        let mut g = b.build_with_weights(&config).unwrap();
        {
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::put(0, 7)).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(g.score(), SimpleScore::new(7 * weight));
        if weight == 0 {
            assert_eq!(calls.get(), 0);
            assert!(g.constraint_matches(&c).unwrap().is_empty());
        } else {
            assert_eq!(
                g.explain_constraint_default(&c, |v| vec![*v])
                    .unwrap()
                    .score,
                SimpleScore::new(7 * weight)
            );
        }
        g.assert_consistent().unwrap();
    }
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let s = b.source::<usize, Decimal>();
    let c = b
        .score_fixed_weighted_decimal(
            "fraction",
            s.port(),
            [0, 1],
            "0.1".parse().unwrap(),
            Clone::clone,
        )
        .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(0, "0.025".parse().unwrap()))
            .unwrap();
        tx.commit().unwrap();
    }
    assert_eq!(g.score(), "0.0025".parse().unwrap());
    let held = g.constraint_matches(&c).unwrap().clone();
    {
        let mut tx = g.transaction();
        tx.apply(&s, TupleChange::put(1, "-0.125".parse().unwrap()))
            .unwrap();
        assert_eq!(tx.score().unwrap(), "-0.0100".parse().unwrap());
        tx.assert_consistent().unwrap();
    }
    assert_eq!(g.constraint_matches(&c).unwrap(), &held);
}
#[test]
fn checked_wrapping_and_structural_errors_match_normal_terminal_rules() {
    for arithmetic in [ScoringArithmetic::Checked, ScoringArithmetic::Wrapping] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(arithmetic);
        let s = b.source::<usize, i64>();
        b.score_fixed("sum", s.port(), [0, 1], |&v| SimpleScore::new(v))
            .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::put(0, i64::MAX)).unwrap();
            tx.commit().unwrap();
        }
        {
            let mut tx = g.transaction();
            let result = tx.apply(&s, TupleChange::put(1, 1));
            if arithmetic == ScoringArithmetic::Checked {
                assert!(matches!(result, Err(GraphError::ScoreOverflow { .. })));
            } else {
                result.unwrap();
                assert_eq!(tx.score().unwrap(), SimpleScore::new(i64::MIN));
            }
        }
        assert_eq!(g.score(), SimpleScore::new(i64::MAX));
        g.assert_consistent().unwrap();
    }
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, ()>();
    b.score_fixed("bad", s.port(), [0], |_| {
        SimpleScore::ZERO.with_structural(-1)
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    assert!(matches!(
        tx.apply(&s, TupleChange::put(0, ())),
        Err(GraphError::StructuralContribution(_))
    ));
}

#[test]
fn weighted_impact_overflow_rolls_back_and_wrapping_is_explicit() {
    for arithmetic in [ScoringArithmetic::Checked, ScoringArithmetic::Wrapping] {
        let mut b = StreamGraphBuilder::<SimpleScore>::new().with_arithmetic(arithmetic);
        let s = b.source::<usize, i64>();
        let c = b
            .score_fixed_weighted("scaled", s.port(), [0], SimpleScore::new(2), |v| *v)
            .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            let result = tx.apply(&s, TupleChange::put(0, i64::MAX));
            if arithmetic == ScoringArithmetic::Checked {
                assert!(matches!(result, Err(GraphError::ScoreOverflow { .. })));
            } else {
                result.unwrap();
                assert_eq!(tx.score().unwrap(), SimpleScore::new(-2));
            }
        }
        assert_eq!(g.score(), SimpleScore::ZERO);
        assert!(g.constraint_matches(&c).unwrap().is_empty());
        g.assert_consistent().unwrap();
    }
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let s = b.source::<usize, Decimal>();
    let weight = SimpleDecimalScore::new(Decimal::new(1.into(), i32::MAX));
    b.score_fixed_weighted_decimal("scaled", s.port(), [0], weight, Clone::clone)
        .unwrap();
    let mut g = b.build();
    {
        let mut tx = g.transaction();
        assert!(matches!(
            tx.apply(&s, TupleChange::put(0, Decimal::new(1.into(), 1))),
            Err(GraphError::DecimalWeightArithmetic { .. })
        ));
    }
    assert_eq!(g.score(), SimpleDecimalScore::ZERO);
}
