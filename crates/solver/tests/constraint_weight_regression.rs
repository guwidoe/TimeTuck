//! Original weighted-graph regressions; no upstream method credit.
use std::cell::Cell;
use timetuck::*;
fn weights(name: &str, n: i64) -> ConstraintWeightOverrides<SimpleScore> {
    [(name.to_owned(), SimpleScore::new(n))]
        .into_iter()
        .collect()
}
#[test]
fn zero_pruning_keeps_shared_and_unweighted_branches_live() {
    let common_calls = Cell::new(0);
    let active_calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let shared = b
        .filter(source.port(), |_| {
            common_calls.set(common_calls.get() + 1);
            true
        })
        .unwrap();
    let dead = b
        .map(&shared, |_: &i64| -> i64 {
            panic!("disabled branch mapping")
        })
        .unwrap();
    let disabled = b
        .score_weighted("disabled", &dead, SimpleScore::ZERO, |_| {
            panic!("disabled factor")
        })
        .unwrap();
    let active = b
        .score_weighted("active", &shared, SimpleScore::new(2), |v| {
            active_calls.set(active_calls.get() + 1);
            *v
        })
        .unwrap();
    b.score("ordinary zero", &shared, |_| SimpleScore::ZERO)
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, 3)).unwrap();
    assert_eq!(tx.score_evaluations(), 2);
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(6));
    assert_eq!(common_calls.get(), 1);
    assert_eq!(active_calls.get(), 1);
    assert!(g.constraint_matches(&disabled).unwrap().is_empty());
    assert_eq!(
        g.constraint_matches(&active).unwrap()[&0].1,
        SimpleScore::new(6)
    );
    assert_eq!(g.constraint_scores().count(), 3);
    {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::put(0, 10)).unwrap();
        assert_eq!(tx.score().unwrap(), SimpleScore::new(20));
    }
    assert_eq!(g.score(), SimpleScore::new(6));
    let e = g.explain_constraint_default(&active, |v| vec![*v]).unwrap();
    assert_eq!(e.score, SimpleScore::new(6));
    assert_eq!(e.indictments[&3].score, SimpleScore::new(6));
    g.assert_consistent().unwrap();
}
#[test]
fn overrides_activate_defaults_and_are_frozen_before_initialization() {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, i64>();
    let filtered = b
        .filter(source.port(), |_| {
            calls.set(calls.get() + 1);
            true
        })
        .unwrap();
    b.score_weighted("weight", &filtered, SimpleScore::ZERO, |n| *n)
        .unwrap();
    let mut config = weights("weight", 5);
    let mut g = b.build_with_weights(&config).unwrap();
    config.insert("weight", SimpleScore::ZERO);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put(0, -3)).unwrap();
    tx.commit().unwrap();
    assert_eq!(g.score(), SimpleScore::new(-15));
    assert_eq!(calls.get(), 1);
}
#[test]
fn rejected_configuration_never_invokes_callbacks() {
    for kind in 0..3 {
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let source = b.source::<usize, ()>();
        b.score_weighted("weight", source.port(), SimpleScore::new(1), |_| panic!())
            .unwrap();
        b.score("direct", source.port(), |_| panic!()).unwrap();
        let mut config = match kind {
            0 => weights("unknown", 1),
            1 => weights("direct", 1),
            _ => weights("weight", 1),
        };
        if kind == 2 {
            config.insert("weight", SimpleScore::new(1).with_structural(-1));
        }
        let err = match b.build_with_weights(&config) {
            Ok(_) => panic!("accepted invalid configuration"),
            Err(e) => e,
        };
        assert!(matches!(
            (kind, err),
            (0, GraphError::UnknownConstraintWeights { .. })
                | (1, GraphError::UnweightedConstraintOverride(_))
                | (2, GraphError::StructuralConstraintWeight(_))
        ));
    }
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = b.source::<usize, ()>();
    assert!(matches!(
        b.score_weighted(
            "bad",
            s.port(),
            SimpleScore::ZERO.with_structural(-1),
            |_| 1
        ),
        Err(GraphError::StructuralConstraintWeight(_))
    ));
    let foreign = StreamGraphBuilder::<SimpleScore>::new().source::<usize, ()>();
    assert!(matches!(
        b.score_weighted("foreign", foreign.port(), SimpleScore::ZERO, |_| 1),
        Err(GraphError::ForeignPort)
    ));
}
#[test]
fn integer_scaling_covers_full_width_and_decimal_levels_without_floats() {
    for weight in [i64::MIN, -13, 0, 7, i64::MAX] {
        for factor in [i64::MIN, -17, -1, 0, 1, 19, i64::MAX] {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = b.source::<usize, i64>();
            b.score_weighted("n", s.port(), SimpleScore::new(weight), |v| *v)
                .unwrap();
            let mut g = b.build();
            let mut tx = g.transaction();
            tx.apply(&s, TupleChange::put(0, factor)).unwrap();
            tx.commit().unwrap();
            assert_eq!(g.score(), SimpleScore::new(weight.wrapping_mul(factor)));
        }
    }
    let mut b = StreamGraphBuilder::<HardSoftDecimalScore>::new();
    let s = b.source::<usize, i64>();
    b.score_weighted(
        "decimal",
        s.port(),
        "0.000000000000000001hard/1.25soft".parse().unwrap(),
        |v| *v,
    )
    .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&s, TupleChange::put(0, -3)).unwrap();
    tx.commit().unwrap();
    assert_eq!(
        g.score(),
        "-0.000000000000000003hard/-3.75soft".parse().unwrap()
    );
    g.assert_consistent().unwrap();
}
#[test]
fn dead_join_ancestors_are_pruned_on_both_sides_without_losing_sources() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let a = b.source::<usize, i64>();
    let z = b.source::<usize, i64>();
    let left = b.filter(a.port(), |_| panic!("dead left")).unwrap();
    let right = b.filter(z.port(), |_| panic!("dead right")).unwrap();
    let pairs = b.join_equal(&left, &right, |v| *v, |v| *v).unwrap();
    b.score_weighted("zero", &pairs, SimpleScore::ZERO, |_| panic!("dead score"))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&a, TupleChange::put(0, 1)).unwrap();
    tx.apply(&z, TupleChange::put(0, 1)).unwrap();
    assert_eq!(tx.score_evaluations(), 0);
    tx.commit().unwrap();
    assert_eq!(g.source_rows(&a).unwrap()[&0], 1);
    assert_eq!(g.source_rows(&z).unwrap()[&0], 1);
    g.assert_consistent().unwrap();
}
#[test]
fn weight_changes_shift_optima_in_basic_list_and_mixed_stream_solves() {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]); 2]).unwrap();
    let lists = ListProblem::new(vec![7usize, 9], vec![ListEntity::new(vec![]); 3]).unwrap();
    let mixed = MixedProblem::new(basic.clone(), lists.clone());
    for mode in 0..3 {
        for weight in [-2, 0, 3] {
            let calls = Cell::new(0);
            let config = weights("values", weight);
            let plan = || SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
                ..Default::default()
            };
            let result = match mode {
                0 => {
                    solve_stream(
                        &basic,
                        || {
                            let mut b = StreamGraphBuilder::new();
                            let s = b.source::<usize, (usize, Option<usize>)>();
                            b.score_weighted("values", s.port(), SimpleScore::new(1), |(_, v)| {
                                calls.set(calls.get() + 1);
                                v.unwrap_or(0) as i64
                            })?;
                            Ok((b.build_with_weights(&config)?, s))
                        },
                        plan(),
                    )
                    .unwrap()
                    .score
                }
                1 => {
                    solve_list_stream(
                        &lists,
                        || {
                            let mut b = StreamGraphBuilder::new();
                            let s = ListStreamSources::<usize>::new(&mut b);
                            b.score_weighted(
                                "values",
                                s.values.port(),
                                SimpleScore::new(1),
                                |v| {
                                    calls.set(calls.get() + 1);
                                    v.position.map_or(0, |p| p.entity as i64)
                                },
                            )?;
                            Ok((b.build_with_weights(&config)?, s))
                        },
                        plan(),
                    )
                    .unwrap()
                    .score
                }
                _ => {
                    solve_mixed_stream(
                        &mixed,
                        || {
                            let mut b = StreamGraphBuilder::new();
                            let s = MixedStreamSources::<usize, usize>::new(&mut b);
                            let basics = b.map(s.basic.port(), |(_, v)| v.unwrap_or(0) as i64)?;
                            let lists = b.map(s.lists.values.port(), |v| {
                                v.position.map_or(0, |p| p.entity as i64)
                            })?;
                            let values = b.concat(&basics, &lists)?;
                            b.score_weighted("values", &values, SimpleScore::new(1), |v| {
                                calls.set(calls.get() + 1);
                                *v
                            })?;
                            Ok((b.build_with_weights(&config)?, s))
                        },
                        MixedSolverSequence {
                            phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
                            ..Default::default()
                        },
                    )
                    .unwrap()
                    .score
                }
            };
            assert_eq!(
                result,
                Some(SimpleScore::new(if weight > 0 {
                    weight * if mode == 2 { 8 } else { 4 }
                } else {
                    0
                }))
            );
            assert_eq!(calls.get() == 0, weight == 0);
        }
    }
}
