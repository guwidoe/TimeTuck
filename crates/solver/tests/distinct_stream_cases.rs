// Adapted from pinned Timefold Community 2.7.0 Abstract{Uni,Bi,Tri,Quad}ConstraintStreamTest.
// Apache-2.0. Test, fixture and assertion-helper provenance: compat/provenance.json.
use std::{collections::BTreeSet, fmt::Debug};
use timetuck::{
    SimpleScore, StreamConstraint, StreamGraph, StreamGraphBuilder, StreamPort, StreamSource,
    TupleChange as Change,
};
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fact {
    Entity(usize),
    Group(usize),
}
#[derive(Clone, Copy)]
enum Variant {
    Identity,
    Map { duplicates: bool, distinct: bool },
}
fn assert_matches<I: Ord + Clone + Debug + 'static>(
    g: &StreamGraph<'_, SimpleScore>,
    c: &StreamConstraint<I, Vec<Fact>>,
    mut expected: Vec<Vec<Fact>>,
) {
    expected.sort();
    let mut actual = g
        .constraint_matches(c)
        .unwrap()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(
        actual,
        expected
            .iter()
            .cloned()
            .map(|v| (v, SimpleScore::new(-1)))
            .collect::<Vec<_>>()
    );
    let score = SimpleScore::new(-(expected.len() as i64));
    assert_eq!(g.score(), score);
    assert_eq!(
        g.constraint_scores().collect::<Vec<_>>(),
        [("testConstraint", score)]
    );
}
fn expected(arity: usize, v: Variant) -> (Vec<Vec<Fact>>, Option<Vec<Vec<Fact>>>) {
    use Fact::{Entity as E, Group as G};
    match v {
        Variant::Identity => (
            match arity {
                1 => vec![vec![E(0)], vec![E(1)]],
                2 => vec![vec![E(0), E(1)], vec![E(0), E(2)], vec![E(1), E(2)]],
                3 => vec![
                    vec![E(0), E(1), E(2)],
                    vec![E(0), E(2), E(1)],
                    vec![E(1), E(2), E(0)],
                ],
                _ => vec![
                    vec![E(0), E(1), G(0), G(1)],
                    vec![E(0), E(2), G(0), G(0)],
                    vec![E(1), E(2), G(1), G(0)],
                ],
            },
            None,
        ),
        Variant::Map {
            duplicates,
            distinct,
        } => {
            let (mut before, after) = match (arity, duplicates) {
                (1, true) => (vec![vec![G(0)], vec![G(0)]], vec![vec![G(0)]]),
                (1, false) => (vec![vec![G(0)], vec![G(1)]], vec![vec![G(1)]]),
                (2 | 4, true) => (
                    vec![vec![G(0), G(1)], vec![G(0), G(1)], vec![G(0)]],
                    vec![vec![G(0), G(1)]],
                ),
                (2, false) => (
                    vec![vec![G(0), G(1)], vec![G(0), G(2)], vec![G(1), G(2)]],
                    vec![vec![G(1), G(2)]],
                ),
                (3, true) => (vec![vec![G(0), G(1)]; 3], vec![]),
                (3, false) => (
                    vec![vec![G(0), G(1)], vec![G(0), G(2)], vec![G(1), G(2)]],
                    vec![],
                ),
                (4, false) => (vec![vec![G(0), G(1)]], vec![]),
                _ => unreachable!(),
            };
            if distinct {
                before.sort();
                before.dedup();
            }
            (before, Some(after))
        }
    }
}
fn execute<I: Ord + Clone + Debug + 'static>(
    mut g: StreamGraph<'_, SimpleScore>,
    entities: StreamSource<usize, Entity>,
    groups: StreamSource<usize, usize>,
    terminal: StreamConstraint<I, Vec<Fact>>,
    rows: Vec<Entity>,
    group_count: usize,
    expect: (Vec<Vec<Fact>>, Option<Vec<Vec<Fact>>>),
) {
    let mut t = g.transaction();
    for id in 0..group_count {
        t.apply(&groups, Change::put(id, id)).unwrap();
    }
    for row in rows {
        t.apply(&entities, Change::put(row.id, row)).unwrap();
    }
    t.commit().unwrap();
    assert_matches(&g, &terminal, expect.0);
    if let Some(after) = expect.1 {
        let mut t = g.transaction();
        t.apply(&entities, Change::retract(0)).unwrap();
        t.commit().unwrap();
        assert_matches(&g, &terminal, after);
    }
}
fn finish<I: Ord + Clone + Debug + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    entities: StreamSource<usize, Entity>,
    groups: StreamSource<usize, usize>,
    port: StreamPort<I, Vec<Fact>>,
    rows: Vec<Entity>,
    group_count: usize,
    config: (usize, Variant),
) {
    let (arity, variant) = config;
    let expect = expected(arity, variant);
    if matches!(
        variant,
        Variant::Identity | Variant::Map { distinct: true, .. }
    ) {
        let port = b.distinct(&port).unwrap();
        let c = b
            .score("testConstraint", &port, |_| SimpleScore::new(-1))
            .unwrap();
        execute(b.build(), entities, groups, c, rows, group_count, expect);
    } else {
        let c = b
            .score("testConstraint", &port, |_| SimpleScore::new(-1))
            .unwrap();
        execute(b.build(), entities, groups, c, rows, group_count, expect);
    }
}
fn group_set(ids: impl IntoIterator<Item = usize>) -> Vec<Fact> {
    ids.into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(Fact::Group)
        .collect()
}
fn run(arity: usize, variant: Variant) {
    let identity = matches!(variant, Variant::Identity);
    let duplicates = matches!(
        variant,
        Variant::Map {
            duplicates: true,
            ..
        }
    );
    let (n, group_count, value_count) = if identity {
        (if arity == 1 { 2 } else { 3 }, 2, 2)
    } else {
        match (arity, duplicates) {
            (1, true) => (2, 1, 1),
            (1 | 4, false) => (2, 2, 1),
            (4, true) => (3, 2, 2),
            (_, true) => (3, 2, 1),
            _ => (3, 3, 1),
        }
    };
    let rows = (0..n)
        .map(|id| Entity {
            id,
            group: id % group_count,
            value: id % value_count,
        })
        .collect();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, Entity>();
    let groups = b.source::<usize, usize>();
    if arity == 1 {
        let port = b
            .map(entities.port(), move |e| {
                if identity {
                    vec![Fact::Entity(e.id)]
                } else {
                    vec![Fact::Group(e.group)]
                }
            })
            .unwrap();
        return finish(
            b,
            entities,
            groups,
            port,
            rows,
            group_count,
            (arity, variant),
        );
    }
    let pairs = b
        .join_equal(entities.port(), entities.port(), |_| (), |_| ())
        .unwrap();
    let pairs = b.filter(&pairs, |(a, b)| a.id < b.id).unwrap();
    if arity == 2 {
        let port = b
            .map(&pairs, move |(a, b)| {
                if identity {
                    vec![Fact::Entity(a.id), Fact::Entity(b.id)]
                } else {
                    group_set([a.group, b.group])
                }
            })
            .unwrap();
        return finish(
            b,
            entities,
            groups,
            port,
            rows,
            group_count,
            (arity, variant),
        );
    }
    if arity == 3 {
        let triple = b
            .join_equal(&pairs, entities.port(), |_| (), |_| ())
            .unwrap();
        let triple = b
            .filter(&triple, |((a, b), c)| a.id != c.id && b.id != c.id)
            .unwrap();
        let port = b
            .map(&triple, move |((a, b), c)| {
                if identity {
                    vec![Fact::Entity(a.id), Fact::Entity(b.id), Fact::Entity(c.id)]
                } else if duplicates {
                    group_set([a.group, b.group, c.group])
                } else {
                    group_set([a.group, b.group])
                }
            })
            .unwrap();
        return finish(
            b,
            entities,
            groups,
            port,
            rows,
            group_count,
            (arity, variant),
        );
    }
    let triple = b
        .join_equal(&pairs, groups.port(), |(a, _)| a.group, |g| *g)
        .unwrap();
    let quad = b
        .join_equal(&triple, groups.port(), |((_, b), _)| b.group, |g| *g)
        .unwrap();
    let port = b
        .map(&quad, move |(((a, b), g1), g2)| {
            if identity {
                vec![
                    Fact::Entity(a.id),
                    Fact::Entity(b.id),
                    Fact::Group(*g1),
                    Fact::Group(*g2),
                ]
            } else {
                group_set([*g1, *g2])
            }
        })
        .unwrap();
    finish(
        b,
        entities,
        groups,
        port,
        rows,
        group_count,
        (arity, variant),
    );
}

#[test]
fn uni_distinct() {
    run(1, Variant::Identity);
}
#[test]
fn uni_map_to_uni_with_duplicates() {
    run(
        1,
        Variant::Map {
            duplicates: true,
            distinct: false,
        },
    );
}
#[test]
fn uni_map_to_uni_and_distinct_with_duplicates() {
    run(
        1,
        Variant::Map {
            duplicates: true,
            distinct: true,
        },
    );
}
#[test]
fn uni_map_to_uni_without_duplicates() {
    run(
        1,
        Variant::Map {
            duplicates: false,
            distinct: false,
        },
    );
}
#[test]
fn uni_map_to_uni_and_distinct_without_duplicates() {
    run(
        1,
        Variant::Map {
            duplicates: false,
            distinct: true,
        },
    );
}

#[test]
fn bi_distinct() {
    run(2, Variant::Identity);
}
#[test]
fn bi_map_to_uni_with_duplicates() {
    run(
        2,
        Variant::Map {
            duplicates: true,
            distinct: false,
        },
    );
}
#[test]
fn bi_map_to_uni_and_distinct_with_duplicates() {
    run(
        2,
        Variant::Map {
            duplicates: true,
            distinct: true,
        },
    );
}
#[test]
fn bi_map_to_uni_without_duplicates() {
    run(
        2,
        Variant::Map {
            duplicates: false,
            distinct: false,
        },
    );
}
#[test]
fn bi_map_to_uni_and_distinct_without_duplicates() {
    run(
        2,
        Variant::Map {
            duplicates: false,
            distinct: true,
        },
    );
}

#[test]
fn tri_distinct() {
    run(3, Variant::Identity);
}
#[test]
fn tri_map_to_uni_with_duplicates() {
    run(
        3,
        Variant::Map {
            duplicates: true,
            distinct: false,
        },
    );
}
#[test]
fn tri_map_to_uni_and_distinct_with_duplicates() {
    run(
        3,
        Variant::Map {
            duplicates: true,
            distinct: true,
        },
    );
}
#[test]
fn tri_map_to_uni_without_duplicates() {
    run(
        3,
        Variant::Map {
            duplicates: false,
            distinct: false,
        },
    );
}
#[test]
fn tri_map_to_uni_and_distinct_without_duplicates() {
    run(
        3,
        Variant::Map {
            duplicates: false,
            distinct: true,
        },
    );
}

#[test]
fn quad_distinct() {
    run(4, Variant::Identity);
}
#[test]
fn quad_map_to_uni_with_duplicates() {
    run(
        4,
        Variant::Map {
            duplicates: true,
            distinct: false,
        },
    );
}
#[test]
fn quad_map_to_uni_and_distinct_with_duplicates() {
    run(
        4,
        Variant::Map {
            duplicates: true,
            distinct: true,
        },
    );
}
#[test]
fn quad_map_to_uni_without_duplicates() {
    run(
        4,
        Variant::Map {
            duplicates: false,
            distinct: false,
        },
    );
}
#[test]
fn quad_map_to_uni_and_distinct_without_duplicates() {
    run(
        4,
        Variant::Map {
            duplicates: false,
            distinct: true,
        },
    );
}
