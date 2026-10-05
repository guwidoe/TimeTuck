//! Original checked arithmetic and transactional regressions.
use timetuck::*;
fn s(n: i64) -> SimpleScore {
    SimpleScore::new(n)
}
#[test]
fn checked_integer_hooks_match_wide_arithmetic_at_signed_boundaries() {
    let values = [
        i64::MIN,
        i64::MIN + 1,
        -7,
        -1,
        0,
        1,
        7,
        i64::MAX - 1,
        i64::MAX,
    ];
    macro_rules! check {
        ($make:expr) => {{
            let make = $make;
            for a in values {
                for b in values {
                    let expected = i64::try_from(i128::from(a) * i128::from(b)).ok().map(make);
                    assert_eq!(make(a).with_structural(-9).checked_scale(b), expected);
                    for c in values {
                        let sum = i64::try_from(i128::from(a) + i128::from(b) + i128::from(c))
                            .ok()
                            .map(make);
                        assert_eq!(
                            AdditiveScore::checked_sum(&[make(a), make(b), make(c)]),
                            sum
                        );
                        let adjustment =
                            i64::try_from(i128::from(a) - i128::from(b) + i128::from(c))
                                .ok()
                                .map(make);
                        assert_eq!(
                            make(a)
                                .with_structural(-9)
                                .checked_adjust([(&make(b), &make(c))]),
                            adjustment
                        );
                    }
                }
            }
        }};
    }
    check!(SimpleScore::new);
    check!(|n| HardSoftScore::new(n, n));
    check!(|n| HardMediumSoftScore::new(n, n, n));
    check!(|n| BendableScore::new([n, n], [n]));
    assert!(HardSoftScore::new(0, i64::MAX).checked_scale(2).is_none());
    assert!(
        HardMediumSoftScore::new(0, i64::MIN, 0)
            .checked_scale(-1)
            .is_none()
    );
    assert!(
        BendableScore::<0, 1>::new([], [i64::MAX])
            .checked_scale(2)
            .is_none()
    );
    assert_eq!(
        BendableScore::<0, 0>::ZERO.checked_scale(i64::MIN),
        Some(BendableScore::ZERO)
    );
}
#[test]
fn stream_removal_overflow_restores_matches_and_diagnostics_allow_different_replay_order() {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, i64>();
    let c = b
        .score_weighted("sum", source.port(), s(1), |v| *v)
        .unwrap();
    let mut g = b.build();
    // Replay in ID order has an overflowing prefix; the retained final sum fits.
    for (id, value) in [(0, i64::MAX), (2, -i64::MAX), (1, i64::MAX)] {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(id, value)).unwrap();
        tx.commit().unwrap();
    }
    g.assert_consistent().unwrap();
    let before = g.explain_constraint_default(&c, |v| vec![*v]).unwrap();
    let mut tx = g.transaction();
    let error = tx.apply(&source, TupleChange::retract(2)).unwrap_err();
    assert_eq!(
        error,
        GraphError::ScoreOverflow {
            constraint: Some("sum".into())
        }
    );
    assert!(std::error::Error::source(&error).is_some());
    assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
    assert_eq!(tx.commit(), Err(GraphError::FailedTransaction));
    assert_eq!(
        g.explain_constraint_default(&c, |v| vec![*v]).unwrap(),
        before
    );
    assert_eq!(g.source_rows(&source).unwrap().len(), 3);
    g.assert_consistent().unwrap();
}
#[test]
fn graph_total_checks_whole_routed_update_and_rolls_back_all_terminals() {
    for overflow in [false, true] {
        let mut b = StreamGraphBuilder::new();
        let source = b.source::<usize, bool>();
        let a = b
            .score("a", source.port(), move |changed| {
                s(if *changed {
                    if overflow { i64::MAX } else { i64::MIN }
                } else if overflow {
                    0
                } else {
                    i64::MAX
                })
            })
            .unwrap();
        let z = b
            .score("z", source.port(), move |changed| {
                s(if *changed {
                    if overflow { 1 } else { i64::MAX }
                } else if overflow {
                    0
                } else {
                    i64::MIN
                })
            })
            .unwrap();
        let mut g = b.build();
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, false)).unwrap();
        tx.commit().unwrap();
        let before = g.score();
        let mut tx = g.transaction();
        let result = tx.apply(&source, TupleChange::put(0, true));
        if overflow {
            assert_eq!(result, Err(GraphError::ScoreOverflow { constraint: None }));
            drop(tx);
            assert_eq!(g.constraint_matches(&a).unwrap()[&0].1, s(0));
            assert_eq!(g.constraint_matches(&z).unwrap()[&0].1, s(0));
        } else {
            result.unwrap();
            tx.commit().unwrap();
            assert_eq!(g.constraint_matches(&a).unwrap()[&0].1, s(i64::MIN));
            assert_eq!(g.constraint_matches(&z).unwrap()[&0].1, s(i64::MAX));
        }
        assert_eq!(g.score(), before);
        g.assert_consistent().unwrap();
    }
}
#[test]
fn scoped_impact_and_total_failures_restore_all_three_models() {
    macro_rules! check {
        ($p:expr,$n:expr,$scope:expr,$changed:expr,$director:ident,$movement:expr,$error:ident) => {{
            let p = $p;
            for total in [false, true] {
                let mut n = $n;
                n.add_constraint("base", vec![], |_| s(if total { i64::MAX } else { 0 }))
                    .unwrap();
                let changed = $changed;
                n.add_weighted_constraint(
                    "impact",
                    vec![$scope],
                    s(if total { 1 } else { i64::MAX }),
                    move |facts| {
                        if changed(facts) {
                            if total { 1 } else { 2 }
                        } else {
                            0
                        }
                    },
                )
                .unwrap();
                let initial = p.initial_assignment();
                let mut director = $director::new(&p, &n, initial.clone()).unwrap();
                let original = director.score();
                let error = match director.apply(&$movement) {
                    Ok(_) => panic!("overflow accepted"),
                    Err(e) => e,
                };
                assert_eq!(
                    error,
                    $error::ScoreOverflow {
                        constraint: if total { None } else { Some(1) }
                    }
                );
                assert!(std::error::Error::source(&error).is_some());
                assert_eq!(director.assignment(), &initial);
                assert_eq!(director.score(), original);
                director.assert_consistent().unwrap();
            }
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
        basic(),
        ConstraintNetwork::new(1),
        0,
        |facts: &[Option<&i32>]| facts[0] == Some(&2),
        ScoreDirector,
        Move::Change {
            variable: 0,
            value: Some(2)
        },
        ScoringError
    );
    check!(
        lists(),
        ListConstraintNetwork::new(2, 1),
        ListScope::Value(0),
        |facts: &[ListFact<'_, i32>]| matches!(&facts[0],ListFact::Value {element:Some(e),..} if e.position.entity==1),
        ListScoreDirector,
        movement(),
        ListScoringError
    );
    check!(
        MixedProblem::new(basic(), lists()),
        MixedConstraintNetwork::new(1, 2, 1),
        MixedScope::Basic(0),
        |facts: &[MixedFact<'_, i32, i32>]| matches!(&facts[0], MixedFact::Basic(Some(2))),
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
fn scoped_totals_widen_initial_sums_and_complete_multi_constraint_adjustments() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("positive", vec![0], |v| {
        s(if v[0] == Some(&0) { i64::MAX } else { i64::MIN })
    })
    .unwrap();
    n.add_constraint("extra", vec![], |_| s(1)).unwrap();
    n.add_constraint("negative", vec![0], |v| {
        s(if v[0] == Some(&0) {
            -i64::MAX
        } else {
            i64::MAX
        })
    })
    .unwrap();
    assert_eq!(n.calculate(&p, &p.initial_assignment()).unwrap(), s(1));
    let mut d = ScoreDirector::new(&p, &n, p.initial_assignment()).unwrap();
    d.apply(&Move::Change {
        variable: 0,
        value: Some(1),
    })
    .unwrap()
    .commit();
    assert_eq!(d.score(), s(0));
    d.assert_consistent().unwrap();
    // Non-overflowing final values also work when replacing a negative match
    // would overflow a bounded subtraction before its replacement is added.
    assert_eq!(
        s(i64::MAX).checked_adjust([(&s(-1), &s(-1))]),
        Some(s(i64::MAX))
    );
}
#[test]
fn overflow_propagates_out_of_all_six_solver_paths() {
    let p = FiniteProblem::new(vec![Variable::new(vec![2i64])]).unwrap();
    let l = ListProblem::new(vec![7i64], vec![ListEntity::new(vec![])]).unwrap();
    let m = MixedProblem::new(p.clone(), l.clone());
    let mut n = ConstraintNetwork::new(1);
    n.add_weighted_constraint("cost", vec![0], s(i64::MAX), |v| v[0].copied().unwrap_or(0))
        .unwrap();
    let mut ln = ListConstraintNetwork::new(1, 1);
    ln.add_weighted_constraint("cost", vec![ListScope::Value(0)], s(i64::MAX), |v| {
        if matches!(
            &v[0],
            ListFact::Value {
                element: Some(_),
                ..
            }
        ) {
            2
        } else {
            0
        }
    })
    .unwrap();
    let mut mn = MixedConstraintNetwork::new(1, 1, 1);
    mn.add_weighted_constraint("cost", vec![MixedScope::Basic(0)], s(i64::MAX), |v| {
        if let MixedFact::Basic(x) = v[0] {
            x.copied().unwrap_or(0)
        } else {
            0
        }
    })
    .unwrap();
    macro_rules! rejects {
        ($result:expr) => {
            assert!(format!("{:?}", $result.unwrap_err()).contains("ScoreOverflow"));
        };
    }
    for exact in [false, true] {
        let config = || {
            if exact {
                SolverSequence {
                    phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
                    ..Default::default()
                }
            } else {
                SolverConfig::default().into()
            }
        };
        let mixed = || {
            if exact {
                MixedSolverSequence {
                    phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
                    ..Default::default()
                }
            } else {
                MixedSolverConfig::default().into()
            }
        };
        rejects!(solve(&p, &n, config()));
        rejects!(solve_list(&l, &ln, config()));
        rejects!(solve_mixed(&m, &mn, mixed()));
        rejects!(solve_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i64>)>();
                b.score_weighted("cost", source.port(), s(i64::MAX), |(_, v)| v.unwrap_or(0))?;
                Ok((b.build(), source))
            },
            config()
        ));
        rejects!(solve_list_stream(
            &l,
            || {
                let mut b = StreamGraphBuilder::new();
                let source = ListStreamSources::<i64>::new(&mut b);
                b.score_weighted("cost", source.values.port(), s(i64::MAX), |v| {
                    if v.position.is_some() { 2 } else { 0 }
                })?;
                Ok((b.build(), source))
            },
            config()
        ));
        rejects!(solve_mixed_stream(
            &m,
            || {
                let mut b = StreamGraphBuilder::new();
                let source = MixedStreamSources::<i64, i64>::new(&mut b);
                b.score_weighted("cost", source.basic.port(), s(i64::MAX), |(_, v)| {
                    v.unwrap_or(0)
                })?;
                Ok((b.build(), source))
            },
            mixed()
        ));
    }
}

#[test]
fn diagnostic_replay_keeps_impact_checks_even_when_modular_products_would_match() {
    use std::cell::Cell;
    let corrupt = Cell::new(false);
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, i64>();
    b.score_weighted("checked", source.port(), s(2), |v| {
        if corrupt.get() { i64::MIN + 1 } else { *v }
    })
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 1)).unwrap();
    tx.commit().unwrap();
    corrupt.set(true);
    assert_eq!(
        g.assert_consistent(),
        Err(GraphError::ScoreOverflow {
            constraint: Some("checked".into())
        })
    );
    assert_eq!(g.score(), s(2));
    corrupt.set(false);
    g.assert_consistent().unwrap();
}
