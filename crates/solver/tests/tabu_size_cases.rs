// Adapted Apache-2.0 pinned Community EntityRatioTabuSizeStrategyTest
// and FixedTabuSizeStrategyTest. See compat/provenance.json.
use timetuck::{AcceptanceError, TabuSize};
#[test]
fn entity_ratio_tabu_size() {
    for (ratio, expected) in [
        (0.1, 10),
        (0.5, 50),
        (0.1051, 11),
        (0.1049, 10),
        (0.0000001, 1),
        (0.9999999, 99),
    ] {
        assert_eq!(TabuSize::ratio(ratio).unwrap().resolve(100), expected);
    }
}
#[test]
fn fixed_tabu_size() {
    assert_eq!(TabuSize::fixed(5).unwrap().resolve(0), 5);
    assert_eq!(TabuSize::fixed(17).unwrap().resolve(0), 17);
}
#[test]
fn invalid_fixed_tabu_size() {
    assert_eq!(TabuSize::fixed(0), Err(AcceptanceError::EmptyTabu));
    assert_eq!(TabuSize::fixed(-1), Err(AcceptanceError::EmptyTabu));
}
