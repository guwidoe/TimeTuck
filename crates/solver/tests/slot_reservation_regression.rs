use std::{collections::HashMap, num::NonZeroUsize};
use timetuck::*;
#[test]
fn signed_validation_preserves_sparse_and_dense_mappings_before_mutation() {
    for dense in [false, true] {
        let mut map = SlotReservationMap::try_with_slot_count(200).unwrap();
        if dense {
            for i in 0..17 {
                map.reserve(i, i + 100).unwrap();
            }
        } else {
            map.reserve_signed(5, 100).unwrap();
        }
        let before: Vec<_> = (0..200).map(|i| map.resolve(i).unwrap()).collect();
        let count = map.len();
        let mode = map.is_dense();
        for (slot, index) in [
            (-1, 0),
            (200, 0),
            (0, -1),
            (0, 200),
            (i64::MAX, 0),
            (0, i64::MAX),
        ] {
            assert!(map.reserve_signed(slot, index).is_err());
            assert_eq!((map.len(), map.is_dense()), (count, mode));
            assert_eq!(
                (0..200)
                    .map(|i| map.resolve(i).unwrap())
                    .collect::<Vec<_>>(),
                before
            );
        }
        for slot in [-1, 200, i64::MAX] {
            assert!(map.resolve_signed(slot).is_err());
            assert!(map.release_signed(slot).is_err());
        }
        assert_eq!(map.resolve_signed(5).unwrap(), before[5]);
        assert_eq!(map.release_signed(5).unwrap(), before[5]);
        assert_eq!(map.resolve_signed(5).unwrap(), 5);
    }
    let mut empty = SlotReservationMap::try_with_slot_count(0).unwrap();
    assert!(empty.reserve_signed(0, 0).is_err());
    assert!(empty.release_signed(0).is_err());
    assert!(empty.is_empty());
}
#[test]
fn mutations_return_previous_values_and_keep_exact_counts() {
    for size in [0, 1, 2, 3, 4, 17, 32, 33, 200] {
        let mut map = SlotReservationMap::new(size);
        let mut oracle: HashMap<usize, usize> = HashMap::new();
        let mut random = SampleRandom::new(912);
        let mut was_dense = false;
        for step in 0..3000 {
            assert_eq!(map.slot_count(), size);
            if let Some(bound) = NonZeroUsize::new(size) {
                let slot = random.index(bound);
                let next = random.index(bound);
                let old = oracle.get(&slot).copied().unwrap_or(slot);
                if step % 7 == 0 {
                    assert_eq!(map.release(slot).unwrap(), old);
                    oracle.remove(&slot);
                } else {
                    assert_eq!(map.reserve(slot, next).unwrap(), old);
                    if slot == next {
                        oracle.remove(&slot);
                    } else {
                        oracle.insert(slot, next);
                    }
                }
            }
            if step % 91 == 0 {
                map.clear();
                oracle.clear();
            }
            assert_eq!(map.len(), oracle.len());
            assert_eq!(map.is_empty(), oracle.is_empty());
            assert!(!was_dense || map.is_dense());
            was_dense = map.is_dense();
            for slot in 0..size {
                assert_eq!(
                    map.resolve(slot).unwrap(),
                    *oracle.get(&slot).unwrap_or(&slot)
                );
            }
        }
    }
}
#[test]
fn invalid_indices_and_failed_promotion_preserve_the_map() {
    for dense in [false, true] {
        let mut map = SlotReservationMap::new(200);
        for slot in 0..if dense { 17 } else { 3 } {
            map.reserve(slot, 100 + slot).unwrap();
        }
        let before = map.to_string();
        for slot in [200, usize::MAX] {
            assert!(matches!(
                map.resolve(slot),
                Err(SlotReservationError::SlotOutOfBounds { .. })
            ));
            assert!(matches!(
                map.release(slot),
                Err(SlotReservationError::SlotOutOfBounds { .. })
            ));
            assert!(matches!(
                map.reserve(slot, 0),
                Err(SlotReservationError::SlotOutOfBounds { .. })
            ));
            assert!(matches!(
                map.reserve(0, slot),
                Err(SlotReservationError::LogicalIndexOutOfBounds { .. })
            ));
            assert_eq!(map.to_string(), before);
        }
        assert_eq!(map.is_dense(), dense);
    }
    let mut huge = SlotReservationMap::new(usize::MAX);
    for slot in 0..16 {
        huge.reserve(slot, slot + 100).unwrap();
    }
    let before = huge.to_string();
    assert_eq!(
        huge.reserve(16, 116),
        Err(SlotReservationError::AllocationFailed)
    );
    assert_eq!(huge.to_string(), before);
    assert!(!huge.is_dense());
    assert_eq!(huge.len(), 16);
}
#[test]
fn unique_sampling_matches_swap_remove_across_storage_boundary() {
    for length in [0, 1, 2, 3, 4, 16, 31, 32, 33, 63, 64, 65, 200] {
        for seed in 0..30 {
            let mut random = SampleRandom::new(seed);
            let mut live: Vec<_> = (0..length).collect();
            let mut expected = vec![];
            let mut draws = vec![];
            while !live.is_empty() {
                let index = if live.len() == 1 {
                    0
                } else {
                    random.index(NonZeroUsize::new(live.len()).unwrap())
                };
                if live.len() > 1 {
                    draws.push(index as u128);
                }
                expected.push(live.swap_remove(index));
            }
            let mut draws = draws.into_iter();
            let mut samples = UniqueIndices::with_draw(length, |_| draws.next().unwrap());
            assert_eq!(
                samples.by_ref().collect::<Result<Vec<_>, _>>().unwrap(),
                expected
            );
            assert_eq!(samples.stored_indices(), 0);
            assert!(draws.next().is_none());
        }
    }
}
