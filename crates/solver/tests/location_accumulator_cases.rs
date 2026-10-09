// Adapted from pinned Timefold Community UniqueLocationAccumulatorTest.
// SPDX-License-Identifier: Apache-2.0
use std::sync::Arc;
use timetuck::*;
fn points() -> [GeoLocation; 2] {
    [
        GeoLocation::new(49.288087, 16.562172).unwrap(),
        GeoLocation::new(50.288087, 17.562172).unwrap(),
    ]
}
#[test]
fn empty_locations() {
    assert!(UniqueLocations::new().locations().is_empty());
}
#[test]
fn duplicate_locations_detected() {
    let [a, b] = points();
    let mut set = UniqueLocations::new();
    assert!(!set.add(Arc::new(a)));
    assert!(!set.add(Arc::new(b)));
    assert!(set.add(Arc::new(a)));
    assert!(set.add(Arc::new(b)));
}
#[test]
fn no_duplicate_locations() {
    let [a, b] = points();
    let mut intern = LocationInterner::new();
    let canonical: Vec<_> = [a, b, a, b].into_iter().map(|p| intern.intern(p)).collect();
    let mut set = UniqueLocations::new();
    for p in canonical {
        assert!(!set.add(p));
    }
    assert_eq!(set.locations().len(), 2);
    assert_eq!(
        set.locations().iter().map(|p| **p).collect::<Vec<_>>(),
        vec![a, b]
    );
}
