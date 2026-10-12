// Original facade validation, real callbacks, override ordering and lifecycle controls.
#[path = "fixtures/managed_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
fn complete(job: &ConfiguredSolverJob<Solution, Failure>) {
    let r = job.wait_timeout(WAIT).unwrap();
    let s = r.as_ref().as_ref().unwrap();
    assert_eq!(
        s.values.iter().map(|v| v.code.as_str()).collect::<Vec<_>>(),
        vec!["v1", "v2"]
    );
    assert_eq!(
        s.entities
            .iter()
            .map(|e| e.code.as_str())
            .collect::<Vec<_>>(),
        vec!["e1", "e2"]
    );
    assert!(s.entities.iter().all(|e| e.value.is_some()));
    assert_eq!(s.score, Some(SimpleScore::ZERO));
}
#[test]
fn all_convenience_methods_run_actual_solves_and_deliver_real_callbacks() {
    let mut manager = factory().build_default_manager().unwrap();
    let job = manager.solve_problem(1, source("direct")).unwrap();
    complete(&job);
    let finals = Arc::new(AtomicUsize::new(0));
    let seen = finals.clone();
    let job = manager
        .solve_with_final(2, source("final"), move |id, s| {
            assert_eq!(id, 2);
            assert_eq!(s.code, "final");
            assert!(s.entities.iter().all(|e| e.value.is_some()));
            seen.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    complete(&job);
    assert_eq!(finals.load(Ordering::SeqCst), 1);
    let best = Arc::new(AtomicUsize::new(0));
    let seen = best.clone();
    let job = manager
        .solve_and_listen(3, source("listen"), move |id, s| {
            assert_eq!(id, 3);
            assert_eq!(s.code, "listen");
            seen.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
    complete(&job);
    assert!(best.load(Ordering::SeqCst) > 0);
    assert!(job.callback_failures().is_empty());
    manager.shutdown().unwrap();
}
#[test]
fn sparse_override_keeps_defaults_and_explicit_clear_and_later_setters_win() {
    let mut config = config();
    config.limits.max_time = Some(Duration::from_secs(10));
    config.limits.max_unimproved_time = Some(Duration::from_secs(1));
    let factory = SolverFactory::new(config, run).unwrap();
    let mut manager = factory.build_manager(2).unwrap();
    let raw = SolverTimeOverride {
        max_time: None,
        max_unimproved_time: Some(Duration::from_millis(500)),
    };
    let a = manager
        .job_builder(1)
        .with_problem(source("a"))
        .with_time_override(raw)
        .run()
        .unwrap();
    let b = manager
        .job_builder(2)
        .with_problem(source("b"))
        .run()
        .unwrap();
    complete(&a);
    complete(&b);
    assert_eq!(
        a.configuration().limits.max_time,
        Some(Duration::from_secs(10))
    );
    assert_eq!(
        a.configuration().limits.max_unimproved_time,
        Some(Duration::from_millis(500))
    );
    assert_eq!(a.time_override(), Some(raw));
    assert_eq!(b.time_override(), None);
    assert_eq!(
        b.configuration().limits.max_unimproved_time,
        Some(Duration::from_secs(1))
    );
    assert_eq!(
        factory.configuration().limits.max_unimproved_time,
        Some(Duration::from_secs(1))
    );
    let c = manager
        .job_builder(1)
        .with_problem(source("c"))
        .with_time_override(raw)
        .with_time_limit(None)
        .with_unimproved_time_limit(None)
        .run()
        .unwrap();
    complete(&c);
    assert_eq!(c.configuration().limits.max_time, None);
    assert_eq!(c.configuration().limits.max_unimproved_time, None);
    assert_eq!(a.clone().time_override(), Some(raw));
    manager.shutdown().unwrap();
}
#[test]
fn custom_override_compiler_output_is_applied_while_original_input_is_retained() {
    let mut manager = factory().build_default_manager().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let raw = SolverTimeOverride {
        max_time: None,
        max_unimproved_time: Some(Duration::from_millis(500)),
    };
    let builder = manager
        .job_builder(1)
        .with_problem(source("compiled"))
        .with_time_override_using(raw, move |input| {
            assert_eq!(input, raw);
            seen.fetch_add(1, Ordering::SeqCst);
            SolverTimeOverride {
                max_time: Some(Duration::from_secs(2)),
                max_unimproved_time: Some(Duration::from_secs(3)),
            }
        });
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        builder.configuration().limits.max_time,
        Some(Duration::from_secs(2))
    );
    let job = builder.run().unwrap();
    complete(&job);
    assert_eq!(job.time_override(), Some(raw));
    assert_eq!(
        job.configuration().limits.max_unimproved_time,
        Some(Duration::from_secs(3))
    );
    manager.shutdown().unwrap();
}
#[test]
fn invalid_factory_or_pool_configuration_rejects_before_running_and_shutdown_rejects_helpers() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let result = ConfiguredSolverManager::<Solution, Solution, Failure>::from_config(
        SolverSequence {
            phases: vec![],
            ..Default::default()
        },
        move |s, _, _| {
            seen.fetch_add(1, Ordering::SeqCst);
            Ok(s)
        },
    );
    assert!(matches!(
        result,
        Err(ConfiguredManagerCreateError::Factory(
            SolverFactoryError::EmptyPhases
        ))
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let result = factory().build_manager_with_config(SolverManagerConfig {
        parallel_solver_count: 0,
        consumer_count: 1,
    });
    assert!(matches!(result,Err(e) if e.kind()==std::io::ErrorKind::InvalidInput));
    let mut manager = factory().build_default_manager().unwrap();
    manager.shutdown().unwrap();
    assert!(matches!(
        manager.solve_problem(1, source("closed")),
        Err(ConfiguredSolverJobBuildError::Submission(
            SolverJobBuildError::Submission(SubmitError::Closed)
        ))
    ));
}
