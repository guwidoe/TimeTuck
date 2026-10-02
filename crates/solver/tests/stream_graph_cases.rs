// Adapted from Timefold Community 2.7.0 AbstractUniConstraintStreamTest.
// Apache-2.0. Native typed graph handles replace Java descriptors/notifications.
// Source methods and fixture/helper changes are recorded in compat/provenance.json.
use timetuck::{SimpleScore, StreamGraphBuilder, TupleChange as Change};

#[test]
fn join_1_mirrored() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, (usize, usize)>();
    let values = b.source::<usize, usize>();
    let av = b
        .join_equal(
            entities.port(),
            values.port(),
            |entity| entity.1,
            |value| *value,
        )
        .unwrap();
    let va = b
        .join_equal(
            values.port(),
            entities.port(),
            |value| *value,
            |entity| entity.1,
        )
        .unwrap();
    let first = b
        .score("testConstraint1", &av, |_| SimpleScore::new(-1))
        .unwrap();
    let second = b
        .score("testConstraint2", &va, |_| SimpleScore::new(-1))
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for id in 0..2 {
        tx.apply(&values, Change::put(id, id)).unwrap();
        tx.apply(&entities, Change::put(id, (id, id))).unwrap();
    }
    tx.commit().unwrap();
    let assert_matches = |graph: &timetuck::StreamGraph<'_, SimpleScore>, value_for_second| {
        assert_eq!(graph.score(), SimpleScore::new(-4));
        assert_eq!(
            graph.constraint_scores().collect::<Vec<_>>(),
            [
                ("testConstraint1", SimpleScore::new(-2)),
                ("testConstraint2", SimpleScore::new(-2))
            ]
        );
        let mut a = graph
            .constraint_matches(&first)
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        a.sort();
        assert_eq!(
            a,
            vec![
                (((0, 0), 0), SimpleScore::new(-1)),
                (
                    ((1, value_for_second), value_for_second),
                    SimpleScore::new(-1)
                )
            ]
        );
        let mut actual = graph
            .constraint_matches(&second)
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = vec![
            ((0, (0, 0)), SimpleScore::new(-1)),
            (
                (value_for_second, (1, value_for_second)),
                SimpleScore::new(-1),
            ),
        ];
        expected.sort();
        assert_eq!(actual, expected);
    };
    assert_matches(&graph, 1);
    let mut tx = graph.transaction();
    tx.apply(&entities, Change::put(1, (1, 0))).unwrap();
    tx.commit().unwrap();
    assert_matches(&graph, 0);
}

#[test]
fn join_2_equal() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, (usize, usize, i32)>();
    let joined = b
        .join_equal(
            entities.port(),
            entities.port(),
            |entity| (entity.1, entity.2),
            |entity| (entity.1, entity.2),
        )
        .unwrap();
    let constraint = b
        .score("testConstraint", &joined, |_| SimpleScore::new(-1))
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    // One generated entity, followed by the three explicitly added entities.
    for row in [(0, 0, 0), (1, 1, 7), (2, 1, 7), (3, 1, 8)] {
        tx.apply(&entities, Change::put(row.0, row)).unwrap();
    }
    tx.commit().unwrap();
    let assert_matches = |graph: &timetuck::StreamGraph<'_, SimpleScore>,
                          mut expected: Vec<(usize, usize)>| {
        expected.sort();
        assert_eq!(graph.score(), SimpleScore::new(-6));
        assert_eq!(
            graph.constraint_scores().collect::<Vec<_>>(),
            [("testConstraint", SimpleScore::new(-6))]
        );
        let matches = graph.constraint_matches(&constraint).unwrap();
        assert_eq!(matches.keys().copied().collect::<Vec<_>>(), expected);
        assert_eq!(
            matches
                .values()
                .map(|((a, b), s)| ((a.0, b.0), *s))
                .collect::<Vec<_>>(),
            expected
                .into_iter()
                .map(|ids| (ids, SimpleScore::new(-1)))
                .collect::<Vec<_>>()
        );
    };
    assert_matches(&graph, vec![(0, 0), (1, 1), (1, 2), (2, 1), (2, 2), (3, 3)]);
    let mut tx = graph.transaction();
    tx.apply(&entities, Change::put(1, (1, 1, 8))).unwrap();
    tx.commit().unwrap();
    assert_matches(&graph, vec![(0, 0), (1, 1), (1, 3), (2, 2), (3, 1), (3, 3)]);
}
