// Adapted from pinned Timefold Community LongArrayDistanceMatrixTest.
// SPDX-License-Identifier: Apache-2.0
use timetuck::*;
fn locations() -> Vec<GeoLocation> {
    [(0.0, 1.0), (1.0, 0.0), (0.25, 0.75), (0.75, 0.5)]
        .into_iter()
        .map(|(a, b)| GeoLocation::new(a, b).unwrap())
        .collect()
}
fn fill(m: &mut DirectedDistanceMatrix<GeoLocation>, ls: &[GeoLocation]) {
    for (i, a) in ls.iter().enumerate() {
        for (j, b) in ls.iter().enumerate() {
            m.put(a, b, (i * ls.len() + j) as i64).unwrap();
        }
    }
}
fn verify(m: &DirectedDistanceMatrix<GeoLocation>, ls: &[GeoLocation]) {
    for (i, a) in ls.iter().enumerate() {
        for (j, b) in ls.iter().enumerate() {
            assert_eq!(m.get(a, b), Some((i * ls.len() + j) as i64), "{i},{j}");
        }
    }
}
#[test]
fn cache_index_in_location() {
    let mut m = DirectedDistanceMatrix::with_capacity(2).unwrap();
    let mut a = CachedLocation::new(GeoLocation::new(0.0, 1.0).unwrap());
    let mut b = CachedLocation::new(GeoLocation::new(1.0, 0.0).unwrap());
    assert!(a.index_for(&m).is_none());
    assert!(b.index_for(&m).is_none());
    assert_eq!(a.bind(&mut m).unwrap().ordinal(), 0);
    assert_eq!(b.bind(&mut m).unwrap().ordinal(), 1);
    fill(&mut m, &[*a.key(), *b.key()]);
    assert_eq!(a.index_for(&m).unwrap().ordinal(), 0);
    assert_eq!(b.index_for(&m).unwrap().ordinal(), 1);
}
#[test]
fn put_no_resize() {
    let mut m = DirectedDistanceMatrix::with_capacity(4).unwrap();
    let ls = locations();
    fill(&mut m, &ls);
    verify(&m, &ls);
}
#[test]
fn put_with_resize() {
    let mut m = DirectedDistanceMatrix::with_capacity(2).unwrap();
    let ls = locations();
    fill(&mut m, &ls);
    verify(&m, &ls);
}
#[test]
fn values_preserved_after_resize() {
    let mut m = DirectedDistanceMatrix::with_capacity(2).unwrap();
    let ls = locations();
    fill(&mut m, &ls);
    m.reserve_locations(24).unwrap();
    verify(&m, &ls);
}
#[test]
fn many_put_with_resize() {
    let ls: Vec<_> = (-900..900)
        .map(|i| {
            let v = f64::from(i) * 0.1;
            GeoLocation::new(v, v).unwrap()
        })
        .collect();
    let mut m = DirectedDistanceMatrix::with_capacity(2).unwrap();
    fill(&mut m, &ls);
    verify(&m, &ls);
}
fn assert_count_error(result: Result<(), RoutingError>, expected: i64) {
    let e = result.unwrap_err();
    assert_eq!(
        e,
        RoutingError::InvalidCapacity {
            expected,
            maximum: MAX_MATRIX_LOCATIONS
        }
    );
    let text = e.to_string();
    assert!(text.contains(&expected.to_string()));
    assert!(text.contains(&MAX_MATRIX_LOCATIONS.to_string()));
    assert!(text.contains("between 0"));
}
#[test]
fn too_large_expected_size_fail_fast() {
    let n = MAX_MATRIX_LOCATIONS as i64 + 1;
    assert_count_error(
        DirectedDistanceMatrix::<GeoLocation>::with_capacity(n).map(|_| ()),
        n,
    );
}
#[test]
fn too_small_expected_size_fail_fast() {
    assert_count_error(
        DirectedDistanceMatrix::<GeoLocation>::with_capacity(-1).map(|_| ()),
        -1,
    );
}
#[test]
fn too_large_resize_fail_fast() {
    let n = MAX_MATRIX_LOCATIONS as i64 + 1;
    assert_count_error(
        DirectedDistanceMatrix::<GeoLocation>::with_capacity(0)
            .unwrap()
            .reserve_locations(n),
        n,
    );
}
#[test]
fn too_small_resize_fail_fast() {
    assert_count_error(
        DirectedDistanceMatrix::<GeoLocation>::with_capacity(0)
            .unwrap()
            .reserve_locations(-1),
        -1,
    );
}
#[test]
fn invalid_latitude_should_fail() {
    let e = GeoLocation::new(-91.0, 1.0).unwrap_err();
    assert!(matches!(e, RoutingError::InvalidLatitude(_)));
    assert!(
        e.to_string()
            .contains("Latitude must be between -90 and 90, found -91")
    );
}
#[test]
fn invalid_longitude_should_fail() {
    let e = GeoLocation::new(1.0, -181.0).unwrap_err();
    assert!(matches!(e, RoutingError::InvalidLongitude(_)));
    assert!(
        e.to_string()
            .contains("Longitude must be between -180 and 180, found -181")
    );
}
