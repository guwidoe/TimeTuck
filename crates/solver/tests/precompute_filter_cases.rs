// Adapted from pinned Community precompute tests and Lavish fixture helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::Cell, collections::BTreeSet};
use timetuck::*;
#[derive(Clone)]
struct Entity {
    id: usize,
    group: usize,
    value: Option<usize>,
}
#[derive(Clone, PartialEq)]
enum Atom {
    Entity(usize, usize),
    Value(usize, Option<usize>),
    Group(usize),
    ValueGroup(usize),
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Identity {
    Entity(usize),
    Value(usize),
    Group(usize),
    ValueGroup(usize),
}
impl Atom {
    fn identity(&self) -> Identity {
        match *self {
            Self::Entity(id, _) => Identity::Entity(id),
            Self::Value(id, _) => Identity::Value(id),
            Self::Group(id) => Identity::Group(id),
            Self::ValueGroup(id) => Identity::ValueGroup(id),
        }
    }
}
fn entity_id(row: &[Atom]) -> usize {
    row.iter()
        .find_map(|a| {
            if let Atom::Entity(id, _) = a {
                Some(*id)
            } else {
                None
            }
        })
        .unwrap()
}
fn verify<I: Ord + Clone + 'static>(
    g: &StreamGraph<'_, SimpleScore>,
    data: &StreamDataset<I, Vec<Identity>>,
    expected: Vec<Vec<Identity>>,
) {
    let rows = g.dataset_rows(data).unwrap();
    assert_eq!(rows.len(), expected.len());
    assert_eq!(
        rows.iter().map(|(_, v)| v.clone()).collect::<BTreeSet<_>>(),
        expected.iter().cloned().collect()
    );
    assert_eq!(g.score(), SimpleScore::new(-(expected.len() as i64)));
}
fn initial_entities(arity: usize, unique_position: Option<usize>) -> Vec<Entity> {
    let mut entities: Vec<_> = (0..7)
        .map(|id| Entity {
            id,
            group: id % 3,
            value: Some(id % 5),
        })
        .collect();
    entities.extend([
        Entity {
            id: 7,
            group: 3,
            value: Some(if arity == 1 { 0 } else { 5 }),
        },
        Entity {
            id: 8,
            group: 3,
            value: Some(if arity == 1 { 0 } else { 5 }),
        },
        Entity {
            id: 9,
            group: 0,
            value: Some(if arity == 1 {
                0
            } else if unique_position == Some(1) {
                6
            } else {
                5
            }),
        },
    ]);
    entities
}
fn expected(entities: &[usize], arity: usize, position: usize) -> Vec<Vec<Identity>> {
    entities
        .iter()
        .flat_map(|id| {
            let values: Vec<_> = if arity == 1 {
                vec![None]
            } else {
                vec![Some(5), Some(6)]
            };
            values.into_iter().map(move |value| {
                let mut fields = Vec::new();
                if let Some(v) = value {
                    fields.push(Identity::Value(v));
                }
                if arity >= 3 {
                    fields.push(Identity::Group(3));
                }
                if arity == 4 {
                    fields.push(Identity::ValueGroup(2));
                }
                fields.insert(position, Identity::Entity(*id));
                fields
            })
        })
        .collect()
}
struct Sources {
    entity: StreamSource<usize, Entity>,
    value: StreamSource<usize, Vec<Atom>>,
    group: StreamSource<usize, Vec<Atom>>,
    value_group: StreamSource<usize, Vec<Atom>>,
}
fn execute<'a, I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'a, SimpleScore>,
    s: Sources,
    parent: StreamPort<I, Vec<Atom>>,
    arity: usize,
    position: usize,
    calls: &'a Cell<usize>,
) {
    let filtered = b
        .filter(&parent, |row| {
            row.iter().all(|v| match v {
                Atom::Entity(id, group) => {
                    if *id == 7 {
                        calls.set(calls.get() + 1);
                    }
                    *group == 3
                }
                Atom::Value(_, group) => *group == Some(2),
                Atom::Group(id) => *id == 3,
                Atom::ValueGroup(id) => *id == 2,
            })
        })
        .unwrap();
    let current = b
        .join_equal(&filtered, s.entity.port(), |v| entity_id(v), |e| e.id)
        .unwrap();
    let visible = b
        .filter(&current, move |(_, e)| arity == 1 || e.value == Some(5))
        .unwrap();
    let output = b
        .map(&visible, |(row, _)| {
            row.iter().map(Atom::identity).collect::<Vec<_>>()
        })
        .unwrap();
    let data = b.dataset(&output).unwrap();
    b.score("precomputed", &output, |_| SimpleScore::new(-1))
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for id in 0..if arity == 1 { 2 } else { 3 } {
        tx.apply(
            &s.value_group,
            TupleChange::put(id, vec![Atom::ValueGroup(id)]),
        )
        .unwrap();
    }
    for id in 0..4 {
        tx.apply(&s.group, TupleChange::put(id, vec![Atom::Group(id)]))
            .unwrap();
    }
    for id in 0..if arity == 1 { 5 } else { 8 } {
        tx.apply(
            &s.value,
            TupleChange::put(
                id,
                vec![Atom::Value(
                    id,
                    if id < 5 {
                        Some(id % 2)
                    } else if id < 7 {
                        Some(2)
                    } else {
                        None
                    },
                )],
            ),
        )
        .unwrap();
    }
    let mut entities = initial_entities(arity, None);
    for e in &entities {
        tx.apply(&s.entity, TupleChange::put(e.id, e.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    verify(&graph, &data, expected(&[7, 8], arity, position));
    assert!(calls.get() > 0);
    calls.set(0);
    entities[7].value = Some(if arity == 1 { 99 } else { 0 });
    let mut tx = graph.transaction();
    tx.apply(&s.entity, TupleChange::put(7, entities[7].clone()))
        .unwrap();
    tx.commit().unwrap();
    verify(
        &graph,
        &data,
        expected(if arity == 1 { &[7, 8] } else { &[8] }, arity, position),
    );
    assert_eq!(calls.get(), 0);
    entities[9].group = 3;
    let mut tx = graph.transaction();
    tx.apply(&s.entity, TupleChange::put(9, entities[9].clone()))
        .unwrap();
    tx.commit().unwrap();
    verify(
        &graph,
        &data,
        expected(
            if arity == 1 { &[7, 8, 9] } else { &[8, 9] },
            arity,
            position,
        ),
    );
    let mut tx = graph.transaction();
    tx.apply(&s.entity, TupleChange::retract(9)).unwrap();
    tx.commit().unwrap();
    verify(
        &graph,
        &data,
        expected(if arity == 1 { &[7, 8] } else { &[8] }, arity, position),
    );
    let mut tx = graph.transaction();
    tx.apply(&s.entity, TupleChange::put(9, entities[9].clone()))
        .unwrap();
    tx.commit().unwrap();
    verify(
        &graph,
        &data,
        expected(
            if arity == 1 { &[7, 8, 9] } else { &[8, 9] },
            arity,
            position,
        ),
    );
}
fn filter(arity: usize, position: usize) {
    let calls = Cell::new(0usize);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = Sources {
        entity: b.source(),
        value: b.source(),
        group: b.source(),
        value_group: b.source(),
    };
    let facts = b
        .precompute(
            s.entity.port(),
            |e| (e.id, e.group),
            |e| vec![Atom::Entity(e.id, e.group)],
        )
        .unwrap();
    let mut ports = vec![
        s.value.port().clone(),
        s.group.port().clone(),
        s.value_group.port().clone(),
    ];
    ports.truncate(arity - 1);
    ports.insert(position, facts);
    macro_rules! join {
        ($l:expr,$r:expr) => {{
            let joined = b.join_equal($l, $r, |_| (), |_| ()).unwrap();
            b.map(&joined, |(a, c)| {
                let mut out = a.clone();
                out.extend(c.iter().cloned());
                out
            })
            .unwrap()
        }};
    }
    if arity == 1 {
        return execute(b, s, ports[0].clone(), arity, position, &calls);
    }
    let bi = join!(&ports[0], &ports[1]);
    if arity == 2 {
        return execute(b, s, bi, arity, position, &calls);
    }
    let tri = join!(&bi, &ports[2]);
    if arity == 3 {
        return execute(b, s, tri, arity, position, &calls);
    }
    let quad = join!(&tri, &ports[3]);
    execute(b, s, quad, arity, position, &calls);
}
fn unique_pair(position: usize) {
    let calls = Cell::new(0usize);
    let spy = 7 + position;
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let facts = b
        .precompute(source.port(), |e| (e.id, e.group), |e| (e.id, e.group))
        .unwrap();
    let pairs = b
        .unique_pairs_equal(&facts, |e| {
            if e.0 == spy {
                calls.set(calls.get() + 1);
            }
            e.1
        })
        .unwrap();
    let live = b
        .join_equal(
            &pairs,
            source.port(),
            move |p| if position == 0 { p.0.0 } else { p.1.0 },
            |e| e.id,
        )
        .unwrap();
    let visible = b.filter(&live, |(_, e)| e.value == Some(5)).unwrap();
    let out = b
        .map(&visible, |((a, c), _)| {
            vec![Identity::Entity(a.0), Identity::Entity(c.0)]
        })
        .unwrap();
    let data = b.dataset(&out).unwrap();
    b.score("pairs", &out, |_| SimpleScore::new(-1)).unwrap();
    let mut graph = b.build();
    let mut entities = initial_entities(2, Some(position));
    let mut tx = graph.transaction();
    for e in &entities {
        tx.apply(&source, TupleChange::put(e.id, e.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    let pair = || vec![vec![Identity::Entity(7), Identity::Entity(8)]];
    let three = || {
        vec![
            vec![Identity::Entity(7), Identity::Entity(8)],
            vec![Identity::Entity(7), Identity::Entity(9)],
            vec![Identity::Entity(8), Identity::Entity(9)],
        ]
    };
    verify(&graph, &data, pair());
    assert!(calls.get() > 0);
    for value in [0, 5] {
        calls.set(0);
        entities[spy].value = Some(value);
        let mut tx = graph.transaction();
        tx.apply(&source, TupleChange::put(spy, entities[spy].clone()))
            .unwrap();
        tx.commit().unwrap();
        verify(&graph, &data, if value == 0 { vec![] } else { pair() });
        assert_eq!(calls.get(), 0);
    }
    entities[9].group = 3;
    entities[9].value = Some(5);
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(9, entities[9].clone()))
        .unwrap();
    tx.commit().unwrap();
    verify(&graph, &data, three());
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::retract(9)).unwrap();
    tx.commit().unwrap();
    verify(&graph, &data, pair());
    let mut tx = graph.transaction();
    tx.apply(&source, TupleChange::put(9, entities[9].clone()))
        .unwrap();
    tx.commit().unwrap();
    verify(&graph, &data, three());
}
#[test]
fn uni_for_each_unfiltered_fact() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, usize>();
    let p = b
        .precompute(source.port(), |v| *v, |v| vec![Identity::Value(*v)])
        .unwrap();
    let data = b.dataset(&p).unwrap();
    b.score("facts", &p, |_| SimpleScore::new(-1)).unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for id in 0..3 {
        tx.apply(&source, TupleChange::put(id, id)).unwrap();
    }
    tx.commit().unwrap();
    verify(
        &g,
        &data,
        (0..3).map(|id| vec![Identity::Value(id)]).collect(),
    );
    let mut tx = g.transaction();
    tx.apply(&source, TupleChange::retract(2)).unwrap();
    tx.commit().unwrap();
    verify(
        &g,
        &data,
        (0..2).map(|id| vec![Identity::Value(id)]).collect(),
    );
}

#[test]
fn uni_filter_0_changed() {
    filter(1, 0);
}

#[test]
fn bi_filter_0_changed() {
    filter(2, 0);
}

#[test]
fn bi_filter_1_changed() {
    filter(2, 1);
}

#[test]
fn tri_filter_0_changed() {
    filter(3, 0);
}

#[test]
fn tri_filter_1_changed() {
    filter(3, 1);
}

#[test]
fn tri_filter_2_changed() {
    filter(3, 2);
}

#[test]
fn quad_filter_0_changed() {
    filter(4, 0);
}

#[test]
fn quad_filter_1_changed() {
    filter(4, 1);
}

#[test]
fn quad_filter_2_changed() {
    filter(4, 2);
}

#[test]
fn quad_filter_3_changed() {
    filter(4, 3);
}

#[test]
fn bi_filter_0_changed_for_each_unfiltered_unique_pair() {
    unique_pair(0);
}

#[test]
fn bi_filter_1_changed_for_each_unfiltered_unique_pair() {
    unique_pair(1);
}
