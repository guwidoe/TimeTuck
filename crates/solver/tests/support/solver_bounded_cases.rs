//! Original bounded-phase integration checks across all six model/scoring paths.
use super::*;
use std::time::Duration;
const ORDERS: [NodeExploration; 4] = [
    NodeExploration::DepthFirst,
    NodeExploration::BreadthFirst,
    NodeExploration::ScoreFirst,
    NodeExploration::OptimisticBoundFirst,
];
const ORDER: MixedConstructionOrder = MixedConstructionOrder::BasicThenLists;
fn phase(order: NodeExploration, bound: PartialScoreBound) -> ExactPhaseConfig<'static> {
    ExactPhaseConfig {
        strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
            exploration: order,
            bound,
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn sequence<'a>(c: ExactPhaseConfig<'a>) -> SolverSequence<'a> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(c)],
        ..Default::default()
    }
}
fn checked(
    mode: usize,
    initialized: bool,
    c: SolverSequence<'_>,
    score: impl Fn(i64) -> SimpleScore,
) -> Outcome {
    let callbacks = Cell::new(0);
    let factories = Cell::new(0);
    let r = run_scored(
        (mode, initialized, ORDER),
        c,
        &factories,
        None,
        || callbacks.set(callbacks.get() + 1),
        score,
    )
    .unwrap();
    assert_eq!(callbacks.get() as u128, r.work.constraint_evaluations);
    assert_eq!(
        factories.get(),
        if mode % 2 == 1 { r.phases.len() } else { 0 }
    );
    assert_eq!(
        r.work.evaluations,
        r.phases.iter().map(|p| p.work.evaluations).sum()
    );
    assert_eq!(
        r.work.move_evaluations,
        r.phases.iter().map(|p| p.work.move_evaluations).sum()
    );
    for p in &r.phases {
        if matches!(p.termination, PhaseTermination::Exact(_)) {
            assert_eq!(p.work.evaluations, p.work.move_evaluations);
            assert_eq!(p.work.completed_steps, 0);
            assert_eq!(p.work.accepted_moves, 0);
        }
    }
    r
}
#[test]
fn bounded_phases_optimize_all_backends_with_unknown_and_valid_bounds() {
    for mode in 0..6 {
        for order in ORDERS {
            for bound in [PartialScoreBound::Unknown, PartialScoreBound::NonIncreasing] {
                let r = checked(mode, false, sequence(phase(order, bound)), |n| {
                    SimpleScore::new(-n)
                });
                assert_eq!(r.reason, SolverTermination::Optimal);
                assert_eq!(r.score, Some(SimpleScore::ZERO));
                assert_eq!(r.phases.len(), 1);
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Exact(ExactPhaseTermination::Exhausted)
                );
                let nodes = if mode < 4 { 4 } else { 13 };
                if bound == PartialScoreBound::Unknown {
                    assert_eq!(r.work.evaluations, nodes);
                } else {
                    assert!(r.work.evaluations < nodes);
                }
                // With positive scores and unknown bounds, every model attains its maximum.
                let r = checked(
                    mode,
                    false,
                    sequence(phase(order, PartialScoreBound::Unknown)),
                    SimpleScore::new,
                );
                assert_eq!(
                    r.score,
                    Some(SimpleScore::new(if mode < 4 { 2 } else { 4 }))
                );
                assert_eq!(r.reason, SolverTermination::Optimal);
            }
        }
    }
}
#[test]
fn every_partial_node_counts_against_local_and_global_score_and_move_limits() {
    for mode in 0..6 {
        for order in ORDERS {
            let nodes = if mode < 4 { 4 } else { 13 };
            for cap in 0..=nodes + 1 {
                for scope in 0..4 {
                    let mut c = phase(order, PartialScoreBound::Unknown);
                    if scope == 0 {
                        c.max_evaluations = cap;
                    }
                    if scope == 1 {
                        c.max_move_evaluations = Some(cap);
                    }
                    let mut c = sequence(c);
                    if scope == 2 {
                        c.limits.max_evaluations = cap;
                    }
                    if scope == 3 {
                        c.limits.max_move_evaluations = Some(cap);
                    }
                    let r = checked(mode, false, c, SimpleScore::new);
                    assert_eq!(r.work.evaluations, cap.min(nodes));
                    assert_eq!(r.reason == SolverTermination::Optimal, cap >= nodes);
                    if cap < nodes {
                        assert_eq!(
                            r.phases[0].termination,
                            PhaseTermination::Exact(if scope % 2 == 0 {
                                ExactPhaseTermination::EvaluationLimit
                            } else {
                                ExactPhaseTermination::MoveEvaluationLimit
                            })
                        );
                        if scope >= 2 {
                            assert_eq!(
                                r.reason,
                                if scope == 2 {
                                    SolverTermination::EvaluationLimit
                                } else {
                                    SolverTermination::MoveEvaluationLimit
                                }
                            );
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn prior_scores_prune_but_initial_values_alone_are_not_scored_incumbents() {
    for mode in 0..6 {
        for order in ORDERS {
            let c = phase(order, PartialScoreBound::NonIncreasing);
            let mut seq = sequence(c.clone());
            seq.phases
                .insert(0, SolverPhase::Construction(ConstructionConfig::default()));
            seq.phases
                .push(SolverPhase::LocalSearch(LocalSearchConfig::default()));
            let r = checked(mode, true, seq, |n| SimpleScore::new(-n));
            assert_eq!(r.phases.len(), 2);
            assert_eq!(r.phases[1].work.evaluations, 1); // root proves retained zero cannot improve
            assert_eq!(r.score, Some(SimpleScore::ZERO));
            assert_eq!(r.reason, SolverTermination::Optimal);
            let r = checked(mode, true, sequence(c), |n| SimpleScore::new(-n));
            assert!(r.work.evaluations > 1);
            assert_eq!(r.reason, SolverTermination::Optimal);
        }
    }
}
#[test]
fn frontier_and_budget_stops_allow_later_phases_to_finish() {
    for mode in 0..6 {
        let mut limited = phase(ORDERS[0], PartialScoreBound::Unknown);
        let ExactSearchStrategy::BranchAndBound(ref mut b) = limited.strategy else {
            unreachable!()
        };
        b.max_frontier = 0;
        let mut c = sequence(limited);
        c.phases
            .push(SolverPhase::Exact(ExactPhaseConfig::default()));
        let r = checked(mode, false, c, SimpleScore::new);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::Exact(ExactPhaseTermination::FrontierLimit)
        );
        assert_eq!(r.phases[0].work.evaluations, 1);
        assert_eq!(r.phases[0].score, None);
        assert_eq!(r.reason, SolverTermination::Optimal);
        let mut partial = phase(ORDERS[0], PartialScoreBound::Unknown);
        partial.max_evaluations = 0;
        let mut c = sequence(partial);
        c.phases
            .push(SolverPhase::Construction(ConstructionConfig::default()));
        c.phases
            .push(SolverPhase::LocalSearch(LocalSearchConfig::default()));
        c.phases.push(SolverPhase::Exact(phase(
            ORDERS[0],
            PartialScoreBound::Unknown,
        )));
        let r = checked(mode, false, c, SimpleScore::new);
        assert_eq!(r.phases.len(), 4);
        assert_eq!(r.reason, SolverTermination::Optimal);
    }
}
#[test]
fn target_feasibility_and_zero_time_controls_apply_to_bounded_phases() {
    for mode in 0..6 {
        for global in [false, true] {
            for feasible in [false, true] {
                let mut c = phase(ORDERS[0], PartialScoreBound::Unknown);
                if !global {
                    if feasible {
                        c.stop_when_feasible = true;
                    } else {
                        c.best_score_limit = Some(SimpleScore::new(1));
                    }
                }
                let mut c = sequence(c);
                if global {
                    if feasible {
                        c.limits.stop_when_feasible = true;
                    } else {
                        c.limits.best_score_limit = Some(SimpleScore::new(1));
                    }
                }
                let r = checked(mode, false, c, SimpleScore::new);
                assert_eq!(
                    r.phases[0].termination,
                    PhaseTermination::Exact(if feasible {
                        ExactPhaseTermination::FeasibleScore
                    } else {
                        ExactPhaseTermination::BestScoreLimit
                    })
                );
                assert!(r.complete);
                if global {
                    assert_eq!(
                        r.reason,
                        if feasible {
                            SolverTermination::FeasibleScore
                        } else {
                            SolverTermination::BestScoreLimit
                        }
                    );
                } else {
                    assert_eq!(r.reason, SolverTermination::PlanCompleted);
                }
            }
        }
    }
    for mode in 0..6 {
        for global in [false, true] {
            let mut c = phase(ORDERS[0], PartialScoreBound::Unknown);
            if !global {
                c.max_time = Some(Duration::ZERO);
            }
            let mut c = sequence(c);
            if global {
                c.limits.max_time = Some(Duration::ZERO);
            }
            let r = checked(mode, false, c, SimpleScore::new);
            assert_eq!(r.work.evaluations, 0);
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::Exact(ExactPhaseTermination::TimeLimit)
            );
        }
    }
}
#[test]
fn observation_cancels_after_retention_and_initialization_events_use_exact_kind() {
    for mode in 0..6 {
        let cancel = CancellationToken::new();
        let mut c = sequence(phase(ORDERS[0], PartialScoreBound::Unknown));
        c.limits.cancellation = Some(&cancel);
        c.phases
            .push(SolverPhase::LocalSearch(LocalSearchConfig::default()));
        let mut updates = Vec::new();
        let mut initialized = Vec::new();
        let r = run_scored_events(
            (mode, false, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            Some(&mut |u| {
                updates.push(u);
                cancel.cancel();
            }),
            Some(&mut |i| initialized.push(i)),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(updates.len(), 1);
        assert_eq!(r.score, Some(updates[0].score));
        assert_eq!(r.values, updates[0].values);
        assert_eq!(updates[0].kind, SolverPhaseKind::Exact);
        assert_eq!(updates[0].phase, 0);
        assert!(updates[0].complete);
        assert_eq!(initialized.len(), 1);
        assert_eq!(initialized[0].initialization.kind, SolverPhaseKind::Exact);
        assert!(initialized[0].initialization.terminated_early);
    }
}
#[test]
fn conflicts_and_scoring_or_factory_errors_are_not_hidden_by_bounded_search() {
    for mode in 0..6 {
        let a = CancellationToken::new();
        let b = CancellationToken::new();
        let mut c = phase(ORDERS[0], PartialScoreBound::Unknown);
        c.cancellation = Some(&a);
        let mut c = sequence(c);
        c.limits.cancellation = Some(&b);
        assert_eq!(
            run_scored(
                (mode, false, ORDER),
                c,
                &Cell::new(0),
                None,
                || panic!("validation must precede scoring"),
                SimpleScore::new
            ),
            Err(Failure::Token(0))
        );
        let c = sequence(phase(ORDERS[0], PartialScoreBound::Unknown));
        assert_eq!(
            run_scored(
                (mode, false, ORDER),
                c.clone(),
                &Cell::new(0),
                None,
                || {},
                |_| SimpleScore::ZERO.with_structural(-1)
            ),
            Err(Failure::Phase(0))
        );
        if mode % 2 == 1 {
            assert_eq!(
                run_scored(
                    (mode, false, ORDER),
                    c,
                    &Cell::new(0),
                    Some(1),
                    || panic!("failed factory"),
                    SimpleScore::new
                ),
                Err(Failure::Phase(0))
            );
        }
    }
}

#[test]
fn visited_bound_violations_return_diagnostics_and_preserve_typed_scoring_errors() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let c = sequence(phase(ORDERS[0], PartialScoreBound::NonIncreasing));
    let error = solve(&p, &n, c.clone()).unwrap_err();
    assert!(error.to_string().contains("phase 0"));
    assert!(matches!(
        error,
        SolverError::InvalidOptimisticBound { phase: 0, .. }
    ));
    let error = solve_stream(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i64>)>();
            b.score("value", source.port(), |(_, v)| {
                SimpleScore::new(v.unwrap_or(0))
            })?;
            Ok((b.build(), source))
        },
        c,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        SolverError::InvalidOptimisticBound { phase: 0, .. }
    ));
    // Unknown bounds recover the actual optimum without a monotonicity assertion.
    assert_eq!(
        solve(
            &p,
            &n,
            sequence(phase(ORDERS[0], PartialScoreBound::Unknown))
        )
        .unwrap()
        .score,
        Some(SimpleScore::new(2))
    );
    let mut bad = ConstraintNetwork::new(1);
    bad.add_constraint("bad", vec![0], |_| SimpleScore::ZERO.with_structural(-1))
        .unwrap();
    assert!(matches!(
        solve(
            &p,
            &bad,
            sequence(phase(ORDERS[0], PartialScoreBound::Unknown))
        ),
        Err(SolverError::Phase {
            phase: 0,
            error: LocalSearchError::Scoring(ScoringError::StructuralContribution {
                constraint: 0
            })
        })
    ));
}
#[test]
fn owned_decimal_scores_remain_exact_through_phase_handoffs() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1])]).unwrap();
    let low: SimpleDecimalScore = "9007199254740992.000000000000000001".parse().unwrap();
    let high: SimpleDecimalScore = "9007199254740992.000000000000000002".parse().unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("precise", vec![0], |v| {
        if v[0] == Some(&1) {
            high.clone()
        } else {
            low.clone()
        }
    })
    .unwrap();
    for order in ORDERS {
        let r = solve(
            &p,
            &n,
            SolverSequence {
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig {
                        max_evaluations: 1,
                        ..Default::default()
                    }),
                    SolverPhase::Exact(ExactPhaseConfig {
                        strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                            exploration: order,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.is_optimal());
        assert_eq!(r.score, Some(high.clone()));
        assert_eq!(r.assignment.values(), &[Some(1)]);
    }
}
#[test]
fn final_leaf_or_proven_pruning_wins_over_observer_cancellation() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1_i64])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let cancel = CancellationToken::new();
    let mut c = sequence(phase(ORDERS[0], PartialScoreBound::Unknown));
    c.limits.cancellation = Some(&cancel);
    let r = solve_with_observer(&p, &n, c, |_| {
        cancel.cancel();
    })
    .unwrap();
    assert!(r.is_optimal());
    assert_eq!(r.work.evaluations, 2);
    for mode in 0..6 {
        let cancel = CancellationToken::new();
        let mut c = sequence(phase(ORDERS[0], PartialScoreBound::NonIncreasing));
        c.limits.cancellation = Some(&cancel);
        let r = run_scored_observed(
            (mode, false, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            |n| SimpleScore::new(-n),
            Some(&mut |_| {
                cancel.cancel();
            }),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Optimal);
    }
}
