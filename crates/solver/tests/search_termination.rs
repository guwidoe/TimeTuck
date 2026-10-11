//! Original integration checks; no upstream coverage credit.
use LocalSearchTermination as Stop;
use timetuck::*;

#[derive(Debug, PartialEq)]
struct Outcome {
    steps: u64,
    unimproved: u64,
    accepted: u64,
    best: Option<HardSoftScore>,
    reason: Stop,
}

// Equivalent one-variable landscapes in all three model types and both backends.
// The mixed fixture also has score-neutral list moves.
fn run_all(c: LocalSearchConfig<HardSoftScore>, score: fn(usize) -> HardSoftScore) -> Vec<Outcome> {
    run_all_with_counts(c, score).0
}
fn run_all_with_counts(
    c: LocalSearchConfig<HardSoftScore>,
    score: fn(usize) -> HardSoftScore,
) -> (Vec<Outcome>, Vec<(u64, u64)>) {
    let mut counts = Vec::new();
    macro_rules! out {
        ($r:expr) => {{
            let r = $r.unwrap();
            counts.push((r.evaluations, r.move_evaluations()));
            Outcome {
                steps: r.completed_steps,
                unimproved: r.unimproved_steps,
                accepted: r.accepted_moves,
                best: r.best.map(|b| b.score),
                reason: r.termination,
            }
        }};
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], move |v| score(*v[0].unwrap()))
        .unwrap();
    let mut b = StreamGraphBuilder::new();
    let s = b.source::<usize, (usize, Option<usize>)>();
    b.score("value", s.port(), move |(_, v)| score(v.unwrap()))
        .unwrap();
    let mut results = vec![
        out!(local_search(&p, &n, c.clone())),
        out!(stream_local_search_from(
            &p,
            b.build(),
            s,
            p.initial_assignment(),
            c.clone()
        )),
    ];

    let p = ListProblem::new(
        vec![7],
        vec![
            ListEntity::new(vec![7]),
            ListEntity::new(vec![]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let mut n = ListConstraintNetwork::new(3, 1);
    n.add_constraint("value", vec![ListScope::Value(0)], move |v| {
        let ListFact::Value { element, .. } = &v[0] else {
            panic!()
        };
        score(element.as_ref().unwrap().position.entity)
    })
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let s = ListStreamSources::<i32>::new(&mut b);
    b.score("value", s.values.port(), move |v| {
        score(v.position.unwrap().entity)
    })
    .unwrap();
    results.push(out!(list_local_search_from(
        &p,
        &n,
        p.initial_assignment(),
        c.clone()
    )));
    results.push(out!(list_stream_local_search_from(
        &p,
        b.build(),
        s,
        p.initial_assignment(),
        c.clone()
    )));

    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0usize, 1, 2]).with_initial(0)]).unwrap(),
        p,
    );
    let mut n = MixedConstraintNetwork::new(1, 3, 1);
    n.add_constraint("value", vec![MixedScope::Basic(0)], move |v| {
        let MixedFact::Basic(v) = v[0] else { panic!() };
        score(*v.unwrap())
    })
    .unwrap();
    let mut b = StreamGraphBuilder::new();
    let s = MixedStreamSources::<usize, i32>::new(&mut b);
    b.score("value", s.basic.port(), move |(_, v)| score(v.unwrap()))
        .unwrap();
    let c = MixedLocalSearchConfig {
        search: c,
        joint_moves: true,
    };
    results.push(out!(mixed_local_search_from(
        &p,
        &n,
        p.initial_assignment(),
        c.clone()
    )));
    results.push(out!(mixed_stream_local_search_from(
        &p,
        b.build(),
        s,
        p.initial_assignment(),
        c
    )));
    (results, counts)
}
fn config() -> LocalSearchConfig<'static, HardSoftScore> {
    LocalSearchConfig {
        max_steps: 10,
        max_evaluations: 10_000,
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::BestAccepted,
        assert_scores: true,
        ..Default::default()
    }
}
fn plateau(_: usize) -> HardSoftScore {
    HardSoftScore::new(-1, -10)
}
fn improved(v: usize) -> HardSoftScore {
    HardSoftScore::new(if v == 0 { -1 } else { 0 }, -10)
}

#[test]
fn ties_stop_at_exact_stagnation_budget_in_every_backend() {
    for r in run_all(
        LocalSearchConfig {
            max_unimproved_steps: Some(3),
            ..config()
        },
        plateau,
    ) {
        assert_eq!(
            r,
            Outcome {
                steps: 3,
                unimproved: 3,
                accepted: 3,
                best: Some(plateau(0)),
                reason: Stop::UnimprovedStepLimit
            }
        );
    }
}
#[test]
fn strict_improvement_resets_stagnation_in_every_backend() {
    for r in run_all(
        LocalSearchConfig {
            max_unimproved_steps: Some(2),
            ..config()
        },
        improved,
    ) {
        assert_eq!(
            r,
            Outcome {
                steps: 3,
                unimproved: 2,
                accepted: 3,
                best: Some(improved(1)),
                reason: Stop::UnimprovedStepLimit
            }
        );
    }
}
#[test]
fn score_and_feasibility_limits_check_initial_and_completed_steps() {
    for initial in [false, true] {
        for feasible in [false, true] {
            let c = LocalSearchConfig {
                best_score_limit: (!feasible).then_some(improved(1)),
                stop_when_feasible: feasible,
                ..config()
            };
            for r in run_all(c, if initial { |_| improved(1) } else { improved }) {
                assert_eq!(r.steps, u64::from(!initial));
                assert_eq!(r.unimproved, 0);
                assert_eq!(r.best, Some(HardSoftScore::new(0, -10)));
                assert_eq!(
                    r.reason,
                    if feasible {
                        Stop::FeasibleScore
                    } else {
                        Stop::BestScoreLimit
                    }
                );
            }
        }
    }
}
#[test]
fn optional_limits_obey_initial_precedence_in_every_backend() {
    let mut c = LocalSearchConfig {
        max_evaluations: 0,
        max_accepted_moves: 0,
        max_steps: 0,
        best_score_limit: Some(improved(1)),
        stop_when_feasible: true,
        max_unimproved_steps: Some(0),
        ..config()
    };
    for reason in [
        Stop::EvaluationLimit,
        Stop::AcceptedMoveLimit,
        Stop::StepLimit,
        Stop::BestScoreLimit,
        Stop::FeasibleScore,
        Stop::UnimprovedStepLimit,
    ] {
        for r in run_all(c.clone(), |_| improved(1)) {
            assert_eq!(r.reason, reason);
            assert_eq!(r.steps, 0);
            assert_eq!(r.best.is_none(), reason == Stop::EvaluationLimit);
        }
        match reason {
            Stop::EvaluationLimit => c.max_evaluations = 10_000,
            Stop::AcceptedMoveLimit => c.max_accepted_moves = 10,
            Stop::StepLimit => c.max_steps = 10,
            Stop::BestScoreLimit => c.best_score_limit = None,
            Stop::FeasibleScore => c.stop_when_feasible = false,
            _ => (),
        }
    }
}
#[test]
fn unreachable_score_limits_leave_existing_step_limit_effective() {
    for r in run_all(
        LocalSearchConfig {
            max_steps: 2,
            best_score_limit: Some(HardSoftScore::new(1, 0)),
            stop_when_feasible: true,
            ..config()
        },
        plateau,
    ) {
        assert_eq!(r.reason, Stop::StepLimit);
        assert_eq!(r.steps, 2);
        assert_eq!(r.unimproved, 2);
    }
}
#[test]
fn evaluation_exhaustion_after_selection_precedes_new_score_limit() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
        .unwrap();
    let c = LocalSearchConfig {
        max_evaluations: 2,
        best_score_limit: Some(SimpleScore::new(1)),
        ..Default::default()
    };
    // FirstAccepted exits on selection before testing exhaustion; the next step
    // boundary sees the new score. BestAccepted exhausts the budget before the
    // second candidate, installing the best candidate seen in its prefix.
    let first = local_search(&p, &n, c.clone()).unwrap();
    assert_eq!(first.termination, Stop::BestScoreLimit);
    let best = local_search(
        &p,
        &n,
        LocalSearchConfig {
            selection: MoveSelection::BestAccepted,
            ..c.clone()
        },
    )
    .unwrap();
    assert_eq!(best.termination, Stop::EvaluationLimit);
    assert_eq!(best.best.unwrap().score, SimpleScore::new(1));
    assert_eq!(best.completed_steps, 1);
    assert_eq!(best.unimproved_steps, 0);
}
#[test]
fn stalled_neighborhood_does_not_manufacture_completed_steps() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let r = local_search(
        &p,
        &n,
        LocalSearchConfig {
            max_unimproved_steps: Some(1),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, Stop::NoAcceptedMove);
    assert_eq!(r.completed_steps, 0);
    assert_eq!(r.unimproved_steps, 0);
}
#[test]
fn best_counter_does_not_reset_on_worsening_ties_or_recovery() {
    let mut p = StepProgress::default();
    let mut best = 0;
    for (score, expected) in [(1, 0), (-2, 1), (1, 2), (1, 3), (2, 0), (-1, 1)] {
        p.complete_step(score > best);
        best = best.max(score);
        assert_eq!(p.unimproved_steps(), expected);
    }
    assert_eq!(p.completed_steps(), 6);
    assert_eq!(p.best_step(), 5);
}
#[test]
fn zero_and_maximum_step_limits_have_finite_capped_progress() {
    assert!(StepCountLimit::new(0).is_reached(0));
    assert_eq!(StepCountLimit::new(0).progress(0), 1.0);
    let max = StepCountLimit::new(u64::MAX);
    assert!(!max.is_reached(u64::MAX - 1));
    assert!(max.is_reached(u64::MAX));
    for count in [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX] {
        assert!((0.0..=1.0).contains(&max.progress(count)));
    }
}

#[test]
fn rejected_fallback_steps_count_toward_stagnation_in_every_backend() {
    for r in run_all(
        LocalSearchConfig {
            acceptance: Acceptance::StrictImprovement,
            rejected_moves: RejectedMovePolicy::BestScore,
            max_unimproved_steps: Some(3),
            ..config()
        },
        plateau,
    ) {
        assert_eq!(
            r,
            Outcome {
                steps: 3,
                unimproved: 3,
                accepted: 0,
                best: Some(plateau(0)),
                reason: Stop::UnimprovedStepLimit
            }
        );
    }
}
#[test]
fn feasibility_uses_every_hard_level_and_ignores_negative_soft_scores() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("hard levels", vec![0], |v| {
        BendableScore::new(
            match v[0].unwrap() {
                0 => [-1, 100],
                1 => [0, -1],
                _ => [0, 0],
            },
            [-100],
        )
    })
    .unwrap();
    let c = LocalSearchConfig {
        stop_when_feasible: true,
        assert_scores: true,
        ..Default::default()
    };
    let r = local_search(&p, &n, c.clone()).unwrap();
    assert_eq!(r.completed_steps, 2);
    assert_eq!(r.termination, Stop::FeasibleScore);
    assert_eq!(r.best.unwrap().score, BendableScore::new([0, 0], [-100]));
    // A lexicographic score target can be met before hard feasibility.
    let r = local_search(
        &p,
        &n,
        LocalSearchConfig {
            best_score_limit: Some(BendableScore::new([0, -1], [-100])),
            ..c.clone()
        },
    )
    .unwrap();
    assert_eq!(r.completed_steps, 1);
    assert_eq!(r.termination, Stop::BestScoreLimit);
}

#[test]
fn candidate_budgets_match_scoring_prefixes_and_distinguish_limit_reasons() {
    for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
        for acceptance in [Acceptance::StrictImprovement, Acceptance::HillClimbing] {
            for rejected_moves in [RejectedMovePolicy::Stop, RejectedMovePolicy::BestScore] {
                for max_evaluations in 0..=12 {
                    for max_moves in 0..=6 {
                        let c = LocalSearchConfig {
                            max_evaluations,
                            selection,
                            acceptance,
                            rejected_moves,
                            move_order: MoveOrder::ShuffledMoves {
                                seed: 73,
                                window: std::num::NonZeroUsize::new(4).unwrap(),
                            },
                            ..config()
                        };
                        let (mut expected, expected_counts) = run_all_with_counts(
                            LocalSearchConfig {
                                max_evaluations: max_evaluations.min(max_moves + 1),
                                ..c.clone()
                            },
                            improved,
                        );
                        let (actual, counts) = run_all_with_counts(
                            LocalSearchConfig {
                                max_move_evaluations: Some(max_moves),
                                ..c.clone()
                            },
                            improved,
                        );
                        if max_moves + 1 < max_evaluations {
                            for r in &mut expected {
                                if r.reason == Stop::EvaluationLimit {
                                    r.reason = Stop::MoveEvaluationLimit;
                                }
                            }
                        }
                        assert_eq!(actual, expected);
                        assert_eq!(counts, expected_counts);
                        for (evaluations, moves) in counts {
                            assert_eq!(moves, evaluations.saturating_sub(1));
                            assert!(moves <= max_moves);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn count_limits_keep_full_width_decisions_separate_from_rounded_progress() {
    let zero = CountLimit::new(0);
    assert!(zero.is_reached(0));
    assert_eq!(zero.progress(0), 1.0);
    assert_eq!(zero.remaining(0), 0);
    for limit in [1, 4, 1 << 53, u64::MAX] {
        let l = CountLimit::new(limit);
        assert!(!l.is_reached(limit - 1));
        assert_eq!(l.remaining(limit - 1), 1);
        assert!(l.is_reached(limit));
        assert_eq!(l.remaining(limit), 0);
        assert!(l.is_reached(u64::MAX));
        assert_eq!(l.remaining(u64::MAX), 0);
        assert_eq!(l.progress(limit), 1.0);
    }
    assert_eq!(CountLimit::new(u64::MAX).progress(u64::MAX - 1), 1.0);
    assert!(CountLimit::try_from(i64::MIN).is_err());
    assert_eq!(
        CountLimit::try_from(i64::MAX).unwrap(),
        CountLimit::new(i64::MAX as u64)
    );
    for r in run_all(
        LocalSearchConfig {
            max_move_evaluations: Some(u64::MAX),
            max_evaluations: u64::MAX,
            max_steps: 1,
            ..config()
        },
        plateau,
    ) {
        assert_eq!(r.reason, Stop::StepLimit);
    }
}

#[test]
fn nested_conditions_follow_independent_boundary_reference_in_every_backend() {
    for minimum_steps in 0..=6 {
        for stagnant in 0..=6 {
            for fallback in [false, true] {
                let alternatives = [
                    StepTermination::UnimprovedSteps(stagnant),
                    StepTermination::AcceptedMoves(4),
                ];
                let requirements = [
                    StepTermination::Steps(minimum_steps),
                    StepTermination::Any(&alternatives),
                ];
                let condition = StepTermination::All(&requirements);
                let expected = (0..10).find(|&steps| {
                    let accepted = if fallback { steps.min(1) } else { steps };
                    steps >= minimum_steps && (steps.saturating_sub(1) >= stagnant || accepted >= 4)
                });
                for r in run_all(
                    LocalSearchConfig {
                        step_termination: Some(&condition),
                        acceptance: if fallback {
                            Acceptance::StrictImprovement
                        } else {
                            Acceptance::HillClimbing
                        },
                        rejected_moves: RejectedMovePolicy::BestScore,
                        ..config()
                    },
                    improved,
                ) {
                    let steps = expected.unwrap_or(10);
                    assert_eq!(r.steps, steps);
                    assert_eq!(r.unimproved, steps.saturating_sub(1));
                    assert_eq!(r.accepted, if fallback { steps.min(1) } else { steps });
                    assert_eq!(
                        r.best,
                        Some(if steps == 0 { plateau(0) } else { improved(1) })
                    );
                    assert_eq!(
                        r.reason,
                        if expected.is_some() {
                            Stop::StepCondition
                        } else {
                            Stop::StepLimit
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn composite_score_targets_and_empty_expressions_check_initial_boundaries() {
    let requirements = [
        StepTermination::Feasible,
        StepTermination::BestScore(improved(1)),
    ];
    let condition = StepTermination::All(&requirements);
    for initial in [false, true] {
        for r in run_all(
            LocalSearchConfig {
                step_termination: Some(&condition),
                ..config()
            },
            if initial { |_| improved(1) } else { improved },
        ) {
            assert_eq!(r.reason, Stop::StepCondition);
            assert_eq!(r.steps, u64::from(!initial));
            assert_eq!(r.best, Some(improved(1)));
        }
    }
    for (condition, reason, steps) in [
        (StepTermination::All(&[]), Stop::StepCondition, 0),
        (StepTermination::Any(&[]), Stop::StepLimit, 10),
    ] {
        for r in run_all(
            LocalSearchConfig {
                step_termination: Some(&condition),
                ..config()
            },
            plateau,
        ) {
            assert_eq!(r.reason, reason);
            assert_eq!(r.steps, steps);
        }
    }
}

#[test]
fn composite_conditions_preserve_existing_resource_and_boundary_precedence() {
    let condition = StepTermination::All(&[]);
    let token = CancellationToken::new();
    token.cancel();
    for (c, reason) in [
        (
            LocalSearchConfig {
                max_evaluations: 0,
                ..config()
            },
            Stop::EvaluationLimit,
        ),
        (
            LocalSearchConfig {
                cancellation: Some(&token),
                ..config()
            },
            Stop::Cancelled,
        ),
        (
            LocalSearchConfig {
                max_accepted_moves: 0,
                ..config()
            },
            Stop::AcceptedMoveLimit,
        ),
        (
            LocalSearchConfig {
                max_steps: 0,
                ..config()
            },
            Stop::StepLimit,
        ),
        (
            LocalSearchConfig {
                max_unimproved_steps: Some(0),
                ..config()
            },
            Stop::UnimprovedStepLimit,
        ),
    ] {
        for r in run_all(
            LocalSearchConfig {
                step_termination: Some(&condition),
                ..c.clone()
            },
            plateau,
        ) {
            assert_eq!(r.reason, reason);
            assert_eq!(r.steps, 0);
        }
    }
    let never = StepTermination::Any(&[]);
    for r in run_all(
        LocalSearchConfig {
            step_termination: Some(&never),
            max_move_evaluations: Some(1),
            ..config()
        },
        improved,
    ) {
        assert_eq!(r.reason, Stop::MoveEvaluationLimit);
        assert_eq!(r.steps, 1);
        assert_eq!(r.best, Some(improved(1)));
    }
    for r in run_all(
        LocalSearchConfig {
            step_termination: Some(&never),
            acceptance: Acceptance::StrictImprovement,
            ..config()
        },
        plateau,
    ) {
        assert_eq!(r.reason, Stop::NoAcceptedMove);
        assert_eq!(r.steps, 0);
    }
}

#[test]
fn reached_stagnation_child_resets_before_all_children_are_satisfied() {
    let children = [
        StepTermination::Steps(4),
        StepTermination::UnimprovedSteps(2),
    ];
    let condition = StepTermination::All(&children);
    let mut progress = StepProgress::default();
    for (improved, expected) in [
        (false, false),
        (false, false),
        (true, false),
        (false, false),
        (false, true),
    ] {
        progress.complete_step(improved);
        assert_eq!(
            condition.is_reached(&SimpleScore::ZERO, 0, progress),
            expected
        );
    }
    assert!(!StepTermination::AcceptedMoves(u64::MAX).is_reached(
        &SimpleScore::ZERO,
        u64::MAX - 1,
        progress
    ));
    assert!(StepTermination::AcceptedMoves(u64::MAX).is_reached(
        &SimpleScore::ZERO,
        u64::MAX,
        progress
    ));
}

#[test]
fn composite_targets_preserve_owned_decimal_and_multilevel_score_ordering() {
    let target: SimpleDecimalScore = "9007199254740992.000000000000000002".parse().unwrap();
    let less: SimpleDecimalScore = "9007199254740992.000000000000000001".parse().unwrap();
    let children = [
        StepTermination::BestScore(target.clone()),
        StepTermination::Steps(0),
    ];
    let condition = StepTermination::All(&children);
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    let expected = target.clone();
    n.add_constraint("decimal", vec![0], move |v| {
        if *v[0].unwrap() == 0 {
            less.clone()
        } else {
            target.clone()
        }
    })
    .unwrap();
    let r = local_search(
        &p,
        &n,
        LocalSearchConfig {
            step_termination: Some(&condition),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, Stop::StepCondition);
    assert_eq!(r.completed_steps, 1);
    assert_eq!(r.best.unwrap().score, expected);

    let score = BendableScore::new([0, -1], [-100]);
    let children = [StepTermination::BestScore(score), StepTermination::Feasible];
    let condition = StepTermination::All(&children);
    assert!(!condition.is_reached(&score, 0, StepProgress::default()));
    assert!(condition.is_reached(
        &BendableScore::new([0, 0], [-1000]),
        0,
        StepProgress::default()
    ));
    assert!(!condition.is_reached(
        &BendableScore::new([0, 0], [0]).with_structural(-1),
        0,
        StepProgress::default()
    ));
}
