//! Original factory dispatch, failure cleanup and pool-lifecycle checks.
use std::{
    io,
    sync::{
        Arc, Barrier, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
fn name() -> String {
    thread::current().name().unwrap().to_owned()
}
#[test]
fn factory_receives_roles_and_indices_and_wraps_all_solver_and_consumer_threads() {
    let caller = thread::current().id();
    let mut calls = vec![];
    let exits = Arc::new(AtomicUsize::new(0));
    let mut manager = SolverManager::<String, String>::with_thread_factory(2, 2, |spec, task| {
        assert_eq!(thread::current().id(), caller);
        calls.push(spec);
        let exits = exits.clone();
        thread::Builder::new()
            .name(format!("custom-{}", spec.default_name()))
            .stack_size(2 * 1024 * 1024)
            .spawn(move || {
                task();
                exits.fetch_add(1, Ordering::SeqCst);
            })
    })
    .unwrap();
    assert_eq!(
        calls,
        vec![
            SolverThreadSpec {
                role: SolverThreadRole::Solver,
                index: 0
            },
            SolverThreadSpec {
                role: SolverThreadRole::Solver,
                index: 1
            },
            SolverThreadSpec {
                role: SolverThreadRole::Consumer,
                index: 0
            },
            SolverThreadSpec {
                role: SolverThreadRole::Consumer,
                index: 1
            },
        ]
    );
    let solve_barrier = Arc::new(Barrier::new(2));
    let consume_barrier = Arc::new(Barrier::new(2));
    let seen = Arc::new(Mutex::new(vec![]));
    let jobs: Vec<_> = (0..2)
        .map(|id| {
            let solve = solve_barrier.clone();
            let consume = consume_barrier.clone();
            let record = seen.clone();
            manager
                .submit_with_callbacks(
                    id,
                    move |_| {
                        solve.wait();
                        Ok(name())
                    },
                    SolverJobCallbacks::default().on_final(move |_, solver_name: &String| {
                        consume.wait();
                        record.lock().unwrap().push((solver_name.clone(), name()));
                    }),
                )
                .unwrap()
        })
        .collect();
    for job in jobs {
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    }
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_ne!(seen[0].0, seen[1].0);
    assert_ne!(seen[0].1, seen[1].1);
    for (solver, consumer) in seen.iter() {
        assert!(solver.starts_with("custom-timetuck-solver-"));
        assert!(consumer.starts_with("custom-timetuck-consumer-"));
    }
    manager.shutdown().unwrap();
    assert_eq!(exits.load(Ordering::SeqCst), 4);
}
#[test]
fn invalid_counts_fail_before_invoking_factory() {
    for (solvers, consumers) in [(0, 1), (1, 0), (usize::MAX, 1)] {
        let mut calls = 0;
        let result =
            SolverManager::<usize, String>::with_thread_factory(solvers, consumers, |_, _| {
                calls += 1;
                panic!("invalid factory call");
            });
        let error = result.err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(calls, 0);
    }
}
#[test]
fn partial_spawn_errors_join_every_returned_thread_and_preserve_error_details() {
    for fail_at in 0..4 {
        let exits = Arc::new(AtomicUsize::new(0));
        let mut calls = 0;
        let result = SolverManager::<usize, String>::with_thread_factory(2, 2, |_, task| {
            let i = calls;
            calls += 1;
            if i == fail_at {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "factory denied",
                ));
            }
            let exits = exits.clone();
            thread::Builder::new().spawn(move || {
                task();
                exits.fetch_add(1, Ordering::SeqCst);
            })
        });
        let e = result.err().unwrap();
        assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(e.to_string(), "factory denied");
        assert_eq!(calls, fail_at + 1);
        assert_eq!(exits.load(Ordering::SeqCst), fail_at);
    }
}
#[test]
fn factory_call_panics_are_errors_and_already_started_threads_are_joined() {
    for fail_at in [0, 3] {
        let exits = Arc::new(AtomicUsize::new(0));
        let mut calls = 0;
        let result = SolverManager::<usize, String>::with_thread_factory(2, 2, |_, task| {
            let i = calls;
            calls += 1;
            if i == fail_at {
                panic!("factory panic");
            }
            let exits = exits.clone();
            thread::Builder::new().spawn(move || {
                task();
                exits.fetch_add(1, Ordering::SeqCst);
            })
        });
        let e = result.err().unwrap();
        assert_eq!(e.kind(), io::ErrorKind::Other);
        assert!(e.to_string().contains("factory panic"));
        assert_eq!(exits.load(Ordering::SeqCst), fail_at);
    }
}
#[test]
fn default_names_and_pool_wait_guards_survive_custom_creation() {
    let mut manager = SolverManager::<String, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit_with_callbacks(
            1,
            |_| Ok(name()),
            SolverJobCallbacks::default().on_final(move |_, _| sent.send(name()).unwrap()),
        )
        .unwrap();
    assert_eq!(
        job.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap(),
        "timetuck-solver-0"
    );
    assert_eq!(seen.recv_timeout(WAIT).unwrap(), "timetuck-consumer-0");
    manager.shutdown().unwrap();
    for in_consumer in [false, true] {
        let mut manager = SolverManager::<usize, String>::with_thread_factory(1, 1, |_, task| {
            thread::Builder::new()
                .name("same-custom-name".into())
                .spawn(task)
        })
        .unwrap();
        let (sent, seen) = mpsc::channel::<SolverJob<usize, String>>();
        let check = move || {
            assert_eq!(name(), "same-custom-name");
            let me = seen.recv_timeout(WAIT).unwrap();
            assert_eq!(
                me.cancel_and_wait().unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
        };
        let job = if in_consumer {
            manager.submit_with_callbacks(
                1,
                |_| Ok(1),
                SolverJobCallbacks::default().on_final(move |_, _| check()),
            )
        } else {
            manager.submit(1, move |_| {
                check();
                Ok(1)
            })
        }
        .unwrap();
        sent.send(job.clone()).unwrap();
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        assert!(job.is_termination_requested());
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}
#[test]
fn live_daemon_restarts_reuse_the_supplied_solver_and_consumer_threads() {
    let mut calls = vec![];
    let exits = Arc::new(AtomicUsize::new(0));
    let mut manager = LiveSolverManager::<usize, (usize, String), String>::with_thread_factory(
        1,
        1,
        |spec, task| {
            calls.push(spec);
            let exits = exits.clone();
            thread::Builder::new()
                .name(format!("live-{}", spec.default_name()))
                .spawn(move || {
                    task();
                    exits.fetch_add(1, Ordering::SeqCst);
                })
        },
    )
    .unwrap();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit(
            1,
            1,
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            |p, _, _| Ok((*p, name())),
            move |_, r| {
                assert_eq!(name(), "live-timetuck-consumer-0");
                sent.send(r.clone()).unwrap();
            },
        )
        .unwrap();
    assert_eq!(
        seen.recv_timeout(WAIT).unwrap(),
        (1, "live-timetuck-solver-0".into())
    );
    let receipt = job
        .add_problem_change(|p| {
            *p = 2;
            Ok(())
        })
        .unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    assert_eq!(
        seen.recv_timeout(WAIT).unwrap(),
        (2, "live-timetuck-solver-0".into())
    );
    job.cancel();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(exits.load(Ordering::SeqCst), 2);
}
