// Adapted from Timefold Community 2.7.0 AbstractBiConstraintStreamTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::fmt::Debug;
use timetuck::*;
fn check<I: Ord + Clone + Debug + 'static, T: Clone + Debug + PartialEq + 'static>(
    g: &StreamGraph<'_, SimpleScore>,
    t: &StreamConstraint<I, T>,
    expected: Vec<(I, T)>,
) {
    let score = SimpleScore::new(-(expected.len() as i64));
    assert_eq!(g.score(), score);
    assert_eq!(
        g.constraint_scores().collect::<Vec<_>>(),
        vec![("testConstraint", score)]
    );
    assert_eq!(
        g.constraint_matches(t)
            .unwrap()
            .iter()
            .map(|(id, (row, score))| (id.clone(), row.clone(), *score))
            .collect::<Vec<_>>(),
        expected
            .into_iter()
            .map(|(id, row)| (id, row, SimpleScore::new(-1)))
            .collect::<Vec<_>>()
    );
}
fn no_joiner(mode: ExistenceMode) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let values = b.source::<usize, (usize, &str)>();
    let groups = b.source::<usize, &str>();
    let pair = b
        .join_compare(
            values.port(),
            values.port(),
            JoinComparison::LessThan,
            |r| r.0,
            |r| r.0,
        )
        .unwrap();
    let exists = b
        .existence_equal(&pair, groups.port(), mode, |_| (), |_| ())
        .unwrap();
    let terminal = b
        .score("testConstraint", &exists, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    tx.apply(&values, TupleChange::put(0, (0, "Generated ValueGroup 0")))
        .unwrap();
    tx.apply(&values, TupleChange::put(1, (1, "MyValueGroup")))
        .unwrap();
    tx.apply(&groups, TupleChange::put(0, "Generated EntityGroup 0"))
        .unwrap();
    tx.commit().unwrap();
    let row = vec![((0, 1), ((0, "Generated ValueGroup 0"), (1, "MyValueGroup")))];
    check(
        &g,
        &terminal,
        if mode == ExistenceMode::Exists {
            row.clone()
        } else {
            vec![]
        },
    );
    let mut tx = g.transaction();
    tx.apply(&groups, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    check(
        &g,
        &terminal,
        if mode == ExistenceMode::NotExists {
            row
        } else {
            vec![]
        },
    );
}
#[derive(Clone, Debug, PartialEq)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
    code: &'static str,
}
fn one_joiner(mode: ExistenceMode) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, Entity>();
    let groups = b.source::<usize, (usize, &str)>();
    let pairs = b
        .join_compare(
            entities.port(),
            entities.port(),
            JoinComparison::LessThan,
            |e| e.id,
            |e| e.id,
        )
        .unwrap();
    let exists = b
        .existence_equal(&pairs, groups.port(), mode, |(a, _)| a.group, |g| g.0)
        .unwrap();
    let terminal = b
        .score("testConstraint", &exists, |_| SimpleScore::new(-1))
        .unwrap();
    let rows = [
        Entity {
            id: 0,
            group: 0,
            value: 0,
            code: "Generated Entity 0",
        },
        Entity {
            id: 1,
            group: 1,
            value: 0,
            code: "MyEntity 1",
        },
        Entity {
            id: 2,
            group: 0,
            value: 0,
            code: "MyEntity 2",
        },
    ];
    let mut g = b.build();
    let mut tx = g.transaction();
    for e in &rows {
        tx.apply(&entities, TupleChange::put(e.id, e.clone()))
            .unwrap();
    }
    for row in [(0, "Generated EntityGroup 0"), (1, "MyEntityGroup")] {
        tx.apply(&groups, TupleChange::put(row.0, row)).unwrap();
    }
    tx.commit().unwrap();
    let expected = |pairs: &[(usize, usize)]| {
        pairs
            .iter()
            .map(|&(i, j)| ((i, j), (rows[i].clone(), rows[j].clone())))
            .collect::<Vec<_>>()
    };
    check(
        &g,
        &terminal,
        expected(if mode == ExistenceMode::Exists {
            &[(0, 1), (0, 2), (1, 2)]
        } else {
            &[]
        }),
    );
    let mut tx = g.transaction();
    tx.apply(&groups, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    check(
        &g,
        &terminal,
        expected(if mode == ExistenceMode::Exists {
            &[(0, 1), (0, 2)]
        } else {
            &[(1, 2)]
        }),
    );
}
#[test]
fn if_exists_0_joiner_0_filter() {
    no_joiner(ExistenceMode::Exists);
}
#[test]
fn if_not_exists_0_joiner_0_filter() {
    no_joiner(ExistenceMode::NotExists);
}
#[test]
fn if_exists_1_join_0_filter() {
    one_joiner(ExistenceMode::Exists);
}
#[test]
fn if_not_exists_1_join_0_filter() {
    one_joiner(ExistenceMode::NotExists);
}
