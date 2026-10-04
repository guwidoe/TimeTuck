//! Original native configuration, execution and factory-isolation checks.
use std::{
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
type ResultValue = SolverResult<Assignment<i64>, SimpleScore>;
fn problem() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap()
}
fn execute(
    p: FiniteProblem<i64>,
    config: SolverSequence<'_, SimpleScore>,
    progress: SolverProgress<ResultValue>,
) -> Result<ResultValue, String> {
    let n = ConstraintNetwork::<_, SimpleScore>::new(p.variables().len());
    let r = solve(&p, &n, config).map_err(|e| e.to_string())?;
    progress.record_work(r.work);
    Ok(r)
}
fn complete(job: &ConfiguredSolverJob<ResultValue, String>) {
    assert!(
        job.wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .complete
    );
}
#[test]
fn effective_limits_change_actual_solving_without_mutating_defaults_or_other_jobs() {
    let config = SolverConfig {
        local_search: vec![],
        limits: SolverLimits {
            max_move_evaluations: Some(0),
            ..Default::default()
        },
        ..Default::default()
    };
    let factory = SolverFactory::new(config, execute).unwrap();
    let mut manager = factory.build_manager(2).unwrap();
    let a = manager
        .job_builder(1)
        .with_problem(problem())
        .run()
        .unwrap();
    let b = manager
        .job_builder(2)
        .with_problem(problem())
        .with_limits(SolverLimits::default())
        .run()
        .unwrap();
    let result = a.wait_timeout(WAIT).unwrap();
    let result = result.as_ref().as_ref().unwrap();
    assert_eq!(result.termination, SolverTermination::MoveEvaluationLimit);
    assert_eq!(result.work.move_evaluations, 0);
    assert!(!result.complete);
    complete(&b);
    assert_eq!(b.statistics().work.unwrap().move_evaluations, 4);
    assert_eq!(factory.configuration().limits.max_move_evaluations, Some(0));
    assert_eq!(a.configuration().limits.max_move_evaluations, Some(0));
    assert_eq!(b.configuration().limits.max_move_evaluations, None);
    let next = manager
        .job_builder(1)
        .with_problem(problem())
        .run()
        .unwrap();
    assert!(
        !next
            .wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .complete
    );
    manager.shutdown().unwrap();
}
#[test]
fn time_and_unimproved_overrides_reach_real_solver_and_setter_order_is_explicit() {
    let factory = SolverFactory::new(SolverConfig::<SimpleScore>::default(), execute).unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let timed = manager
        .job_builder(1)
        .with_problem(problem())
        .with_time_limit(Some(Duration::ZERO))
        .run()
        .unwrap();
    let r = timed.wait_timeout(WAIT).unwrap();
    let r = r.as_ref().as_ref().unwrap();
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(r.work.evaluations, 1);
    assert!(!r.complete);
    let unimproved = manager
        .job_builder(2)
        .with_problem(problem())
        .with_time_limit(Some(WAIT))
        .with_unimproved_time_limit(Some(Duration::ZERO))
        .run()
        .unwrap();
    let r = unimproved.wait_timeout(WAIT).unwrap();
    let r = r.as_ref().as_ref().unwrap();
    assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
    assert!(r.complete);
    assert_eq!(unimproved.configuration().limits.max_time, Some(WAIT));
    assert_eq!(
        unimproved.configuration().limits.max_unimproved_time,
        Some(Duration::ZERO)
    );
    let reset = manager
        .job_builder(3)
        .with_problem(problem())
        .with_time_limit(Some(Duration::ZERO))
        .with_limits(SolverLimits::default())
        .with_unimproved_time_limit(Some(Duration::ZERO))
        .with_unimproved_time_limit(None)
        .with_phases(vec![SolverPhase::Exact(ExactPhaseConfig::default())])
        .run()
        .unwrap();
    let r = reset.wait_timeout(WAIT).unwrap();
    assert!(r.as_ref().as_ref().unwrap().is_optimal());
    assert_eq!(reset.configuration().limits.max_time, None);
    assert_eq!(reset.configuration().limits.max_unimproved_time, None);
    assert_eq!(factory.configuration().phases.len(), 2);
    manager.shutdown().unwrap();
}
#[test]
fn factories_and_job_overrides_reject_empty_phases_and_shared_tokens_before_lookup() {
    static TOKEN: OnceLock<CancellationToken> = OnceLock::new();
    let token = TOKEN.get_or_init(CancellationToken::new);
    let empty = SolverSequence::<SimpleScore> {
        phases: vec![],
        ..Default::default()
    };
    assert!(matches!(
        SolverFactory::new(empty, execute),
        Err(SolverFactoryError::EmptyPhases)
    ));
    let config = SolverConfig {
        limits: SolverLimits {
            cancellation: Some(token),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(matches!(
        SolverFactory::new(config, execute),
        Err(SolverFactoryError::SharedCancellationToken)
    ));
    for p in [
        SolverPhase::Construction(ConstructionConfig {
            cancellation: Some(token),
            ..Default::default()
        }),
        SolverPhase::LocalSearch(LocalSearchConfig {
            cancellation: Some(token),
            ..Default::default()
        }),
        SolverPhase::Exact(ExactPhaseConfig {
            cancellation: Some(token),
            ..Default::default()
        }),
        SolverPhase::Custom(CustomPhaseConfig {
            cancellation: Some(token),
            ..Default::default()
        }),
    ] {
        let config = SolverSequence {
            phases: vec![p],
            ..Default::default()
        };
        assert!(matches!(
            SolverFactory::new(config.clone(), execute),
            Err(SolverFactoryError::PhaseCancellationToken { phase: 0 })
        ));
        let factory = SolverFactory::new(SolverConfig::default(), execute).unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        assert!(matches!(
            manager
                .job_builder(1)
                .with_problem_finder(|_| panic!("must not lookup"))
                .with_phases(config.phases)
                .run(),
            Err(ConfiguredSolverJobBuildError::Configuration(
                SolverFactoryError::PhaseCancellationToken { phase: 0 }
            ))
        ));
        assert_eq!(manager.status(1), SolverStatus::NotSolving);
        assert!(matches!(
            manager.job_builder(1).with_phases(vec![]).run(),
            Err(ConfiguredSolverJobBuildError::Configuration(
                SolverFactoryError::EmptyPhases
            ))
        ));
        assert!(matches!(
            manager.job_builder(1).run(),
            Err(ConfiguredSolverJobBuildError::Submission(
                SolverJobBuildError::MissingProblem
            ))
        ));
        complete(
            &manager
                .job_builder(1)
                .with_problem(problem())
                .run()
                .unwrap(),
        );
        manager.shutdown().unwrap();
    }
}
#[test]
fn concurrent_jobs_and_reused_factories_get_independent_cancellation() {
    let (sent, seen) = mpsc::channel();
    // Each input owns its release channel; the shared factory itself stays Sync.
    let factory = SolverFactory::new(
        SolverConfig::<SimpleScore>::default(),
        move |(id, wait): (u64, mpsc::Receiver<()>), config, _| {
            let token = config.limits.cancellation.unwrap();
            sent.send(id).unwrap();
            wait.recv_timeout(WAIT).unwrap();
            Ok::<_, String>(token.is_cancelled())
        },
    )
    .unwrap();
    let mut manager = factory.build_manager(2).unwrap();
    let (release1, wait1) = mpsc::channel();
    let (release2, wait2) = mpsc::channel();
    let a = manager
        .job_builder(1)
        .with_problem((1, wait1))
        .run()
        .unwrap();
    let b = manager
        .job_builder(2)
        .with_problem((2, wait2))
        .run()
        .unwrap();
    let mut ids = vec![
        seen.recv_timeout(WAIT).unwrap(),
        seen.recv_timeout(WAIT).unwrap(),
    ];
    ids.sort();
    assert_eq!(ids, [1, 2]);
    assert!(a.cancel());
    release1.send(()).unwrap();
    release2.send(()).unwrap();
    assert!(*a.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap());
    assert!(!*b.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap());
    manager.shutdown().unwrap();
    let pool = SolverManager::with_thread_factory(1, 2, |spec, task| {
        std::thread::Builder::new()
            .name(spec.default_name())
            .spawn(task)
    })
    .unwrap();
    let mut other = factory.clone().with_manager(pool);
    let (release, wait) = mpsc::channel();
    release.send(()).unwrap();
    let fresh = other.job_builder(1).with_problem((1, wait)).run().unwrap();
    assert!(!*fresh.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap());
    other.shutdown().unwrap();
}
#[test]
fn queued_settings_and_size_survive_cancellation_and_id_reuse_without_eager_lookup() {
    let factory = SolverFactory::new(SolverConfig::<SimpleScore>::default(), execute).unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let (release, wait) = mpsc::channel();
    let active = manager
        .submit_with_progress(
            0,
            move |_, progress| {
                sent.send(()).unwrap();
                wait.recv_timeout(WAIT).unwrap();
                execute(problem(), SolverSequence::default(), progress)
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    seen.recv_timeout(WAIT).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let queued = manager
        .job_builder(1)
        .with_problem_finder(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(problem())
        })
        .with_problem_size(problem().problem_size_statistics())
        .with_time_limit(Some(Duration::from_secs(60)))
        .run()
        .unwrap();
    assert_eq!(queued.status(), SolverStatus::Scheduled);
    assert_eq!(
        queued
            .statistics()
            .problem_size
            .unwrap()
            .size
            .assignment_space
            .decimal(),
        "4"
    );
    assert_eq!(
        queued.configuration().limits.max_time,
        Some(Duration::from_secs(60))
    );
    assert!(matches!(
        manager.job_builder(1).with_problem(problem()).run(),
        Err(ConfiguredSolverJobBuildError::Submission(
            SolverJobBuildError::Submission(SubmitError::AlreadySolving)
        ))
    ));
    assert!(matches!(
        queued.cancel_and_wait().unwrap().as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    release.send(()).unwrap();
    assert!(active.wait_timeout(WAIT).unwrap().is_ok());
    let count = calls.clone();
    let next = manager
        .job_builder(1)
        .with_problem_finder(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(problem())
        })
        .with_time_limit(None)
        .run()
        .unwrap();
    complete(&next);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        queued.clone().configuration().limits.max_time,
        Some(Duration::from_secs(60))
    );
    assert_eq!(next.configuration().limits.max_time, None);
    manager.shutdown().unwrap();
    assert!(matches!(
        manager.job_builder(2).with_problem(problem()).run(),
        Err(ConfiguredSolverJobBuildError::Submission(
            SolverJobBuildError::Submission(SubmitError::Closed)
        ))
    ));
}
#[test]
fn reusable_factory_failure_is_contained_and_does_not_require_clone_inputs_or_results() {
    struct Input(std::cell::Cell<usize>);
    struct Output(usize);
    let errors = Arc::new(Mutex::new(vec![]));
    let factory =
        SolverFactory::new(
            SolverConfig::<SimpleScore>::default(),
            |p: Input, _, _| match p.0.get() {
                0 => Err("factory failure".to_owned()),
                1 => panic!("factory panic"),
                n => Ok(Output(n)),
            },
        )
        .unwrap();
    let mut manager = factory.clone().build_manager(1).unwrap();
    for n in 0..3 {
        let seen = errors.clone();
        let job = manager
            .job_builder(1)
            .with_problem(Input(std::cell::Cell::new(n)))
            .with_time_limit(Some(Duration::from_secs(30)))
            .with_callbacks(SolverJobCallbacks::default().on_error(move |_, e| {
                let SolverJobFailure::Solving(e) = e else {
                    panic!("callback failure")
                };
                seen.lock().unwrap().push(e.to_string());
            }))
            .run()
            .unwrap();
        let result = job.wait_timeout(WAIT).unwrap();
        match (n, result.as_ref()) {
            (0, Err(SolverJobError::Solve(s))) => assert_eq!(s, "factory failure"),
            (1, Err(SolverJobError::Panicked(s))) => assert_eq!(s, "factory panic"),
            (2, Ok(r)) => assert_eq!(r.0, 2),
            _ => panic!("unexpected result"),
        }
        assert!(job.callback_failures().is_empty());
        assert_eq!(
            job.configuration().limits.max_time,
            Some(Duration::from_secs(30))
        );
    }
    assert_eq!(errors.lock().unwrap().len(), 2);
    manager.shutdown().unwrap();
}

#[test]
fn time_gradients_handle_or_zero_disabled_and_large_durations_without_job_clock_guessing() {
    let mut limits = SolverLimits::<SimpleScore>::default();
    assert_eq!(limits.time_gradient(WAIT, WAIT), None);
    limits.max_time = Some(Duration::from_secs(1));
    assert_eq!(
        limits.time_gradient(Duration::from_millis(50), Duration::ZERO),
        Some(0.05)
    );
    limits.max_time = Some(Duration::from_millis(100));
    assert_eq!(
        limits.time_gradient(Duration::from_millis(50), Duration::ZERO),
        Some(0.5)
    );
    assert_eq!(limits.time_gradient(WAIT, Duration::ZERO), Some(1.0));
    limits.max_unimproved_time = Some(Duration::from_millis(10));
    assert_eq!(
        limits.time_gradient(Duration::from_millis(50), Duration::from_millis(8)),
        Some(0.8)
    );
    limits.max_time = None;
    assert_eq!(
        limits.time_gradient(WAIT, Duration::from_millis(8)),
        Some(0.8)
    );
    limits.max_unimproved_time = Some(Duration::ZERO);
    assert_eq!(
        limits.time_gradient(Duration::ZERO, Duration::ZERO),
        Some(1.0)
    );
    limits.max_unimproved_time = None;
    limits.max_time = Some(Duration::MAX);
    let mid = limits
        .time_gradient(Duration::MAX / 2, Duration::ZERO)
        .unwrap();
    assert!((mid - 0.5).abs() < 1e-15);
}

#[test]
fn managed_jobs_preserve_bounded_strategy_and_isolate_per_job_overrides() {
    let config = SolverSequence::<SimpleScore> {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            ..Default::default()
        })],
        ..Default::default()
    };
    let factory = SolverFactory::new(config, execute).unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let normal = manager
        .job_builder(501)
        .with_problem(problem())
        .run()
        .unwrap();
    let bounded = manager
        .job_builder(502)
        .with_problem(problem())
        .with_phases(vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig {
                bound: PartialScoreBound::NonIncreasing,
                ..Default::default()
            }),
            ..Default::default()
        })])
        .run()
        .unwrap();
    for (job, expected) in [(&normal, 7), (&bounded, 4)] {
        let result = job.wait_timeout(WAIT).unwrap();
        let result = result.as_ref().as_ref().unwrap();
        assert!(result.is_optimal());
        assert_eq!(result.score, Some(SimpleScore::ZERO));
        assert_eq!(result.work.evaluations, expected);
        assert_eq!(result.work.move_evaluations, expected);
        assert_eq!(job.statistics().work, Some(result.work));
    }
    assert_ne!(
        normal.configuration().phases,
        bounded.configuration().phases
    );
    assert_eq!(
        factory.configuration().phases,
        normal.configuration().phases
    );
    manager.shutdown().unwrap();
}
