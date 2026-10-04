// Adapted pinned Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest, Apache-2.0.
// See compat/provenance.json; captured sort keys replace Java comparable functions.
use timetuck::*;
#[derive(Clone, Debug, PartialEq, Eq)]
struct Text(&'static str); // Deliberately not Ord: only the captured key is ordered.
type Row = (usize, &'static str, Text);
fn capture<const N: usize>(id: usize, value: &'static str) -> Row {
    let mut tuple = [None; N];
    tuple[0] = Some(value);
    let key = tuple[0].unwrap();
    (id, key, Text(key))
}
fn usage<const N: usize, const MAX: bool>() {
    let mut c = ExtremumCollector::<usize, &str, Text, MAX>::default();
    let result = |c: &ExtremumCollector<usize, &str, Text, MAX>| c.finish().map(|r| r.value.0);
    let a = capture::<N>(0, "b");
    let b = capture::<N>(1, "a");
    assert_eq!(result(&c), None);
    c.insert(&a);
    assert_eq!(result(&c), Some("b"));
    c.insert(&b);
    assert_eq!(result(&c), Some(if MAX { "b" } else { "a" }));
    let dup = if MAX { &a } else { &b };
    c.insert(dup);
    assert_eq!(result(&c), Some(if MAX { "b" } else { "a" }));
    assert!(c.remove(dup));
    assert_eq!(result(&c), Some(if MAX { "b" } else { "a" }));
    assert!(c.remove(dup));
    assert_eq!(result(&c), Some(if MAX { "a" } else { "b" }));
    assert!(c.remove(if MAX { &b } else { &a }));
    assert_eq!(result(&c), None);
}
fn update<const N: usize, const MAX: bool>() {
    let mut n = CollectNode::<usize, (), Row, ExtremumCollector<usize, &str, Text, MAX>>::new();
    let result = |n: &CollectNode<usize, (), Row, ExtremumCollector<usize, &str, Text, MAX>>| {
        n.groups().next().and_then(|(_, v)| v.map(|r| r.value.0))
    };
    n.apply(TupleChange::put(0, ((), capture::<N>(0, "b"))));
    n.apply(TupleChange::put(1, ((), capture::<N>(1, "a"))));
    assert_eq!(result(&n), Some(if MAX { "b" } else { "a" }));
    for _ in 0..2 {
        n.apply(TupleChange::put(1, ((), capture::<N>(1, "c"))));
        assert_eq!(result(&n), Some(if MAX { "c" } else { "b" }));
    }
    n.apply(TupleChange::put(0, ((), capture::<N>(0, "a"))));
    assert_eq!(result(&n), Some(if MAX { "c" } else { "a" }));
    n.apply(TupleChange::retract(1));
    assert_eq!(result(&n), Some("a"));
    n.apply(TupleChange::retract(0));
    assert_eq!(result(&n), None);
}
#[test]
fn uni_min_not_comparable() {
    usage::<1, false>();
}
#[test]
fn uni_min_not_comparable_update() {
    update::<1, false>();
}
#[test]
fn uni_max_not_comparable() {
    usage::<1, true>();
}
#[test]
fn uni_max_not_comparable_update() {
    update::<1, true>();
}
#[test]
fn bi_min_not_comparable() {
    usage::<2, false>();
}
#[test]
fn bi_min_not_comparable_update() {
    update::<2, false>();
}
#[test]
fn bi_max_not_comparable() {
    usage::<2, true>();
}
#[test]
fn bi_max_not_comparable_update() {
    update::<2, true>();
}
#[test]
fn tri_min_not_comparable() {
    usage::<3, false>();
}
#[test]
fn tri_min_not_comparable_update() {
    update::<3, false>();
}
#[test]
fn tri_max_not_comparable() {
    usage::<3, true>();
}
#[test]
fn tri_max_not_comparable_update() {
    update::<3, true>();
}
#[test]
fn quad_min_not_comparable() {
    usage::<4, false>();
}
#[test]
fn quad_min_not_comparable_update() {
    update::<4, false>();
}
#[test]
fn quad_max_not_comparable() {
    usage::<4, true>();
}
#[test]
fn quad_max_not_comparable_update() {
    update::<4, true>();
}
