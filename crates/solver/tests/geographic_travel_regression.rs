use std::{cell::Cell, sync::Arc};
use timetuck::*;
fn p(a: f64, b: f64) -> GeoLocation {
    GeoLocation::new(a, b).unwrap()
}
#[test]
fn scalar_callbacks_and_identity_are_real() {
    let calls = Cell::new(0);
    let no_route = NativeTravelTime::unreachable();
    assert!(Arc::ptr_eq(&no_route, &NativeTravelTime::unreachable()));
    let adjusted = no_route
        .adjust(Some(|s| {
            calls.set(calls.get() + 1);
            s + 1
        }))
        .unwrap();
    assert!(Arc::ptr_eq(&adjusted, &no_route));
    assert_eq!(calls.get(), 0);
    for n in [0, 1, 99, 100000] {
        let original = NativeTravelTime::of(n).unwrap();
        let value = original
            .adjust(Some(|s| {
                calls.set(calls.get() + 1);
                s + 7
            }))
            .unwrap();
        assert_eq!(value.seconds(), Some(n + 7));
        assert!(!Arc::ptr_eq(&original, &value));
        assert_eq!(original.seconds(), Some(n));
    }
    assert_eq!(calls.get(), 4);
    assert!(
        NativeTravelTime::of(1)
            .unwrap()
            .adjust(Some(|_| -2))
            .is_err()
    );
}
#[test]
fn geographic_matrix_current_entries_limits_and_roundtrip() {
    let points = [p(0., 179.9), p(0., -179.9), p(89., 0.), p(-89., 0.)];
    assert!(great_circle_distance(points[0], points[1]) < 23000);
    assert!(GeographicTravelMatrices::calculate(&points, &points, 15).is_err());
    let mut matrix = GeographicTravelMatrices::calculate(&points[..2], &points[2..], 16).unwrap();
    assert_eq!(matrix.distance(points[0], points[1]), None);
    assert!(matrix.distance(points[0], points[2]).unwrap() > 0);
    matrix.set_unreachable(points[0], points[2]).unwrap();
    let restored = GeographicTravelMatrices::from_json(&matrix.to_json(), 16).unwrap();
    assert_eq!(restored.distance(points[0], points[1]), None);
    assert!(Arc::ptr_eq(
        &restored.travel_time(points[0], points[2]).unwrap(),
        &NativeTravelTime::unreachable()
    ));
    assert_eq!(
        restored.distance(points[1], points[3]),
        matrix.distance(points[1], points[3])
    );
    let arcs = points.into_iter().map(Arc::new).collect::<Vec<_>>();
    let waypoints = spherical_waypoints(&arcs);
    for (a, b) in arcs.iter().zip(waypoints) {
        assert!(Arc::ptr_eq(a, &b));
    }
    assert!(GeographicTravelMatrices::from_json(&matrix.to_json(), 15).is_err());
    assert!(
        GeographicTravelMatrices::from_json(
            br#"{"version":1,"locations":[[0,0],[0,0]],"distance_meters":[],"travel_seconds":[]}"#,
            4
        )
        .is_err()
    );
}
