//! Original ownership, cancellation and wait-boundary regressions.
use std::{
    io,
    sync::{Arc, mpsc},
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
fn blocker<R: Send + Sync + 'static>(
    m: &SolverManager<R, String>,
    value: R,
) -> (SolverJob<R, String>, mpsc::Sender<()>) {
    let (entered, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let job = m
        .submit(1, move |_| {
            entered.send(()).unwrap();
            resume.recv_timeout(WAIT).unwrap();
            Ok(value)
        })
        .unwrap();
    ready.recv_timeout(WAIT).unwrap();
    (job, release)
}
#[test]
fn queued_owned_input_preserves_identity_without_invoking_callbacks_or_solving() {
    struct Input(Vec<u8>);
    let mut manager = SolverManager::<Arc<Input>, String>::new(1).unwrap();
    let input = Arc::new(Input(vec![1, 2, 3]));
    let (first, release) = blocker(&manager, input.clone());
    let queued = manager
        .submit_owned(
            2,
            input.clone(),
            |_, _, _| panic!("queued solve"),
            SolverJobCallbacks::default()
                .on_started(|_| panic!("started"))
                .on_final(|_, _| panic!("final")),
        )
        .unwrap();
    assert!(!queued.is_termination_requested());
    let result = queued.cancel_and_wait().unwrap();
    let returned = result.as_ref().as_ref().unwrap();
    assert!(Arc::ptr_eq(returned, &input));
    assert_eq!(returned.0, vec![1, 2, 3]);
    assert!(queued.is_termination_requested());
    assert_eq!(queued.status(), SolverStatus::NotSolving);
    assert!(queued.callback_failures().is_empty());
    let replacement = manager
        .submit_owned(
            2,
            input.clone(),
            |p, _, _| Ok(p),
            SolverJobCallbacks::default(),
        )
        .unwrap();
    assert!(!queued.cancel());
    assert!(!replacement.is_termination_requested());
    release.send(()).unwrap();
    assert!(first.wait_timeout(WAIT).unwrap().is_ok());
    assert!(replacement.wait_timeout(WAIT).unwrap().is_ok());
    assert!(!replacement.cancel());
    assert!(!replacement.is_termination_requested());
    assert!(manager.terminate_early_and_wait(999).unwrap().is_none());
    manager.shutdown().unwrap();
}
#[test]
fn shutdown_returns_owned_inputs_but_preserves_opaque_job_cancellation_errors() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let (active, release) = blocker(&manager, 1);
    let owned = manager
        .submit_owned(
            2,
            42,
            |_, _, _| panic!("owned ran"),
            SolverJobCallbacks::default(),
        )
        .unwrap();
    let opaque = manager.submit(3, |_| panic!("opaque ran")).unwrap();
    manager.request_shutdown();
    assert_eq!(
        owned.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap(),
        &42
    );
    assert!(matches!(
        opaque.wait_timeout(WAIT).unwrap().as_ref(),
        Err(SolverJobError::CancelledBeforeStart)
    ));
    assert!(owned.is_termination_requested() && opaque.is_termination_requested());
    release.send(()).unwrap();
    assert!(active.wait_timeout(WAIT).unwrap().is_ok());
    manager.shutdown().unwrap();
}
#[test]
fn active_owned_job_still_runs_and_synchronous_cancellation_waits_for_consumer_return() {
    let manager = Arc::new(SolverManager::<usize, String>::new(1).unwrap());
    let (entered, started) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let (consuming, consumer_started) = mpsc::channel();
    let (finish, consumer_resume) = mpsc::channel();
    let job = manager
        .submit_owned(
            7,
            10,
            |p, token, _| {
                assert!(token.is_cancelled());
                Ok(p + 1)
            },
            SolverJobCallbacks::default()
                .on_started(move |_| {
                    entered.send(()).unwrap();
                    resume.recv_timeout(WAIT).unwrap();
                })
                .on_final(move |_, n| {
                    assert_eq!(*n, 11);
                    consuming.send(()).unwrap();
                    consumer_resume.recv_timeout(WAIT).unwrap();
                }),
        )
        .unwrap();
    started.recv_timeout(WAIT).unwrap();
    job.cancel();
    let waiter = job.clone();
    let (done, completed) = mpsc::channel();
    let thread = std::thread::spawn(move || {
        done.send(waiter.cancel_and_wait().unwrap()).unwrap();
    });
    release.send(()).unwrap();
    consumer_started.recv_timeout(WAIT).unwrap();
    assert!(completed.try_recv().is_err());
    assert!(job.try_result().is_none());
    finish.send(()).unwrap();
    assert_eq!(
        completed
            .recv_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap(),
        &11
    );
    thread.join().unwrap();
    assert!(job.is_termination_requested());
    assert_eq!(job.status(), SolverStatus::NotSolving);
    Arc::try_unwrap(manager).ok().unwrap().shutdown().unwrap();
}
#[test]
fn synchronous_waits_from_solver_and_consumer_threads_request_cancel_then_refuse_to_block() {
    for in_consumer in [false, true] {
        let mut manager = SolverManager::<usize, String>::new(1).unwrap();
        let (send, receive) = mpsc::channel::<SolverJob<usize, String>>();
        let (result, seen) = mpsc::channel();
        let job = if in_consumer {
            manager.submit_with_callbacks(
                1,
                |_| Ok(1),
                SolverJobCallbacks::default().on_final(move |_, _| {
                    let job = receive.recv_timeout(WAIT).unwrap();
                    result
                        .send(job.cancel_and_wait().unwrap_err().kind())
                        .unwrap();
                }),
            )
        } else {
            manager.submit(1, move |_| {
                let job = receive.recv_timeout(WAIT).unwrap();
                result
                    .send(job.cancel_and_wait().unwrap_err().kind())
                    .unwrap();
                Ok(1)
            })
        }
        .unwrap();
        send.send(job.clone()).unwrap();
        assert_eq!(seen.recv_timeout(WAIT).unwrap(), io::ErrorKind::WouldBlock);
        assert!(job.wait_timeout(WAIT).unwrap().is_ok());
        assert!(job.is_termination_requested());
        assert!(job.callback_failures().is_empty());
        manager.shutdown().unwrap();
    }
}
#[test]
fn pool_threads_can_cancel_queued_jobs_and_wait_on_independent_pools() {
    let mut a = SolverManager::<usize, String>::new(1).unwrap();
    let mut b = SolverManager::<usize, String>::new(1).unwrap();
    let (first, release) = blocker(&a, 1);
    let queued = a
        .submit_owned(
            2,
            42,
            |_, _, _| panic!("queued"),
            SolverJobCallbacks::default(),
        )
        .unwrap();
    let other = b
        .submit(1, move |_| {
            Ok(*queued.cancel_and_wait().unwrap().as_ref().as_ref().unwrap())
        })
        .unwrap();
    assert_eq!(
        other.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap(),
        &42
    );
    release.send(()).unwrap();
    first.wait_timeout(WAIT).unwrap();
    let (send, receive) = mpsc::channel::<SolverJob<usize, String>>();
    let own = a
        .submit(3, move |_| {
            let next = receive.recv_timeout(WAIT).unwrap();
            Ok(*next.cancel_and_wait().unwrap().as_ref().as_ref().unwrap())
        })
        .unwrap();
    let next = a
        .submit_owned(
            4,
            99,
            |_, _, _| panic!("queued"),
            SolverJobCallbacks::default(),
        )
        .unwrap();
    send.send(next).unwrap();
    assert_eq!(
        own.wait_timeout(WAIT).unwrap().as_ref().as_ref().unwrap(),
        &99
    );
    a.shutdown().unwrap();
    b.shutdown().unwrap();
}
#[test]
fn owned_inputs_need_no_clone_and_errors_still_deliver_once() {
    struct Input(usize);
    let mut manager = SolverManager::<Input, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit_owned(
            1,
            Input(42),
            |p, _, _| Err(p.0.to_string()),
            SolverJobCallbacks::default().on_error(move |_, _| sent.send(()).unwrap()),
        )
        .unwrap();
    assert!(
        matches!(job.wait_timeout(WAIT).unwrap().as_ref(), Err(SolverJobError::Solve(e)) if e=="42")
    );
    assert!(!job.is_termination_requested());
    seen.recv_timeout(WAIT).unwrap();
    assert!(seen.try_recv().is_err());
    manager.shutdown().unwrap();
}
