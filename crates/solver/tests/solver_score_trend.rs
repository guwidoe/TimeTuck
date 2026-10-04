//! Original real-scorer tests of declared trends in all six solve paths.
use std::cell::Cell;
use timetuck::*;
const ORDERS: [NodeExploration; 4] = [
    NodeExploration::DepthFirst,
    NodeExploration::BreadthFirst,
    NodeExploration::ScoreFirst,
    NodeExploration::OptimisticBoundFirst,
];
fn config(
    order: NodeExploration,
    bound: PartialScoreBound,
) -> ExactPhaseConfig<'static, HardSoftScore> {
    ExactPhaseConfig {
        strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
            exploration: order,
            bound,
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn run(
    mode: usize,
    c: ExactPhaseConfig<'_, HardSoftScore>,
    score: impl Fn(i64) -> HardSoftScore + Copy,
) -> Result<(SolverTermination, Option<HardSoftScore>, SolverWork), String> {
    let basic = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]); 2]).unwrap();
    let lists = ListProblem::new(vec![7usize, 9], vec![ListEntity::new(vec![]); 3]).unwrap();
    macro_rules! finish {
        ($r:expr,$p:expr) => {{
            let r = $r.map_err(|e| e.to_string())?;
            $p.validate(&r.assignment).unwrap();
            assert_eq!(r.complete, $p.is_complete(&r.assignment));
            Ok((r.termination, r.score, r.work))
        }};
    }
    if mode < 2 {
        let plan = SolverSequence {
            phases: vec![SolverPhase::Exact(c)],
            ..Default::default()
        };
        if mode == 0 {
            let mut n = ConstraintNetwork::new(2);
            for i in 0..2 {
                n.add_constraint(format!("v{i}"), vec![i], move |v| {
                    score(v[0].copied().unwrap_or(0))
                })
                .unwrap();
            }
            return finish!(solve(&basic, &n, plan), basic);
        }
        return finish!(
            solve_stream(
                &basic,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = b.source::<usize, (usize, Option<i64>)>();
                    b.score("v", s.port(), move |(_, v)| score(v.unwrap_or(0)))?;
                    Ok((b.build(), s))
                },
                plan
            ),
            basic
        );
    }
    if mode < 4 {
        let plan = SolverSequence {
            phases: vec![SolverPhase::Exact(c)],
            ..Default::default()
        };
        if mode == 2 {
            let mut n = ListConstraintNetwork::new(3, 2);
            for i in 0..2 {
                n.add_constraint(format!("v{i}"), vec![ListScope::Value(i)], move |v| {
                    let ListFact::Value { element, .. } = &v[0] else {
                        panic!()
                    };
                    score(element.as_ref().map_or(0, |v| v.position.entity as i64))
                })
                .unwrap();
            }
            return finish!(solve_list(&lists, &n, plan), lists);
        }
        return finish!(
            solve_list_stream(
                &lists,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = ListStreamSources::<usize>::new(&mut b);
                    b.score("v", s.values.port(), move |v| {
                        score(v.position.map_or(0, |p| p.entity as i64))
                    })?;
                    Ok((b.build(), s))
                },
                plan
            ),
            lists
        );
    }
    let p = MixedProblem::new(basic, lists);
    let plan = MixedSolverSequence {
        phases: vec![MixedSolverPhase::Exact(c)],
        ..Default::default()
    };
    if mode == 4 {
        let mut n = MixedConstraintNetwork::new(2, 3, 2);
        for i in 0..2 {
            n.add_constraint(format!("b{i}"), vec![MixedScope::Basic(i)], move |v| {
                let MixedFact::Basic(v) = v[0] else { panic!() };
                score(v.copied().unwrap_or(0))
            })
            .unwrap();
            n.add_constraint(format!("l{i}"), vec![MixedScope::ListValue(i)], move |v| {
                let MixedFact::ListValue { element, .. } = &v[0] else {
                    panic!()
                };
                score(element.as_ref().map_or(0, |v| v.position.entity as i64))
            })
            .unwrap();
        }
        return finish!(solve_mixed(&p, &n, plan), p);
    }
    finish!(
        solve_mixed_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::<i64, usize>::new(&mut b);
                b.score("b", s.basic.port(), move |(_, v)| score(v.unwrap_or(0)))?;
                b.score("l", s.lists.values.port(), move |v| {
                    score(v.position.map_or(0, |p| p.entity as i64))
                })?;
                Ok((b.build(), s))
            },
            plan
        ),
        p
    )
}
fn score(v: i64) -> HardSoftScore {
    HardSoftScore::new(-(v / 2), v)
}
#[test]
fn mixed_level_bounds_preserve_soft_improvements_and_prune_all_six_paths() {
    for mode in 0..6 {
        for order in ORDERS {
            let baseline = run(mode, config(order, PartialScoreBound::Unknown), score).unwrap();
            // Independent objective: each component chooses 1 for hard 0 / soft 1.
            let optimum = HardSoftScore::new(0, if mode < 4 { 2 } else { 4 });
            assert_eq!(baseline.1, Some(optimum));
            for text in ["ANY", "ONLY_DOWN/ANY", "ONLY_DOWN/ONLY_UP"] {
                let trend = InitializingScoreTrend::parse(text, 2).unwrap();
                let r = run(mode, config(order, PartialScoreBound::Trend(trend)), score).unwrap();
                assert_eq!((r.0, r.1), (SolverTermination::Optimal, Some(optimum)));
                assert_eq!(r.2.evaluations, r.2.move_evaluations);
                assert_eq!(r.2.completed_steps, 0);
                if text == "ANY" {
                    assert_eq!(r.2.evaluations, baseline.2.evaluations);
                } else {
                    assert!(r.2.evaluations < baseline.2.evaluations, "{mode} {order:?}");
                }
            }
        }
    }
}
#[test]
fn every_unknown_trend_budget_prefix_matches_unknown_bound_work_and_quality() {
    for mode in 0..6 {
        for order in ORDERS {
            let total = run(mode, config(order, PartialScoreBound::Unknown), score)
                .unwrap()
                .2
                .evaluations;
            for cap in 0..=total {
                let mut unknown = config(order, PartialScoreBound::Unknown);
                unknown.max_evaluations = cap;
                let mut trend = unknown.clone();
                trend.strategy = ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                    exploration: order,
                    bound: PartialScoreBound::Trend(
                        InitializingScoreTrend::parse("ANY", 2).unwrap(),
                    ),
                    ..Default::default()
                });
                assert_eq!(
                    run(mode, unknown, score).unwrap(),
                    run(mode, trend, score).unwrap()
                );
            }
        }
    }
}
#[test]
fn invalid_width_is_a_typed_phase_error_before_any_scoring() {
    for mode in 0..6 {
        let calls = Cell::new(0);
        let r = run(
            mode,
            config(
                ORDERS[0],
                PartialScoreBound::Trend(InitializingScoreTrend::parse("ANY", 1).unwrap()),
            ),
            |v| {
                calls.set(calls.get() + 1);
                score(v)
            },
        );
        assert_eq!(calls.get(), 0);
        assert!(
            r.unwrap_err()
                .contains("phase 0: score trend has 1 levels; expected 2")
        );
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64])]).unwrap();
    let n = ConstraintNetwork::<_, HardSoftScore>::new(1);
    let e = solve(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Exact(config(
                ORDERS[0],
                PartialScoreBound::Trend(InitializingScoreTrend::new(vec![])),
            ))],
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(matches!(
        e,
        SolverError::InvalidScoreTrend {
            phase: 0,
            error: ScoreTrendError::LevelCount {
                expected: 2,
                actual: 0
            }
        }
    ));
    assert!(std::error::Error::source(&e).is_some());
}
#[test]
fn finite_trend_bounds_match_existing_nonincreasing_mode() {
    for mode in 0..6 {
        for order in ORDERS {
            let score = |v: i64| HardSoftScore::new(-v, -v);
            let old = run(mode, config(order, PartialScoreBound::NonIncreasing), score).unwrap();
            let new = run(
                mode,
                config(
                    order,
                    PartialScoreBound::Trend(
                        InitializingScoreTrend::parse("ONLY_DOWN", 2).unwrap(),
                    ),
                ),
                score,
            )
            .unwrap();
            assert_eq!(old, new);
        }
    }
}

#[test]
fn decimal_prefix_pruning_keeps_sub_float_hard_differences_and_soft_improvements() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]); 2]).unwrap();
    let scores:Vec<HardSoftDecimalScore>=["0hard/0.000000000000000001soft", "0hard/0.000000000000000002soft", "-0.0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001hard/9007199254740993soft"].into_iter().map(|s|s.parse().unwrap()).collect();
    let mut n = ConstraintNetwork::new(2);
    for i in 0..2 {
        let scores = &scores;
        n.add_constraint(format!("v{i}"), vec![i], move |v| {
            v[0].map_or(HardSoftDecimalScore::ZERO, |&j| scores[j as usize].clone())
        })
        .unwrap();
    }
    for order in ORDERS {
        let r = solve(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                    strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                        exploration: order,
                        bound: PartialScoreBound::Trend(
                            InitializingScoreTrend::parse("ONLY_DOWN/ANY", 2).unwrap(),
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                })],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!(r.assignment.values(), &[Some(1), Some(1)]);
        assert_eq!(
            r.score,
            Some("0hard/0.000000000000000004soft".parse().unwrap())
        );
    }
}
