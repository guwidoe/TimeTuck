//! Original integration checks; no upstream coverage credit.
use crate::*;
use LocalSearchTermination as Stop;
use std::time::Duration;

#[derive(Debug, PartialEq)]
struct Outcome {
    evaluations: u64,
    steps: u64,
    unimproved: u64,
    accepted: u64,
    best: Option<HardSoftScore>,
    reason: Stop,
}

// Equivalent one-variable landscapes in all three model types and both backends.
// The mixed fixture also has score-neutral list moves.
fn run_all<C: FnMut() -> Duration>(
    c: LocalSearchConfig<HardSoftScore>,
    score: fn(usize) -> HardSoftScore,
    clocks: impl Fn() -> C,
) -> Vec<Outcome> {
    macro_rules! out {
        ($r:expr) => {{
            let r = $r.unwrap();
            Outcome {
                evaluations: r.evaluations,
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
        out!(local_search::search_impl_with_clock(
            &p,
            Some(p.initial_assignment()),
            c,
            |a| ScoreDirector::new(&p, &n, a),
            clocks()
        )),
        out!(local_search::search_impl_with_clock(
            &p,
            Some(p.initial_assignment()),
            c,
            |a| StreamScoreDirector::new(&p, b.build(), s, a),
            clocks()
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
    results.push(out!(list_local_search::search_impl_with_clock(
        &p,
        p.initial_assignment(),
        c,
        |a| ListScoreDirector::new(&p, &n, a),
        clocks()
    )));
    results.push(out!(list_local_search::search_impl_with_clock(
        &p,
        p.initial_assignment(),
        c,
        |a| ListStreamScoreDirector::new(&p, b.build(), s, a),
        clocks()
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
    results.push(out!(mixed_local_search::search_impl_with_clock(
        &p,
        p.initial_assignment(),
        c,
        |a| MixedScoreDirector::new(&p, &n, a),
        clocks()
    )));
    results.push(out!(mixed_local_search::search_impl_with_clock(
        &p,
        p.initial_assignment(),
        c,
        |a| MixedStreamScoreDirector::new(&p, b.build(), s, a),
        clocks()
    )));
    results
}
fn config() -> LocalSearchConfig<HardSoftScore> {
    LocalSearchConfig {
        max_time: Some(Duration::from_millis(3)),
        max_steps: 2,
        selection: MoveSelection::BestAccepted,
        assert_scores: true,
        ..Default::default()
    }
}
fn score(v: usize) -> HardSoftScore {
    HardSoftScore::new(0, v as i64)
}
fn ticking() -> impl FnMut() -> Duration {
    let mut tick = 0;
    move || {
        tick += 1;
        Duration::from_millis(tick)
    }
}
#[test]
fn expiry_installs_best_evaluated_prefix_in_all_six_search_backends() {
    for r in run_all(config(), score, ticking) {
        assert_eq!(
            r,
            Outcome {
                evaluations: 2,
                steps: 1,
                unimproved: 0,
                accepted: 1,
                best: Some(score(1)),
                reason: Stop::TimeLimit
            }
        );
    }
}
#[test]
fn timed_out_rejected_prefix_keeps_initial_without_claiming_neighborhood_exhaustion() {
    for r in run_all(config(), |v| HardSoftScore::new(0, -(v as i64)), ticking) {
        assert_eq!(
            r,
            Outcome {
                evaluations: 2,
                steps: 0,
                unimproved: 0,
                accepted: 0,
                best: Some(score(0)),
                reason: Stop::TimeLimit
            }
        );
    }
}
#[test]
fn initial_scoring_time_counts_and_disabled_timing_never_reads_clock() {
    for r in run_all(config(), score, || || Duration::from_secs(1)) {
        assert_eq!(r.evaluations, 1);
        assert_eq!(r.steps, 0);
        assert_eq!(r.best, Some(score(0)));
        assert_eq!(r.reason, Stop::TimeLimit);
    }
    for r in run_all(
        LocalSearchConfig {
            max_time: None,
            max_steps: 1,
            ..config()
        },
        score,
        || || panic!("disabled clock read"),
    ) {
        assert_eq!(r.steps, 1);
        assert_eq!(r.best, Some(score(2)));
        assert_eq!(r.reason, Stop::StepLimit);
    }
}
#[test]
fn candidate_time_expiry_beats_a_new_boundary_score_target() {
    for selection in [MoveSelection::FirstAccepted, MoveSelection::BestAccepted] {
        for r in run_all(
            LocalSearchConfig {
                best_score_limit: Some(score(1)),
                selection,
                ..config()
            },
            score,
            ticking,
        ) {
            assert_eq!(r.steps, 1);
            assert_eq!(r.reason, Stop::TimeLimit);
            assert_eq!(r.best, Some(score(1)));
        }
    }
}
#[test]
fn initial_count_limits_precede_time_and_zero_time_retains_initial_solution() {
    for (evaluations, steps, expected) in [
        (0, 1, Stop::EvaluationLimit),
        (10, 0, Stop::StepLimit),
        (10, 1, Stop::TimeLimit),
    ] {
        for r in run_all(
            LocalSearchConfig {
                max_time: Some(Duration::ZERO),
                max_evaluations: evaluations,
                max_steps: steps,
                ..config()
            },
            score,
            ticking,
        ) {
            assert_eq!(r.reason, expected);
            assert_eq!(r.steps, 0);
            assert_eq!(r.best.is_some(), evaluations > 0);
        }
    }
}
#[test]
fn real_clock_public_entry_point_honors_zero_without_sleep() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let r = local_search(
        &p,
        &n,
        LocalSearchConfig {
            max_time: Some(Duration::ZERO),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, Stop::TimeLimit);
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.completed_steps, 0);
}
#[test]
fn time_budget_clamps_regression_and_latches_expiry() {
    let mut times = [
        Duration::from_millis(5),
        Duration::from_millis(2),
        Duration::from_millis(10),
    ]
    .into_iter();
    let mut budget = time_limit::TimeBudget::new(Some(Duration::from_millis(10)), || {
        times.next().expect("expiry must latch")
    });
    assert!(!budget.expired());
    assert!(!budget.expired());
    assert!(budget.expired());
    assert!(budget.expired());
    assert_eq!(
        ElapsedTimeLimit::new(Duration::ZERO).progress(Duration::ZERO),
        1.0
    );
    let max = ElapsedTimeLimit::new(Duration::MAX);
    assert!(!max.is_reached(Duration::MAX - Duration::from_nanos(1)));
    assert!(max.is_reached(Duration::MAX));
    assert!((0.0..=1.0).contains(&max.progress(Duration::MAX / 2)));
    assert_eq!(
        ElapsedTimeLimit::new(Duration::from_nanos(4)).progress(Duration::from_nanos(1)),
        0.25
    );
}

#[test]
fn first_observed_candidate_limit_determines_time_vs_evaluation_reason() {
    for (millis, expected) in [(3, Stop::TimeLimit), (4, Stop::EvaluationLimit)] {
        for r in run_all(
            LocalSearchConfig {
                max_evaluations: 2,
                max_time: Some(Duration::from_millis(millis)),
                ..config()
            },
            score,
            ticking,
        ) {
            assert_eq!(r.evaluations, 2);
            assert_eq!(r.steps, 1);
            assert_eq!(r.best, Some(score(1)));
            assert_eq!(r.reason, expected);
        }
    }
}
