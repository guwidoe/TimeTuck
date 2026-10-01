// Adapted from pinned Timefold Community LateAcceptanceAcceptorTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::{AcceptanceError, HardSoftScore, LateAcceptance, SimpleScore};
fn check(state: &LateAcceptance<SimpleScore>, current: i64, cases: &[(i64, bool)]) {
    for &(candidate, expected) in cases {
        assert_eq!(
            state.accepts(SimpleScore::new(current), SimpleScore::new(candidate)),
            expected,
            "candidate {candidate}, current {current}"
        );
    }
}
#[test]
fn late_acceptance_size() {
    let mut state = LateAcceptance::new(3, false, SimpleScore::new(-1000)).unwrap();
    let mut current = i64::from(i32::MIN);
    let steps: [(i64, &[(i64, bool)]); 6] = [
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
                (-900, true),
                (-2000, false),
                (-1001, false),
                (-1000, true),
                (-400, true),
                (-900, true),
            ],
        ),
        (
            -200,
            &[
                (-900, false),
                (-500, true),
                (-501, false),
                (-200, true),
                (-2000, false),
                (-900, false),
            ],
        ),
        (
            -300,
            &[
                (-700, true),
                (-300, true),
                (-500, true),
                (-2000, false),
                (-701, false),
                (-700, true),
            ],
        ),
        (
            -300,
            &[
                (-401, false),
                (-400, true),
                (-300, true),
                (-2000, false),
                (-600, false),
                (-401, false),
            ],
        ),
    ];
    for (chosen, cases) in steps {
        check(&state, current, cases);
        state.step_ended(SimpleScore::new(chosen));
        current = chosen;
    }
}
#[test]
fn hill_climbing_enabled() {
    let mut state = LateAcceptance::new(2, true, SimpleScore::new(-1000)).unwrap();
    let mut current = -1000;
    let steps: [(i64, &[(i64, bool)]); 6] = [
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
            -200,
            &[
                (-900, false),
                (-700, true),
                (-701, false),
                (-200, true),
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
    ];
    for (chosen, cases) in steps {
        check(&state, current, cases);
        state.step_ended(SimpleScore::new(chosen));
        current = chosen;
    }
}
#[test]
fn reset_late_scores_on_hard_score_improvement() {
    let initial = HardSoftScore::new(-1, -1000);
    let mut state = LateAcceptance::new(3, false, initial).unwrap();
    state.step_ended(HardSoftScore::new(-1, -500));
    assert_eq!(state.history()[1], initial);
    assert_eq!(state.history()[2], initial);
    let improved = HardSoftScore::new(0, -200);
    state.step_ended(improved);
    for i in 0..3 {
        assert_eq!(state.history()[i], improved);
    }
}
#[test]
fn zero_late_acceptance_size() {
    assert!(matches!(
        LateAcceptance::new(0, true, SimpleScore::ZERO),
        Err(AcceptanceError::EmptyHistory)
    ));
}
