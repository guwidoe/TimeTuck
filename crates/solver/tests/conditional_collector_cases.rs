// Adapted from pinned Community Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::*;

type Collector = ConditionalCollector<IntCollector>;
fn captured<const N: usize>(first: i64) -> Option<i64> {
    let mut tuple = [0; N];
    tuple[0] = first;
    let selected = if N == 1 { tuple[0] > 1 } else { tuple[0] < 2 };
    selected.then(|| tuple.into_iter().sum())
}
fn conditionally<const N: usize>() {
    let mut c = Collector::default();
    let read = |c: &Collector| {
        if N == 1 {
            c.finish().min
        } else {
            c.finish().max
        }
    };
    assert_eq!(read(&c), None);
    let first = captured::<N>(if N == 1 { 2 } else { 1 });
    let second = captured::<N>(if N == 1 { 1 } else { 2 });
    let expected = if N == 1 { 2 } else { 1 };
    c.insert(&first);
    assert_eq!(read(&c), Some(expected));
    c.insert(&second);
    assert_eq!(read(&c), Some(expected));
    c.insert(&first);
    assert_eq!(read(&c), Some(expected));
    assert!(c.remove(&first));
    assert_eq!(read(&c), Some(expected));
    assert!(c.remove(&second));
    assert_eq!(read(&c), Some(expected));
    assert!(c.remove(&first));
    assert_eq!(read(&c), None);
}
fn conditionally_update<const N: usize>() {
    let mut node = CollectNode::<usize, (), Option<i64>, Collector>::new();
    let read = |node: &CollectNode<usize, (), Option<i64>, Collector>| {
        let summary = node
            .groups()
            .next()
            .map_or_else(|| Collector::default().finish(), |(_, s)| s);
        // The binary update method uses min, while its insertion method uses max.
        if N <= 2 { summary.min } else { summary.max }
    };
    let values = if N == 1 { [2, 1, 3, 4] } else { [1, 2, 0, 3] };
    let expected = if N == 1 {
        [Some(2), None, Some(3), Some(4)]
    } else {
        [Some(1), None, Some(0), None]
    };
    for (value, expected) in values.into_iter().zip(expected) {
        node.apply(TupleChange::put(0, ((), captured::<N>(value))));
        assert_eq!(read(&node), expected);
    }
    node.apply(TupleChange::retract(0));
    assert_eq!(read(&node), None);
}
#[test]
fn uni_conditionally() {
    conditionally::<1>();
}
#[test]
fn bi_conditionally() {
    conditionally::<2>();
}
#[test]
fn tri_conditionally() {
    conditionally::<3>();
}
#[test]
fn quad_conditionally() {
    conditionally::<4>();
}
#[test]
fn uni_conditionally_update() {
    conditionally_update::<1>();
}
#[test]
fn bi_conditionally_update() {
    conditionally_update::<2>();
}
#[test]
fn tri_conditionally_update() {
    conditionally_update::<3>();
}
#[test]
fn quad_conditionally_update() {
    conditionally_update::<4>();
}
