// Adapted from Timefold Community MoveCountTerminationTest and
// ScoreCalculationCountTerminationTest (Apache-2.0).
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::CountLimit;

#[test]
fn move_count_phase_termination() {
    let limit = CountLimit::new(4);
    for (count, reached, gradient) in [
        (0, false, 0.0),
        (1, false, 0.25),
        (2, false, 0.5),
        (3, false, 0.75),
        (4, true, 1.0),
        (5, true, 1.0),
    ] {
        assert_eq!(limit.is_reached(count), reached);
        assert_eq!(limit.progress(count), gradient);
    }
}
#[test]
fn move_count_solver_termination() {
    let limit = CountLimit::new(4);
    for (count, reached, gradient) in [
        (0, false, 0.0),
        (1, false, 0.25),
        (2, false, 0.5),
        (3, false, 0.75),
        (4, true, 1.0),
        (5, true, 1.0),
    ] {
        assert_eq!(limit.is_reached(count), reached);
        assert_eq!(limit.progress(count), gradient);
    }
}
#[test]
fn move_count_invalid_termination() {
    assert!(CountLimit::try_from(-1_i64).is_err());
}
#[test]
fn score_calculation_count_solve_termination() {
    let limit = CountLimit::new(1000);
    for (count, reached, gradient) in [
        (0, false, 0.0),
        (100, false, 0.1),
        (500, false, 0.5),
        (700, false, 0.7),
        (1000, true, 1.0),
        (1200, true, 1.0),
    ] {
        assert_eq!(limit.is_reached(count), reached);
        assert_eq!(limit.progress(count), gradient);
    }
}
#[test]
fn score_calculation_count_phase_termination() {
    let limit = CountLimit::new(1000);
    for (count, reached, gradient) in [
        (0, false, 0.0),
        (100, false, 0.1),
        (500, false, 0.5),
        (700, false, 0.7),
        (1000, true, 1.0),
        (1200, true, 1.0),
    ] {
        assert_eq!(limit.is_reached(count), reached);
        assert_eq!(limit.progress(count), gradient);
    }
}
