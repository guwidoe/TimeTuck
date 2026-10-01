// Adapted from pinned Timefold Community SimulatedAnnealingAcceptorTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::{AcceptanceError, HardMediumSoftScore, SimpleScore, SimulatedAnnealing};

#[test]
fn late_acceptance_size() {
    // Upstream names this annealing test "lateAcceptanceSize".
    let mut state = SimulatedAnnealing::new(SimpleScore::new(200), 2, 0).unwrap();
    let mut current = SimpleScore::new(-1000);
    type Step = (f64, i64, &'static [(i64, f64, bool)]);
    let steps: [Step; 3] = [
        (
            0.0,
            -500,
            &[
                (-1300, 0.3, false),
                (-1200, 0.3, true),
                (-1200, 0.4, false),
                (-500, 0.4, true),
            ],
        ),
        (
            0.5,
            -800,
            &[(-700, 0.13, true), (-700, 0.14, false), (-800, 0.04, true)],
        ),
        (
            1.0,
            -400,
            &[
                (-800, 0.01, true),
                (-801, 0.01, false),
                (-1200, 0.01, false),
                (-700, 0.01, true),
                (-400, 0.01, true),
            ],
        ),
    ];
    for (gradient, selected, cases) in steps {
        assert_eq!(state.gradient(), gradient);
        for &(candidate, draw, expected) in cases {
            assert_eq!(
                state.accepts_at(current, SimpleScore::new(candidate), gradient, draw),
                Ok(expected)
            );
        }
        current = SimpleScore::new(selected);
        state.step_ended();
    }
}

#[test]
fn negative_simulated_annealing_size() {
    assert!(matches!(
        SimulatedAnnealing::new(HardMediumSoftScore::new(1, -1, 2), 1, 0),
        Err(AcceptanceError::InvalidTemperature)
    ));
}
