// Adapted pinned Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest, Apache-2.0.
// See compat/provenance.json; independent expectations replace tree-built results.
mod range_support;
use range_support::*;
use timetuck::*;
// Preserve the source tuple mappings: uni interval; bi endpoints; tri/quad ignore
// their trailing values. Native tuple values are captured at insertion.
fn capture<const N: usize>(id: i64, a: i64, b: i64) -> Row {
    assert!((1..=4).contains(&N));
    let tuple = (a, b, [(); N]);
    row(id, tuple.0, tuple.1)
}
fn usage<const N: usize>() {
    let mut c = C::default();
    let a = capture::<N>(0, 1, 3);
    let b = capture::<N>(1, 2, 4);
    let d = capture::<N>(2, 2, 4);
    c.insert(&a);
    verify(&c, &[a]);
    c.insert(&b);
    verify(&c, &[a, b]);
    c.insert(&d);
    verify(&c, &[a, b, d]);
    assert!(c.remove(&b));
    verify(&c, &[a, d]);
    assert!(c.remove(&d));
    verify(&c, &[a]);
    assert!(c.remove(&a));
    verify(&c, &[]);
}
fn dynamic<const N: usize>() {
    let mut c = C::default();
    let first = capture::<N>(0, 0, 10);
    let second = capture::<N>(1, 10, 20);
    let third = capture::<N>(2, 20, 30);
    c.insert(&first);
    verify(&c, &[first]);
    c.insert(&third);
    verify(&c, &[first, third]);
    c.insert(&second);
    verify(&c, &[first, second, third]);
    let changed = capture::<N>(0, -5, 5);
    assert!(c.remove(&changed));
    c.insert(&changed);
    verify(&c, &[changed, second, third]);
}
fn update<const N: usize>() {
    let mut n = CollectNode::<usize, (), Row, C>::new();
    let a = capture::<N>(0, 1, 3);
    let b = capture::<N>(1, 10, 20);
    let changed = capture::<N>(0, 8, 12);
    let check_node = |n: &CollectNode<usize, (), Row, C>, rows: &[Row]| {
        let c = n
            .groups()
            .next()
            .map_or_else(|| C::default().finish(), |(_, c)| c);
        check(&c, &reference(rows.to_vec()));
    };
    n.apply(TupleChange::put(0, ((), a)));
    n.apply(TupleChange::put(1, ((), b)));
    check_node(&n, &[a, b]);
    for _ in 0..2 {
        n.apply(TupleChange::put(0, ((), changed)));
        check_node(&n, &[changed, b]);
    }
    n.apply(TupleChange::retract(1));
    check_node(&n, &[changed]);
    n.apply(TupleChange::retract(0));
    check_node(&n, &[]);
}
#[test]
fn uni_consecutive_usage() {
    usage::<1>();
}
#[test]
fn bi_consecutive_usage() {
    usage::<2>();
}
#[test]
fn tri_consecutive_usage() {
    usage::<3>();
}
#[test]
fn quad_consecutive_usage() {
    usage::<4>();
}
#[test]
fn uni_consecutive_usage_dynamic() {
    dynamic::<1>();
}
#[test]
fn bi_consecutive_usage_dynamic() {
    dynamic::<2>();
}
#[test]
fn tri_consecutive_usage_dynamic() {
    dynamic::<3>();
}
#[test]
fn quad_consecutive_usage_dynamic() {
    dynamic::<4>();
}
#[test]
fn uni_consecutive_usage_update() {
    update::<1>();
}
#[test]
fn bi_consecutive_usage_update() {
    update::<2>();
}
#[test]
fn tri_consecutive_usage_update() {
    update::<3>();
}
#[test]
fn quad_consecutive_usage_update() {
    update::<4>();
}
