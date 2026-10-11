// Adapted from TerminationTest and test-only TestdataEasyScoreCalculator.
// Timefold Community, Apache-2.0; see provenance additions and THIRD_PARTY_NOTICES.md.
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use timetuck::*;

fn run_deadlines(two_phases: bool) {
    let ticks = Arc::new(AtomicU64::new(8_000)); // Nonzero epoch is intentional.
    let source = ticks.clone();
    let clock = SolverClock::new(move || Duration::from_millis(source.load(Ordering::SeqCst)));
    let (entered, phases) = mpsc::channel();
    let (resume, resumes) = mpsc::channel();
    let (finished, results) = mpsc::channel();
    let worker = thread::spawn(move || {
        let p = FiniteProblem::new(vec![Variable::new(vec![1_i32, 2]); 2]).unwrap();
        let mut n = ConstraintNetwork::new(2);
        // Two ordered unequal-entity pairs sharing a non-null value each cost one.
        n.add_constraint("matching ordered pairs", vec![0, 1], |v| {
            SimpleScore::new(if v[0].is_some() && v[0] == v[1] {
                -2
            } else {
                0
            })
        })
        .unwrap();
        let local = |limit| {
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_time: limit,
                max_steps: u64::MAX,
                max_accepted_moves: u64::MAX,
                max_evaluations: u64::MAX,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            })
        };
        let mut plan = SolverSequence {
            phases: vec![
                SolverPhase::Construction(Default::default()),
                local(Some(Duration::from_secs(if two_phases { 1 } else { 2 }))),
            ],
            limits: SolverLimits {
                max_time: Some(Duration::from_secs(if two_phases { 2 } else { 1 })),
                max_evaluations: u64::MAX,
                ..Default::default()
            },
        };
        if two_phases {
            plan.phases.push(local(None));
        }
        let mut barrier = |event| {
            if let SolverLifecycleEvent::StepStarted {
                phase,
                kind: SolverPhaseKind::LocalSearch,
                step: 0,
            } = event
            {
                entered.send(phase).unwrap();
                resumes.recv_timeout(Duration::from_secs(10)).unwrap();
            }
        };
        let result = solve_with_extensions(
            &p,
            &n,
            plan,
            SolverExtensions {
                clock: Some(&clock),
                lifecycle: Some(&mut barrier),
                ..Default::default()
            },
        );
        finished.send(result).unwrap();
    });
    // Replace the source's sleeps/start counter with a stronger actual local-step barrier.
    assert_eq!(phases.recv_timeout(Duration::from_secs(10)).unwrap(), 1);
    assert!(matches!(results.try_recv(), Err(mpsc::TryRecvError::Empty)));
    ticks.fetch_add(1_001, Ordering::SeqCst);
    resume.send(()).unwrap();
    if two_phases {
        assert_eq!(phases.recv_timeout(Duration::from_secs(10)).unwrap(), 2);
        assert!(matches!(results.try_recv(), Err(mpsc::TryRecvError::Empty)));
        ticks.fetch_add(1_000, Ordering::SeqCst);
        resume.send(()).unwrap();
    }
    let result = results
        .recv_timeout(Duration::from_secs(10))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    // Successful owned result is the native non-null assertion; additional checks
    // prove which deadline ended each real phase.
    assert!(result.complete);
    assert_eq!(result.termination, SolverTermination::TimeLimit);
    assert_eq!(result.phases.len(), if two_phases { 3 } else { 2 });
    for phase in &result.phases[1..] {
        assert_eq!(
            phase.termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::TimeLimit)
        );
    }
}
#[test]
fn global_time_spent_termination_at_phase_level() {
    run_deadlines(false);
}
#[test]
fn global_time_spent_termination_at_phase_level_two_phases() {
    run_deadlines(true);
}
