//! Original fractional-weight and transactional failure regressions.
use timetuck::*;
fn d(text: &str) -> Decimal {
    text.parse().unwrap()
}
fn s(text: &str) -> SimpleDecimalScore {
    text.parse().unwrap()
}

#[test]
fn exact_products_retain_scale_and_digits_instead_of_flooring() {
    assert_eq!(d("1.20").multiply_exact(&d("3.00")).unwrap(), d("3.6000"));
    assert_eq!(
        d("0.00").multiply_exact(&d("7E+3")).unwrap(),
        Decimal::new(0.into(), -1)
    );
    let weight = s("0.1");
    assert_eq!(
        weight.multiply(&d("0.025")).unwrap(),
        SimpleDecimalScore::ZERO
    );
    assert_eq!(weight.multiply_exact(&d("0.025")).unwrap(), s("0.0025"));
    assert_eq!(weight.multiply_exact(&d("-0.025")).unwrap(), s("-0.0025"));
    let large = d("123456789012345678901234567890.123456789");
    let product = large.multiply_exact(&d("0.000000000000000001")).unwrap();
    assert_eq!(product.coefficient(), large.coefficient());
    assert_eq!(product.scale(), large.scale() + 18);
    let bendable: BendableDecimalScore<1, 2> = "[0.01]hard/[2.5/0.0001]soft".parse().unwrap();
    assert_eq!(
        bendable.multiply_exact(&d("0.001")).unwrap(),
        "[0.00001]hard/[0.0025/0.0000001]soft".parse().unwrap()
    );
}
#[test]
fn scale_overflow_is_typed_even_for_zero_and_never_allocates_a_scale_gap() {
    for (a, b) in [(i32::MAX, 1), (i32::MIN, -1)] {
        for coefficient in [0, 1] {
            assert_eq!(
                Decimal::new(coefficient.into(), a).multiply_exact(&Decimal::new(1.into(), b)),
                Err(DecimalArithmeticError::ScaleOverflow)
            );
        }
    }
    assert_eq!(
        Decimal::new(3.into(), i32::MAX)
            .multiply_exact(&Decimal::new(2.into(), i32::MIN))
            .unwrap(),
        Decimal::new(6.into(), -1)
    );
}
#[test]
fn weighted_fractions_use_overrides_undo_and_exact_explanations() {
    let mut b = StreamGraphBuilder::<HardSoftDecimalScore>::new();
    let source = b.source::<usize, (u32, Decimal)>();
    let constraint = b
        .score_weighted_decimal(
            "fraction",
            source.port(),
            "1hard/1soft".parse().unwrap(),
            |(_, factor)| factor.clone(),
        )
        .unwrap();
    let weights = [("fraction".to_owned(), "0.1hard/2.0soft".parse().unwrap())]
        .into_iter()
        .collect();
    let mut g = b.build_with_weights(&weights).unwrap();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, (7, d("-0.025"))))
        .unwrap();
    tx.apply(&source, TupleChange::put(1, (8, d("0.1"))))
        .unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), "0.0075hard/0.15soft".parse().unwrap());
    let old = g
        .explain_constraint_default(&constraint, |(fact, _)| vec![*fact])
        .unwrap();
    assert_eq!(
        old.indictments[&7].score,
        "-0.0025hard/-0.05soft".parse().unwrap()
    );
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, (7, d("0.03"))))
            .unwrap();
        assert_eq!(tx.score().unwrap(), "0.013hard/0.26soft".parse().unwrap());
    }
    assert_eq!(
        g.explain_constraint_default(&constraint, |(fact, _)| vec![*fact])
            .unwrap(),
        old
    );
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), "-0.0025hard/-0.05soft".parse().unwrap());
    g.assert_consistent().unwrap();
}
#[test]
fn failed_impact_poisons_trial_and_restores_all_preceding_updates_on_drop() {
    use std::error::Error;
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let source = b.source::<usize, Decimal>();
    b.score("first", source.port(), |value| {
        if value.scale() == 0 { s("1") } else { s("2") }
    })
    .unwrap();
    let c = b
        .score_weighted_decimal("fraction", source.port(), s("0.1"), Clone::clone)
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, d("1"))).unwrap();
    tx.commit().unwrap();
    let old = g
        .explain_constraint_default(&c, |value| vec![value.clone()])
        .unwrap();
    {
        let mut tx = g.transaction();
        let error = tx
            .apply(
                &source,
                TupleChange::put(0, Decimal::new(1.into(), i32::MAX)),
            )
            .unwrap_err();
        assert_eq!(
            error,
            GraphError::DecimalWeightArithmetic {
                constraint: "fraction".into(),
                error: DecimalArithmeticError::ScaleOverflow
            }
        );
        assert_eq!(
            error
                .source()
                .unwrap()
                .downcast_ref::<DecimalArithmeticError>(),
            Some(&DecimalArithmeticError::ScaleOverflow)
        );
        assert_eq!(tx.score(), Err(GraphError::FailedTransaction));
        assert_eq!(
            tx.apply(&source, TupleChange::retract(0)),
            Err(GraphError::FailedTransaction)
        );
    }
    assert_eq!(g.score(), s("1.1"));
    assert_eq!(g.source_rows(&source).unwrap()[&0], d("1"));
    assert_eq!(
        g.explain_constraint_default(&c, |value| vec![value.clone()])
            .unwrap(),
        old
    );
    g.assert_consistent().unwrap();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, d("0.25"))).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), s("2.025"));
}
#[test]
fn zero_overrides_prune_fraction_callbacks_and_their_ancestors() {
    let mut b = StreamGraphBuilder::<SimpleDecimalScore>::new();
    let source = b.source::<usize, Decimal>();
    let filtered = b
        .filter(source.port(), |_| panic!("disabled predicate"))
        .unwrap();
    let c = b
        .score_weighted_decimal("off", &filtered, s("1"), |_| panic!("disabled factor"))
        .unwrap();
    let weights = [("off".into(), SimpleDecimalScore::ZERO)]
        .into_iter()
        .collect();
    let mut g = b.build_with_weights(&weights).unwrap();
    let mut tx = g.transaction();
    tx.apply(
        &source,
        TupleChange::put(0, Decimal::new(1.into(), i32::MAX)),
    )
    .unwrap();
    assert_eq!(tx.score_evaluations(), 0);
    tx.commit().unwrap();
    assert!(g.constraint_matches(&c).unwrap().is_empty());
    g.assert_consistent().unwrap();
}
#[test]
fn tiny_fractional_impacts_drive_all_three_stream_solvers_to_the_exact_optimum() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]); 2]).unwrap();
    let lists = ListProblem::new(vec![7usize, 9], vec![ListEntity::new(vec![]); 3]).unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    for mode in 0..3 {
        let plan = || SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
            ..Default::default()
        };
        macro_rules! finish {
            ($r:expr) => {{
                let r = $r.unwrap();
                assert!(r.complete);
                assert_eq!(r.termination, SolverTermination::Optimal);
                r.score
            }};
        }
        let score = match mode {
            0 => finish!(solve_stream(
                &basic,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = b.source::<usize, (usize, Option<usize>)>();
                    b.score_weighted_decimal("value", source.port(), s("0.001"), |(_, v)| {
                        Decimal::new(v.unwrap_or(0).into(), 4)
                    })?;
                    Ok((b.build(), source))
                },
                plan()
            )),
            1 => finish!(solve_list_stream(
                &lists,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = ListStreamSources::<usize>::new(&mut b);
                    b.score_weighted_decimal("value", source.values.port(), s("0.001"), |v| {
                        Decimal::new(v.position.map_or(0, |p| p.entity).into(), 4)
                    })?;
                    Ok((b.build(), source))
                },
                plan()
            )),
            _ => finish!(solve_mixed_stream(
                &mixed,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let source = MixedStreamSources::<usize, usize>::new(&mut b);
                    let a = b.map(source.basic.port(), |(_, v)| v.unwrap_or(0))?;
                    let z = b.map(source.lists.values.port(), |v| {
                        v.position.map_or(0, |p| p.entity)
                    })?;
                    let values = b.concat(&a, &z)?;
                    b.score_weighted_decimal("value", &values, s("0.001"), |v| {
                        Decimal::new((*v).into(), 4)
                    })?;
                    Ok((b.build(), source))
                },
                MixedSolverSequence {
                    phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
                    ..Default::default()
                }
            )),
        };
        assert_eq!(
            score,
            Some(s(if mode == 2 { "0.0000008" } else { "0.0000004" }))
        );
    }
}
