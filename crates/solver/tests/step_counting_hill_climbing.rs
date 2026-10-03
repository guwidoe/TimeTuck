// Adapted from pinned Timefold Community StepCountingHillClimbingAcceptorTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::{AcceptanceError, SimpleScore, StepCountingHillClimbing, StepCountingMode};
type Stage = (i64, &'static [(i64, bool)]);
fn exercise(mode: StepCountingMode, stages: &[Stage]) {
    let mut state = StepCountingHillClimbing::new(2, mode, SimpleScore::new(-1000)).unwrap();
    let mut current = SimpleScore::new(-1000);
    for &(next, checks) in stages {
        for &(candidate, expected) in checks {
            assert_eq!(
                state.accepts(current, SimpleScore::new(candidate)),
                expected,
                "mode={mode:?},current={current},candidate={candidate}"
            );
        }
        current = SimpleScore::new(next);
        state.step_ended(current);
    }
}

#[test]
fn type_step() {
    exercise(
        StepCountingMode::Step,
        &[
            (
                -500,
                &[
                    (-900, true),
                    (-500, true),
                    (-800, true),
                    (-2000, false),
                    (-1000, true),
                    (-900, true),
                ],
            ),
            (
                -700,
                &[
                    (-900, true),
                    (-2000, false),
                    (-700, true),
                    (-1000, true),
                    (-1001, false),
                    (-900, true),
                ],
            ),
            (
                -400,
                &[
                    (-700, true),
                    (-2000, false),
                    (-701, false),
                    (-600, true),
                    (-400, true),
                    (-700, true),
                ],
            ),
            (
                -400,
                &[
                    (-900, false),
                    (-700, true),
                    (-701, false),
                    (-400, true),
                    (-2000, false),
                    (-900, false),
                ],
            ),
            (
                -300,
                &[
                    (-400, true),
                    (-300, true),
                    (-500, false),
                    (-2000, false),
                    (-401, false),
                    (-400, true),
                ],
            ),
            (
                -300,
                &[
                    (-301, true),
                    (-400, true),
                    (-300, true),
                    (-2000, false),
                    (-600, false),
                    (-301, true),
                ],
            ),
        ],
    );
}

#[test]
fn type_equal_or_improving_step() {
    exercise(
        StepCountingMode::EqualOrImprovingStep,
        &[
            (
                -500,
                &[
                    (-900, true),
                    (-500, true),
                    (-800, true),
                    (-2000, false),
                    (-1000, true),
                    (-900, true),
                ],
            ),
            (
                -700,
                &[
                    (-900, true),
                    (-2000, false),
                    (-700, true),
                    (-1000, true),
                    (-1001, false),
                    (-900, true),
                ],
            ),
            (
                -400,
                &[
                    (-700, true),
                    (-2000, false),
                    (1000, true),
                    (-1001, false),
                    (-600, true),
                    (-400, true),
                    (-700, true),
                ],
            ),
            (
                -400,
                &[
                    (-900, false),
                    (-401, false),
                    (-400, true),
                    (-2000, false),
                    (-900, false),
                ],
            ),
            (
                -300,
                &[
                    (-400, true),
                    (-300, true),
                    (-500, false),
                    (-2000, false),
                    (-401, false),
                    (-400, true),
                ],
            ),
            (
                -300,
                &[
                    (-301, false),
                    (-400, false),
                    (-300, true),
                    (-2000, false),
                    (-600, false),
                    (-301, false),
                ],
            ),
        ],
    );
}

#[test]
fn type_improving_step() {
    exercise(
        StepCountingMode::ImprovingStep,
        &[
            (
                -500,
                &[
                    (-900, true),
                    (-500, true),
                    (-800, true),
                    (-2000, false),
                    (-1000, true),
                    (-900, true),
                ],
            ),
            (
                -700,
                &[
                    (-900, true),
                    (-2000, false),
                    (-700, true),
                    (-1000, true),
                    (-1001, false),
                    (-900, true),
                ],
            ),
            (
                -400,
                &[
                    (-700, true),
                    (-2000, false),
                    (1000, true),
                    (-1001, false),
                    (-600, true),
                    (-400, true),
                    (-700, true),
                ],
            ),
            (
                -400,
                &[
                    (-900, false),
                    (-401, false),
                    (-400, true),
                    (-2000, false),
                    (-900, false),
                ],
            ),
            (
                -300,
                &[
                    (-400, true),
                    (-300, true),
                    (-500, false),
                    (-2000, false),
                    (-401, false),
                    (-400, true),
                ],
            ),
            (
                -300,
                &[
                    (-301, true),
                    (-400, true),
                    (-401, false),
                    (-300, true),
                    (-2000, false),
                    (-600, false),
                    (-301, true),
                ],
            ),
        ],
    );
}

#[test]
fn zero_step_counting_hill_climbing_size() {
    assert!(matches!(
        StepCountingHillClimbing::new(0, StepCountingMode::Step, SimpleScore::ZERO),
        Err(AcceptanceError::ZeroStepCountingPeriod)
    ));
}
