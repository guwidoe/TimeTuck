// Adapted from pinned Community UnimprovedTimeMillisSpentScoreDifferenceThresholdTerminationTest.
// Apache-2.0; see compat/provenance.json for complete method attribution.
use std::time::Duration;
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn policy() -> SignificantImprovement<SimpleScore> {
    SignificantImprovement::new(
        SignificantImprovementConfig::new(ms(1000), SimpleScore::new(7)).unwrap(),
    )
}
// Preserve both phase and solver query assertions through the same native policy;
// Java scope spies and clock mocks become explicit retained scores and times.
fn queries(
    t: &mut SignificantImprovement<SimpleScore>,
    now: u64,
    score: i64,
    done: bool,
    gradient: Option<f64>,
) {
    for _ in 0..2 {
        assert_eq!(t.is_terminated(ms(now), &SimpleScore::new(score)), done);
        if let Some(g) = gradient {
            assert_eq!(t.progress(ms(now)), g);
        }
    }
}
#[test]
fn score_improves_termination_is_postponed() {
    let mut t = policy();
    t.start(ms(0), SimpleScore::ZERO);
    assert!(!t.is_terminated(ms(0), &SimpleScore::ZERO));
    queries(&mut t, 500, 0, false, Some(0.5));
    assert!(!t.is_terminated(ms(500), &SimpleScore::new(10)));
    assert_eq!(t.progress(ms(500)), 0.0);
    assert_eq!(t.progress(ms(500)), 0.0);
    queries(&mut t, 1500, 10, false, Some(1.0));
    queries(&mut t, 1501, 10, true, None);
}
#[test]
fn score_improves_too_late_terminates() {
    let mut t = policy();
    t.start(ms(0), SimpleScore::ZERO);
    assert!(!t.is_terminated(ms(0), &SimpleScore::ZERO));
    queries(&mut t, 500, 0, false, Some(0.5));
    assert!(!t.is_terminated(ms(1000), &SimpleScore::new(5)));
    queries(&mut t, 1000, 5, false, Some(1.0));
    assert!(t.is_terminated(ms(1001), &SimpleScore::new(10)));
    queries(&mut t, 1001, 10, true, None);
    // Native policy drops naturally; Java phaseEnded/solvingEnded have no asserted output.
}
