use timetuck::*;
include!("fixtures/maps_client_fixture.rs");
#[test]
fn cache_merge_errors_and_identity_use_actual_transport() {
    let a = p(4.);
    let b = p(5.);
    let c = p(6.);
    let mut count = 0;
    let mut transport = |r: &MapsRequest| {
        count += 1;
        let mut out = response(vec![Some(
            if matches!(r.kind, MapsRequestKind::Update { .. }) {
                frame(a, c, 30, 300)
            } else {
                frame(a, b, 20, 200)
            },
        )]);
        out.resolved_map = if count == 1 {
            Some("region".into())
        } else {
            None
        };
        Ok(out)
    };
    let mut client = MapsClient::new(&mut transport, false, "custom", 9);
    let seed = model(Some("dataset"), &[4., 5.]);
    let first = client.enrich(&seed).unwrap();
    assert!(Arc::ptr_eq(&first.model.locations[0], &seed.locations[0]));
    let updated = client
        .enrich(&model(Some("dataset"), &[4., 5., 6.]))
        .unwrap();
    assert_eq!(updated.distance(a, b, None).unwrap(), 200);
    assert_eq!(updated.distance(a, c, None).unwrap(), 300);
    assert_eq!(client.resolved_map(), Some("region"));
    assert!(client.enrich(&model(None, &[0., 1., 2., 3.])).is_err());
    drop(client);
    assert_eq!(count, 2);
}
#[test]
fn invalid_responses_do_not_publish_context_and_waypoint_failures_retry() {
    let mut attempts = 0;
    let mut c = MapsClient::new(
        |r: &MapsRequest| {
            attempts += 1;
            if r.kind == MapsRequestKind::Waypoints && attempts > 1 {
                let mut out = response(vec![]);
                out.waypoints = r.model.locations.iter().map(|p| **p).collect();
                Ok(out)
            } else {
                Err(MapsTransportError::Failure("network".into()))
            }
        },
        false,
        "",
        4,
    );
    let m = model(None, &[0., 1.]);
    c.publish_routes("dataset", MapsSolutionEvent::Initialized, routes(&m))
        .unwrap();
    assert!(c.waypoints("dataset", &ids(&[])).is_err());
    let result = c.waypoints("dataset", &ids(&[])).unwrap();
    assert!(Arc::ptr_eq(&result[0].locations[0], &m.locations[0]));
    drop(c);
    assert_eq!(attempts, 2);
    let mut invalid = response(vec![Some(frame(p(0.), p(8.), 2, 3))]);
    invalid.resolved_map = Some("must-not-publish".into());
    let mut c = MapsClient::new(|_: &MapsRequest| Ok(invalid.clone()), false, "", 4);
    assert!(c.enrich(&m).is_err());
    assert_eq!(c.resolved_map(), None);
}

#[test]
fn unchanged_named_matrix_rebinds_new_location_allocations() {
    let mut c = client();
    let initial = model(Some("current"), &[0., 1.]);
    let old = c.enrich(&initial).unwrap();
    let current = model(Some("current"), &[0., 1.]);
    assert!(!Arc::ptr_eq(&initial.locations[0], &current.locations[0]));
    let rebound = c.enrich(&current).unwrap();
    assert!(Arc::ptr_eq(
        &rebound.model.locations[0],
        &current.locations[0]
    ));
    assert!(!Arc::ptr_eq(&rebound, &old));
    assert_eq!(rebound.distance(p(0.), p(1.), None).unwrap(), 157249);
}

#[test]
fn named_cache_is_atomic_after_malformed_response_and_missing_frames_error() {
    let a = p(0.);
    let b = p(1.);
    let mut attempt = 0;
    let mut c = MapsClient::new(
        |_: &MapsRequest| {
            attempt += 1;
            if attempt == 3 {
                return Err(MapsTransportError::NotModified);
            }
            let mut out = response(vec![Some(if attempt == 1 {
                frame(a, b, 7, 70)
            } else {
                frame(a, p(9.), 99, 990)
            })]);
            out.resolved_map = Some(if attempt == 1 { "good" } else { "bad" }.into());
            Ok(out)
        },
        false,
        "",
        4,
    );
    let m = model(Some("atomic"), &[0., 1.]);
    let original = c.enrich(&m).unwrap();
    assert!(c.enrich(&m).is_err());
    assert_eq!(c.resolved_map(), Some("good"));
    let cached = c.enrich(&m).unwrap();
    assert!(Arc::ptr_eq(&cached, &original));
    assert_eq!(cached.travel_time(a, b, None).unwrap(), 7);
    let reply = response(vec![Some(frame(a, b, 3, 30)), None, None]);
    let mut c = MapsClient::new(|_: &MapsRequest| Ok(reply.clone()), true, "", 4);
    let enriched = c.enrich(&model(None, &[0., 1.])).unwrap();
    assert!(
        enriched
            .travel_time(a, b, Some(OffsetDeparture::new(22 * 3600, 0).unwrap()))
            .is_err()
    );
    assert_eq!(enriched.travel_time(a, b, None).unwrap(), 3);
}
#[test]
fn malformed_waypoint_endpoint_is_retryable_and_not_cached() {
    let mut attempt = 0;
    let mut c = MapsClient::new(
        |r: &MapsRequest| {
            attempt += 1;
            let mut out = response(vec![]);
            out.waypoints = if attempt == 1 {
                vec![p(8.), p(9.)]
            } else {
                r.model.locations.iter().map(|p| **p).collect()
            };
            Ok(out)
        },
        false,
        "",
        4,
    );
    let m = model(None, &[0., 1.]);
    c.publish_routes("route", MapsSolutionEvent::Initialized, routes(&m))
        .unwrap();
    assert!(c.waypoints("route", &ids(&[])).is_err());
    let valid = c.waypoints("route", &ids(&[])).unwrap();
    assert!(Arc::ptr_eq(&valid[0].locations[1], &m.locations[1]));
    c.waypoints("route", &ids(&[])).unwrap();
    drop(c);
    assert_eq!(attempt, 2);
}

#[test]
fn gone_full_replacement_does_not_retain_deleted_cached_edges() {
    let mut calls = 0;
    let mut c = MapsClient::new(
        |r: &MapsRequest| {
            calls += 1;
            if matches!(r.kind, MapsRequestKind::Update { .. }) {
                return Err(MapsTransportError::Gone);
            }
            Ok(response(vec![Some(if calls == 1 {
                frame(p(0.), p(1.), 7, 70)
            } else {
                MapsMatrixFrame::new()
            })]))
        },
        false,
        "",
        4,
    );
    let m = model(Some("replacement"), &[0., 1.]);
    c.enrich(&m).unwrap();
    let replaced = c.enrich(&m).unwrap();
    assert!(replaced.travel_time(p(0.), p(1.), None).is_err());
    drop(c);
    assert_eq!(calls, 3);
}
