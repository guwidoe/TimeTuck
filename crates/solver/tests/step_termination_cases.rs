// Adapted from Timefold Community termination tests (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::{StepCountLimit, StepProgress};

#[test]
fn step_count_phase_termination() {
    let limit = StepCountLimit::new(4);
    for (next_step, reached, gradient) in [
        (0, false, 0.0),
        (1, false, 0.25),
        (2, false, 0.5),
        (3, false, 0.75),
        (4, true, 1.0),
        (5, true, 1.0),
    ] {
        assert_eq!(limit.is_reached(next_step), reached);
        assert_eq!(limit.progress(next_step), gradient);
    }
}

#[test]
fn unimproved_step_count_phase_termination() {
    let limit = StepCountLimit::new(4);
    let mut progress = StepProgress::default();
    // Source best step index 10 corresponds to our eleventh completed step.
    for index in 0..=10 {
        progress.complete_step(index == 10);
    }
    for (last_index, next_index, reached, gradient) in [
        (10, 11, false, 0.0),
        (11, 12, false, 0.25),
        (12, 13, false, 0.5),
        (13, 14, false, 0.75),
        (14, 15, true, 1.0),
        (15, 16, true, 1.0),
    ] {
        assert_eq!(progress.best_step(), 11);
        assert_eq!(progress.completed_steps(), last_index + 1);
        assert_eq!(progress.completed_steps(), next_index);
        assert_eq!(limit.is_reached(progress.unimproved_steps()), reached);
        assert_eq!(limit.progress(progress.unimproved_steps()), gradient);
        progress.complete_step(false);
    }
}
