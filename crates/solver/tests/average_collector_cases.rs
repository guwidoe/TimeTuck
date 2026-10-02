// Adapted from Timefold Community 2.7.0 Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
use timetuck::*;
fn average<const N: usize>(inputs: [[i64; N]; 3]) {
    let mut c = AverageCollector::default();
    assert_eq!(c.finish(), None);
    let [first, second, third] = inputs.map(|v| v.into_iter().sum());
    c.insert(&first);
    assert_eq!(c.finish(), Some(4.0));
    c.insert(&second);
    assert_eq!(c.finish(), Some(2.5));
    c.insert(&third);
    assert_eq!(c.finish(), Some(2.0));
    assert!(c.remove(&second));
    assert_eq!(c.finish(), Some(2.5));
    assert!(c.remove(&third));
    assert_eq!(c.finish(), Some(4.0));
    assert!(c.remove(&first));
    assert_eq!(c.finish(), None);
}
fn update<const N: usize>(inputs: [[i64; N]; 3]) {
    let mut c = CollectNode::<usize, (), i64, AverageCollector>::new();
    let [first, second, new] = inputs.map(|v| v.into_iter().sum());
    let read =
        |c: &CollectNode<usize, (), i64, AverageCollector>| c.groups().next().and_then(|(_, v)| v);
    c.apply(TupleChange::put(0, ((), first)));
    c.apply(TupleChange::put(1, ((), second)));
    assert_eq!(read(&c), Some(3.0));
    c.apply(TupleChange::put(0, ((), new)));
    assert_eq!(read(&c), Some(4.0));
    c.apply(TupleChange::put(0, ((), new)));
    assert_eq!(read(&c), Some(4.0));
    c.apply(TupleChange::retract(1));
    assert_eq!(read(&c), Some(6.0));
    c.apply(TupleChange::retract(0));
    assert_eq!(read(&c), None);
}

#[test]
fn uni_average() {
    average([[4], [1], [1]]);
}

#[test]
fn uni_average_update() {
    update([[4], [2], [6]]);
}

#[test]
fn bi_average() {
    average([[4, 0], [1, 0], [1, 0]]);
}

#[test]
fn bi_average_update() {
    update([[4, 0], [2, 0], [6, 0]]);
}

#[test]
fn tri_average() {
    average([[4, 0, 0], [1, 0, 0], [1, 0, 0]]);
}

#[test]
fn tri_average_update() {
    update([[4, 0, 0], [2, 0, 0], [6, 0, 0]]);
}

#[test]
fn quad_average() {
    average([[4, 0, 0, 0], [1, 0, 0, 0], [1, 0, 0, 0]]);
}

#[test]
fn quad_average_update() {
    update([[4, 0, 0, 0], [2, 0, 0, 0], [6, 0, 0, 0]]);
}
