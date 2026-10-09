// Adapted from pinned Timefold Community
// HaversineTravelTimeAndDistanceMatrixProviderTest#calculateDistance.
// SPDX-License-Identifier: Apache-2.0
use timetuck::{GeoLocation, great_circle_distance};
#[test]
fn calculate_distance() {
    let gent = GeoLocation::new(51.0441461, 3.7336349).unwrap();
    let brno = GeoLocation::new(49.1913945, 16.6122723).unwrap();
    assert_eq!(great_circle_distance(gent, brno), 939748);
    let svolvaer = GeoLocation::new(68.2359953, 14.5644379).unwrap();
    let lulea = GeoLocation::new(65.5887708, 22.1518707).unwrap();
    assert_eq!(great_circle_distance(svolvaer, lulea), 442297);
}
