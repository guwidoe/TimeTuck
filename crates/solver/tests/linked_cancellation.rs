//! Original linked cancellation ownership and real manager/live-run regressions.
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::Duration,
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);

#[test]
fn independent_requests_nested_sources_and_configuration_identity() {
    let root = CancellationToken::new();
    let other = CancellationToken::new();
    let left = CancellationToken::linked([&root, &other, &root]);
    let right = CancellationToken::linked([&root]);
    let leaf = CancellationToken::linked([&left, &right]);
    assert_eq!(left, left.clone());
    assert_ne!(left, root);
    assert_ne!(right, CancellationToken::linked([&root]));
    assert!(!leaf.is_cancelled());
    assert!(left.cancel());
    assert!(leaf.is_cancelled());
    assert!(!root.is_cancelled());
    assert!(!other.is_cancelled());
    assert!(!right.is_cancelled());
    assert!(!left.cancel());
    assert!(root.cancel());
    assert!(right.is_cancelled());
    // Inheritance does not consume the independently observable direct request.
    assert!(right.cancel());
    assert!(!right.cancel());
    let already_cancelled = CancellationToken::linked([&root]);
    assert!(already_cancelled.is_cancelled());
    let independent = CancellationToken::linked([]);
    assert!(!independent.is_cancelled());
    drop(root);
    drop(left);
    drop(right);
    assert!(leaf.is_cancelled());
    assert!(!independent.is_cancelled());
}

#[test]
fn deep_links_survive_parent_drops_and_concurrent_local_requests_remain_unique() {
    let root = CancellationToken::new();
    let mut leaf = root.clone();
    for _ in 0..256 {
        leaf = CancellationToken::linked([&leaf, &root]);
    }
    assert!(!leaf.is_cancelled());
    root.cancel();
    assert!(leaf.is_cancelled());
    let first = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..16 {
            scope.spawn(|| {
                if leaf.cancel() {
                    first.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    assert_eq!(first.load(Ordering::Relaxed), 1);
    assert!(!leaf.cancel());
}

#[test]
fn application_and_job_requests_stop_real_managed_solves_independently() {
    let mut manager =
        SolverManager::<SolverResult<Assignment<i32>, SimpleScore>, String>::new(3).unwrap();
    let application = CancellationToken::new();
    let independent = CancellationToken::new();
    let (entered, entering) = mpsc::channel();
    let mut jobs = Vec::new();
    let mut releases = Vec::new();
    for id in 0..3 {
        let application = if id < 2 {
            application.clone()
        } else {
            independent.clone()
        };
        let entered = entered.clone();
        let (release, released) = mpsc::channel();
        releases.push(release);
        jobs.push(
            manager
                .submit(id, move |job_token| {
                    let linked = CancellationToken::linked([&job_token, &application]);
                    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)])
                        .unwrap();
                    let once = std::cell::Cell::new(true);
                    let mut network = ConstraintNetwork::new(1);
                    // Constraint callbacks are Fn; the channel is only used on the first score.
                    network
                        .add_constraint("value", vec![0], |values| {
                            if once.replace(false) {
                                entered
                                    .send((id, job_token.clone(), linked.clone()))
                                    .unwrap();
                                released.recv_timeout(WAIT).unwrap();
                            }
                            SimpleScore::new(values[0].copied().unwrap_or(0) as i64)
                        })
                        .unwrap();
                    solve(
                        &p,
                        &network,
                        SolverConfig {
                            limits: SolverLimits {
                                cancellation: Some(&linked),
                                ..Default::default()
                            },
                            local_search: vec![LocalSearchConfig {
                                max_steps: 4,
                                ..Default::default()
                            }],
                            ..Default::default()
                        },
                    )
                    .map_err(|e| e.to_string())
                })
                .unwrap(),
        );
    }
    let mut tokens = [None, None, None];
    for _ in 0..3 {
        let (id, job, linked) = entering.recv_timeout(WAIT).unwrap();
        tokens[id as usize] = Some((job, linked));
    }
    assert!(jobs[0].cancel());
    assert!(tokens[0].as_ref().unwrap().1.is_cancelled());
    assert!(!tokens[1].as_ref().unwrap().1.is_cancelled());
    assert!(!application.is_cancelled());
    releases[0].send(()).unwrap();
    assert_eq!(
        jobs[0]
            .wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .termination,
        SolverTermination::Cancelled
    );
    assert!(application.cancel());
    assert!(tokens[1].as_ref().unwrap().1.is_cancelled());
    assert!(!tokens[1].as_ref().unwrap().0.is_cancelled());
    assert!(!tokens[2].as_ref().unwrap().1.is_cancelled());
    assert!(!jobs[1].is_termination_requested());
    releases[1].send(()).unwrap();
    releases[2].send(()).unwrap();
    assert_eq!(
        jobs[1]
            .wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .termination,
        SolverTermination::Cancelled
    );
    let outcome = jobs[2].wait_timeout(WAIT).unwrap();
    let result = outcome.as_ref().as_ref().unwrap();
    assert_eq!(result.termination, SolverTermination::PlanCompleted);
    assert_eq!(result.score, Some(SimpleScore::new(1)));
    manager.shutdown().unwrap();
}

#[test]
fn live_revision_links_use_fresh_run_tokens_and_explicit_daemon_finish() {
    let mut manager = LiveSolverManager::<usize, usize, String>::new(1).unwrap();
    let application = CancellationToken::new();
    let app = application.clone();
    let (entered, entering) = mpsc::channel();
    let (next, restart) = mpsc::channel();
    let (finish, finishing) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            7,
            0,
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            move |revision, old, token, progress| {
                let linked = CancellationToken::linked([token, &app]);
                assert!(!linked.is_cancelled());
                if *revision == 0 {
                    assert!(old.is_none());
                    entered.send(linked.clone()).unwrap();
                    restart.recv_timeout(WAIT).unwrap();
                    assert!(linked.is_cancelled());
                    assert!(!app.is_cancelled());
                } else {
                    assert_eq!(*revision, 1);
                    assert_eq!(old, Some(&0));
                    entered.send(linked.clone()).unwrap();
                    finishing.recv_timeout(WAIT).unwrap();
                    assert!(linked.is_cancelled());
                    assert!(!token.is_cancelled());
                    // An application request does not wake/stop the live loop itself.
                    // This explicit action also prevents daemon idling after returning.
                    assert!(progress.finish_after_run());
                }
                let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
                let mut n = ConstraintNetwork::<_, SimpleScore>::new(1);
                n.add_constraint("never", vec![0], |_| {
                    panic!("pre-cancelled live solve scored")
                })
                .unwrap();
                let result = solve(
                    &p,
                    &n,
                    SolverConfig {
                        limits: SolverLimits {
                            cancellation: Some(&linked),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
                assert_eq!(result.termination, SolverTermination::Cancelled);
                assert_eq!(result.work.evaluations, 0);
                Ok(*revision)
            },
            |_, _| {},
        )
        .unwrap();
    let old = entering.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(|revision| {
            *revision = 1;
            Ok(())
        })
        .unwrap();
    assert!(old.is_cancelled());
    next.send(()).unwrap();
    let fresh = entering.recv_timeout(WAIT).unwrap();
    assert!(!fresh.is_cancelled());
    assert!(old.is_cancelled());
    application.cancel();
    assert!(fresh.is_cancelled());
    assert!(job.try_result().is_none());
    finish.send(()).unwrap();
    assert_eq!(
        *job.wait_timeout(WAIT)
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap()
            .solution,
        1
    );
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    manager.shutdown().unwrap();
}
