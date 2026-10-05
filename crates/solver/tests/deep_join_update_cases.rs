// Adapted from pinned Community BavetRegressionTest and its predicate-flip helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone, Debug, PartialEq)]
struct Entity {
    id: usize,
    group: usize,
    value: Option<usize>,
}
#[derive(Clone, Copy)]
enum Shape {
    Indexed,
    Unindexed,
    Mapped,
    Exists,
    Absent,
    NullKeys,
}
type ReadRows = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Vec<Vec<usize>>;
struct Harness {
    graph: StreamGraph<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    read: Box<ReadRows>,
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    port: StreamPort<I, Vec<usize>>,
) -> Harness {
    let data = b.dataset(&port).unwrap();
    b.score("regression", &port, |_| SimpleScore::new(-1))
        .unwrap();
    Harness {
        graph: b.build(),
        source,
        read: Box::new(move |g| {
            let mut rows: Vec<_> = g
                .dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(_, r)| r.clone())
                .collect();
            rows.sort();
            rows
        }),
    }
}
fn build(shape: Shape) -> Harness {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    if matches!(shape, Shape::NullKeys) {
        let pairs = b
            .join_equal(source.port(), source.port(), |e| e.value, |e| e.value)
            .unwrap();
        let out = b.map(&pairs, |(a, c)| vec![a.id, c.id]).unwrap();
        return finish(b, source, out);
    }
    let assigned = b.filter(source.port(), |e| e.value.is_some()).unwrap();
    let once = b.map(&assigned, Clone::clone).unwrap();
    let twice = b.map(&once, Clone::clone).unwrap();
    if matches!(shape, Shape::Exists | Shape::Absent) {
        let mode = if matches!(shape, Shape::Exists) {
            ExistenceMode::Exists
        } else {
            ExistenceMode::NotExists
        };
        let visible = b
            .existence(
                &twice,
                &assigned,
                mode,
                |e| e.group,
                |e| e.group,
                |_, e| e.value == Some(0),
            )
            .unwrap();
        let out = b.map(&visible, |e| vec![e.id]).unwrap();
        return finish(b, source, out);
    }
    let indexed = !matches!(shape, Shape::Unindexed);
    let pairs = b
        .join_equal(
            &assigned,
            &twice,
            move |e| if indexed { e.group } else { 0 },
            move |e| if indexed { e.group } else { 0 },
        )
        .unwrap();
    let retained = b.filter(&pairs, |(a, _)| a.value == Some(0)).unwrap();
    if matches!(shape, Shape::Mapped) {
        let projected = b.map(&retained, |(a, _)| a.clone()).unwrap();
        let thrice = b.map(&twice, Clone::clone).unwrap();
        let joined = b
            .join_equal(&projected, &thrice, |a| a.group, |c| c.group)
            .unwrap();
        let guarded = b
            .filter(&joined, |(a, _)| {
                assert_eq!(a.value, Some(0), "stale filtered row");
                true
            })
            .unwrap();
        let out = b.map(&guarded, |(a, c)| vec![a.id, c.id]).unwrap();
        return finish(b, source, out);
    }
    let joined = b
        .join_equal(
            &retained,
            &twice,
            move |(a, _)| if indexed { a.group } else { 0 },
            move |c| if indexed { c.group } else { 0 },
        )
        .unwrap();
    let guarded = b
        .filter(&joined, |((a, _), _)| {
            assert_eq!(a.value, Some(0), "stale filtered row");
            true
        })
        .unwrap();
    let out = b
        .map(&guarded, |((a, c), d)| vec![a.id, c.id, d.id])
        .unwrap();
    finish(b, source, out)
}
fn put(h: &mut Harness, entities: &[Entity]) {
    let mut tx = h.graph.transaction();
    for e in entities {
        tx.apply(&h.source, TupleChange::put(e.id, e.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
}
fn assert_rows(h: &Harness, mut expected: Vec<Vec<usize>>) {
    expected.sort();
    assert_eq!((h.read)(&h.graph), expected);
    assert_eq!(h.graph.score(), SimpleScore::new(-(expected.len() as i64)));
}
fn fresh(h: &Harness, shape: Shape, entities: &[Entity]) {
    let mut rebuilt = build(shape);
    put(&mut rebuilt, entities);
    assert_eq!(h.graph.score(), rebuilt.graph.score());
    assert_eq!((h.read)(&h.graph), (rebuilt.read)(&rebuilt.graph));
}
fn triples(first: &[usize], mapped: bool) -> Vec<Vec<usize>> {
    first
        .iter()
        .flat_map(|a| {
            (0..2).flat_map(move |b| {
                (0..2).map(move |c| if mapped { vec![*a, c] } else { vec![*a, b, c] })
            })
        })
        .collect()
}
fn flip(shape: Shape) {
    let mut entities: Vec<_> = (0..2)
        .map(|id| Entity {
            id,
            group: 0,
            value: Some(0),
        })
        .collect();
    let mut h = build(shape);
    put(&mut h, &entities);
    let exists = matches!(shape, Shape::Exists);
    let absent = matches!(shape, Shape::Absent);
    let mapped = matches!(shape, Shape::Mapped);
    assert_rows(
        &h,
        if exists {
            vec![vec![0], vec![1]]
        } else if absent {
            vec![]
        } else {
            triples(&[0, 1], mapped)
        },
    );
    entities[0].value = Some(1);
    put(&mut h, &entities[0..1]);
    assert_rows(
        &h,
        if exists {
            vec![vec![0], vec![1]]
        } else if absent {
            vec![]
        } else {
            triples(&[1], mapped)
        },
    );
    fresh(&h, shape, &entities);
    if exists || absent {
        entities[1].value = Some(1);
        put(&mut h, &entities[1..2]);
        assert_rows(
            &h,
            if exists {
                vec![]
            } else {
                vec![vec![0], vec![1]]
            },
        );
        fresh(&h, shape, &entities);
    }
}
#[test]
fn filtering_join_predicate_flip_behind_deferring_join() {
    flip(Shape::Indexed);
}
#[test]
fn filtering_join_predicate_flip_behind_deferring_join_unindexed() {
    flip(Shape::Unindexed);
}
#[test]
fn filtering_join_predicate_flip_behind_mapped_deferring_join() {
    flip(Shape::Mapped);
}
#[test]
fn filtering_if_exists_predicate_flip_when_deferring() {
    flip(Shape::Exists);
}
#[test]
fn filtering_if_not_exists_predicate_flip_when_deferring() {
    flip(Shape::Absent);
}
#[test]
fn join_with_null_key_from_right() {
    let mut entities = [
        Entity {
            id: 0,
            group: 0,
            value: None,
        },
        Entity {
            id: 1,
            group: 0,
            value: Some(0),
        },
    ];
    let mut h = build(Shape::NullKeys);
    put(&mut h, &entities);
    assert_rows(&h, vec![vec![0, 0], vec![1, 1]]);
    entities[0].value = Some(0);
    entities[1].value = None;
    put(&mut h, &entities);
    assert_rows(&h, vec![vec![0, 0], vec![1, 1]]);
    entities[0].value = None;
    put(&mut h, &entities);
    assert_rows(&h, vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]]);
}

// Original changing-state regression: complete row multisets, all shapes and rollback.
#[test]
fn unequal_depth_relations_match_enumeration_through_edits_and_rollback() {
    use std::collections::BTreeMap;
    for shape in [
        Shape::Indexed,
        Shape::Unindexed,
        Shape::Mapped,
        Shape::Exists,
        Shape::Absent,
        Shape::NullKeys,
    ] {
        let mut h = build(shape);
        let mut model = BTreeMap::<usize, Entity>::new();
        let expected = |model: &BTreeMap<usize, Entity>| {
            let mut rows = Vec::new();
            let values: Vec<_> = model
                .values()
                .filter(|e| matches!(shape, Shape::NullKeys) || e.value.is_some())
                .collect();
            for a in &values {
                if matches!(shape, Shape::Exists | Shape::Absent) {
                    let exists = values
                        .iter()
                        .any(|b| a.group == b.group && b.value == Some(0));
                    if exists == matches!(shape, Shape::Exists) {
                        rows.push(vec![a.id]);
                    }
                    continue;
                }
                for b in &values {
                    if matches!(shape, Shape::NullKeys) {
                        if a.value == b.value {
                            rows.push(vec![a.id, b.id]);
                        }
                        continue;
                    }
                    if a.value != Some(0)
                        || (!matches!(shape, Shape::Unindexed) && a.group != b.group)
                    {
                        continue;
                    }
                    for c in &values {
                        if matches!(shape, Shape::Unindexed) || a.group == c.group {
                            rows.push(if matches!(shape, Shape::Mapped) {
                                vec![a.id, c.id]
                            } else {
                                vec![a.id, b.id, c.id]
                            });
                        }
                    }
                }
            }
            rows
        };
        for step in 0..300 {
            let id = (step * 7) % 9;
            let mut next = model.clone();
            let mut tx = h.graph.transaction();
            if step % 5 == 0 {
                next.remove(&id);
                tx.apply(&h.source, TupleChange::retract(id)).unwrap();
            } else {
                let e = Entity {
                    id,
                    group: (step / 7) % 3,
                    value: if step % 4 == 0 {
                        None
                    } else {
                        Some((step / 3) % 2)
                    },
                };
                next.insert(id, e.clone());
                tx.apply(&h.source, TupleChange::put(id, e)).unwrap();
            }
            let mut wanted = expected(&next);
            wanted.sort();
            assert_eq!((h.read)(tx.graph().unwrap()), wanted);
            assert_eq!(
                tx.score().unwrap(),
                SimpleScore::new(-(wanted.len() as i64))
            );
            if step % 7 == 0 {
                drop(tx);
            } else {
                tx.commit().unwrap();
                model = next;
            }
            assert_rows(&h, expected(&model));
        }
    }
}
