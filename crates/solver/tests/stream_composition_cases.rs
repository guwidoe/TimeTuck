// Adapted from pinned AbstractBi/Tri/QuadConstraintStreamTest and lavish fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

type D = DistinctCountCollector<usize>;
fn grouped<C, V>(
    value: impl Fn(usize) -> V + 'static,
    counts: impl Fn(C::Output) -> Vec<usize> + 'static,
    arity: usize,
) where
    V: Clone + 'static,
    C: ReversibleCollector<V> + 'static,
    C::Output: Clone + 'static,
{
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, usize>();
    let extras = b.source::<usize, ()>();
    let grouped = b
        .group_collect_with::<_, _, _, _, C>(entities.port(), move |v| ((), value(*v)))
        .unwrap();
    let grouped = b
        .map(&grouped, move |(_, output)| counts(output.clone()))
        .unwrap();
    let visible = b
        .existence_equal(
            &grouped,
            extras.port(),
            ExistenceMode::Exists,
            |_| (),
            |_| (),
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &visible, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for id in [0, 1] {
        tx.apply(&entities, TupleChange::put(id, 0)).unwrap();
        tx.apply(&extras, TupleChange::put(id, ())).unwrap();
    }
    tx.commit().unwrap();
    let check = |g: &StreamGraph<'_, SimpleScore>, expected: Option<usize>| {
        let actual: Vec<_> = g
            .constraint_matches(&terminal)
            .unwrap()
            .values()
            .map(|(row, score)| {
                assert_eq!(*score, SimpleScore::new(-1));
                row.clone()
            })
            .collect();
        assert_eq!(
            actual,
            expected.map(|n| vec![vec![n; arity]]).unwrap_or_default()
        );
        assert_eq!(
            g.score(),
            SimpleScore::new(if expected.is_some() { -1 } else { 0 })
        );
    };
    check(&g, Some(1));
    let mut tx = g.transaction();
    tx.apply(&entities, TupleChange::put(1, 1)).unwrap();
    tx.commit().unwrap();
    check(&g, Some(2));
    let mut tx = g.transaction();
    tx.apply(&entities, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    check(&g, Some(1));
    // Additional native coverage: right multiplicity, rollback, and reactivation.
    let mut tx = g.transaction();
    tx.apply(&extras, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    check(&g, Some(1));
    {
        let mut tx = g.transaction();
        tx.apply(&extras, TupleChange::retract(1)).unwrap();
    }
    check(&g, Some(1));
    let mut tx = g.transaction();
    tx.apply(&extras, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    check(&g, None);
    let mut tx = g.transaction();
    tx.apply(&entities, TupleChange::put(1, 1)).unwrap();
    tx.commit().unwrap();
    check(&g, None);
    let mut tx = g.transaction();
    tx.apply(&extras, TupleChange::put(2, ())).unwrap();
    tx.commit().unwrap();
    check(&g, Some(2));
}
#[test]
fn bi_if_exists_after_group_by() {
    grouped::<(D, D), _>(|v| (v, v), |(a, b)| vec![a, b], 2);
}
#[test]
fn tri_if_exists_after_group_by() {
    grouped::<(D, D, D), _>(|v| (v, v, v), |(a, b, c)| vec![a, b, c], 3);
}
#[test]
fn quad_if_exists_after_group_by() {
    grouped::<(D, D, D, D), _>(|v| (v, v, v, v), |(a, b, c, d)| vec![a, b, c, d], 4);
}

#[derive(Clone)]
struct Entity {
    id: usize,
    group: usize,
    assigned: bool,
}
fn triples(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    source: &StreamSource<usize, Entity>,
) -> StreamPort<((usize, usize), usize), Vec<Entity>> {
    let assigned = b.filter(source.port(), |e| e.assigned).unwrap();
    let pairs = b.join_equal(&assigned, &assigned, |_| (), |_| ()).unwrap();
    let pairs = b.filter(&pairs, |(a, b)| a.id < b.id).unwrap();
    let triples = b.join_equal(&pairs, &assigned, |_| (), |_| ()).unwrap();
    let triples = b
        .filter(&triples, |((a, b), c)| a.id != c.id && b.id != c.id)
        .unwrap();
    b.map(&triples, |((a, b), c)| {
        vec![a.clone(), b.clone(), c.clone()]
    })
    .unwrap()
}
fn unassigned(absent: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let triples = triples(&mut b, &source);
    let assigned = b.filter(source.port(), |e| e.assigned).unwrap();
    let visible = b
        .existence(
            &triples,
            &assigned,
            if absent {
                ExistenceMode::NotExists
            } else {
                ExistenceMode::Exists
            },
            |_| (),
            |_| (),
            |row, witness| row.iter().all(|e| e.id != witness.id),
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &visible, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for id in 0..4 {
        tx.apply(
            &source,
            TupleChange::put(
                id,
                Entity {
                    id,
                    group: 0,
                    assigned: id != 3,
                },
            ),
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let check = |g: &StreamGraph<'_, SimpleScore>| {
        let mut actual: Vec<Vec<usize>> = g
            .constraint_matches(&terminal)
            .unwrap()
            .values()
            .map(|(row, s)| {
                assert_eq!(*s, SimpleScore::new(-1));
                row.iter().map(|e| e.id).collect()
            })
            .collect();
        actual.sort();
        assert_eq!(
            actual,
            if absent {
                vec![vec![0, 1, 2], vec![0, 2, 1], vec![1, 2, 0]]
            } else {
                vec![]
            }
        );
        assert_eq!(g.score(), SimpleScore::new(if absent { -3 } else { 0 }));
    };
    check(&g);
    if absent {
        let mut tx = g.transaction();
        tx.apply(&source, TupleChange::retract(3)).unwrap();
        tx.commit().unwrap();
        check(&g);
    }
}
#[test]
fn tri_if_exists_does_not_include_unassigned() {
    unassigned(false);
}
#[test]
fn tri_if_not_exists_does_not_include_unassigned() {
    unassigned(true);
}

fn flatten_finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    rows: StreamPort<I, Vec<Entity>>,
    tri: bool,
) {
    let flat = b
        .flat_map(&rows, move |row| {
            let groups: Vec<_> = if tri {
                row.iter().map(|e| e.group).collect()
            } else {
                vec![row[0].group, row[1].group, 1]
            };
            groups
                .into_iter()
                .map(|group| (row.iter().map(|e| e.id).collect::<Vec<_>>(), group))
                .collect::<Vec<_>>()
        })
        .unwrap();
    let terminal = b
        .score("testConstraint", &flat, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for id in 0..if tri { 3 } else { 2 } {
        tx.apply(
            &source,
            TupleChange::put(
                id,
                Entity {
                    id,
                    group: if tri { id % 2 } else { 0 },
                    assigned: true,
                },
            ),
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let mut actual: Vec<_> = g
        .constraint_matches(&terminal)
        .unwrap()
        .values()
        .map(|(row, s)| {
            assert_eq!(*s, SimpleScore::new(-1));
            row.clone()
        })
        .collect();
    actual.sort();
    let mut expected = if tri {
        vec![
            (vec![0, 1, 2], 0),
            (vec![0, 1, 2], 1),
            (vec![0, 1, 2], 0),
            (vec![0, 2, 1], 0),
            (vec![0, 2, 1], 0),
            (vec![0, 2, 1], 1),
            (vec![1, 2, 0], 1),
            (vec![1, 2, 0], 0),
            (vec![1, 2, 0], 0),
        ]
    } else {
        vec![(vec![0, 1], 0), (vec![0, 1], 0), (vec![0, 1], 1)]
    };
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(g.score(), SimpleScore::new(-(expected.len() as i64)));
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::retract(0)).unwrap();
    tx.commit().unwrap();
    assert!(g.constraint_matches(&terminal).unwrap().is_empty());
    assert_eq!(g.score(), SimpleScore::new(0));
}
#[test]
fn bi_flatten() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let pairs = b
        .join_equal(source.port(), source.port(), |_| (), |_| ())
        .unwrap();
    let pairs = b.filter(&pairs, |(a, b)| a.id < b.id).unwrap();
    let rows = b.map(&pairs, |(a, b)| vec![a.clone(), b.clone()]).unwrap();
    flatten_finish(b, source, rows, false);
}
#[test]
fn tri_flatten() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let rows = triples(&mut b, &source);
    flatten_finish(b, source, rows, true);
}
