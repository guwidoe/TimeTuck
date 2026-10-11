// Adapted from Community LinkedIdentityHashSetTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::{IdentityOrderedSet, IdentitySnapshot};

fn same<T>(actual: &[Arc<T>], expected: &[&Arc<T>]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert!(Arc::ptr_eq(a, b));
    }
}
fn buffer_same<T>(actual: &[Option<Arc<T>>], expected: &[&Arc<T>]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert!(Arc::ptr_eq(a.as_ref().unwrap(), b));
    }
}
#[test]
fn size() {
    let mut set = IdentityOrderedSet::new();
    assert!(set.is_empty());
    set.insert(Arc::new(1));
    set.insert(Arc::new(2));
    assert_eq!(set.len(), 2);
}
#[test]
fn contains() {
    let mut set = IdentityOrderedSet::new();
    let (a, b) = (Arc::new(()), Arc::new(()));
    assert!(!set.contains(&a));
    assert!(!set.contains(&b));
    assert!(!set.iter().any(|v| Arc::ptr_eq(v, &b)));
    set.insert(a.clone());
    assert!(set.contains(&a));
    assert!(!set.contains(&b));
}
#[test]
fn iterator() {
    let (a, b, c) = (Arc::new(()), Arc::new(()), Arc::new(()));
    let set: IdentityOrderedSet<_> = [a.clone(), b.clone(), c.clone()].into_iter().collect();
    let mut iter = set.iter();
    assert!(Arc::ptr_eq(iter.next().unwrap(), &a));
    assert!(Arc::ptr_eq(iter.next().unwrap(), &b));
    assert!(Arc::ptr_eq(iter.next().unwrap(), &c));
}
#[test]
fn to_array() {
    let (a, b, c) = (Arc::new(()), Arc::new(()), Arc::new(()));
    let set: IdentityOrderedSet<_> = [a.clone(), b.clone(), c.clone()].into_iter().collect();
    same(&set.snapshot(), &[&a, &b, &c]);
    let mut exact = [None, None, None];
    assert!(matches!(
        set.snapshot_into(&mut exact),
        IdentitySnapshot::Borrowed(_)
    ));
    buffer_same(&exact, &[&a, &b, &c]);
    let mut short = [None, None];
    let result = set.snapshot_into(&mut short);
    assert!(matches!(&result, IdentitySnapshot::Owned(_)));
    buffer_same(result.as_slice(), &[&a, &b, &c]);
    assert!(short.iter().all(Option::is_none));
    let mut long = [None, None, None, None];
    assert!(matches!(
        set.snapshot_into(&mut long),
        IdentitySnapshot::Borrowed(_)
    ));
    buffer_same(&long[..3], &[&a, &b, &c]);
    assert!(long[3].is_none());
}
#[test]
fn add_and_remove() {
    let mut set = IdentityOrderedSet::new();
    let (a, b) = (Arc::new(()), Arc::new(()));
    assert!(set.insert(a.clone()));
    assert!(set.insert(b.clone()));
    assert!(!set.insert(b.clone()));
    same(&set.snapshot(), &[&a, &b]);
    assert!(set.remove(&a));
    same(&set.snapshot(), &[&b]);
}
#[test]
fn contains_all() {
    let (a, b, c) = (Arc::new(()), Arc::new(()), Arc::new(()));
    let set: IdentityOrderedSet<_> = [a.clone(), b.clone()].into_iter().collect();
    assert!(set.contains_all([&a, &b]));
    assert!(!set.contains_all([&a, &c]));
}
#[test]
fn add_all() {
    let (a, b, c) = (Arc::new(()), Arc::new(()), Arc::new(()));
    let mut set: IdentityOrderedSet<_> = [a.clone()].into_iter().collect();
    assert!(set.extend([a.clone(), b.clone(), c.clone()]));
    assert!(!set.extend([a.clone(), b.clone(), c.clone()]));
    same(&set.snapshot(), &[&a, &b, &c]);
}
#[test]
fn remove_all() {
    let (a, b, c) = (Arc::new(()), Arc::new(()), Arc::new(()));
    let mut set: IdentityOrderedSet<_> = [a.clone(), b.clone(), c.clone()].into_iter().collect();
    assert!(set.remove_all([&b, &a]));
    assert!(!set.remove_all([&b, &a]));
    same(&set.snapshot(), &[&c]);
}
#[test]
fn clear() {
    let mut set: IdentityOrderedSet<_> = [Arc::new(()), Arc::new(()), Arc::new(())]
        .into_iter()
        .collect();
    set.clear();
    assert!(set.is_empty());
}
