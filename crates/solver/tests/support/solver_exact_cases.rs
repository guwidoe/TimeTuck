//! Original exact-orchestration regressions; no upstream test-method credit.
use super::*;
use std::time::Duration;

fn exact(c: ExactPhaseConfig<'_>) -> SolverPhase<'_> {
    SolverPhase::Exact(c)
}
fn plan() -> SolverPlan<'static> {
    SolverPlan {
        phases: vec![exact(ExactPhaseConfig::default())],
        ..Default::default()
    }
}
fn execute(
    mode: usize,
    initialized: bool,
    c: SolverPlan<'_>,
    make: impl Fn(i64) -> SimpleScore,
) -> Outcome {
    let factories = Cell::new(0);
    let callbacks = Cell::new(0);
    let r = run_scored(
        (mode, initialized, MixedConstructionOrder::BasicThenLists),
        c,
        &factories,
        None,
        || callbacks.set(callbacks.get() + 1),
        make,
    )
    .unwrap();
    assert_eq!(
        factories.get(),
        if mode % 2 == 1 { r.phases.len() } else { 0 }
    );
    assert_eq!(callbacks.get() as u128, r.work.constraint_evaluations);
    assert_eq!(
        r.work.evaluations,
        r.phases.iter().map(|p| p.work.evaluations).sum::<u64>()
    );
    assert_eq!(
        r.work.move_evaluations,
        r.phases
            .iter()
            .map(|p| p.work.move_evaluations)
            .sum::<u64>()
    );
    for phase in &r.phases {
        if matches!(phase.termination, PhaseTermination::Exact(_)) {
            assert_eq!(phase.work.evaluations, phase.work.move_evaluations);
            assert_eq!(phase.work.completed_steps, 0);
            assert_eq!(phase.work.accepted_moves, 0);
        }
    }
    r
}

#[test]
fn exhaustive_phases_prove_optimality_and_skip_remaining_phases_in_all_backends() {
    for mode in 0..6 {
        let mut c = plan();
        c.phases
            .push(SolverPhase::LocalSearch(LocalSearchConfig::default()));
        let r = execute(mode, true, c, SimpleScore::new);
        assert_eq!(r.reason, SolverTermination::Optimal);
        assert_eq!(
            r.score,
            Some(SimpleScore::new(if mode < 4 { 2 } else { 4 }))
        );
        assert_eq!(r.phases.len(), 2);
        assert_eq!(
            r.phases[1].termination,
            PhaseTermination::Exact(ExactPhaseTermination::Exhausted)
        );
        assert_eq!(r.phases[1].work.evaluations, if mode < 4 { 3 } else { 9 });
    }
}
#[test]
fn prefixes_are_not_proofs_and_shared_counts_bound_every_candidate() {
    for mode in 0..6 {
        let size = if mode < 4 { 3 } else { 9 };
        for cap in 0..=size + 1 {
            for scope in 0..4 {
                let mut c = plan();
                match scope {
                    0 => {
                        c.phases = vec![exact(ExactPhaseConfig {
                            max_evaluations: cap,
                            ..Default::default()
                        })]
                    }
                    1 => {
                        c.phases = vec![exact(ExactPhaseConfig {
                            max_move_evaluations: Some(cap),
                            ..Default::default()
                        })]
                    }
                    2 => c.limits.max_evaluations = cap + 1, // construction initial score
                    3 => c.limits.max_move_evaluations = Some(cap),
                    _ => unreachable!(),
                }
                let r = execute(mode, true, c, SimpleScore::new);
                assert_eq!(
                    r.reason == SolverTermination::Optimal,
                    cap >= size,
                    "mode={mode},scope={scope},cap={cap}"
                );
                assert_eq!(r.work.evaluations, 1 + cap.min(size));
                assert_eq!(r.work.move_evaluations, cap.min(size));
                if scope >= 2 && cap < size {
                    assert_eq!(
                        r.reason,
                        if scope == 2 {
                            SolverTermination::EvaluationLimit
                        } else {
                            SolverTermination::MoveEvaluationLimit
                        }
                    );
                } else if cap < size {
                    assert_eq!(r.reason, SolverTermination::PlanCompleted);
                    assert_eq!(
                        r.phases[1].termination,
                        PhaseTermination::Exact(if scope == 0 {
                            ExactPhaseTermination::EvaluationLimit
                        } else {
                            ExactPhaseTermination::MoveEvaluationLimit
                        })
                    );
                }
            }
        }
    }
}
#[test]
fn exact_can_finish_partial_construction_but_local_search_cannot() {
    for mode in 0..6 {
        for construction_cap in 0..=1 {
            let mut c = plan();
            c.construction.max_evaluations = construction_cap;
            let r = execute(mode, false, c.clone(), SimpleScore::new);
            assert!(r.complete);
            assert_eq!(r.reason, SolverTermination::Optimal);
            c.phases
                .insert(0, SolverPhase::LocalSearch(LocalSearchConfig::default()));
            let r = execute(mode, false, c, SimpleScore::new);
            assert!(!r.complete);
            assert_eq!(r.reason, SolverTermination::IncompleteConstruction);
            assert_eq!(r.phases.len(), 1);
        }
    }
}
#[test]
fn zero_work_exact_can_handoff_partial_state_to_another_exact_phase() {
    for mode in 0..6 {
        let mut c = plan();
        c.construction.max_evaluations = 0;
        c.phases.insert(
            0,
            exact(ExactPhaseConfig {
                max_evaluations: 0,
                ..Default::default()
            }),
        );
        let r = execute(mode, false, c, SimpleScore::new);
        assert_eq!(r.reason, SolverTermination::Optimal);
        assert_eq!(r.phases.len(), 3);
        assert_eq!(r.phases[1].score, None);
    }
}
#[test]
fn bounded_exact_retains_better_incumbents_and_equal_scores_keep_earlier_assignments() {
    for mode in 0..6 {
        let mut c = plan();
        c.phases[0] = exact(ExactPhaseConfig {
            max_evaluations: 1,
            ..Default::default()
        });
        let r = execute(mode, false, c, SimpleScore::new); // greedy construction finds maximum
        assert_eq!(
            r.score,
            Some(SimpleScore::new(if mode < 4 { 2 } else { 4 }))
        );
        assert_eq!(r.phases[1].score, r.score);
        assert!(r.values.iter().all(|v| *v == Some(2)));
        // For lists, enumeration's first candidate owns value 7 on entity 2.
        let r = execute(mode, true, plan(), |_| SimpleScore::ZERO);
        assert_eq!(r.reason, SolverTermination::Optimal);
        assert!(r.values.iter().all(|v| *v == Some(0)));
    }
}
#[test]
fn exact_and_local_phases_mix_in_both_orders() {
    for mode in 0..6 {
        let mut c = plan();
        c.phases = vec![
            exact(ExactPhaseConfig {
                max_evaluations: 1,
                ..Default::default()
            }),
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
            exact(ExactPhaseConfig::default()),
        ];
        let r = execute(mode, true, c, SimpleScore::new);
        assert_eq!(r.reason, SolverTermination::Optimal);
        assert_eq!(r.phases.len(), 4);
        assert_eq!(
            r.score,
            Some(SimpleScore::new(if mode < 4 { 2 } else { 4 }))
        );
        assert!(matches!(
            r.phases[2].termination,
            PhaseTermination::LocalSearch(_)
        ));
    }
}
#[test]
fn exact_goals_have_phase_scope_and_never_claim_proof_for_a_prefix() {
    for mode in 0..6 {
        for feasible in [false, true] {
            let mut c = plan();
            c.phases.insert(
                0,
                exact(ExactPhaseConfig {
                    best_score_limit: (!feasible).then_some(SimpleScore::ZERO),
                    stop_when_feasible: feasible,
                    ..Default::default()
                }),
            );
            let r = execute(mode, true, c.clone(), SimpleScore::new);
            assert_eq!(r.reason, SolverTermination::Optimal);
            assert_eq!(r.phases.len(), 3);
            assert_eq!(r.phases[1].work.evaluations, 0);
            assert_eq!(
                r.phases[1].termination,
                PhaseTermination::Exact(if feasible {
                    ExactPhaseTermination::FeasibleScore
                } else {
                    ExactPhaseTermination::BestScoreLimit
                })
            );
            // An incomplete construction cannot satisfy either shared goal.
            c.construction.max_evaluations = 0;
            c.limits.stop_when_feasible = feasible;
            c.limits.best_score_limit = (!feasible).then_some(SimpleScore::ZERO);
            let r = execute(mode, false, c, SimpleScore::new);
            assert!(r.complete);
            assert_eq!(r.phases.len(), 2);
            assert_eq!(r.phases[1].work.evaluations, 1);
            assert_eq!(
                r.reason,
                if feasible {
                    SolverTermination::FeasibleScore
                } else {
                    SolverTermination::BestScoreLimit
                }
            );
        }
    }
}
#[test]
fn exact_cancellation_retains_evaluated_prefix_and_final_exhaustion_wins() {
    for mode in 0..6 {
        let size = if mode < 4 { 3 } else { 9 };
        for stop in [1, size - 1, size] {
            let token = CancellationToken::new();
            let callbacks = Cell::new(0);
            let mut c = plan();
            c.construction.max_evaluations = 0;
            c.limits.cancellation = Some(&token);
            let r = run_scored(
                (mode, false, MixedConstructionOrder::BasicThenLists),
                c,
                &Cell::new(0),
                None,
                || {
                    callbacks.set(callbacks.get() + 1);
                    if callbacks.get() == stop {
                        token.cancel();
                    }
                },
                SimpleScore::new,
            )
            .unwrap();
            assert_eq!(r.work.evaluations, stop);
            assert!(r.complete);
            assert_eq!(
                r.reason,
                if stop == size {
                    SolverTermination::Optimal
                } else {
                    SolverTermination::Cancelled
                }
            );
        }
    }
}
#[test]
fn exact_zero_time_is_phase_local_and_zero_shared_stagnation_stops_before_candidates() {
    for mode in 0..6 {
        let mut c = plan();
        c.phases.insert(
            0,
            exact(ExactPhaseConfig {
                max_time: Some(Duration::ZERO),
                ..Default::default()
            }),
        );
        let r = execute(mode, true, c, SimpleScore::new);
        assert_eq!(r.reason, SolverTermination::Optimal);
        assert_eq!(r.phases[1].work.evaluations, 0);
        assert_eq!(
            r.phases[1].termination,
            PhaseTermination::Exact(ExactPhaseTermination::TimeLimit)
        );
        for steps in [false, true] {
            let mut c = plan();
            if steps {
                c.limits.max_unimproved_steps = Some(0)
            } else {
                c.limits.max_unimproved_time = Some(Duration::ZERO)
            };
            let r = execute(mode, true, c, SimpleScore::new);
            assert_eq!(r.work.evaluations, 1);
            assert_eq!(r.phases.len(), 2);
            assert_eq!(
                r.reason,
                if steps {
                    SolverTermination::UnimprovedStepLimit
                } else {
                    SolverTermination::UnimprovedTimeLimit
                }
            );
        }
    }
}
#[test]
fn exact_plan_validates_skipped_tokens_acceptance_and_factory_errors() {
    for mode in 0..6 {
        let a = CancellationToken::new();
        let b = CancellationToken::new();
        let mut c = plan();
        c.limits.cancellation = Some(&a);
        c.phases.push(exact(ExactPhaseConfig {
            cancellation: Some(&b),
            ..Default::default()
        }));
        let result = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &Cell::new(0),
            None,
            || panic!("must validate before scoring"),
            SimpleScore::new,
        );
        assert_eq!(result, Err(Failure::Token(2)));
        let mut c = plan();
        c.phases.push(SolverPhase::LocalSearch(LocalSearchConfig {
            acceptance: Acceptance::LateAcceptance {
                history_size: 0,
                hill_climbing: false,
            },
            ..Default::default()
        }));
        assert_eq!(
            run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &Cell::new(0),
                None,
                || panic!("must validate before scoring"),
                SimpleScore::new
            ),
            Err(Failure::Acceptance(2))
        );
        if mode % 2 == 1 {
            assert_eq!(
                run_scored(
                    (mode, true, MixedConstructionOrder::BasicThenLists),
                    plan(),
                    &Cell::new(0),
                    Some(2),
                    || {},
                    SimpleScore::new
                ),
                Err(Failure::Phase(1))
            );
        }
    }
}

#[test]
fn empty_models_still_have_one_exact_candidate_in_all_six_paths() {
    let p = FiniteProblem::<usize>::new(vec![]).unwrap();
    let l = ListProblem::<usize>::new(vec![], vec![]).unwrap();
    let m = MixedProblem::new(p.clone(), l.clone());
    macro_rules! check {
        ($r:expr) => {{
            let r = $r.unwrap();
            assert!(r.is_optimal());
            assert_eq!(r.score, Some(SimpleScore::ZERO));
            assert_eq!(r.work.evaluations, 1);
            assert_eq!(r.work.move_evaluations, 1);
            assert_eq!(r.work.constraint_evaluations, 0);
            assert_eq!(r.phases.len(), 2);
        }};
    }
    let mut c = plan();
    c.construction.max_evaluations = 0;
    check!(solve(&p, &ConstraintNetwork::new(0), c.clone()));
    check!(solve_list(&l, &ListConstraintNetwork::new(0, 0), c.clone()));
    check!(solve_stream(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = b.source();
            Ok((b.build(), s))
        },
        c.clone()
    ));
    check!(solve_list_stream(
        &l,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = ListStreamSources::new(&mut b);
            Ok((b.build(), s))
        },
        c.clone()
    ));
    let c = MixedSolverPlan {
        limits: c.limits,
        construction: MixedConstructionConfig {
            construction: c.construction,
            ..Default::default()
        },
        phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
    };
    check!(solve_mixed(
        &m,
        &MixedConstraintNetwork::new(0, 0, 0),
        c.clone()
    ));
    check!(solve_mixed_stream(
        &m,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = MixedStreamSources::new(&mut b);
            Ok((b.build(), s))
        },
        c
    ));
}
#[test]
fn impossible_models_do_not_gain_an_optimality_claim() {
    let p = FiniteProblem::new(vec![Variable::<usize>::new(vec![])]).unwrap();
    let r = solve(&p, &ConstraintNetwork::new(1), plan()).unwrap();
    assert!(!r.is_optimal());
    assert_eq!(r.termination, SolverTermination::NoCompleteAssignment);
    assert_eq!(r.work.evaluations, 0);
    assert_eq!(r.phases.len(), 1);
    let l = ListProblem::new(vec![7usize], vec![]).unwrap();
    let r = solve_list(&l, &ListConstraintNetwork::new(0, 1), plan()).unwrap();
    assert!(!r.is_optimal());
    assert_eq!(r.termination, SolverTermination::NoCompleteAssignment);
    assert_eq!(r.work.evaluations, 0);
}
#[test]
fn exact_stream_binding_validates_sources_even_when_the_phase_budget_is_zero() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize])]).unwrap();
    for foreign in [false, true] {
        let calls = Cell::new(0);
        let r = solve_stream(
            &p,
            || {
                calls.set(calls.get() + 1);
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = b.source();
                let mut g = b.build();
                if calls.get() == 2 {
                    if foreign {
                        let mut other = StreamGraphBuilder::<SimpleScore>::new();
                        return Ok((g, other.source()));
                    }
                    let mut tx = g.transaction();
                    tx.apply(&s, TupleChange::put(0, (0, Some(0))))?;
                    tx.commit()?;
                }
                Ok((g, s))
            },
            SolverPlan {
                phases: vec![exact(ExactPhaseConfig {
                    max_evaluations: 0,
                    ..Default::default()
                })],
                ..Default::default()
            },
        );
        assert!(matches!(r, Err(SolverError::Phase { phase: 1, .. })));
    }
}
#[test]
fn owned_decimal_exact_plans_honor_pins_optionality_and_infeasible_optima() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1_i64, 2]).allowing_unassigned(),
        Variable::new(vec![9]).with_initial(9).with_pinned(true),
    ])
    .unwrap();
    let score = |v: Option<i64>| {
        HardSoftDecimalScore::new(
            Decimal::from(-1),
            format!("{}.25", v.unwrap_or(7)).parse().unwrap(),
        )
    };
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("value", vec![0], |v| score(v[0].copied()))
        .unwrap();
    let expected = exhaustive_search(&p, |a| n.calculate(&p, a).unwrap(), 10);
    let config = || SolverPlan {
        construction: ConstructionConfig {
            max_evaluations: 0,
            ..Default::default()
        },
        phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
        ..Default::default()
    };
    let a = solve(&p, &n, config()).unwrap();
    let b = solve_stream(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i64>)>();
            let first = b.filter(source.port(), |(i, _)| *i == 0)?;
            b.score("value", &first, |(_, v)| score(*v))?;
            Ok((b.build(), source))
        },
        config(),
    )
    .unwrap();
    assert_eq!(a.assignment, b.assignment);
    assert_eq!(a.score, b.score);
    assert_eq!(a.assignment, expected.best.as_ref().unwrap().assignment);
    assert_eq!(a.score.as_ref(), Some(&expected.best.unwrap().score));
    assert_eq!(a.assignment.values(), &[None, Some(9)]);
    assert!(a.is_optimal() && b.is_optimal());
    assert!(!a.score.unwrap().is_feasible());
    assert_eq!(a.work.evaluations, 3);
    assert_eq!(b.work.evaluations, 3);
}

#[test]
fn exact_phase_escapes_a_local_optimum_and_matches_independent_enumeration() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0); 2]).unwrap();
    let score = |a: Option<i64>, b: Option<i64>| {
        SimpleScore::new(match (a, b) {
            (Some(1), Some(1)) => 2,
            (Some(0), Some(0)) => 0,
            _ => -1,
        })
    };
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("coupling", vec![0, 1], |v| {
        score(v[0].copied(), v[1].copied())
    })
    .unwrap();
    let oracle = exhaustive_search(&p, |a| score(a.values()[0], a.values()[1]), 4);
    let c = || SolverPlan {
        phases: vec![
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
            exact(ExactPhaseConfig::default()),
        ],
        ..Default::default()
    };
    let a = solve(&p, &n, c()).unwrap();
    let b = solve_stream(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<i64>)>();
            let left = b.filter(s.port(), |(i, _)| *i == 0)?;
            let right = b.filter(s.port(), |(i, _)| *i == 1)?;
            let pair = b.join_equal(&left, &right, |_| (), |_| ())?;
            b.score("coupling", &pair, |((_, a), (_, b))| score(*a, *b))?;
            Ok((b.build(), s))
        },
        c(),
    )
    .unwrap();
    assert!(a.is_optimal() && b.is_optimal());
    assert_eq!(a.assignment, b.assignment);
    assert_eq!(a.assignment, oracle.best.as_ref().unwrap().assignment);
    assert_eq!(a.score, Some(oracle.best.unwrap().score));
    for r in [a, b] {
        assert_eq!(r.phases[1].score, Some(SimpleScore::ZERO));
        assert_eq!(r.phases[1].work.accepted_moves, 0);
        assert_eq!(r.phases[2].work.evaluations, 4);
    }
}
