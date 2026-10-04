// Adapted from pinned Community Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
mod sequence_support;
use sequence_support::check;
use timetuck::*;
type C = SequenceCollector<i64, i64, i64>;
fn capture<const N: usize>(value: i64) -> (i64, i64, i64) {
    let mut t = [0; N];
    t[0] = value;
    let v = t.into_iter().sum();
    (v, v, v)
}
fn expected(c: &C, values: &[i64]) {
    let runs = if values.is_empty() {
        vec![]
    } else {
        vec![values.iter().map(|&x| (x, x, x)).collect()]
    };
    check(&c.finish(), &runs, 1);
}
fn add_remove<const N: usize>() {
    let mut c = C::default();
    let first = capture::<N>(2);
    let second = capture::<N>(1);
    c.insert(&first);
    expected(&c, &[2]);
    c.insert(&second);
    expected(&c, &[1, 2]);
    c.insert(&second);
    expected(&c, &[1, 2]);
    assert!(c.remove(&second));
    expected(&c, &[1, 2]);
    assert!(c.remove(&second));
    expected(&c, &[2]);
    assert!(c.remove(&first));
    expected(&c, &[]);
}
fn update<const N: usize>() {
    let mut n = CollectNode::<usize, (), (i64, i64, i64), C>::new();
    let read = |n: &CollectNode<usize, (), (i64, i64, i64), C>| {
        n.groups()
            .next()
            .map_or_else(|| C::default().finish(), |(_, s)| s)
    };
    n.apply(TupleChange::put(0, ((), capture::<N>(1))));
    n.apply(TupleChange::put(1, ((), capture::<N>(3))));
    check(&read(&n), &[vec![(1, 1, 1)], vec![(3, 3, 3)]], 1);
    for _ in 0..2 {
        n.apply(TupleChange::put(0, ((), capture::<N>(2))));
        check(&read(&n), &[vec![(2, 2, 2), (3, 3, 3)]], 1);
    }
    n.apply(TupleChange::retract(1));
    check(&read(&n), &[vec![(2, 2, 2)]], 1);
    n.apply(TupleChange::retract(0));
    check(&read(&n), &[], 1);
}
#[test]
fn uni_to_consecutive_sequences() {
    add_remove::<1>();
}
#[test]
fn bi_to_consecutive_sequences() {
    add_remove::<2>();
}
#[test]
fn tri_to_consecutive_sequences() {
    add_remove::<3>();
}
#[test]
fn quad_to_consecutive_sequences() {
    add_remove::<4>();
}
#[test]
fn uni_to_consecutive_sequences_update() {
    update::<1>();
}
#[test]
fn bi_to_consecutive_sequences_update() {
    update::<2>();
}
#[test]
fn tri_to_consecutive_sequences_update() {
    update::<3>();
}
#[test]
fn quad_to_consecutive_sequences_update() {
    update::<4>();
}
