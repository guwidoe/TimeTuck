// Adapted pinned Community WaypointsServiceImplTest; Apache-2.0.
use timetuck::*;
include!("fixtures/maps_client_fixture.rs");
#[test]
fn test_remote_connection_with_map_server() {
    let mut c = client();
    c.publish_routes(
        "dataset",
        MapsSolutionEvent::Initialized,
        routes(&model(None, &[0., 1.])),
    )
    .unwrap();
    assert_eq!(c.waypoints("dataset", &ids(&[])).unwrap().len(), 1);
    assert_eq!(c.transport().requests.len(), 1);
}
#[test]
fn test_one_location_should_return_empty_waypoints() {
    let mut c = client();
    let m = model(None, &[0.]);
    c.publish_routes("dataset", MapsSolutionEvent::Initialized, routes(&m))
        .unwrap();
    let w = c.waypoints("dataset", &ids(&[])).unwrap();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].locations.len(), 1);
    assert_eq!(w[0].locations[0], m.locations[0]);
}
#[test]
fn test_remote_connection_with_map_server_calculate_on_change_only() {
    let mut c = client();
    let m = model(None, &[0., 1.]);
    for (event, count) in [
        (MapsSolutionEvent::Initialized, 1),
        (MapsSolutionEvent::Best, 2),
        (MapsSolutionEvent::Final, 3),
    ] {
        c.publish_routes("dataset", event, routes(&m)).unwrap();
        assert_eq!(c.waypoints("dataset", &ids(&[])).unwrap().len(), 1);
        assert_eq!(c.transport().requests.len(), count);
        if count < 3 {
            assert_eq!(c.waypoints("dataset", &ids(&[])).unwrap().len(), 1);
            assert_eq!(c.transport().requests.len(), count);
        }
    }
}
fn verify(w: &[MapsWaypointRoute], groups: &[usize]) {
    assert_eq!(w.len(), groups.len());
    for (route, i) in w.iter().zip(groups) {
        assert_eq!(route.locations.len(), 2);
        assert!(route.locations.contains(&Arc::new(p((*i * 2) as f64))));
        assert!(route.locations.contains(&Arc::new(p((*i * 2 + 1) as f64))));
    }
}
#[test]
fn test_remote_connection_with_map_server_calculate_on_change_only_filtered() {
    let mut c = client();
    let m = model(None, &[0., 1., 2., 3.]);
    c.publish_routes("dataset", MapsSolutionEvent::Initialized, routes(&m))
        .unwrap();
    for (filter, groups, count) in [
        (ids(&["id_0"]), vec![0], 1),
        (ids(&["id_0"]), vec![0], 1),
        (ids(&["id_1"]), vec![1], 2),
        (ids(&["id_0", "id_1"]), vec![0, 1], 2),
    ] {
        verify(&c.waypoints("dataset", &filter).unwrap(), &groups);
        assert_eq!(c.transport().requests.len(), count);
    }
    c.publish_routes("dataset", MapsSolutionEvent::Best, routes(&m))
        .unwrap();
    verify(&c.waypoints("dataset", &ids(&["id_0"])).unwrap(), &[0]);
    assert_eq!(c.transport().requests.len(), 3);
    verify(&c.waypoints("dataset", &ids(&["id_1"])).unwrap(), &[1]);
    assert_eq!(c.transport().requests.len(), 4);
    c.publish_routes("dataset", MapsSolutionEvent::Best, routes(&m))
        .unwrap();
    verify(
        &c.waypoints("dataset", &ids(&["id_0", "id_1"])).unwrap(),
        &[0, 1],
    );
    assert_eq!(c.transport().requests.len(), 6);
}
