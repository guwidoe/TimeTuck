// Adapted from pinned Community ScalingSequencedSetTest. Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Arc,
};
use timetuck::*;
fn values<T: Copy>(s: &ScalingSequencedSet<T>) -> Vec<T> {
    s.iter().map(|v| *v).collect()
}
fn hash<T: Hash>(v: &T) -> u64 {
    let mut h = DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
#[test]
fn empty_set_properties() {
    let s = ScalingSequencedSet::new();
    assert!(!s.contains(&"test"));
    assert!(s.is_empty());
}
#[test]
fn add_single_element() {
    let s = ScalingSequencedSet::new();
    assert!(s.insert("test").unwrap());
    assert_eq!(s.len(), 1);
    assert!(s.contains(&"test"));
}
#[test]
fn add_duplicate_element() {
    let s = ScalingSequencedSet::new();
    s.insert("test").unwrap();
    assert!(!s.insert("test").unwrap());
    assert_eq!(s.len(), 1);
    assert_eq!(values(&s), ["test"]);
}
#[test]
fn add_all_with_new_elements() {
    let s = ScalingSequencedSet::new();
    assert!(s.extend(["a", "b", "c"]).unwrap());
    assert_eq!(s.len(), 3);
    assert_eq!(values(&s), ["a", "b", "c"]);
}
#[test]
fn add_all_with_duplicate_elements() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    assert!(s.extend(["b", "c"]).unwrap());
    assert_eq!(s.len(), 3);
    assert_eq!(values(&s), ["a", "b", "c"]);
}
#[test]
fn add_all_with_all_duplicate_elements() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    assert!(!s.extend(["a", "b"]).unwrap());
    assert_eq!(s.len(), 2);
    assert_eq!(values(&s), ["a", "b"]);
}
#[test]
fn remove_single_element() {
    let s = ScalingSequencedSet::new();
    s.insert("a").unwrap();
    assert!(s.remove(&"a").unwrap());
    assert!(s.is_empty());
}
#[test]
fn remove_nonexistent_element() {
    let s = ScalingSequencedSet::new();
    s.insert("a").unwrap();
    assert!(!s.remove(&"b").unwrap());
    assert_eq!(s.len(), 1);
    assert!(s.contains(&"a"));
}
#[test]
fn clear_empties_the_set() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    s.clear().unwrap();
    assert!(s.is_empty());
}
#[test]
fn to_array_returns_correct_array() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    assert_eq!(
        s.snapshot().iter().map(|v| **v).collect::<Vec<_>>(),
        ["a", "b"]
    );
}
#[test]
fn to_array_with_type_returns_correct_array() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    let a: [Arc<&str>; 2] = s.snapshot().try_into().unwrap();
    assert_eq!(a.map(|v| *v), ["a", "b"]);
}
#[test]
fn iterator_returns_all_elements() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    let mut i = s.iter().peekable();
    assert!(i.peek().is_some());
    assert_eq!(*i.next().unwrap(), "a");
    assert!(i.peek().is_some());
    assert_eq!(*i.next().unwrap(), "b");
    assert!(i.peek().is_none());
}
#[test]
fn contains_works() {
    let s = ScalingSequencedSet::new();
    s.insert("a").unwrap();
    assert!(s.contains(&"a"));
    assert!(!s.contains(&"b"));
}
#[test]
fn contains_all_works() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    assert!(s.contains_all([&"a", &"b"]));
    assert!(!s.contains_all([&"a", &"c"]));
}
#[test]
fn to_string_works() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    let text = format!("{s:?}");
    assert!(text.contains('a') && text.contains('b'));
}
#[test]
fn scaling_from_list_to_set() {
    let s = ScalingSequencedSet::new();
    let n = SEQUENCED_SET_ARRAY_LIMIT;
    s.extend(0..n).unwrap();
    assert_eq!(s.len(), n);
    s.insert(n).unwrap();
    assert_eq!(s.len(), n + 1);
    for v in 0..=n {
        assert!(s.contains(&v));
    }
}
#[test]
fn scaling_from_set_to_list() {
    let s = ScalingSequencedSet::new();
    let n = SEQUENCED_SET_ARRAY_LIMIT;
    s.extend(0..=n).unwrap();
    assert_eq!(s.len(), n + 1);
    s.remove(&n).unwrap();
    assert_eq!(s.len(), n);
    s.remove(&0).unwrap();
    assert_eq!(s.len(), n - 1);
    assert!(!s.contains(&0));
    for v in 1..n {
        assert!(s.contains(&v));
    }
}
#[test]
fn add_all_causing_scaling() {
    let s = ScalingSequencedSet::new();
    let n = SEQUENCED_SET_ARRAY_LIMIT;
    s.extend(0..n - 5).unwrap();
    assert!(s.extend([n - 5, n - 4, n - 3, n - 2, n - 1, n]).unwrap());
    assert_eq!(s.len(), n + 1);
    for v in 0..=n {
        assert!(s.contains(&v));
    }
}
#[test]
fn attempt_to_remove_non_existent_element_from_set() {
    let s = ScalingSequencedSet::new();
    let n = SEQUENCED_SET_ARRAY_LIMIT;
    s.extend(0..=n).unwrap();
    assert!(!s.remove(&999).unwrap());
    assert_eq!(s.len(), n + 1);
}
#[test]
fn equals_and_hash_code_follow_the_set_contract() {
    let a = ScalingSequencedSet::new();
    a.extend(["a", "b", "c"]).unwrap();
    let b = ScalingSequencedSet::new();
    b.extend(["c", "a", "b"]).unwrap();
    assert_eq!(a, b);
    assert_eq!(b, a);
    assert_eq!(hash(&a), hash(&b));
}
#[test]
fn equals_and_hash_code_follow_the_set_contract_above_the_threshold() {
    let a = ScalingSequencedSet::new();
    let b = ScalingSequencedSet::new();
    let n = SEQUENCED_SET_ARRAY_LIMIT;
    for i in 0..=n {
        a.insert(i).unwrap();
        b.insert(n - i).unwrap();
    }
    assert_eq!(a, b);
    assert_eq!(b, a);
    assert_eq!(hash(&a), hash(&b));
}
#[test]
fn null_is_a_legal_element() {
    let s = ScalingSequencedSet::new();
    assert!(s.insert(None).unwrap());
    assert!(!s.insert(None).unwrap());
    assert_eq!(s.len(), 1);
    assert!(s.contains(&None));
    assert_eq!(s.first().as_deref(), Some(&None));
    s.insert(Some("a")).unwrap();
    assert_eq!(values(&s), [None, Some("a")]);
    assert_eq!(s.last().as_deref(), Some(&Some("a")));
}
#[test]
fn empty_set_is_distinct_from_a_set_holding_null() {
    let a = ScalingSequencedSet::<Option<&str>>::new();
    let b = ScalingSequencedSet::new();
    b.insert(None).unwrap();
    assert!(a.is_empty());
    assert!(!a.contains(&None));
    assert!(!b.is_empty());
    assert_eq!(b.len(), 1);
    assert!(b.contains(&None));
    assert_ne!(a, b);
}
#[test]
fn tiers_advance_on_add() {
    let s = ScalingSequencedSet::new();
    assert!(s.is_empty());
    s.insert(0).unwrap();
    assert_eq!(values(&s), [0]);
    s.insert(1).unwrap();
    assert_eq!(values(&s), [0, 1]);
    let n = SEQUENCED_SET_ARRAY_LIMIT;
    s.extend(2..n).unwrap();
    assert_eq!(s.len(), n);
    s.insert(n).unwrap();
    assert_eq!(s.len(), n + 1);
    for v in 0..=n {
        assert!(s.contains(&v));
    }
}
#[test]
fn a_hint_above_the_threshold_starts_in_the_set_tier() {
    let n = SEQUENCED_SET_ARRAY_LIMIT + 4;
    let s = ScalingSequencedSet::with_capacity(n);
    assert!(s.is_empty());
    s.insert(0).unwrap();
    assert_eq!(values(&s), [0]);
    assert_eq!(*s.first().unwrap(), 0);
    s.extend(1..n).unwrap();
    assert_eq!(s.len(), n);
    assert_eq!(*s.last().unwrap(), n - 1);
}
#[test]
fn get_first_and_get_last_follow_insertion_order() {
    let s = ScalingSequencedSet::new();
    assert!(s.first().is_none());
    assert!(s.last().is_none());
    s.insert(0).unwrap();
    assert_eq!(*s.first().unwrap(), 0);
    assert_eq!(*s.last().unwrap(), 0);
    s.insert(1).unwrap();
    assert_eq!(*s.first().unwrap(), 0);
    assert_eq!(*s.last().unwrap(), 1);
    s.extend(2..=SEQUENCED_SET_ARRAY_LIMIT).unwrap();
    assert_eq!(*s.first().unwrap(), 0);
    assert_eq!(*s.last().unwrap(), SEQUENCED_SET_ARRAY_LIMIT);
}
#[test]
fn reversed_iterates_in_reverse_order() {
    let s = ScalingSequencedSet::new();
    assert!(s.reversed().is_empty());
    s.insert(0).unwrap();
    assert_eq!(s.reversed().iter().map(|v| *v).collect::<Vec<_>>(), [0]);
    s.extend([1, 2]).unwrap();
    let r = s.reversed();
    assert_eq!(r.iter().map(|v| *v).collect::<Vec<_>>(), [2, 1, 0]);
    assert_eq!(*r.first().unwrap(), 2);
    assert_eq!(*r.last().unwrap(), 0);
}
#[test]
fn reversed_is_a_live_view() {
    let s = ScalingSequencedSet::new();
    s.extend([0, 1]).unwrap();
    let r = s.reversed();
    assert_eq!(r.iter().map(|v| *v).collect::<Vec<_>>(), [1, 0]);
    s.extend(2..=SEQUENCED_SET_ARRAY_LIMIT).unwrap();
    assert_eq!(r.len(), SEQUENCED_SET_ARRAY_LIMIT + 1);
    assert_eq!(*r.first().unwrap(), SEQUENCED_SET_ARRAY_LIMIT);
    assert_eq!(*r.last().unwrap(), 0);
}
#[test]
fn reversed_of_reversed_is_the_original_set() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b"]).unwrap();
    assert!(std::ptr::eq(s.reversed().reversed(), &s));
}
#[test]
fn clear_returns_to_empty() {
    let s = ScalingSequencedSet::new();
    s.extend(["a", "b", "c"]).unwrap();
    s.clear().unwrap();
    assert!(s.is_empty());
    assert!(!s.contains(&"a"));
    assert!(s.first().is_none());
    s.insert("z").unwrap();
    assert_eq!(values(&s), ["z"]);
    assert_eq!(*s.first().unwrap(), "z");
}
