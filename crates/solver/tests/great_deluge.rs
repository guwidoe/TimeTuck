// Adapted from pinned Timefold Community GreatDelugeAcceptorTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::{GreatDeluge, GreatDelugeRise, HardMediumSoftScore, SimpleScore};
type Stage = (i64, &'static [(i64, bool)]);
fn exercise(initial: i64, rise: GreatDelugeRise<SimpleScore>, stages: &[Stage]) {
    let mut state = GreatDeluge::new(None, rise, SimpleScore::new(initial)).unwrap();
    let mut current = SimpleScore::new(initial);
    for &(next, checks) in stages {
        for &(candidate, expected) in checks {
            assert_eq!(
                state.accepts(current, SimpleScore::new(candidate)),
                expected,
                "current={current},candidate={candidate},water={:?}",
                state.water_levels()
            );
        }
        current = SimpleScore::new(next);
        state.step_ended();
    }
}
#[test]
fn water_level_increment_score_simple_score() {
    exercise(
        -1000,
        GreatDelugeRise::Score(SimpleScore::new(100)),
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
                -600,
                &[
                    (-2000, false),
                    (-700, true),
                    (-1000, false),
                    (-600, true),
                    (-500, true),
                    (-901, false),
                ],
            ),
            (
                -350,
                &[
                    (-900, false),
                    (-2000, false),
                    (-700, true),
                    (-801, false),
                    (-350, true),
                    (-500, true),
                ],
            ),
        ],
    );
}
#[test]
fn water_level_increment_score_hard_medium_soft_score() {
    let current = HardMediumSoftScore::new(0, -200, -1000);
    let mut state = GreatDeluge::new(
        Some(HardMediumSoftScore::new(0, -100, -400)),
        GreatDelugeRise::Score(HardMediumSoftScore::new(0, 100, 100)),
        current,
    )
    .unwrap();
    for (h, m, s, expected) in [
        (0, -100, -300, true),
        (0, -100, -500, true),
        (0, -50, -800, true),
        (-5, -50, -100, false),
        (0, -22, -200, true),
    ] {
        assert_eq!(
            state.accepts(current, HardMediumSoftScore::new(h, m, s)),
            expected
        );
    }
    state.step_ended();
}
#[test]
fn water_level_increment_ratio() {
    exercise(
        -8,
        GreatDelugeRise::Ratio {
            numerator: 1,
            denominator: 10,
        },
        &[
            (-5, &[(-8, true), (-5, true), (-7, true), (-9, false)]),
            (
                -6,
                &[
                    (-10, false),
                    (-7, true),
                    (-9, false),
                    (-6, true),
                    (-8, true),
                ],
            ),
            (-4, &[(-9, false), (-8, false), (-7, true), (-4, true)]),
        ],
    );
}
