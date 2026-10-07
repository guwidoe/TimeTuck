// Adapted from pinned AbstractBiConstraintStreamTest and lavish fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
fn run(indexed: bool, absent: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, (usize, usize)>();
    let groups = b.source::<usize, usize>();
    let pair = b
        .join_equal(entities.port(), entities.port(), |_| (), |_| ())
        .unwrap();
    let pair = b.filter(&pair, |(a, b)| a.0 < b.0).unwrap();
    let out = b
        .existence(
            &pair,
            groups.port(),
            if absent {
                ExistenceMode::NotExists
            } else {
                ExistenceMode::Exists
            },
            move |(a, _)| if indexed { a.1 } else { 0 },
            move |group| if indexed { *group } else { 0 },
            move |(a, b), group| {
                if indexed {
                    a.0 > 0 || *group == 1
                } else {
                    a.1 == *group && b.1 == *group
                }
            },
        )
        .unwrap();
    let c = b
        .score("testConstraint", &out, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (id, group) in [(0, 0), (1, 1), (2, 0)] {
        tx.apply(&entities, TupleChange::put(id, (id, group)))
            .unwrap();
    }
    for id in [0, 1] {
        tx.apply(&groups, TupleChange::put(id, id)).unwrap();
    }
    tx.commit().unwrap();
    let check = |g: &StreamGraph<'_, SimpleScore>, expected: &[(usize, usize)]| {
        let mut actual: Vec<_> = g
            .constraint_matches(&c)
            .unwrap()
            .values()
            .map(|((a, b), s)| {
                assert_eq!(*s, SimpleScore::new(-1));
                (a.0, b.0)
            })
            .collect();
        actual.sort();
        assert_eq!(actual, expected);
        assert_eq!(g.score(), SimpleScore::new(-(expected.len() as i64)));
    };
    let expected: &[(usize, usize)] = match (indexed, absent) {
        (false, false) => &[(0, 2)],
        (false, true) => &[(0, 1), (1, 2)],
        (true, false) => &[(1, 2)],
        (true, true) => &[(0, 1), (0, 2)],
    };
    check(&g, expected);
    let mut tx = g.transaction();
    tx.apply(&groups, TupleChange::retract(usize::from(indexed)))
        .unwrap();
    tx.commit().unwrap();
    check(
        &g,
        if absent {
            &[(0, 1), (0, 2), (1, 2)]
        } else {
            &[]
        },
    );
}
#[test]
fn if_exists_0_join_1_filter() {
    run(false, false)
}
#[test]
fn if_exists_1_join_1_filter() {
    run(true, false)
}
#[test]
fn if_not_exists_0_join_1_filter() {
    run(false, true)
}
#[test]
fn if_not_exists_1_join_1_filter() {
    run(true, true)
}
fn excludes_unassigned(absent: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, (usize, Option<usize>)>();
    let assigned = b.filter(source.port(), |e| e.1.is_some()).unwrap();
    let pair = b.join_equal(&assigned, &assigned, |_| (), |_| ()).unwrap();
    let pair = b.filter(&pair, |(a, b)| a.0 < b.0).unwrap();
    let out = b
        .existence(
            &pair,
            &assigned,
            if absent {
                ExistenceMode::NotExists
            } else {
                ExistenceMode::Exists
            },
            |_| (),
            |_| (),
            |(a, b), c| a.0 != c.0 && b.0 != c.0,
        )
        .unwrap();
    let c = b
        .score("testConstraint", &out, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for (id, value) in [(0, Some(0)), (1, Some(0)), (2, None)] {
        tx.apply(&source, TupleChange::put(id, (id, value)))
            .unwrap();
    }
    tx.commit().unwrap();
    let check = |g: &StreamGraph<'_, SimpleScore>| {
        let actual: Vec<_> = g
            .constraint_matches(&c)
            .unwrap()
            .values()
            .map(|((a, b), s)| {
                assert_eq!(*s, SimpleScore::new(-1));
                (a.0, b.0)
            })
            .collect();
        assert_eq!(actual, if absent { vec![(0, 1)] } else { vec![] });
        assert_eq!(g.score(), SimpleScore::new(if absent { -1 } else { 0 }));
    };
    check(&g);
    if absent {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::retract(2)).unwrap();
        tx.commit().unwrap();
        check(&g);
    }
}
#[test]
fn if_exists_does_not_include_unassigned() {
    excludes_unassigned(false)
}
#[test]
fn if_not_exists_does_not_include_unassigned() {
    excludes_unassigned(true)
}
