// Adapted from pinned Timefold Community Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::*;
fn d(seconds: i64) -> SignedDuration {
    SignedDuration::from_seconds(seconds)
}
fn sum<const N: usize>(first: [i64; N], second: [i64; N], results: [i64; 5]) {
    let mut c = DurationSumCollector::default();
    assert_eq!(c.finish(), SignedDuration::ZERO);
    let first = d(first.into_iter().sum());
    let second = d(second.into_iter().sum());
    c.insert(&first);
    assert_eq!(c.finish(), d(results[0]));
    c.insert(&second);
    assert_eq!(c.finish(), d(results[1]));
    c.insert(&second);
    assert_eq!(c.finish(), d(results[2]));
    assert!(c.remove(&second));
    assert_eq!(c.finish(), d(results[3]));
    assert!(c.remove(&second));
    assert_eq!(c.finish(), d(results[4]));
    assert!(c.remove(&first));
    assert_eq!(c.finish(), SignedDuration::ZERO);
}
fn sum_update<const N: usize>() {
    // Higher arity mappings select the first Duration and ignore nullable fields.
    let map = |seconds| {
        let mut tuple = [None; N];
        tuple[0] = Some(d(seconds));
        tuple[0].unwrap()
    };
    let mut c = CollectNode::<usize, (), SignedDuration, DurationSumCollector>::new();
    let read = |c: &CollectNode<usize, (), SignedDuration, DurationSumCollector>| {
        c.groups().next().map_or(SignedDuration::ZERO, |(_, v)| v)
    };
    c.apply(TupleChange::put(0, ((), map(1))));
    c.apply(TupleChange::put(1, ((), map(2))));
    assert_eq!(read(&c), d(3));
    c.apply(TupleChange::put(0, ((), map(4))));
    assert_eq!(read(&c), d(6));
    c.apply(TupleChange::put(0, ((), map(4))));
    assert_eq!(read(&c), d(6));
    c.apply(TupleChange::retract(1));
    assert_eq!(read(&c), d(4));
    c.apply(TupleChange::retract(0));
    assert_eq!(read(&c), SignedDuration::ZERO);
}
fn average<const N: usize>() {
    let map = |seconds| {
        let mut tuple = [0; N];
        tuple[0] = seconds;
        d(tuple.into_iter().sum())
    };
    let mut c = DurationAverageCollector::default();
    let a = map(4);
    let b = map(1);
    let half = SignedDuration::from_nanos(2_500_000_000).unwrap();
    assert_eq!(c.finish(), None);
    c.insert(&a);
    assert_eq!(c.finish(), Some(d(4)));
    c.insert(&b);
    assert_eq!(c.finish(), Some(half));
    c.insert(&b);
    assert_eq!(c.finish(), Some(d(2)));
    assert!(c.remove(&b));
    assert_eq!(c.finish(), Some(half));
    assert!(c.remove(&b));
    assert_eq!(c.finish(), Some(d(4)));
    assert!(c.remove(&a));
    assert_eq!(c.finish(), None);
}
fn average_update<const N: usize>() {
    let map = |seconds| {
        let mut tuple = [0; N];
        tuple[0] = seconds;
        d(tuple.into_iter().sum())
    };
    let mut c = CollectNode::<usize, (), SignedDuration, DurationAverageCollector>::new();
    let read = |c: &CollectNode<usize, (), SignedDuration, DurationAverageCollector>| {
        c.groups().next().and_then(|(_, v)| v)
    };
    c.apply(TupleChange::put(0, ((), map(4))));
    c.apply(TupleChange::put(1, ((), map(2))));
    assert_eq!(read(&c), Some(d(3)));
    c.apply(TupleChange::put(0, ((), map(6))));
    assert_eq!(read(&c), Some(d(4)));
    c.apply(TupleChange::put(0, ((), map(6))));
    assert_eq!(read(&c), Some(d(4)));
    c.apply(TupleChange::retract(1));
    assert_eq!(read(&c), Some(d(6)));
    c.apply(TupleChange::retract(0));
    assert_eq!(read(&c), None);
}

#[test]
fn uni_sum_duration() {
    sum([1], [60], [1, 61, 121, 61, 1]);
}

#[test]
fn uni_sum_duration_update() {
    sum_update::<1>();
}

#[test]
fn uni_average_duration() {
    average::<1>();
}

#[test]
fn uni_average_duration_update() {
    average_update::<1>();
}

#[test]
fn bi_sum_duration() {
    sum([2, 3], [4, 5], [5, 14, 23, 14, 5]);
}

#[test]
fn bi_sum_duration_update() {
    sum_update::<2>();
}

#[test]
fn bi_average_duration() {
    average::<2>();
}

#[test]
fn bi_average_duration_update() {
    average_update::<2>();
}

#[test]
fn tri_sum_duration() {
    sum([2, 3, 1], [4, 5, 1], [6, 16, 26, 16, 6]);
}

#[test]
fn tri_sum_duration_update() {
    sum_update::<3>();
}

#[test]
fn tri_average_duration() {
    average::<3>();
}

#[test]
fn tri_average_duration_update() {
    average_update::<3>();
}

#[test]
fn quad_sum_duration() {
    sum([2, 3, 1, 4], [4, 5, 1, 2], [10, 22, 34, 22, 10]);
}

#[test]
fn quad_sum_duration_update() {
    sum_update::<4>();
}

#[test]
fn quad_average_duration() {
    average::<4>();
}

#[test]
fn quad_average_duration_update() {
    average_update::<4>();
}
