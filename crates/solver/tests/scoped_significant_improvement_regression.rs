use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
#[test]
fn real_profile_construction_ignores_budget_and_handoff_gap_counts_toward_solver() {
    let now = Arc::new(AtomicU64::new(0));
    let source = now.clone();
    let clock = SolverClock::new(move || ms(source.load(Ordering::SeqCst)));
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]); 2]).unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(2);
    let profile = ScoredTerminationProfile::new(
        TerminationProfile::new(None, Some(ms(1000)), None, None, None, None).unwrap(),
        Some(SimpleScore::new(7)),
        None,
    )
    .unwrap();
    let mut events = vec![];
    let mut lifecycle = |event| {
        events.push(event);
        match event {
            SolverLifecycleEvent::StepEnded {
                kind: SolverPhaseKind::Construction,
                completed: true,
                ..
            } => {
                now.fetch_add(600, Ordering::SeqCst);
            }
            SolverLifecycleEvent::PhaseEnded {
                kind: SolverPhaseKind::Construction,
                ..
            } => {
                now.fetch_add(500, Ordering::SeqCst);
            }
            SolverLifecycleEvent::StepEnded {
                kind: SolverPhaseKind::LocalSearch,
                completed: true,
                ..
            } => {
                now.fetch_add(500, Ordering::SeqCst);
            }
            _ => {}
        }
    };
    let result = profile
        .with_sequence(
            SolverSequence {
                phases: vec![
                    SolverPhase::Construction(ConstructionConfig::default()),
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 100,
                        max_evaluations: 1000,
                        acceptance: Acceptance::HillClimbing,
                        selection: MoveSelection::FirstAccepted,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            },
            |sequence| {
                solve_with_extensions(
                    &p,
                    &n,
                    sequence,
                    SolverExtensions {
                        clock: Some(&clock),
                        lifecycle: Some(&mut lifecycle),
                        ..Default::default()
                    },
                )
            },
        )
        .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::ZERO));
    assert_eq!(result.phases[0].work.construction_steps, 2);
    assert_eq!(result.phases[1].work.completed_steps, 2);
    assert_eq!(
        result.termination,
        SolverTermination::SignificantImprovementTimeLimit
    );
    assert!(events.contains(&SolverLifecycleEvent::PhaseStarted {
        phase: 1,
        kind: SolverPhaseKind::LocalSearch
    }));
}
#[test]
fn phases_reset_only_local_anchor_and_expired_thresholds_never_revive() {
    let mut t = ScopedSignificantImprovement::new(
        SignificantImprovementConfig::new(ms(1000), SimpleScore::new(7)).unwrap(),
    );
    t.phase_started(SolverPhaseKind::LocalSearch, ms(0));
    t.observe_best(ms(0), &SimpleScore::ZERO);
    t.observe_best(ms(400), &SimpleScore::new(8));
    assert_eq!(t.progress(TerminationProgressScope::Solve, ms(600)), 0.2);
    t.phase_ended(SolverPhaseKind::LocalSearch, ms(600));
    t.phase_started(SolverPhaseKind::LocalSearch, ms(700));
    t.observe_best(ms(700), &SimpleScore::new(8));
    assert_eq!(t.progress(TerminationProgressScope::Phase, ms(900)), 0.2);
    assert_eq!(t.progress(TerminationProgressScope::Solve, ms(900)), 0.5);
    t.observe_best(ms(1401), &SimpleScore::new(100));
    assert!(t.is_terminated(TerminationProgressScope::Solve, ms(1401)));
    assert!(!t.is_terminated(TerminationProgressScope::Phase, ms(1401))); // Timely for phase, too late for solve.
    assert_eq!(t.progress(TerminationProgressScope::Phase, ms(1401)), 0.0);
}
