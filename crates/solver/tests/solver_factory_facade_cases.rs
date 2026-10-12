// Complete remaining SolverManagerTest methods, Apache-2.0; see provenance/notices.
#[path = "fixtures/managed_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    any::Any,
    cell::RefCell,
    error::Error,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use timetuck::*;
const LIMIT: Duration = Duration::from_secs(10);
#[test]
fn create() {
    ConfiguredSolverManager::from_config(config(), run)
        .unwrap()
        .shutdown()
        .unwrap();
    ConfiguredSolverManager::from_config_with_manager(
        config(),
        run,
        SolverManagerConfig::default(),
    )
    .unwrap()
    .shutdown()
    .unwrap();
    let f = factory();
    f.build_default_manager().unwrap().shutdown().unwrap();
    f.build_manager_with_config(SolverManagerConfig::default())
        .unwrap()
        .shutdown()
        .unwrap();
}
#[test]
fn error_throwable_in_solver() {
    // A typed recoverable failure models the source's explicitly thrown Java
    // error. This does not claim recovery from actual Rust allocator OOM/abort.
    let f = SolverFactory::<Solution, Solution, Failure>::new(
        SolverPlan {
            phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
            ..Default::default()
        },
        |_, _, _| Err(Failure::ModeledAllocation("exceptionInSolver")),
    )
    .unwrap();
    let mut manager = f.build_manager(1).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let job = manager
        .job_builder(1)
        .with_problem_finder(|id| Ok(source(format!("Generated solution {id}"))))
        .with_callbacks(SolverJobCallbacks::default().on_error(move |id, e| {
            assert_eq!(id, 1);
            assert!(matches!(
                e,
                SolverJobFailure::Solving(SolverJobError::Solve(Failure::ModeledAllocation(
                    "exceptionInSolver"
                )))
            ));
            seen.fetch_add(1, Ordering::SeqCst);
        }))
        .run()
        .unwrap();
    let result = job.wait_timeout(LIMIT).unwrap();
    let err = result.as_ref().as_ref().unwrap_err();
    assert_eq!(err.source().unwrap().to_string(), "exceptionInSolver");
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(manager.status(1), SolverStatus::NotSolving);
    assert_eq!(job.status(), SolverStatus::NotSolving);
    manager.shutdown().unwrap();
}
#[test]
fn solve_generics() {
    let final_consumer: fn(&dyn Any) = |_| {};
    let error_consumer: fn(&dyn Any, &dyn Any) = |_, _| panic!("Solving failed.");
    let mut manager = factory().build_default_manager().unwrap();
    let job = manager
        .job_builder(1)
        .with_problem_finder(|id| Ok(source(format!("Generated solution {id}"))))
        .with_callbacks(
            SolverJobCallbacks::default()
                .on_final(move |_, s: &Solution| final_consumer(s))
                .on_error(move |id, e| match e {
                    SolverJobFailure::Solving(e) => error_consumer(&id, e),
                    SolverJobFailure::Callback(e) => error_consumer(&id, e),
                }),
        )
        .run()
        .unwrap();
    assert!(job.wait_timeout(LIMIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}
#[test]
fn solve_with_termination_unimproved_spent_limit() {
    let mut default = config();
    default.limits.max_time = Some(Duration::from_secs(10));
    default.limits.max_unimproved_time = Some(Duration::from_secs(1));
    let received = Arc::new(Mutex::new(vec![]));
    let seen = received.clone();
    let f = SolverFactory::new(default, move |s, config, progress| {
        seen.lock()
            .unwrap()
            .push((config.limits.max_time, config.limits.max_unimproved_time));
        run(s, config, progress)
    })
    .unwrap();
    let mut manager = f.build_default_manager().unwrap();
    let input = SolverTimeOverride {
        max_time: None,
        max_unimproved_time: Some(Duration::from_millis(500)),
    };
    let compiled = Arc::new(Mutex::new(vec![]));
    let seen = compiled.clone();
    let job = manager
        .job_builder(1)
        .with_problem(source("s1"))
        .with_time_override_using(input, move |raw| {
            seen.lock().unwrap().push(raw);
            raw
        })
        .run()
        .unwrap();
    assert_eq!(input.max_time, None);
    assert_eq!(input.max_unimproved_time, Some(Duration::from_millis(500)));
    assert_eq!(&*compiled.lock().unwrap(), &[input]);
    assert_eq!(job.time_override(), Some(input));
    assert!(job.wait_timeout(LIMIT).unwrap().is_ok());
    assert_eq!(
        &*received.lock().unwrap(),
        &[(
            Some(Duration::from_secs(10)),
            Some(Duration::from_millis(500))
        )]
    );
    manager.shutdown().unwrap();
}
#[derive(Default)]
struct Trace {
    ids: usize,
    problems: usize,
    finals: usize,
    best: usize,
}
struct MockManager(RefCell<Trace>);
struct MockBuilder<'a>(&'a RefCell<Trace>);
impl SolverSubmissionBuilder<(), ()> for MockBuilder<'_> {
    type Job = ();
    type Error = ();
    fn with_problem(self, _: ()) -> Self {
        self.0.borrow_mut().problems += 1;
        self
    }
    fn with_final_consumer(self, _: impl FnOnce(u64, &()) + Send + 'static) -> Self {
        self.0.borrow_mut().finals += 1;
        self
    }
    fn with_best_consumer(self, _: impl FnMut(u64, &()) + Send + 'static) -> Self {
        self.0.borrow_mut().best += 1;
        self
    }
    fn run(self) -> Result<(), ()> {
        Ok(())
    }
}
impl SolverSubmissionManager<(), ()> for MockManager {
    type Job = ();
    type Error = ();
    type Builder<'a> = MockBuilder<'a>;
    fn prepare_job(&self, _: u64) -> Self::Builder<'_> {
        self.0.borrow_mut().ids += 1;
        MockBuilder(&self.0)
    }
}
#[test]
fn test_solve_builder_for_existing_solving_methods() {
    let manager = MockManager(RefCell::new(Trace::default()));
    manager.solve_problem(1, ()).unwrap();
    assert_eq!(manager.0.borrow().ids, 1);
    assert_eq!(manager.0.borrow().problems, 1);
    manager.solve_with_final(1, (), |_, _| {}).unwrap();
    assert_eq!(manager.0.borrow().ids, 2);
    assert_eq!(manager.0.borrow().problems, 2);
    assert_eq!(manager.0.borrow().finals, 1);
    manager.solve_and_listen(1, (), |_, _| {}).unwrap();
    assert_eq!(manager.0.borrow().ids, 3);
    assert_eq!(manager.0.borrow().problems, 3);
    assert_eq!(manager.0.borrow().best, 1);
}
