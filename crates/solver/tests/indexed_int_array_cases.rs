// Adapted from pinned Timefold Community DynamicIntArrayTest (Apache-2.0).
// Complete nested/parameterized methods and attribution: compat/provenance.json.
use timetuck::{IndexArrayError, IndexedIntArray};
#[test]
fn default_constructor() {
    let a = IndexedIntArray::new();
    assert_eq!(a.first_index().unwrap_err().to_string(), "Array is empty");
    assert_eq!(a.last_index().unwrap_err().to_string(), "Array is empty");
    assert!(!a.contains_index(0));
    assert_eq!(a.get(0), 0);
}
#[test]
fn constructor_with_max_size() {
    let mut a = IndexedIntArray::with_max_size(100);
    assert_eq!(a.first_index(), Err(IndexArrayError::Empty));
    assert_eq!(a.set(100, 5), Err(IndexArrayError::IndexOutOfBounds));
}
#[test]
fn set_first_element() {
    let mut a = IndexedIntArray::new();
    a.set(10, 42).unwrap();
    assert_eq!(a.get(10), 42);
    assert_eq!(a.first_index(), Ok(10));
    assert_eq!(a.last_index(), Ok(10));
    assert!(a.contains_index(10));
    assert!(!a.contains_index(9));
    assert!(!a.contains_index(11));
}
#[test]
fn set_lower_index() {
    let mut a = IndexedIntArray::new();
    a.set(10, 42).unwrap();
    a.set(5, 24).unwrap();
    assert_eq!(a.get(5), 24);
    assert_eq!(a.get(10), 42);
    assert_eq!(a.first_index(), Ok(5));
    assert_eq!(a.last_index(), Ok(10));
    assert!(a.contains_index(5));
    assert!(a.contains_index(10));
}
#[test]
fn set_higher_index() {
    let mut a = IndexedIntArray::new();
    a.set(5, 24).unwrap();
    a.set(10, 42).unwrap();
    assert_eq!(a.get(5), 24);
    assert_eq!(a.get(10), 42);
    assert_eq!(a.first_index(), Ok(5));
    assert_eq!(a.last_index(), Ok(10));
    assert!(a.contains_index(5));
    assert!(a.contains_index(10));
}
#[test]
fn set_existing_index() {
    let mut a = IndexedIntArray::new();
    a.set(5, 24).unwrap();
    a.set(10, 42).unwrap();
    a.set(7, 99).unwrap();
    a.set(7, 100).unwrap();
    assert_eq!(a.get(7), 100);
    assert_eq!(a.first_index(), Ok(5));
    assert_eq!(a.last_index(), Ok(10));
}
#[test]
fn set_negative_index() {
    // Preserve all three @ValueSource invocations as one upstream method.
    for index in [-1, -5, -100] {
        let mut a = IndexedIntArray::new();
        assert_eq!(a.set(index, 42), Err(IndexArrayError::IndexOutOfBounds));
    }
}
#[test]
fn set_index_greater_than_max_size() {
    let mut a = IndexedIntArray::with_max_size(50);
    assert_eq!(a.set(50, 42), Err(IndexArrayError::IndexOutOfBounds));
    assert_eq!(a.set(100, 42), Err(IndexArrayError::IndexOutOfBounds));
}
#[test]
fn get_from_empty_array() {
    assert_eq!(IndexedIntArray::new().get(5), 0);
}
#[test]
fn get_index_lower_than_first_index() {
    let mut a = IndexedIntArray::new();
    a.set(10, 42).unwrap();
    assert_eq!(a.get(5), 0);
}
#[test]
fn get_index_higher_than_last_index() {
    let mut a = IndexedIntArray::new();
    a.set(10, 42).unwrap();
    assert_eq!(a.get(15), 0);
}
#[test]
fn get_existing_index() {
    let mut a = IndexedIntArray::new();
    a.set(5, 24).unwrap();
    a.set(10, 42).unwrap();
    assert_eq!(a.get(5), 24);
    assert_eq!(a.get(10), 42);
}
#[test]
fn contains_index_empty_array() {
    let a = IndexedIntArray::new();
    assert!(!a.contains_index(0));
    assert!(!a.contains_index(5));
}
#[test]
fn contains_index_within_range() {
    let mut a = IndexedIntArray::new();
    a.set(5, 24).unwrap();
    a.set(10, 42).unwrap();
    assert!(a.contains_index(5));
    assert!(a.contains_index(7));
    assert!(a.contains_index(10));
}
#[test]
fn contains_index_outside_range() {
    let mut a = IndexedIntArray::new();
    a.set(5, 24).unwrap();
    a.set(10, 42).unwrap();
    assert!(!a.contains_index(4));
    assert!(!a.contains_index(11));
}
#[test]
fn test_multiple_operations() {
    let mut a = IndexedIntArray::new();
    a.set(10, 42).unwrap();
    assert_eq!(a.get(10), 42);
    assert_eq!(a.first_index(), Ok(10));
    assert_eq!(a.last_index(), Ok(10));
    a.set(5, 24).unwrap();
    assert_eq!(a.get(5), 24);
    assert_eq!(a.get(10), 42);
    assert_eq!(a.first_index(), Ok(5));
    assert_eq!(a.last_index(), Ok(10));
    a.set(15, 99).unwrap();
    assert_eq!(a.get(5), 24);
    assert_eq!(a.get(10), 42);
    assert_eq!(a.get(15), 99);
    assert_eq!(a.first_index(), Ok(5));
    assert_eq!(a.last_index(), Ok(15));
    a.set(10, 100).unwrap();
    assert_eq!(a.get(10), 100);
    assert_eq!(a.get(7), 0);
    assert_eq!(a.get(12), 0);
    assert!(a.contains_index(5));
    assert!(a.contains_index(7));
    assert!(a.contains_index(15));
    assert!(!a.contains_index(4));
    assert!(!a.contains_index(16));
}
#[test]
fn test_with_sparse_indices() {
    let mut a = IndexedIntArray::new();
    a.set(100, 1).unwrap();
    a.set(1000, 2).unwrap();
    a.set(10, 3).unwrap();
    assert_eq!(a.first_index(), Ok(10));
    assert_eq!(a.last_index(), Ok(1000));
    assert_eq!(a.get(10), 3);
    assert_eq!(a.get(100), 1);
    assert_eq!(a.get(1000), 2);
}
#[test]
fn clear_empty_array() {
    IndexedIntArray::new().clear();
}
#[test]
fn clear_resets_values() {
    let mut a = IndexedIntArray::new();
    a.set(5, 24).unwrap();
    a.set(10, 42).unwrap();
    a.clear();
    assert_eq!(a.get(5), 0);
    assert_eq!(a.get(10), 0);
    assert!(!a.contains_index(5));
    assert!(!a.contains_index(10));
}
#[test]
fn clear_and_set_new_values() {
    let mut a = IndexedIntArray::new();
    a.set(5, 24).unwrap();
    a.set(10, 42).unwrap();
    a.clear();
    a.set(7, 99).unwrap();
    assert_eq!(a.get(7), 99);
    assert_eq!(a.get(5), 0);
    assert_eq!(a.get(10), 0);
    assert_eq!(a.first_index(), Ok(7));
    assert_eq!(a.last_index(), Ok(7));
}

#[test]
fn native_window_matches_independent_sparse_reference_through_clear_and_errors() {
    use std::collections::BTreeMap;
    let mut a = IndexedIntArray::with_max_size(64);
    let mut written = BTreeMap::new();
    let mut state = 173u64;
    for step in 0..600 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        if step % 37 == 0 {
            a.clear();
            written.clear();
        } else {
            let index = ((state >> 32) % 80) as i32 - 8;
            let value = state as i32;
            let result = a.set(index, value);
            if (0..64).contains(&index) {
                result.unwrap();
                written.insert(index, value);
            } else {
                assert_eq!(result, Err(IndexArrayError::IndexOutOfBounds));
            }
        }
        assert_eq!(
            a.first_index(),
            written
                .first_key_value()
                .map(|(k, _)| *k as usize)
                .ok_or(IndexArrayError::Empty)
        );
        assert_eq!(
            a.last_index(),
            written
                .last_key_value()
                .map(|(k, _)| *k as usize)
                .ok_or(IndexArrayError::Empty)
        );
        for index in -8..72 {
            assert_eq!(a.get(index), written.get(&index).copied().unwrap_or(0));
            assert_eq!(
                a.contains_index(index),
                written
                    .first_key_value()
                    .zip(written.last_key_value())
                    .is_some_and(|((lo, _), (hi, _))| index >= *lo && index <= *hi)
            );
        }
    }
    let mut upper = IndexedIntArray::new();
    upper.set(i32::MAX - 1, 7).unwrap();
    assert_eq!(
        upper.set(i32::MAX, 8),
        Err(IndexArrayError::IndexOutOfBounds)
    );
    assert_eq!(upper.get(i32::MAX - 1), 7);
    assert_eq!(
        IndexedIntArray::with_max_size(0).set(0, 1),
        Err(IndexArrayError::IndexOutOfBounds)
    );
}
