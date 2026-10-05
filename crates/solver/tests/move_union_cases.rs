// Adapted from Timefold Community UniformRandomUnionMoveIteratorTest.
// Apache-2.0; upstream identities and substitutions in compat/provenance.json.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::{MoveUnion, UnionOrder};
#[test]
fn draws_from_every_iterator_within_one_run() {
    let children = (0..3).map(|i| (std::iter::repeat(i), 1));
    let union = MoveUnion::new(children, UnionOrder::Random { seed: 0 });
    assert_eq!(
        union.take(30).collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1, 2])
    );
}
#[test]
fn exhausts_every_iterator_exactly_once() {
    let children = [(0, 3), (1, 1), (2, 5)].map(|(i, n)| (std::iter::repeat_n(i, n), 1));
    let mut union = MoveUnion::new(children, UnionOrder::Random { seed: 0 });
    let mut counts = BTreeMap::new();
    for i in union.by_ref() {
        *counts.entry(i).or_insert(0) += 1;
    }
    assert_eq!(counts, BTreeMap::from([(0, 3), (1, 1), (2, 5)]));
    assert_eq!(union.next(), None);
}
