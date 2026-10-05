// Adapted from Timefold Community 2.7.0 AbstractUniConstraintStreamTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
type Row = (&'static str, i32);
type Pair = ((&'static str, &'static str), (Row, Row));
fn check(
    g: &StreamGraph<'_, SimpleScore>,
    terminal: &StreamConstraint<(&'static str, &'static str), (Row, Row)>,
    expected: Vec<Pair>,
) {
    let score = SimpleScore::new(-(expected.len() as i64));
    assert_eq!(g.score(), score);
    assert_eq!(
        g.constraint_scores().collect::<Vec<_>>(),
        vec![("testConstraint", score)]
    );
    assert_eq!(
        g.constraint_matches(terminal)
            .unwrap()
            .iter()
            .map(|(id, (row, score))| {
                assert_eq!(*score, SimpleScore::new(-1));
                (*id, *row)
            })
            .collect::<Vec<_>>(),
        expected
    );
}
#[test]
fn for_each_unique_pair_0() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<&str, Row>();
    let pairs = b.unique_pairs(source.port()).unwrap();
    let terminal = b
        .score("testConstraint", &pairs, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for code in ["B", "A", "C"] {
        tx.apply(&source, TupleChange::put(code, (code, 0)))
            .unwrap();
    }
    tx.commit().unwrap();
    check(
        &g,
        &terminal,
        vec![
            (("A", "B"), (("A", 0), ("B", 0))),
            (("A", "C"), (("A", 0), ("C", 0))),
            (("B", "C"), (("B", 0), ("C", 0))),
        ],
    );
}
#[test]
fn for_each_unique_pair_1_equals() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<&str, Row>();
    let pairs = b.unique_pairs_equal(source.port(), |row| row.1).unwrap();
    let terminal = b
        .score("testConstraint", &pairs, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (code, key) in [("B", 2), ("A", 2), ("C", 10)] {
        tx.apply(&source, TupleChange::put(code, (code, key)))
            .unwrap();
    }
    tx.commit().unwrap();
    check(&g, &terminal, vec![(("A", "B"), (("A", 2), ("B", 2)))]);
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::put("B", ("B", 10))).unwrap();
    tx.commit().unwrap();
    check(&g, &terminal, vec![(("B", "C"), (("B", 10), ("C", 10)))]);
}
