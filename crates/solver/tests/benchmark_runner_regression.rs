// Original native runner checks; no upstream method credit.
use std::{convert::Infallible, num::NonZeroUsize};
use timetuck::*;
fn schedule(warmup: usize, measured: usize) -> BenchmarkSchedule {
    BenchmarkSchedule {
        warmup_runs: warmup,
        measured_runs: NonZeroUsize::new(measured).unwrap(),
        seed: u64::MAX,
    }
}
fn observation(score: i64) -> BenchmarkObservation<SimpleScore> {
    BenchmarkObservation {
        score: SimpleScore::new(score),
        initialized: true,
        work: SolverWork::default(),
    }
}
#[test]
fn matrix_order_matched_seeds_and_fresh_clones_are_reproducible() {
    let problems = [vec![10], vec![20]];
    let run = || {
        schedule(2, 2)
            .run(&[1, 2], &problems, None, |factor, mut p, t| {
                assert_eq!(p.len(), 1);
                p.push(99);
                Ok::<_, Infallible>(observation(factor * p[0] + t.repetition as i64))
            })
            .unwrap()
    };
    let a = run();
    let b = run();
    assert_eq!(a.planned_trials(), 16);
    assert_eq!(a.trials().len(), 16);
    assert!(a.is_successful());
    assert!(a.is_complete());
    assert_eq!(problems, [vec![10], vec![20]]);
    assert_eq!(
        a.trials().iter().map(|t| t.trial).collect::<Vec<_>>(),
        b.trials().iter().map(|t| t.trial).collect::<Vec<_>>()
    );
    for (i, t) in a.trials().iter().enumerate() {
        assert_eq!(t.trial.warmup, i < 8);
        assert_eq!(t.trial.repetition, (i % 8) / 4);
        assert_eq!(t.trial.problem, (i % 4) / 2);
        assert_eq!(t.trial.configuration, i % 2);
    }
    for pair in a.trials().chunks_exact(2) {
        assert_eq!(pair[0].trial.seed, pair[1].trial.seed);
    }
    assert_ne!(a.trials()[0].trial.seed, a.trials()[8].trial.seed);
    assert_ne!(a.trials()[8].trial.seed, a.trials()[12].trial.seed);
    assert_eq!(a.summary(0).unwrap().unwrap().run_count(), 4);
    assert!(a.summary(2).unwrap().is_none());
    assert!(a.timing(0, 2).is_none());
}
#[test]
fn warmup_errors_are_retained_but_excluded_from_rankings() {
    let r = schedule(1, 2)
        .run(&[()], &[()], None, |_, _, t| {
            if t.warmup {
                Err(String::from("warm-up cause"))
            } else {
                Ok(observation(4))
            }
        })
        .unwrap();
    assert!(r.is_complete());
    assert!(!r.is_successful());
    assert_eq!(r.trials()[0].result.as_ref().unwrap_err(), "warm-up cause");
    let s = r.summary(0).unwrap().unwrap();
    assert_eq!((s.run_count(), s.failure_count()), (2, 0));
    assert_eq!(s.total_score().unwrap().levels(), &[Decimal::from(8)]);
    assert_eq!(r.timing(0, 0).unwrap().failed_runs, 0);
    assert_eq!(r.into_trials().len(), 3);
}
#[test]
fn cancellation_preserves_finished_callbacks_without_fabricating_outcomes() {
    let token = CancellationToken::new();
    token.cancel();
    let r = schedule(1, 2)
        .run(
            &[()],
            &[()],
            Some(&token),
            |_, _, _| -> Result<BenchmarkObservation<SimpleScore>, ()> { panic!("must not start") },
        )
        .unwrap();
    assert!(r.trials().is_empty());
    assert!(!r.is_complete());
    assert!(!r.is_successful());
    assert_eq!(r.summary(0).unwrap().unwrap().run_count(), 0);
    assert_eq!(r.timing(0, 0).unwrap().mean_seconds(), None);
    let token = CancellationToken::new();
    let r = schedule(0, 3)
        .run(&[(), ()], &[()], Some(&token), |_, _, t| {
            if t.configuration == 1 {
                token.cancel();
            }
            Ok::<_, Infallible>(observation(1))
        })
        .unwrap();
    assert_eq!(r.trials().len(), 2);
    assert_eq!(r.planned_trials(), 6);
    assert!(!r.is_complete());
    assert!(r.trials().iter().all(|t| t.result.is_ok()));
    let token = CancellationToken::new();
    let r = schedule(0, 1)
        .run(&[()], &[()], Some(&token), |_, _, _| {
            token.cancel();
            Ok::<_, Infallible>(observation(1))
        })
        .unwrap();
    assert!(r.is_complete());
    assert!(r.is_successful());
}
#[test]
fn invalid_shapes_and_overflow_reject_before_callbacks() {
    let callback =
        |_: &(), _: (), _: BenchmarkTrial| -> Result<BenchmarkObservation<SimpleScore>, ()> {
            panic!("must not start")
        };
    assert_eq!(
        schedule(0, 1).run(&[], &[()], None, callback).unwrap_err(),
        BenchmarkScheduleError::NoConfigurations
    );
    assert_eq!(
        schedule(0, 1).run(&[()], &[], None, callback).unwrap_err(),
        BenchmarkScheduleError::NoProblems
    );
    assert_eq!(
        schedule(usize::MAX, 1)
            .run(&[()], &[()], None, callback)
            .unwrap_err(),
        BenchmarkScheduleError::TooManyTrials
    );
    assert_eq!(
        schedule(0, usize::MAX)
            .run(&[(), ()], &[()], None, callback)
            .unwrap_err(),
        BenchmarkScheduleError::TooManyTrials
    );
}
#[test]
fn benchmark_real_search_configs_over_multiple_problems() {
    let problem = |values| FiniteProblem::new(vec![Variable::new(values)]).unwrap();
    let problems = [problem(vec![-10_i64, -5, -1]), problem(vec![-20, -15, -2])];
    let r = schedule(1, 2)
        .run(&[1, 3], &problems, None, |&budget, p, _| {
            let result =
                exhaustive_search(&p, |a| SimpleScore::new(a.values()[0].unwrap()), budget);
            Ok::<_, Infallible>(BenchmarkObservation {
                score: result.best.unwrap().score,
                initialized: true,
                work: SolverWork {
                    evaluations: result.evaluations,
                    ..Default::default()
                },
            })
        })
        .unwrap();
    assert!(r.is_successful());
    let short = r.summary(0).unwrap().unwrap();
    let long = r.summary(1).unwrap().unwrap();
    assert!(long.compare(&short, BenchmarkRanking::TotalScore).is_gt());
    assert_eq!(short.total_score().unwrap().levels(), &[Decimal::from(-60)]);
    assert_eq!(long.total_score().unwrap().levels(), &[Decimal::from(-6)]);
    for trial in r.trials() {
        assert_eq!(
            trial.result.as_ref().unwrap().work.evaluations,
            [1, 3][trial.trial.configuration]
        );
    }
    assert_eq!(r.timing(1, 1).unwrap().successful_runs, 2);
}
