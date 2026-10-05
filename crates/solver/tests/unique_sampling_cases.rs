// Adapted from pinned Community DefaultUniqueRandomIteratorTest and
// DefaultRepeatingRandomIteratorTest (Apache-2.0). See compat/provenance.json.
use std::collections::BTreeSet;
use timetuck::*;
#[test]
fn draws_every_element_exactly_once() {
    let values = ["A", "B", "C", "D", "E"];
    let mut iterator = unique_slice_samples(&values, 0);
    let mut seen = BTreeSet::new();
    for _ in 0..values.len() {
        assert!(iterator.has_next());
        seen.insert(*iterator.next().unwrap().unwrap());
    }
    assert_eq!(seen, BTreeSet::from(values));
    assert!(!iterator.has_next());
    assert_eq!(iterator.next(), None);
}
#[test]
fn for_each_remaining_drains_every_element_exactly_once() {
    let values = ["A", "B", "C", "D", "E"];
    let mut iterator = unique_slice_samples(&values, 0);
    let mut seen = BTreeSet::new();
    iterator.by_ref().for_each(|v| {
        seen.insert(*v.unwrap());
    });
    assert_eq!(seen, BTreeSet::from(values));
    assert!(!iterator.has_next());
    iterator.by_ref().for_each(|v| {
        seen.insert(*v.unwrap());
    });
    assert_eq!(seen, BTreeSet::from(values));
}
#[test]
fn unique_empty_source() {
    let mut iterator = unique_slice_samples::<String>(&[], 0);
    assert!(!iterator.has_next());
    assert_eq!(iterator.next(), None);
}
#[test]
fn repeating_never_ends() {
    let values = ["A", "B", "C"];
    let range = EnumeratedValueRange::new(values.to_vec());
    let mut iterator = range.random_iter(0).peekable();
    for _ in 0..values.len() * 100 {
        assert!(iterator.peek().is_some());
        assert!(values.contains(&iterator.next().unwrap()));
    }
    assert!(iterator.peek().is_some());
}
#[test]
fn single_element_repeats() {
    let range = EnumeratedValueRange::new(vec!["A"]);
    let mut iterator = range.random_iter(0).peekable();
    for _ in 0..10 {
        assert!(iterator.peek().is_some());
        assert_eq!(iterator.next(), Some("A"));
    }
}
