// Adapted from Timefold Community SlotReservationMapTest, Apache-2.0.
// See compat/provenance.json for the pinned source and changes.
use std::{collections::HashMap, num::NonZeroUsize};
use timetuck::*;
fn dense() -> SlotReservationMap {
    let mut map = SlotReservationMap::new(200);
    for slot in 0..=16 {
        map.reserve(slot, slot + 100).unwrap();
    }
    map
}
#[test]
fn matches_hash_map_oracle() {
    let mut random = SampleRandom::new(0);
    let count = NonZeroUsize::new(200).unwrap();
    let mut map = SlotReservationMap::new(200);
    let mut oracle = HashMap::new();
    for _ in 0..10_000 {
        let slot = random.index(count);
        if random.index(NonZeroUsize::new(3).unwrap()) == 0 {
            map.release(slot).unwrap();
            oracle.remove(&slot);
        } else {
            let index = random.index(count);
            map.reserve(slot, index).unwrap();
            oracle.insert(slot, index);
        }
        for slot in 0..200 {
            assert_eq!(
                map.resolve(slot).unwrap(),
                *oracle.get(&slot).unwrap_or(&slot)
            );
        }
    }
}
#[test]
fn upgrades_to_dense_only_when_the_log_fills_up() {
    let mut map = SlotReservationMap::new(200);
    for slot in 0..16 {
        map.reserve(slot, slot + 100).unwrap();
        assert!(!map.is_dense());
    }
    map.reserve(16, 116).unwrap();
    assert!(map.is_dense());
    for slot in 0..=16 {
        assert_eq!(map.resolve(slot).unwrap(), slot + 100);
    }
    for slot in 17..200 {
        assert_eq!(map.resolve(slot).unwrap(), slot);
    }
}
#[test]
fn reserving_the_same_slot_again_does_not_grow_the_log() {
    let mut map = SlotReservationMap::new(200);
    for index in 100..120 {
        map.reserve(5, index).unwrap();
    }
    assert!(!map.is_dense());
    assert_eq!(map.resolve(5).unwrap(), 119);
}
#[test]
fn identity_reservations_do_not_fill_the_log() {
    let mut map = SlotReservationMap::new(200);
    for slot in 0..20 {
        map.reserve(slot, slot).unwrap();
    }
    assert!(!map.is_dense());
    for slot in 0..20 {
        assert_eq!(map.resolve(slot).unwrap(), slot);
    }
}
#[test]
fn an_identity_reservation_clears_an_existing_one() {
    let mut map = SlotReservationMap::new(200);
    map.reserve(5, 100).unwrap();
    map.reserve(5, 5).unwrap();
    assert_eq!(map.resolve(5).unwrap(), 5);
}
#[test]
fn dense_stage_distinguishes_logical_index_zero_from_the_identity() {
    let mut map = dense();
    map.reserve(3, 0).unwrap();
    assert!(map.is_dense());
    assert_eq!(map.resolve(3).unwrap(), 0);
    assert_eq!(map.resolve(0).unwrap(), 100);
}
#[test]
fn release_restores_the_identity_in_both_stages() {
    let mut sparse = SlotReservationMap::new(200);
    sparse.reserve(5, 100).unwrap();
    sparse.release(5).unwrap();
    let mut dense = dense();
    dense.release(3).unwrap();
    assert!(!sparse.is_dense());
    assert_eq!(sparse.resolve(5).unwrap(), 5);
    assert!(dense.is_dense());
    assert_eq!(dense.resolve(3).unwrap(), 3);
    assert_eq!(dense.resolve(4).unwrap(), 104);
}
#[test]
fn single_slot_source_never_deviates_from_the_identity() {
    let mut map = SlotReservationMap::new(1);
    map.reserve(0, 0).unwrap();
    assert!(!map.is_dense());
    assert_eq!(map.resolve(0).unwrap(), 0);
}
#[test]
fn two_slot_source_upgrades_on_the_second_reservation() {
    let mut map = SlotReservationMap::new(2);
    map.reserve(0, 1).unwrap();
    assert!(!map.is_dense());
    map.reserve(1, 0).unwrap();
    assert!(map.is_dense());
    assert_eq!(map.resolve(0).unwrap(), 1);
    assert_eq!(map.resolve(1).unwrap(), 0);
}
#[test]
fn four_slot_source_upgrades_on_the_third_reservation() {
    let mut map = SlotReservationMap::new(4);
    map.reserve(0, 1).unwrap();
    map.reserve(1, 2).unwrap();
    assert!(!map.is_dense());
    map.reserve(2, 3).unwrap();
    assert!(map.is_dense());
    assert_eq!(map.resolve(0).unwrap(), 1);
    assert_eq!(map.resolve(1).unwrap(), 2);
    assert_eq!(map.resolve(2).unwrap(), 3);
    assert_eq!(map.resolve(3).unwrap(), 3);
}
#[test]
fn empty_source_accepts_no_slot_at_all() {
    let mut map = SlotReservationMap::new(0);
    assert!(!map.is_dense());
    assert_eq!(map.to_string(), "{}");
    assert!(map.reserve(0, 0).is_err());
    assert!(map.release(0).is_err());
}
#[test]
fn to_string_renders_the_reservations_in_both_stages() {
    let empty = SlotReservationMap::new(200);
    let mut sparse = SlotReservationMap::new(200);
    sparse.reserve(3, 7).unwrap();
    let mut dense = SlotReservationMap::new(2);
    dense.reserve(0, 1).unwrap();
    dense.reserve(1, 0).unwrap();
    assert_eq!(empty.to_string(), "{}");
    assert_eq!(sparse.to_string(), "{3=7}");
    assert_eq!(dense.to_string(), "{0=1, 1=0}");
}
