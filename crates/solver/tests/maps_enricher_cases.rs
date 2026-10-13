// Adapted pinned Community TravelTimeMatrixEnricherTest; Apache-2.0.
use timetuck::*;
include!("fixtures/maps_client_fixture.rs");
fn costs(e: &EnrichedMapsModel) {
    assert_eq!(e.travel_time(p(0.), p(1.), None).unwrap(), 11322);
    assert_eq!(e.distance(p(0.), p(1.), None).unwrap(), 157249);
}
#[test]
fn test_remote_connection_with_map_server() {
    let mut c = client();
    let e = c.enrich(&model(None, &[0., 1.])).unwrap();
    assert_eq!(e.model.name, None);
    costs(&e);
    assert!(e.not_in_map.is_empty());
    assert_eq!(c.resolved_map(), Some("us-georgia"));
}
#[test]
fn test_remote_connection_with_map_server_with_named_locations() {
    let mut c = client();
    let e = c.enrich(&model(Some("name"), &[0., 1.])).unwrap();
    assert_eq!(e.model.name.as_deref(), Some("name"));
    costs(&e);
    assert!(e.not_in_map.is_empty());
}
#[test]
fn test_remote_connection_with_map_server_with_unknown_named_location() {
    let mut c = client();
    let m = model(Some("unknown"), &[0., 1.]);
    c.enrich(&m).unwrap();
    c.transport_mut().sets.remove("unknown");
    let e = c.enrich(&m).unwrap();
    assert_eq!(e.model.name.as_deref(), Some("unknown"));
    costs(&e);
    assert!(e.not_in_map.is_empty());
    assert!(matches!(
        c.transport().requests[1].kind,
        MapsRequestKind::Update { .. }
    ));
    assert_eq!(c.transport().requests[2].kind, MapsRequestKind::Full);
}
#[test]
fn test_distance_matrix_with_updates() {
    let mut c = client();
    let e = c.enrich(&model(Some("name2"), &[0., 1., 2., 3.])).unwrap();
    assert_eq!(e.model.name.as_deref(), Some("name2"));
    costs(&e);
    assert_eq!(e.travel_time(p(0.), p(2.), None).unwrap(), 22642);
    assert_eq!(e.distance(p(0.), p(2.), None).unwrap(), 314475);
    assert_eq!(e.travel_time(p(1.), p(0.), None).unwrap(), 11322);
    assert_eq!(e.distance(p(1.), p(0.), None).unwrap(), 157249);
    assert_eq!(e.distance(p(2.), p(3.), None).unwrap(), 157178);
    assert!(e.not_in_map.is_empty());
    assert_eq!(c.resolved_map(), Some("us-georgia"));
}
#[test]
fn test_one_location_should_return_empty_distance_matrix() {
    let mut c = client();
    let m = model(None, &[0.]);
    let e = c.enrich(&m).unwrap();
    assert_eq!(e.model.name, None);
    assert_eq!(e.model.locations.len(), 1);
    assert_eq!(e.model.locations[0], m.locations[0]);
    assert_eq!(e.travel_time(p(0.), p(0.), None).unwrap(), 0);
    assert_eq!(e.distance(p(0.), p(0.), None).unwrap(), 0);
    assert!(e.not_in_map.is_empty());
}
#[test]
fn distance_matrix_with_update_to_client_cache_retrieves_from_cache() {
    let mut c = client();
    let m = model(Some("name"), &[0., 1.]);
    let e = c.enrich(&m).unwrap();
    let e2 = c.enrich(&m).unwrap();
    assert!(e.not_in_map.is_empty());
    assert_eq!(e, e2);
    assert!(Arc::ptr_eq(&e, &e2));
    for l in m.locations {
        assert_ne!(e2.travel_time(p(0.), *l, None).unwrap(), -1);
    }
}
#[test]
fn test_distance_matrix_with_locations_out_of_map() {
    let mut c = client();
    let mut m = model(None, &[0., 1., 2.]);
    let outside = Arc::new(GeoLocation::new(-90., -90.).unwrap());
    m.locations.push(outside.clone());
    let e = c.enrich(&m).unwrap();
    assert!(e.not_in_map.contains(&outside));
    costs(&e);
    assert_eq!(e.travel_time(p(0.), p(2.), None).unwrap(), 22642);
    assert_eq!(e.distance(p(0.), p(2.), None).unwrap(), 314475);
    assert_eq!(e.travel_time(p(1.), p(0.), None).unwrap(), 11322);
    assert_eq!(e.distance(p(1.), p(0.), None).unwrap(), 157249);
    assert_eq!(e.distance(p(2.), *outside, None).unwrap(), 0);
}
#[test]
fn update_path_falls_back_to_cached_map_location_when_header_is_missing() {
    let mut c = client();
    c.enrich(&model(Some("with-updates"), &[0., 1.])).unwrap();
    assert_eq!(c.resolved_map(), Some("us-georgia"));
    c.enrich(&model(Some("with-updates"), &[0., 1., 2.]))
        .unwrap();
    assert_eq!(c.resolved_map(), Some("us-georgia"));
    assert!(matches!(
        c.transport().requests[1].kind,
        MapsRequestKind::Update { .. }
    ));
}
