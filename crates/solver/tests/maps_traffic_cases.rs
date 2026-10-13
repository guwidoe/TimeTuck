// Adapted pinned Community TrafficTest full scalar/timeframe assertions; Apache-2.0.
use timetuck::*;
include!("fixtures/maps_client_fixture.rs");
fn at(h: i64) -> OffsetDeparture {
    OffsetDeparture::new(1704067200 + h * 3600, 0).unwrap()
}
#[test]
fn regular_model_uses_single_matrix() {
    let mut calls = Vec::new();
    let reply = response(vec![Some(frame(p(0.), p(1.), 75, 750))]);
    let mut c = MapsClient::new(
        |r: &MapsRequest| {
            calls.push(r.kind.clone());
            Ok(reply.clone())
        },
        false,
        "sourceOption3:1000.0",
        4,
    );
    let e = c.enrich(&model(None, &[0., 1.])).unwrap();
    assert_eq!(e.travel_time(p(0.), p(1.), None).unwrap(), 75);
    assert_eq!(e.distance(p(0.), p(1.), None).unwrap(), 750);
    drop(c);
    assert_eq!(calls, [MapsRequestKind::Full]);
}
#[test]
fn regular_model_with_traffic_enabled_fetches_all_timeframes() {
    let mut calls = Vec::new();
    let reply = response(vec![
        Some(frame(p(0.), p(1.), 100, 1000)),
        Some(frame(p(0.), p(1.), 500, 1200)),
        None,
    ]);
    let mut c = MapsClient::new(
        |r: &MapsRequest| {
            calls.push(r.clone());
            Ok(reply.clone())
        },
        true,
        "sourceOption3:1000.0",
        4,
    );
    let m = model(None, &[0., 1.]);
    let e = c.enrich(&m).unwrap();
    assert_eq!(e.travel_time(p(0.), p(1.), Some(at(8))).unwrap(), 100);
    assert_eq!(e.travel_time(p(0.), p(1.), Some(at(16))).unwrap(), 500);
    assert_eq!(e.distance(p(0.), p(1.), Some(at(16))).unwrap(), 1200);
    assert_eq!(e.travel_time(p(0.), p(1.), None).unwrap(), 100);
    assert_eq!(e.distance(p(0.), p(1.), None).unwrap(), 1000);
    drop(c);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].kind, MapsRequestKind::Timeframes);
    assert_eq!(calls[0].model.locations, m.locations);
}
#[test]
fn single_bucket_result_uses_scalar_matrices() {
    let reply = response(vec![Some(frame(p(0.), p(1.), 75, 750))]);
    let mut c = MapsClient::new(|_: &MapsRequest| Ok(reply.clone()), true, "", 4);
    let e = c.enrich(&model(None, &[0., 1.])).unwrap();
    assert_eq!(e.travel_time(p(0.), p(1.), None).unwrap(), 75);
    assert_eq!(e.distance(p(0.), p(1.), None).unwrap(), 750);
    assert_eq!(e.travel_time(p(0.), p(1.), Some(at(8))).unwrap(), 75);
    assert_eq!(e.distance(p(0.), p(1.), Some(at(16))).unwrap(), 750);
}
#[test]
fn accepts_locations_aware_solver_model() {
    assert!(MapsClient::<Server>::accepts(&model(None, &[])));
    assert!(!MapsClient::<Server>::accepts(&()));
}
#[test]
fn wraps_non_timefold_exception_from_map_service() {
    let mut c = MapsClient::new(
        |_: &MapsRequest| Err(MapsTransportError::Failure("boom".into())),
        true,
        "",
        4,
    );
    let e = c.enrich(&model(None, &[0., 1.])).unwrap_err();
    assert!(
        e.to_string()
            .contains("Error getting travel time and distances")
    );
}
