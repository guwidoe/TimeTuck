// Adapted from Timefold Community 2.7.0 Inner{Uni,Bi,Tri,Quad}ConstraintCollectorsTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Explicit occurrence IDs preserve every tested list order and duplicate.
use timetuck::*;
fn insert_remove<const N: usize>(inputs: [[i64; N]; 3], expected: [i64; 2]) {
    let mut c = ListCollector::<usize, i64>::default();
    let read = |c: &ListCollector<usize, i64>| c.finish().iter().copied().collect::<Vec<_>>();
    let [a, b] = expected;
    assert!(read(&c).is_empty());
    let first = (0, inputs[0].into_iter().sum());
    c.insert(&first);
    assert_eq!(read(&c), [a]);
    let second = (1, inputs[1].into_iter().sum());
    c.insert(&second);
    assert_eq!(read(&c), [a, b]);
    let third = (2, inputs[2].into_iter().sum());
    c.insert(&third);
    assert_eq!(read(&c), [a, b, b]);
    assert!(c.remove(&second));
    assert_eq!(read(&c), [a, b]);
    assert!(c.remove(&third));
    assert_eq!(read(&c), [a]);
    assert!(c.remove(&first));
    assert!(read(&c).is_empty());
}
fn update<const N: usize>(inputs: [[i64; N]; 3]) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, [i64; N]>();
    let list = b.collect_list(source.port(), |v| v[0]).unwrap();
    let terminal = b.score("list", &list, |_| SimpleScore::ZERO).unwrap();
    let mut g = b.build();
    let mut apply = |id, value| {
        let mut t = g.transaction();
        t.apply(&source, TupleChange { id, value }).unwrap();
        t.commit().unwrap();
        g.constraint_matches(&terminal)
            .unwrap()
            .get(&())
            .map(|(v, _)| v.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default()
    };
    apply(0, Some(inputs[0]));
    assert_eq!(apply(1, Some(inputs[1])), [1, 2]);
    assert_eq!(apply(0, Some(inputs[2])), [3, 2]);
    assert_eq!(apply(0, Some(inputs[2])), [3, 2]);
    assert_eq!(apply(1, None), [3]);
    assert!(apply(0, None).is_empty());
}
#[test]
fn uni_to_list() {
    insert_remove([[2], [1], [1]], [2, 1]);
}
#[test]
fn bi_to_list() {
    insert_remove([[2, 1], [3, 4], [3, 4]], [3, 7]);
}
#[test]
fn tri_to_list() {
    insert_remove([[2, 1, 3], [3, 4, 2], [3, 4, 2]], [6, 9]);
}
#[test]
fn quad_to_list() {
    insert_remove([[2, 1, 3, 4], [3, 4, 2, 5], [3, 4, 2, 5]], [10, 14]);
}

#[test]
fn uni_to_list_update() {
    update([[1], [2], [3]]);
}

#[test]
fn uni_to_collection_update() {
    update([[1], [2], [3]]);
}

#[test]
fn bi_to_list_update() {
    update([[1, 0], [2, 0], [3, 0]]);
}

#[test]
fn bi_to_collection_update() {
    update([[1, 0], [2, 0], [3, 0]]);
}

#[test]
fn tri_to_list_update() {
    update([[1, 0, 0], [2, 0, 0], [3, 0, 0]]);
}

#[test]
fn tri_to_collection_update() {
    update([[1, 0, 0], [2, 0, 0], [3, 0, 0]]);
}

#[test]
fn quad_to_list_update() {
    update([[1, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0]]);
}

#[test]
fn quad_to_collection_update() {
    update([[1, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0]]);
}
