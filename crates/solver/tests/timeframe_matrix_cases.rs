// Adapted from pinned Timefold Community LocationTest.
// SPDX-License-Identifier: Apache-2.0
// Native hour inputs preserve the test resolver's only timestamp observation.
use timetuck::*;
fn points() -> (GeoLocation, GeoLocation) {
    (
        GeoLocation::new(0.0, 0.0).unwrap(),
        GeoLocation::new(1.0, 1.0).unwrap(),
    )
}
fn matrix(cost: i64) -> DirectedDistanceMatrix<GeoLocation> {
    let (a, b) = points();
    let mut m = DirectedDistanceMatrix::with_capacity(2).unwrap();
    m.put(&a, &b, cost).unwrap();
    m
}
fn frame(hour: &u8) -> usize {
    if *hour < 12 {
        0
    } else if *hour < 18 {
        1
    } else {
        2
    }
}
#[test]
fn picks_correct_travel_time_per_timeframe() {
    let (a, b) = points();
    let morning = matrix(100);
    let afternoon = matrix(500);
    let mut set = TimeframeMatrices::new();
    set.set_timeframes(vec![Some(&morning), Some(&afternoon), None], frame);
    assert_eq!(set.get(&a, &b, Some(&8)), Ok(100));
    assert_eq!(set.get(&a, &b, Some(&16)), Ok(500));
}
#[test]
fn picks_correct_distance_per_timeframe() {
    let (a, b) = points();
    let morning = matrix(1000);
    let afternoon = matrix(1200);
    let mut set = TimeframeMatrices::new();
    set.set_timeframes(vec![Some(&morning), Some(&afternoon), None], frame);
    assert_eq!(set.get(&a, &b, Some(&8)), Ok(1000));
    assert_eq!(set.get(&a, &b, Some(&16)), Ok(1200));
}
#[test]
fn timeframeless_and_timeframe_lookups_use_their_own_matrices() {
    let (a, b) = points();
    let st = matrix(10);
    let sd = matrix(20);
    let mt = matrix(100);
    let md = matrix(1000);
    let mut time = TimeframeMatrices::new();
    let mut distance = TimeframeMatrices::new();
    time.set_single(&st);
    distance.set_single(&sd);
    time.set_timeframes(vec![Some(&mt), None, None], frame);
    distance.set_timeframes(vec![Some(&md), None, None], frame);
    assert_eq!(time.get(&a, &b, None), Ok(10));
    assert_eq!(distance.get(&a, &b, None), Ok(20));
    assert_eq!(time.get(&a, &b, Some(&8)), Ok(100));
    assert_eq!(distance.get(&a, &b, Some(&8)), Ok(1000));
}
#[test]
fn timeframeless_lookup_falls_back_to_first_timeframe() {
    let (a, b) = points();
    let t = matrix(500);
    let d = matrix(1200);
    let mut time = TimeframeMatrices::new();
    let mut distance = TimeframeMatrices::new();
    time.set_timeframes(vec![None, Some(&t), None], frame);
    distance.set_timeframes(vec![None, Some(&d), None], frame);
    assert_eq!(time.get(&a, &b, None), Ok(500));
    assert_eq!(distance.get(&a, &b, None), Ok(1200));
}
#[test]
fn timeframe_lookup_falls_back_to_single() {
    let (a, b) = points();
    let t = matrix(10);
    let d = matrix(20);
    let mut time = TimeframeMatrices::<_, u8>::new();
    let mut distance = TimeframeMatrices::<_, u8>::new();
    time.set_single(&t);
    distance.set_single(&d);
    assert_eq!(time.get(&a, &b, Some(&8)), Ok(10));
    assert_eq!(time.get(&a, &b, Some(&22)), Ok(10));
    assert_eq!(distance.get(&a, &b, Some(&16)), Ok(20));
}
#[test]
fn missing_timeframe_cell_throws() {
    let (a, b) = points();
    let m = matrix(100);
    let mut set = TimeframeMatrices::new();
    set.set_timeframes(vec![Some(&m), None, None], frame);
    let e = set.get(&a, &b, Some(&22)).unwrap_err();
    assert_eq!(e, RoutingError::MissingTimeframe { index: 2 });
    assert!(e.to_string().contains("No matrix fetched"));
}
#[test]
fn unconfigured_timeframe_lookup_throws() {
    let (a, b) = points();
    let time = TimeframeMatrices::<GeoLocation, u8>::new();
    let distance = TimeframeMatrices::<GeoLocation, u8>::new();
    assert_eq!(time.get(&a, &b, Some(&8)), Err(RoutingError::Unconfigured));
    assert_eq!(
        distance.get(&a, &b, Some(&8)),
        Err(RoutingError::Unconfigured)
    );
}
#[test]
fn out_of_bounds_index_throws() {
    let (a, b) = points();
    let m = matrix(100);
    let mut set = TimeframeMatrices::new();
    set.set_timeframes(vec![Some(&m)], |_: &u8| 5);
    let e = set.get(&a, &b, Some(&8)).unwrap_err();
    assert_eq!(e, RoutingError::TimeframeOutOfBounds { index: 5, count: 1 });
    assert!(e.to_string().contains("out of bounds"));
}
