// Adapted pinned Community maps TESTS; Apache-2.0.
use std::sync::Arc;
use timetuck::*;
fn p(a: f64, b: f64) -> GeoLocation {
    GeoLocation::new(a, b).unwrap()
}
#[test]
fn adjust_identity_returns_same_instance_reachable() {
    let original = NativeTravelTime::of(123).unwrap();
    assert!(Arc::ptr_eq(
        &original,
        &original.adjust(Some(|s| s)).unwrap()
    ));
}
#[test]
fn adjust_identity_returns_same_instance_unreachable() {
    let original = NativeTravelTime::unreachable();
    assert!(Arc::ptr_eq(
        &original,
        &original.adjust(Some(|s| s)).unwrap()
    ));
}
#[test]
fn adjust_null_adjuster_throws_exception() {
    let original = NativeTravelTime::of(123).unwrap();
    assert!(original.adjust(None::<fn(i64) -> i64>).is_err());
}
#[test]
fn adjust_non_identity_adjusts_seconds() {
    assert_eq!(
        NativeTravelTime::of(10)
            .unwrap()
            .adjust(Some(|s| s + 5))
            .unwrap()
            .seconds(),
        Some(15)
    );
}
#[test]
fn adjust_non_identity_does_not_change_unreachable_and_returns_same_instance() {
    let original = NativeTravelTime::unreachable();
    assert!(Arc::ptr_eq(
        &original,
        &original.adjust(Some(|s| s + 5)).unwrap()
    ));
}
#[test]
fn calculate_travel_time() {
    for (a, b, meters) in [
        (p(51.0441461, 3.7336349), p(49.1913945, 16.6122723), 939748),
        (p(68.2359953, 14.5644379), p(65.5887708, 22.1518707), 442297),
    ] {
        assert_eq!(
            spherical_travel_time(a, b).seconds(),
            Some(spherical_driving_seconds(meters).unwrap())
        );
    }
}
#[test]
fn get_locations_out_of_map() {
    let locations = vec![
        Arc::new(p(51.0441461, 3.7336349)),
        Arc::new(p(49.1913945, 16.6122723)),
    ];
    assert!(spherical_locations_out_of_map(&locations).is_empty());
}
#[test]
fn get_waypoints() {
    let points = vec![
        Arc::new(p(42.529529, 1.572601)),
        Arc::new(p(42.516657, 1.528570)),
        Arc::new(p(42.504635, 1.522390)),
    ];
    assert_eq!(spherical_waypoints(&points), points);
}
#[test]
fn matrix_deserialization_should_be_successful() {
    let one = p(33.700770256281146, -84.32057487796858);
    let two = p(33.69706463808921, -84.3949320945087);
    let three = p(33.75, -84.30);
    let points = [one, two, three];
    let matrix = GeographicTravelMatrices::calculate(&points, &points, 9).unwrap();
    let restored = GeographicTravelMatrices::from_json(&matrix.to_json(), 9).unwrap();
    assert_eq!(restored.distance(one, three), Some(5795));
    assert_eq!(restored.distance(two, three), Some(10570));
    assert_eq!(restored.distance(one, two), Some(6891));
}
