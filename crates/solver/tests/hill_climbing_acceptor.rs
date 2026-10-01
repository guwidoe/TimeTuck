// Adapted from Timefold Community HillClimbingAcceptorTest.hillClimbingEnabled.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::{Acceptance, SimpleScore};
#[test]
fn hill_climbing_enabled() {
    let acceptor = Acceptance::HillClimbing;
    let mut last = SimpleScore::new(-1000);
    for (candidate, expected) in [
        (-900, true),
        (-500, true),
        (-800, true),
        (-2000, false),
        (-1000, true),
        (-900, true),
    ] {
        assert_eq!(
            acceptor.accepts(last, SimpleScore::new(candidate)),
            expected
        );
    }
    // Completing a step changes the current score, independent of best score.
    last = SimpleScore::new(-500);
    for (candidate, expected) in [
        (-900, false),
        (-2000, false),
        (-700, false),
        (-1000, false),
        (600, true),
        (-500, true),
        (-501, false),
        (-900, false),
    ] {
        assert_eq!(
            acceptor.accepts(last, SimpleScore::new(candidate)),
            expected
        );
    }
}
