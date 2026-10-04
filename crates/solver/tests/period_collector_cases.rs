// Adapted pinned Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest sumPeriod methods.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::*;
fn p(days: i32) -> CalendarPeriod {
    CalendarPeriod::from_days(days)
}
fn sum<const N: usize>(first: [i32; N], second: [i32; N], results: [i32; 5]) {
    let mut c = PeriodSumCollector::default();
    assert_eq!(c.finish(), CalendarPeriod::ZERO);
    let a = p(first.into_iter().sum());
    let b = p(second.into_iter().sum());
    c.insert(&a);
    assert_eq!(c.finish(), p(results[0]));
    c.insert(&b);
    assert_eq!(c.finish(), p(results[1]));
    c.insert(&b);
    assert_eq!(c.finish(), p(results[2]));
    assert!(c.remove(&b));
    assert_eq!(c.finish(), p(results[3]));
    assert!(c.remove(&b));
    assert_eq!(c.finish(), p(results[4]));
    assert!(c.remove(&a));
    assert_eq!(c.finish(), CalendarPeriod::ZERO);
}
fn update<const N: usize>() {
    let map = |days| {
        let mut tuple = [None; N];
        tuple[0] = Some(p(days));
        tuple[0].unwrap()
    };
    let mut n = CollectNode::<usize, (), CalendarPeriod, PeriodSumCollector>::new();
    let read = |n: &CollectNode<usize, (), CalendarPeriod, PeriodSumCollector>| {
        n.groups().next().map_or(CalendarPeriod::ZERO, |(_, p)| p)
    };
    n.apply(TupleChange::put(0, ((), map(1))));
    n.apply(TupleChange::put(1, ((), map(2))));
    assert_eq!(read(&n), p(3));
    n.apply(TupleChange::put(0, ((), map(4))));
    assert_eq!(read(&n), p(6));
    n.apply(TupleChange::put(0, ((), map(4))));
    assert_eq!(read(&n), p(6));
    n.apply(TupleChange::retract(1));
    assert_eq!(read(&n), p(4));
    n.apply(TupleChange::retract(0));
    assert_eq!(read(&n), CalendarPeriod::ZERO);
}
#[test]
fn uni_sum_period() {
    sum([1], [2], [1, 3, 5, 3, 1]);
}
#[test]
fn bi_sum_period() {
    sum([2, 3], [4, 5], [5, 14, 23, 14, 5]);
}
#[test]
fn tri_sum_period() {
    sum([2, 3, 1], [4, 5, 1], [6, 16, 26, 16, 6]);
}
#[test]
fn quad_sum_period() {
    sum([2, 3, 1, 4], [4, 5, 1, 2], [10, 22, 34, 22, 10]);
}
#[test]
fn uni_sum_period_update() {
    update::<1>();
}
#[test]
fn bi_sum_period_update() {
    update::<2>();
}
#[test]
fn tri_sum_period_update() {
    update::<3>();
}
#[test]
fn quad_sum_period_update() {
    update::<4>();
}
