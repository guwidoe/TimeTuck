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
    macro_rules! out {
        ($r:expr) => {{
            let r = $r.unwrap();
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
        out!(local_search(&p, &n, c)),
        out!(stream_local_search_from(
            &p,
            b.build(),
            s,
            p.initial_assignment(),
            c
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
        c
    )));
    results.push(out!(list_stream_local_search_from(
        &p,
        b.build(),
        s,
        p.initial_assignment(),
        c
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
        c
    )));
    results.push(out!(mixed_stream_local_search_from(
        &p,
        b.build(),
        s,
        p.initial_assignment(),
        c
    )));
    results
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
        for r in run_all(c, |_| improved(1)) {
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
    let first = local_search(&p, &n, c).unwrap();
    assert_eq!(first.termination, Stop::BestScoreLimit);
    let best = local_search(
        &p,
        &n,
        LocalSearchConfig {
            selection: MoveSelection::BestAccepted,
            ..c
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
    let r = local_search(&p, &n, c).unwrap();
    assert_eq!(r.completed_steps, 2);
    assert_eq!(r.termination, Stop::FeasibleScore);
    assert_eq!(r.best.unwrap().score, BendableScore::new([0, 0], [-100]));
    // A lexicographic score target can be met before hard feasibility.
    let r = local_search(
        &p,
        &n,
        LocalSearchConfig {
            best_score_limit: Some(BendableScore::new([0, -1], [-100])),
            ..c
        },
    )
    .unwrap();
    assert_eq!(r.completed_steps, 1);
    assert_eq!(r.termination, Stop::BestScoreLimit);
}
