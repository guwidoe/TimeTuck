// Adapted from Timefold Community SetValueRangeTest at the pinned revision.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{fmt::Debug, hash::Hash};
use timetuck::*;

fn all<V: Clone + Eq + Hash + Debug>(range: SetValueRange<V>, expected: &[V]) {
    let mut iter = range.iter();
    for value in expected {
        assert_eq!(iter.next().as_ref(), Some(value));
    }
    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
}
fn draws<V: Clone + Eq + Hash + Debug>(range: SetValueRange<V>, expected: &[V]) {
    let mut ranks = [2, 0].into_iter();
    let mut iter = range.random_iter_with(|bound| {
        let rank = ranks.next().unwrap();
        assert!(rank < bound);
        rank
    });
    for value in expected {
        assert_eq!(iter.next().as_ref(), Some(value));
    }
}
#[test]
fn get_size() {
    assert_eq!(SetValueRange::new([0, 2, 5, 10]).len(), 4);
    assert_eq!(SetValueRange::new([100, 120, 5, 7, 8]).len(), 5);
    assert_eq!(SetValueRange::new([-15, 25, 0]).len(), 3);
    assert_eq!(SetValueRange::new(["b", "z", "a"]).len(), 3);
    assert_eq!(SetValueRange::<i32>::new([]).len(), 0);
}
#[test]
fn get() {
    assert_eq!(SetValueRange::new([0, 2, 5, 10]).get(2), Some(5));
    assert_eq!(SetValueRange::new([100, -120]).get(1), Some(-120));
    assert_eq!(
        SetValueRange::new(["b", "z", "a", "c", "g", "d"]).get(3),
        Some("c")
    );
}
#[test]
fn contains() {
    let r = SetValueRange::new([0, 2, 5, 10]);
    assert!(r.contains(&5));
    assert!(!r.contains(&4));
    assert!(!r.contains_assignment(None));
    let r = SetValueRange::new([100, 120, 5, 7, 8]);
    assert!(r.contains(&7));
    assert!(!r.contains(&9));
    let r = SetValueRange::new([-15, 25, 0]);
    assert!(r.contains(&-15));
    assert!(!r.contains(&-14));
    let r = SetValueRange::new(["b", "z", "a"]);
    assert!(r.contains(&"a"));
    assert!(!r.contains(&"n"));
}
#[test]
fn original_iterator() {
    all(SetValueRange::new([0, 2, 5, 10]), &[0, 2, 5, 10]);
    all(
        SetValueRange::new([100, 120, 5, 7, 8]),
        &[100, 120, 5, 7, 8],
    );
    all(SetValueRange::new([-15, 25, 0]), &[-15, 25, 0]);
    all(SetValueRange::new(["b", "z", "a"]), &["b", "z", "a"]);
    all(SetValueRange::<i32>::new([]), &[]);
}
#[test]
fn random_iterator() {
    draws(SetValueRange::new([0, 2, 5, 10]), &[5, 0]);
    draws(SetValueRange::new([100, 120, 5, 7, 8]), &[5, 100]);
    draws(SetValueRange::new([-15, 25, 0]), &[0, -15]);
    draws(SetValueRange::new(["b", "z", "a"]), &["a", "b"]);
    let empty = SetValueRange::<i32>::new([]);
    let mut random = empty.random_iter(0);
    assert_eq!(random.next(), None);
    assert_eq!(random.next(), None);
}
#[test]
fn sort() {
    let r = SetValueRange::new([-15, 25, 0, 1, -1]);
    all(r.reordered_by(i32::cmp), &[-15, -1, 0, 1, 25]);
    // Native comparator factory captures solution facts instead of Java scopes.
    fn comparator_factory(_: ()) -> impl FnMut(&i32, &i32) -> std::cmp::Ordering {
        i32::cmp
    }
    all(r.reordered_by(comparator_factory(())), &[-15, -1, 0, 1, 25]);
    all(r.reordered_by(|a, b| b.cmp(a)), &[25, 1, 0, -1, -15]);
    let mut compare = comparator_factory(());
    all(r.reordered_by(|a, b| compare(b, a)), &[25, 1, 0, -1, -15]);
}
