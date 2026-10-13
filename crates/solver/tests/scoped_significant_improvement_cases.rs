// Adapted from pinned Community UnimprovedTimeMillisSpentScoreDifferenceThresholdTerminationTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::time::Duration;
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn queries(
    t: &mut ScopedSignificantImprovement<SimpleScore>,
    now: u64,
    done: bool,
    phase: f64,
    solver: f64,
) {
    assert_eq!(
        t.is_terminated(TerminationProgressScope::Phase, ms(now)),
        done
    );
    assert_eq!(t.progress(TerminationProgressScope::Phase, ms(now)), phase);
    assert_eq!(
        t.is_terminated(TerminationProgressScope::Solve, ms(now)),
        done
    );
    assert_eq!(t.progress(TerminationProgressScope::Solve, ms(now)), solver);
}
#[test]
fn with_construction_heuristic_and_local_search() {
    let config = SignificantImprovementConfig::from_millis(1000, SimpleScore::new(7)).unwrap();
    let mut t = ScopedSignificantImprovement::new(config);
    t.phase_started(SolverPhaseKind::Construction, ms(0));
    t.observe_best(ms(0), &SimpleScore::ZERO);
    queries(&mut t, 500, false, 0.0, 0.0);
    t.observe_best(ms(500), &SimpleScore::ZERO);
    queries(&mut t, 1001, false, 0.0, 0.0);
    t.phase_ended(SolverPhaseKind::Construction, ms(1001));
    queries(&mut t, 1001, false, 0.0, 0.0);
    t.phase_started(SolverPhaseKind::LocalSearch, ms(1501));
    t.observe_best(ms(1501), &SimpleScore::new(5));
    queries(&mut t, 1501, false, 0.0, 0.5);
    t.observe_best(ms(1502), &SimpleScore::new(10));
    queries(&mut t, 1502, false, 0.001, 0.501);
    t.observe_best(ms(2001), &SimpleScore::new(10));
    queries(&mut t, 2001, false, 0.5, 1.0);
    t.observe_best(ms(2502), &SimpleScore::new(10));
    queries(&mut t, 2502, true, 1.0, 1.0);
    // Source invokes phaseEnded on its earlier CH scope; no output is asserted.
    t.phase_ended(SolverPhaseKind::Construction, ms(2502));
}
