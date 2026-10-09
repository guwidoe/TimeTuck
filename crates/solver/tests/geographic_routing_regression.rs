//! Original spherical geometry, matrix generation and clock regressions.
use timetuck::*;
fn point(a: f64, b: f64) -> GeoLocation {
    GeoLocation::new(a, b).unwrap()
}
#[test]
fn geometry_handles_antipodes_poles_and_dateline() {
    let zero = point(0., 0.);
    assert_eq!(great_circle_distance(zero, zero), 0);
    assert_eq!(great_circle_distance(zero, point(0., 90.)), 10_007_543);
    assert_eq!(great_circle_distance(zero, point(0., 180.)), 20_015_087);
    assert_eq!(great_circle_distance(point(0., 180.), point(0., -180.)), 0);
    assert_eq!(great_circle_distance(point(90., 14.), point(90., -70.)), 0);
    for lat in [-90., -45., 0., 45., 90.] {
        for lon in [-180., -90., 0., 90., 180.] {
            let p = point(lat, lon);
            assert_eq!(
                great_circle_distance(p, zero),
                great_circle_distance(zero, p)
            );
            assert!((0..=20_015_087).contains(&great_circle_distance(p, zero)));
        }
    }
}
#[test]
fn explicit_speed_uses_checked_integer_ceiling() {
    assert!(matches!(
        GreatCircleRouting::new(0),
        Err(GreatCircleError::ZeroSpeed)
    ));
    let speed = GreatCircleRouting::new(3000).unwrap();
    for (meters, seconds) in [(0, 0), (1, 1), (3, 1), (4, 2), (300, 100)] {
        assert_eq!(speed.meters_to_seconds(meters), Ok(seconds));
    }
    assert_eq!(
        speed.meters_to_seconds(-1),
        Err(GreatCircleError::NegativeDistance(-1))
    );
    assert_eq!(
        GreatCircleRouting::new(1)
            .unwrap()
            .meters_to_seconds(i64::MAX),
        Err(GreatCircleError::TimeOverflow)
    );
    assert!(
        GreatCircleRouting::new(u64::MAX)
            .unwrap()
            .meters_to_seconds(i64::MAX)
            .is_ok()
    );
}
#[test]
fn generated_matrices_preserve_all_pairs_and_unique_order() {
    let speed = GreatCircleRouting::new(12345).unwrap();
    let a = point(-23., -19.);
    let b = point(74., 171.);
    let c = point(0., 0.);
    let matrices = speed.matrices(&[a, b, a, c, b]).unwrap();
    assert_eq!(matrices.distance_meters.len(), 3);
    assert_eq!(matrices.travel_seconds.len(), 3);
    for (i, from) in [a, b, c].iter().enumerate() {
        assert_eq!(matrices.distance_meters.index(from).unwrap().ordinal(), i);
        for to in [a, b, c] {
            let estimate = speed.estimate(*from, to);
            assert_eq!(
                matrices.distance_meters.get(from, &to),
                Some(estimate.distance_meters)
            );
            assert_eq!(
                matrices.travel_seconds.get(from, &to),
                Some(estimate.travel_seconds)
            );
        }
    }
    let foreign = matrices.distance_meters.index(&a).unwrap();
    assert_eq!(
        matrices.travel_seconds.get_indexed(foreign, foreign),
        Err(RoutingError::ForeignLocation)
    );
    let empty = speed.matrices(&[]).unwrap();
    assert!(empty.distance_meters.is_empty() && empty.travel_seconds.is_empty());
}
#[test]
fn clock_boundaries_offsets_extremes_and_overnight_intervals() {
    assert_eq!(
        OffsetDeparture::new(0, 64801),
        Err(RoutingTimeError::InvalidOffset(64801))
    );
    assert_eq!(
        OffsetDeparture::new(0, -64801),
        Err(RoutingTimeError::InvalidOffset(-64801))
    );
    for offset in [-64800, -1, 0, 1, 64800] {
        for unix in [i64::MIN, -86401, -1, 0, 1, 86400, i64::MAX] {
            let t = OffsetDeparture::new(unix, offset).unwrap();
            let local = (i128::from(unix) + i128::from(offset)).rem_euclid(86400);
            assert_eq!(t.local_hour(), (local / 3600) as u8);
            assert_eq!(Daypart::covering(t, t), Ok(vec![Daypart::at(t)]));
        }
    }
    let at = |s| OffsetDeparture::new(s, 0).unwrap();
    assert_eq!(
        Daypart::covering(at(23 * 3600), at(30 * 3600)).unwrap(),
        vec![Daypart::Night]
    );
    assert_eq!(
        Daypart::covering(at(23 * 3600), at(30 * 3600 + 1)).unwrap(),
        vec![Daypart::Morning, Daypart::Night]
    );
    assert_eq!(
        Daypart::covering(at(i64::MIN), at(i64::MAX)).unwrap(),
        Daypart::ALL
    );
    assert_eq!(
        Daypart::covering(at(1), at(0)),
        Err(RoutingTimeError::ReversedInterval)
    );
    let start = OffsetDeparture::new(9 * 3600, 0).unwrap();
    let end = OffsetDeparture::new(11 * 3600, 7200).unwrap();
    assert_eq!(
        Daypart::covering(start, end).unwrap(),
        vec![Daypart::Morning]
    );
}
#[test]
fn interval_buckets_match_independent_minute_walk() {
    for start in (-1440..1440).step_by(37) {
        for duration in [0, 1, 17, 359, 360, 361, 719, 720, 1000, 1439, 1440, 2000] {
            let mut seen = [false; 3];
            for minute in start..start + duration.max(1) {
                let local: i64 = minute;
                let m = local.rem_euclid(1440);
                let index = if m < 360 {
                    2
                } else if m < 720 {
                    0
                } else if m < 1080 {
                    1
                } else {
                    2
                };
                seen[index] = true;
            }
            let expected: Vec<_> = Daypart::ALL
                .into_iter()
                .filter(|p| seen[p.index()])
                .collect();
            let from = OffsetDeparture::new(start * 60, 0).unwrap();
            let to = OffsetDeparture::new((start + duration) * 60, 0).unwrap();
            assert_eq!(
                Daypart::covering(from, to).unwrap(),
                expected,
                "{start}, {duration}"
            );
        }
    }
}
#[test]
fn generated_matrices_compose_with_departure_lookup() {
    let a = point(0., 0.);
    let b = point(0., 0.01);
    let fast = GreatCircleRouting::new(10000)
        .unwrap()
        .matrices(&[a, b])
        .unwrap();
    let slow = GreatCircleRouting::new(1000)
        .unwrap()
        .matrices(&[a, b])
        .unwrap();
    let mut frames = TimeframeMatrices::new();
    frames.set_timeframes(
        vec![
            Some(&fast.travel_seconds),
            Some(&slow.travel_seconds),
            Some(&fast.travel_seconds),
        ],
        |t: &OffsetDeparture| Daypart::at(*t).index(),
    );
    assert_eq!(
        frames.get(&a, &b, Some(&OffsetDeparture::new(11 * 3600, 0).unwrap())),
        Ok(112)
    );
    assert_eq!(
        frames.get(&a, &b, Some(&OffsetDeparture::new(12 * 3600, 0).unwrap())),
        Ok(1112)
    );
    assert_eq!(fast.distance_meters.get(&a, &b), Some(1112));
}
