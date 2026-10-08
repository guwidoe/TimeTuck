// Adapted from Timefold Community EmptyValueRangeTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::{EnumeratedValueRange, ValueRange};
fn range() -> EnumeratedValueRange<i32> {
    EnumeratedValueRange::new(vec![])
}
#[test]
fn get_size() {
    assert_eq!(range().len(), 0);
}
#[test]
fn get() {
    assert_eq!(range().get(0), None);
}
#[test]
fn contains() {
    let r = range();
    assert!(!r.contains(&5));
    assert!(!r.contains_assignment(None));
}
#[test]
fn create_original_iterator() {
    assert_eq!(range().iter().count(), 0);
}
#[test]
fn create_random_iterator() {
    assert_eq!(range().random_iter(0).count(), 0);
}
#[test]
fn sort() {
    // The source passes a null comparator: empty sorting must not consult it.
    // Native sorted copies require a closure; a panicking closure detects use.
    let sorted = range()
        .sorted_by(0, |_, _| panic!("empty sorting consulted comparator"))
        .unwrap();
    assert!(sorted.is_empty());
}
