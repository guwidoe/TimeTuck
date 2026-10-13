// Adapted pinned Community utility TESTs, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn int_arithmetic() {
    let mut n = SignedCounter32::new(1);
    assert_eq!(n.increment(), 2);
    assert_eq!(n.as_i32(), 2);
    assert_eq!(n.decrement(), 1);
    assert_eq!(n.as_i32(), 1);
}
#[test]
fn long_arithmetic() {
    let mut n = SignedCounter64::new(1);
    assert_eq!(n.increment(), 2);
    assert_eq!(n.as_i64(), 2);
    assert_eq!(n.decrement(), 1);
    assert_eq!(n.as_i64(), 1);
}
#[test]
fn int_comparison() {
    let a = SignedCounter32::new(1);
    let b = SignedCounter32::new(2);
    assert_eq!(a, a);
    assert_ne!(a, b);
    assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    assert!(a < b);
    assert!(b > a);
}
#[test]
fn long_comparison() {
    let a = SignedCounter64::new(1);
    let b = SignedCounter64::new(2);
    assert_eq!(a, a);
    assert_ne!(a, b);
    assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    assert!(a < b);
    assert!(b > a);
}
#[test]
fn int_values() {
    let n = SignedCounter32::new(i32::MAX);
    assert_eq!(n.as_i32(), i32::MAX);
    assert_eq!(n.as_i64(), i64::from(i32::MAX));
    assert_eq!(n.as_f32(), i32::MAX as f32);
    assert_eq!(n.as_f64(), i32::MAX as f64);
}
#[test]
fn long_values() {
    let n = SignedCounter64::new(i64::MAX);
    assert_eq!(n.as_i32(), -1);
    assert_eq!(n.as_i64(), i64::MAX);
    assert_eq!(n.as_f32(), i64::MAX as f32);
    assert_eq!(n.as_f64(), i64::MAX as f64);
}
#[test]
fn count() {
    let mut c = OccurrenceCounts::new();
    assert_eq!([c.get_count(0), c.get_count(1), c.get_count(2)], [0, 0, 0]);
    assert_eq!(c.capacity(), 0);
    c.increment(1).unwrap();
    c.increment(2).unwrap();
    c.increment(2).unwrap();
    assert_eq!([c.get_count(0), c.get_count(1), c.get_count(2)], [0, 1, 2]);
    assert_eq!(c.capacity(), 2);
    c.decrement(2).unwrap();
    c.decrement(1).unwrap();
    assert_eq!([c.get_count(0), c.get_count(1), c.get_count(2)], [0, 0, 1]);
    assert_eq!(c.capacity(), 2);
    c.increment(0).unwrap();
    assert_eq!([c.get_count(0), c.get_count(1), c.get_count(2)], [1, 0, 1]);
    assert_eq!(c.capacity(), 2);
}
#[test]
fn test_to_string() {
    let mut c = OccurrenceCounts::new();
    assert_eq!(c.to_string(), "{}");
    c.increment(1).unwrap();
    c.increment(2).unwrap();
    c.increment(2).unwrap();
    assert_eq!(c.to_string(), "{1: 1, 2: 2}");
    c.decrement(2).unwrap();
    c.decrement(1).unwrap();
    assert_eq!(c.to_string(), "{2: 1}");
    c.increment(0).unwrap();
    assert_eq!(c.to_string(), "{0: 1, 2: 1}");
}
