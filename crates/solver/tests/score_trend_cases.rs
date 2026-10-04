// Adapted from pinned Timefold Community InitializingScoreTrendTest.
// Apache-2.0; all fixture assertions retained. See compat/provenance.json.
use timetuck::{InitializingScoreTrend as Trend, InitializingScoreTrendLevel::*};
#[test]
fn parse_trend() {
    assert_eq!(
        Trend::parse("ONLY_DOWN/ANY/ONLY_UP", 3).unwrap().levels(),
        &[OnlyDown, Any, OnlyUp]
    );
}
#[test]
fn is_only_up() {
    assert!(
        Trend::parse("ONLY_UP/ONLY_UP/ONLY_UP", 3)
            .unwrap()
            .is_only_up()
    );
    assert!(!Trend::parse("ONLY_UP/ANY/ONLY_UP", 3).unwrap().is_only_up());
    assert!(
        !Trend::parse("ONLY_UP/ONLY_UP/ONLY_DOWN", 3)
            .unwrap()
            .is_only_up()
    );
}
#[test]
fn is_only_down() {
    assert!(
        Trend::parse("ONLY_DOWN/ONLY_DOWN/ONLY_DOWN", 3)
            .unwrap()
            .is_only_down()
    );
    assert!(
        !Trend::parse("ONLY_DOWN/ANY/ONLY_DOWN", 3)
            .unwrap()
            .is_only_down()
    );
    assert!(
        !Trend::parse("ONLY_DOWN/ONLY_DOWN/ONLY_UP", 3)
            .unwrap()
            .is_only_down()
    );
}
