//! Original scoped configuration, pruning, transaction and solving regressions.
use std::cell::Cell;
use timetuck::*;
fn weights<S>(items: &[(&str, S)]) -> ConstraintWeightOverrides<S>
where
    S: Clone,
{
    items
        .iter()
        .map(|(n, w)| ((*n).to_owned(), w.clone()))
        .collect()
}
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
fn d(n: &str) -> Decimal {
    n.parse().unwrap()
}
fn ds(n: &str) -> SimpleDecimalScore {
    n.parse().unwrap()
}

#[test]
fn configuration_is_atomic_and_only_weighted_constraints_can_be_overridden() {
    macro_rules! check {
        ($network:expr) => {{
            let mut n = $network;
            n.add_weighted_constraint("a", vec![], s(2), |_| {
                panic!("configuration invoked callback")
            })
            .unwrap();
            n.add_constraint("direct", vec![], |_| {
                panic!("configuration invoked callback")
            })
            .unwrap();
            n.add_weighted_constraint("zero", vec![], s(0), |_| {
                panic!("configuration invoked callback")
            })
            .unwrap();
            for (name, value, expected) in [
                (
                    "unknown",
                    s(1),
                    ConstraintWeightError::UnknownConstraint("unknown".into()),
                ),
                (
                    "direct",
                    s(1),
                    ConstraintWeightError::UnweightedConstraint("direct".into()),
                ),
                (
                    "zero",
                    s(1).with_structural(-1),
                    ConstraintWeightError::StructuralWeight("zero".into()),
                ),
            ] {
                assert_eq!(
                    n.apply_weight_overrides(&weights(&[("a", s(9)), (name, value)])),
                    Err(expected)
                );
                assert_eq!(
                    n.constraint_weights()
                        .map(|(n, w)| (n, *w))
                        .collect::<Vec<_>>(),
                    vec![("a", s(2)), ("zero", s(0))]
                );
            }
            assert!(
                n.add_weighted_constraint("bad", vec![], s(1).with_structural(-1), |_| 1)
                    .is_err()
            );
            assert_eq!(n.constraint_count(), 3);
            assert_eq!(n.active_constraint_count(), 2);
            n.apply_weight_overrides(&weights(&[("a", s(0)), ("zero", s(-3))]))
                .unwrap();
            n.apply_weight_overrides(&weights(&[("a", s(4))])).unwrap();
            assert_eq!(
                n.constraint_weights()
                    .map(|(n, w)| (n, *w))
                    .collect::<Vec<_>>(),
                vec![("a", s(4)), ("zero", s(-3))]
            );
            assert_eq!(n.active_constraint_count(), 3);
        }};
    }
    check!(ConstraintNetwork::<i64, SimpleScore>::new(0));
    check!(ListConstraintNetwork::<i64, SimpleScore>::new(0, 0));
    check!(MixedConstraintNetwork::<i64, i64, SimpleScore>::new(
        0, 0, 0
    ));
}

#[test]
fn zero_weights_skip_callbacks_and_counts_and_can_be_reactivated_between_sessions() {
    let calls = Cell::new(0);
    macro_rules! check {
        ($network:expr,$problem:expr,$scope:expr,$director:ident,$movement:expr) => {{
            calls.set(0);
            let p = $problem;
            let mut n = $network;
            n.add_weighted_constraint("constant", vec![], s(2), |_| 3)
                .unwrap();
            n.add_weighted_constraint("disabled", vec![$scope], s(0), |_| {
                calls.set(calls.get() + 1);
                7
            })
            .unwrap();
            n.add_constraint("direct", vec![$scope], |_| s(0)).unwrap();
            let initial = p.initial_assignment();
            let mut director = $director::new(&p, &n, initial.clone()).unwrap();
            assert_eq!(director.score(), s(6));
            let movement = $movement;
            {
                let trial = director.apply(&movement).unwrap();
                assert_eq!(trial.constraint_evaluations(), 1);
                assert_eq!(trial.score(), s(6));
            }
            assert_eq!(director.assignment(), &initial);
            director.apply(&movement).unwrap().commit();
            director.assert_consistent().unwrap();
            assert_eq!(calls.get(), 0);
            assert_eq!(n.calculate(&p, director.assignment()).unwrap(), s(6));
            assert_eq!(calls.get(), 0);
            drop(director);
            n.apply_weight_overrides(&weights(&[("disabled", s(5))]))
                .unwrap();
            let mut director = $director::new(&p, &n, initial).unwrap();
            assert_eq!(director.score(), s(41));
            assert_eq!(calls.get(), 1);
            let trial = director.apply(&movement).unwrap();
            assert_eq!(trial.constraint_evaluations(), 2);
            assert_eq!(trial.score(), s(41));
            trial.commit();
            director.assert_consistent().unwrap();
        }};
    }
    let basic = || FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap();
    let lists = || {
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap()
    };
    let movement = || ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    check!(
        ConstraintNetwork::new(1),
        basic(),
        0,
        ScoreDirector,
        Move::Change {
            variable: 0,
            value: Some(2)
        }
    );
    check!(
        ListConstraintNetwork::new(2, 1),
        lists(),
        ListScope::Value(0),
        ListScoreDirector,
        movement()
    );
    check!(
        MixedConstraintNetwork::new(1, 2, 1),
        MixedProblem::new(basic(), lists()),
        MixedScope::ListValue(0),
        MixedScoreDirector,
        MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(2)
            },
            lists: movement()
        }
    );
}

#[test]
fn exact_decimal_errors_and_panics_restore_every_model_and_cached_contribution() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let fail = Cell::new(false);
    let panic = Cell::new(false);
    macro_rules! check {
        ($network:expr,$problem:expr,$scope:expr,$director:ident,$movement:expr,$error:ident) => {{
            let p = $problem;
            let mut n = $network;
            n.add_constraint("earlier", vec![$scope], |_| ds("2"))
                .unwrap();
            n.add_decimal_weighted_constraint("decimal", vec![$scope], ds("0.1"), |_| {
                if fail.get() {
                    assert!(!panic.get(), "factor panic");
                    Decimal::new(1.into(), i32::MAX)
                } else {
                    d("0.025")
                }
            })
            .unwrap();
            n.add_decimal_weighted_constraint("disabled", vec![$scope], ds("0"), |_| {
                panic!("disabled decimal")
            })
            .unwrap();
            let initial = p.initial_assignment();
            let mut director = $director::new(&p, &n, initial.clone()).unwrap();
            let original = ds("2.0025");
            assert_eq!(director.score(), original);
            let movement = $movement;
            for panicking in [false, true] {
                fail.set(true);
                panic.set(panicking);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    director.apply(&movement).map(|trial| trial.commit())
                }));
                if panicking {
                    assert!(result.is_err());
                } else {
                    let error = result.unwrap().unwrap_err();
                    assert!(std::error::Error::source(&error).is_some());
                    assert!(matches!(
                        error,
                        $error::DecimalWeightArithmetic {
                            constraint: 1,
                            error: DecimalArithmeticError::ScaleOverflow
                        }
                    ));
                }
                fail.set(false);
                panic.set(false);
                assert_eq!(director.assignment(), &initial);
                assert_eq!(director.score(), original);
                director.assert_consistent().unwrap();
            }
            director.apply(&movement).unwrap().commit();
            assert_ne!(director.assignment(), &initial);
            director.assert_consistent().unwrap();
        }};
    }
    let basic = || FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)]).unwrap();
    let lists = || {
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap()
    };
    let movement = || ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    };
    check!(
        ConstraintNetwork::new(1),
        basic(),
        0,
        ScoreDirector,
        Move::Change {
            variable: 0,
            value: Some(2)
        },
        ScoringError
    );
    check!(
        ListConstraintNetwork::new(2, 1),
        lists(),
        ListScope::Value(0),
        ListScoreDirector,
        movement(),
        ListScoringError
    );
    check!(
        MixedConstraintNetwork::new(1, 2, 1),
        MixedProblem::new(basic(), lists()),
        MixedScope::ListValue(0),
        MixedScoreDirector,
        MixedMove {
            basic: Move::Change {
                variable: 0,
                value: Some(2)
            },
            lists: movement()
        },
        MixedScoringError
    );
}

#[test]
fn fractional_overrides_select_different_optima_in_all_scoped_solvers() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]); 2]).unwrap();
    let lists = ListProblem::new(vec![7usize, 9], vec![ListEntity::new(vec![]); 3]).unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    let mut bn = ConstraintNetwork::new(2);
    bn.add_decimal_weighted_constraint("value", vec![0, 1], ds("0"), |values| {
        Decimal::new(
            values
                .iter()
                .filter_map(|v| v.copied())
                .sum::<usize>()
                .into(),
            4,
        )
    })
    .unwrap();
    let mut ln = ListConstraintNetwork::new(3, 2);
    ln.add_decimal_weighted_constraint(
        "value",
        vec![ListScope::Value(0), ListScope::Value(1)],
        ds("0"),
        |facts| {
            Decimal::new(
                facts
                    .iter()
                    .map(|f| match f {
                        ListFact::Value { element, .. } => {
                            element.as_ref().map_or(0, |e| e.position.entity)
                        }
                        _ => unreachable!(),
                    })
                    .sum::<usize>()
                    .into(),
                4,
            )
        },
    )
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(2, 3, 2);
    mn.add_decimal_weighted_constraint(
        "value",
        vec![
            MixedScope::Basic(0),
            MixedScope::Basic(1),
            MixedScope::ListValue(0),
            MixedScope::ListValue(1),
        ],
        ds("0"),
        |facts| {
            Decimal::new(
                facts
                    .iter()
                    .map(|f| match f {
                        MixedFact::Basic(v) => v.copied().unwrap_or(0),
                        MixedFact::ListValue { element, .. } => {
                            element.as_ref().map_or(0, |e| e.position.entity)
                        }
                        _ => unreachable!(),
                    })
                    .sum::<usize>()
                    .into(),
                4,
            )
        },
    )
    .unwrap();
    for weight in ["0.001", "-0.001", "0"] {
        let overrides = weights(&[("value", ds(weight))]);
        bn.apply_weight_overrides(&overrides).unwrap();
        ln.apply_weight_overrides(&overrides).unwrap();
        mn.apply_weight_overrides(&overrides).unwrap();
        let plan = || SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        };
        let b = solve(&basic, &bn, plan()).unwrap();
        let l = solve_list(&lists, &ln, plan()).unwrap();
        let m = solve_mixed(
            &mixed,
            &mn,
            MixedSolverSequence {
                phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
                ..Default::default()
            },
        )
        .unwrap();
        let want = if weight == "0.001" { "0.0000004" } else { "0" };
        assert_eq!(b.score, Some(ds(want)));
        assert_eq!(l.score, Some(ds(want)));
        assert_eq!(
            m.score,
            Some(ds(if weight == "0.001" { "0.0000008" } else { "0" }))
        );
        for (complete, reason, work) in [
            (b.complete, b.termination, b.work),
            (l.complete, l.termination, l.work),
            (m.complete, m.termination, m.work),
        ] {
            assert!(complete);
            assert_eq!(reason, SolverTermination::Optimal);
            if weight == "0" {
                assert_eq!(work.constraint_evaluations, 0);
            } else {
                assert!(work.constraint_evaluations > 0);
            }
        }
        assert_eq!(
            solve(&basic, &bn, SolverConfig::default()).unwrap().score,
            Some(ds(want))
        );
        assert_eq!(
            solve_list(&lists, &ln, SolverConfig::default())
                .unwrap()
                .score,
            Some(ds(want))
        );
        assert_eq!(
            solve_mixed(&mixed, &mn, MixedSolverConfig::default())
                .unwrap()
                .score,
            m.score
        );
    }
}
