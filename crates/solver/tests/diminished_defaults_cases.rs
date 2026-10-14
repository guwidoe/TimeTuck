// Adapted from pinned Community TerminationFactoryTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::time::Duration;
use timetuck::*;
#[test]
fn diminished_returns_default() {
    let config = default_diminished_returns_config();
    let _: DiminishedReturns<SimpleScore> = DiminishedReturns::new(config).unwrap();
    assert_eq!(config.window(), Duration::from_secs(30));
    assert_eq!(config.ratio(), (1.into(), 10_000.into()));
}
#[test]
fn diminished_returns_custom() {
    let window = Duration::from_millis(1)
        + Duration::from_secs(2)
        + Duration::from_secs(3 * 60)
        + Duration::from_secs(4 * 3600)
        + Duration::from_secs(5 * 86400);
    let config = DiminishedReturnsConfig::new(window, 1, 2).unwrap();
    let _: DiminishedReturns<SimpleScore> = DiminishedReturns::new(config).unwrap();
    assert_eq!(config.window().as_nanos(), 446_582_001_000_000);
    assert_eq!(config.ratio(), (1.into(), 2.into()));
}
#[test]
fn diminished_returns_custom_duration() {
    let config = DiminishedReturnsConfig::new(Duration::from_secs(123 * 60), 1, 2).unwrap();
    let _: DiminishedReturns<SimpleScore> = DiminishedReturns::new(config).unwrap();
    assert_eq!(config.window(), Duration::from_secs(123 * 60));
    assert_eq!(config.ratio(), (1.into(), 2.into()));
}
