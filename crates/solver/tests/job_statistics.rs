//! Original metadata, assignment-space and managed telemetry checks.
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use timetuck::*;
const WAIT: Duration = Duration::from_secs(10);
fn basic(n: usize, values: usize) -> FiniteProblem<usize> {
    FiniteProblem::new(vec![Variable::new((0..values).collect()); n]).unwrap()
}
#[test]
fn basic_size_handles_pins_optional_values_identity_and_large_exact_counts() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1, 2]).with_initial(1).with_pinned(true),
        Variable::new(vec![2, 3]).allowing_unassigned(),
        Variable::new(vec![3, 4]),
    ])
    .unwrap();
    let s = p.problem_size_statistics();
    assert_eq!((s.entity_count, s.variable_count, s.value_count), (3, 3, 4));
    assert_eq!(s.assignment_space.decimal(), "6");
    assert!(s.assignment_space.exact);
    assert_eq!(
        basic(0, 9)
            .problem_size_statistics()
            .assignment_space
            .decimal(),
        "1"
    );
    assert_eq!(basic(0, 9).problem_size_statistics().value_count, 0);
    assert_eq!(
        basic(1, 0)
            .problem_size_statistics()
            .assignment_space
            .decimal(),
        "0"
    );
    assert_eq!(
        basic(130, 2)
            .problem_size_statistics()
            .assignment_space
            .decimal(),
        "1361129467683753853853498429727072845824"
    );
}
fn enumerate_lists(p: &ListProblem<usize>) -> usize {
    fn visit(p: &ListProblem<usize>, value: usize, lists: &mut Vec<Vec<usize>>) -> usize {
        if value == p.values().len() {
            return usize::from(p.is_complete(&ListAssignment::new(lists.clone())));
        }
        let mut n = 0;
        if p.allows_unassigned() {
            n += visit(p, value + 1, lists);
        }
        for owner in 0..lists.len() {
            for index in 0..=lists[owner].len() {
                lists[owner].insert(index, value);
                n += visit(p, value + 1, lists);
                lists[owner].remove(index);
            }
        }
        n
    }
    visit(p, 0, &mut vec![vec![]; p.entities().len()])
}
#[test]
fn list_assignment_counts_match_enumeration_and_restricted_ranges_are_labelled() {
    for optional in [false, true] {
        for pinned in 0..3 {
            for restricted in [false, true] {
                let mut first = ListEntity::new(vec![0]);
                if pinned == 1 {
                    first = first.with_pinned_prefix(1);
                }
                if pinned == 2 {
                    first = first.with_pinned(true);
                }
                let mut second = ListEntity::new(vec![]);
                if restricted {
                    second = second.with_value_range(vec![1, 2]);
                }
                let mut p = ListProblem::new(vec![0, 1, 2], vec![first, second]).unwrap();
                if optional {
                    p = p.allowing_unassigned();
                }
                let actual = enumerate_lists(&p);
                let s = p.problem_size_statistics();
                let size: usize = s.assignment_space.decimal().parse().unwrap();
                assert!(size >= actual);
                if s.assignment_space.exact {
                    assert_eq!(size, actual);
                } else {
                    assert!(restricted);
                }
                assert_eq!((s.entity_count, s.variable_count, s.value_count), (2, 2, 3));
            }
        }
    }
    for owners in 0..=3 {
        for values in 0..=4 {
            for optional in [false, true] {
                let mut p =
                    ListProblem::new((0..values).collect(), vec![ListEntity::new(vec![]); owners])
                        .unwrap();
                if optional {
                    p = p.allowing_unassigned();
                }
                assert_eq!(
                    p.problem_size_statistics().assignment_space.decimal(),
                    enumerate_lists(&p).to_string()
                );
            }
        }
    }
}
#[test]
fn mixed_size_multiplies_independent_spaces_and_preserves_bound_quality() {
    let lists = ListProblem::new(
        vec![0, 1],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
    )
    .unwrap();
    let p = MixedProblem::new(basic(2, 2), lists);
    let s = p.problem_size_statistics();
    assert_eq!((s.entity_count, s.variable_count, s.value_count), (4, 4, 4));
    assert!(s.assignment_space.exact);
    assert_eq!(s.assignment_space.decimal(), "24");
    let restricted = ListProblem::new(
        vec![0, 1],
        vec![
            ListEntity::new(vec![]).with_value_range(vec![0]),
            ListEntity::new(vec![]),
        ],
    )
    .unwrap();
    let bound = MixedProblem::new(basic(2, 2), restricted).problem_size_statistics();
    assert!(!bound.assignment_space.exact);
    assert_eq!(bound.assignment_space.decimal(), "24");
}
#[test]
fn queued_started_and_consumer_delays_do_not_count_as_solving() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let (started, ready) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let first = manager
        .submit(1, move |_| {
            started.send(()).unwrap();
            resume.recv_timeout(WAIT).unwrap();
            Ok(1)
        })
        .unwrap();
    ready.recv_timeout(WAIT).unwrap();
    let (starting, in_start) = mpsc::channel();
    let (allow, start_resume) = mpsc::channel();
    let (delivering, in_final) = mpsc::channel();
    let (finish, final_resume) = mpsc::channel();
    let size = basic(2, 2).problem_size_statistics();
    let job = manager
        .submit_with_statistics(
            2,
            size.clone(),
            |_, progress| {
                assert!(progress.record_work(SolverWork {
                    evaluations: 5,
                    move_evaluations: 4,
                    ..Default::default()
                }));
                Ok(2)
            },
            SolverJobCallbacks::default()
                .on_started(move |_| {
                    starting.send(()).unwrap();
                    start_resume.recv_timeout(WAIT).unwrap();
                })
                .on_final(move |_, _| {
                    delivering.send(()).unwrap();
                    final_resume.recv_timeout(WAIT).unwrap();
                }),
        )
        .unwrap();
    let queued = job.statistics();
    assert_eq!(queued.solving_duration, Duration::ZERO);
    assert!(!queued.solving);
    assert!(queued.work.is_none());
    assert_eq!(*queued.problem_size.unwrap().size, size);
    release.send(()).unwrap();
    first.wait_timeout(WAIT).unwrap();
    in_start.recv_timeout(WAIT).unwrap();
    assert_eq!(job.statistics().solving_duration, Duration::ZERO);
    allow.send(()).unwrap();
    in_final.recv_timeout(WAIT).unwrap();
    let final_stats = job.statistics();
    assert!(!final_stats.solving);
    assert!(job.try_result().is_none());
    assert_eq!(final_stats.work.unwrap().evaluations, 5);
    assert_eq!(final_stats, job.statistics());
    finish.send(()).unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(final_stats, job.statistics());
    manager.shutdown().unwrap();
}
#[test]
fn counters_are_monotone_overflow_is_explicit_and_stale_publishers_are_rejected() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let job = manager
        .submit_with_progress(
            1,
            move |_, progress| {
                assert!(progress.record_work(SolverWork {
                    evaluations: 5,
                    ..Default::default()
                }));
                assert!(!progress.record_work(SolverWork {
                    evaluations: 4,
                    ..Default::default()
                }));
                assert!(progress.add_work(SolverWork {
                    evaluations: u64::MAX,
                    constraint_evaluations: u128::MAX,
                    ..Default::default()
                }));
                assert!(progress.add_work(SolverWork {
                    constraint_evaluations: 1,
                    ..Default::default()
                }));
                assert!(progress.record_problem_size(2, basic(2, 3).problem_size_statistics()));
                assert!(!progress.record_problem_size(1, basic(2, 2).problem_size_statistics()));
                sent.send(progress).unwrap();
                Ok(1)
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    let stale = seen.recv_timeout(WAIT).unwrap();
    job.wait_timeout(WAIT).unwrap();
    let s = job.statistics();
    assert!(s.work_overflowed);
    assert_eq!(s.work.unwrap().evaluations, u64::MAX);
    assert_eq!(s.work.unwrap().constraint_evaluations, u128::MAX);
    assert_eq!(s.problem_size.unwrap().revision, 2);
    assert!(!stale.record_work(SolverWork::default()));
    assert!(!stale.record_problem_size(3, basic(2, 4).problem_size_statistics()));
    let next = manager.submit(1, |_| Ok(2)).unwrap();
    next.wait_timeout(WAIT).unwrap();
    assert!(next.statistics().work.is_none());
    assert!(next.statistics().problem_size.is_none());
    manager.shutdown().unwrap();
}
#[test]
fn live_statistics_follow_revisions_actual_work_and_exclude_idle_waits() {
    let mut manager = LiveSolverManager::<usize, Vec<Option<usize>>, String>::new(1).unwrap();
    let (sent, seen) = mpsc::channel();
    let works = Arc::new(std::sync::Mutex::new(vec![]));
    let observed = works.clone();
    let job = manager
        .submit_with_statistics(
            1,
            2,
            LiveSolverConfig {
                daemon: true,
                ..Default::default()
            },
            |n| basic(2, *n).problem_size_statistics(),
            move |n, _, token| {
                let p = basic(2, *n);
                let mut network = ConstraintNetwork::new(2);
                network
                    .add_constraint("sum", vec![0, 1], |v| {
                        SimpleScore::new(v.iter().filter_map(|x| x.copied()).sum::<usize>() as i64)
                    })
                    .unwrap();
                let r = solve(
                    &p,
                    &network,
                    SolverConfig {
                        limits: SolverLimits {
                            cancellation: Some(token),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                )
                .map_err(|e| e.to_string())?;
                observed.lock().unwrap().push(r.work);
                Ok(LiveRun {
                    solution: r.assignment.values().to_vec(),
                    work: r.work,
                })
            },
            move |_, v| sent.send(v.clone()).unwrap(),
        )
        .unwrap();
    seen.recv_timeout(WAIT).unwrap();
    let receipt = job
        .add_problem_change(|n| {
            *n = 3;
            Ok(())
        })
        .unwrap();
    assert_eq!(receipt.wait_timeout(WAIT), Some(Ok(())));
    seen.recv_timeout(WAIT).unwrap();
    let deadline = Instant::now() + WAIT;
    while job.statistics().solving {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    let s = job.statistics();
    assert_eq!(s, job.statistics());
    assert_eq!(s.problem_size.as_ref().unwrap().revision, 1);
    assert_eq!(
        s.problem_size
            .as_ref()
            .unwrap()
            .size
            .assignment_space
            .decimal(),
        "9"
    );
    let work = works.lock().unwrap();
    assert_eq!(work.len(), 2);
    assert_eq!(
        s.work.unwrap().evaluations,
        work.iter().map(|w| w.evaluations).sum::<u64>()
    );
    job.cancel();
    job.wait_timeout(WAIT).unwrap();
    assert_eq!(s, job.statistics());
    manager.shutdown().unwrap();
}
#[test]
fn failure_keeps_reported_work_and_queued_cancellation_has_no_fabricated_counters() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let fail = manager
        .submit_with_progress(
            1,
            |_, p| {
                p.record_work(SolverWork {
                    evaluations: 7,
                    ..Default::default()
                });
                Err("failed".into())
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    fail.wait_timeout(WAIT).unwrap();
    assert_eq!(fail.statistics().work.unwrap().evaluations, 7);
    assert!(!fail.statistics().solving);
    let (sent, seen) = mpsc::channel();
    let (release, resume) = mpsc::channel();
    let active = manager
        .submit(1, move |_| {
            sent.send(()).unwrap();
            resume.recv_timeout(WAIT).unwrap();
            Ok(1)
        })
        .unwrap();
    seen.recv_timeout(WAIT).unwrap();
    let pending = manager
        .submit_with_statistics(
            2,
            basic(4, 4).problem_size_statistics(),
            |_, _| panic!("queued"),
            SolverJobCallbacks::default(),
        )
        .unwrap();
    pending.cancel();
    pending.wait_timeout(WAIT).unwrap();
    let s = pending.statistics();
    assert_eq!(s.solving_duration, Duration::ZERO);
    assert!(s.work.is_none());
    assert_eq!(
        s.problem_size.unwrap().size.assignment_space.decimal(),
        "256"
    );
    release.send(()).unwrap();
    active.wait_timeout(WAIT).unwrap();
    manager.shutdown().unwrap();
}

#[test]
fn speeds_handle_unknown_zero_duration_flooring_and_large_values() {
    let mut s = SolverJobStatistics::default();
    assert_eq!(s.score_calculation_speed(), None);
    assert_eq!(s.move_evaluation_speed(), None);
    s.work = Some(SolverWork {
        evaluations: 5,
        move_evaluations: 4,
        ..Default::default()
    });
    assert_eq!(s.score_calculation_speed(), Some(0));
    s.solving_duration = Duration::from_secs(2);
    assert_eq!(s.score_calculation_speed(), Some(2));
    assert_eq!(s.move_evaluation_speed(), Some(2));
    s.solving_duration = Duration::from_secs(6);
    assert_eq!(s.score_calculation_speed(), Some(0));
    s.work.as_mut().unwrap().evaluations = u64::MAX;
    s.solving_duration = Duration::from_nanos(1);
    assert_eq!(s.score_calculation_speed(), Some(u64::MAX));
}

#[test]
fn repair_statistics_preserve_monotonicity_and_report_overflow() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let job = manager
        .submit_with_progress(
            1,
            |_, p| {
                let initial = SolverWork {
                    repair_attempts: 4,
                    complete_reconstructions: 3,
                    ..Default::default()
                };
                assert!(p.record_work(initial));
                assert!(!p.record_work(SolverWork {
                    repair_attempts: 3,
                    ..initial
                }));
                assert!(!p.record_work(SolverWork {
                    complete_reconstructions: 2,
                    ..initial
                }));
                assert!(p.add_work(SolverWork {
                    repair_attempts: 2,
                    complete_reconstructions: 1,
                    ..Default::default()
                }));
                assert!(!p.record_work(SolverWork {
                    repair_attempts: 5,
                    complete_reconstructions: 4,
                    ..Default::default()
                }));
                assert!(!p.record_work(SolverWork {
                    repair_attempts: 6,
                    complete_reconstructions: 3,
                    ..Default::default()
                }));
                assert!(p.add_work(SolverWork {
                    repair_attempts: u64::MAX,
                    complete_reconstructions: u64::MAX,
                    ..Default::default()
                }));
                Ok(1)
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    let stats = job.statistics();
    assert!(stats.work_overflowed);
    assert_eq!(stats.work.unwrap().repair_attempts, u64::MAX);
    assert_eq!(stats.work.unwrap().complete_reconstructions, u64::MAX);
    manager.shutdown().unwrap();
}

#[test]
fn fine_move_statistics_preserve_absolute_updates_and_additive_overflow() {
    let mut manager = SolverManager::<usize, String>::new(1).unwrap();
    let job = manager
        .submit_with_progress(
            1,
            |_, p| {
                let work = SolverWork {
                    fine_move_attempts: 7,
                    ..Default::default()
                };
                assert!(p.record_work(work));
                assert!(!p.record_work(SolverWork {
                    fine_move_attempts: 6,
                    ..work
                }));
                assert!(p.add_work(SolverWork {
                    fine_move_attempts: 3,
                    ..Default::default()
                }));
                assert!(!p.record_work(SolverWork {
                    fine_move_attempts: 9,
                    ..work
                }));
                assert!(p.record_work(SolverWork {
                    fine_move_attempts: 10,
                    ..work
                }));
                assert!(p.add_work(SolverWork {
                    fine_move_attempts: u64::MAX,
                    ..Default::default()
                }));
                Ok(1)
            },
            SolverJobCallbacks::default(),
        )
        .unwrap();
    assert!(job.wait_timeout(WAIT).unwrap().is_ok());
    assert_eq!(job.statistics().work.unwrap().fine_move_attempts, u64::MAX);
    assert!(job.statistics().work_overflowed);
    manager.shutdown().unwrap();
}
