// Adapted from pinned Community Abstract{Uni,Bi,Tri,Quad}ConstraintStreamTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::rc::Rc;
use timetuck::*;
#[derive(Clone, Debug, PartialEq)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
    code: String,
    long: i64,
    integer: i32,
}
#[derive(Clone, Debug, PartialEq)]
struct Group {
    id: usize,
    code: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Field {
    Entity(usize),
    Group(usize),
    Value(usize),
    Extra(usize),
    Text(String),
    Long(i64),
    Integer(i32),
    Pair(usize, i32),
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Match {
    fields: Vec<Field>,
    penalty: i64,
}
fn matched(penalty: i64, mut fields: Vec<Field>) -> Match {
    fields.sort();
    Match { fields, penalty }
}
fn one(fields: Vec<Field>) -> Match {
    matched(1, fields)
}
type Reader = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Vec<Match>;
struct Sources {
    entities: StreamSource<usize, Entity>,
    groups: StreamSource<usize, Group>,
    extras: StreamSource<usize, usize>,
}
struct Network {
    graph: StreamGraph<'static, SimpleScore>,
    sources: Sources,
    read: Box<Reader>,
}
fn builder() -> (StreamGraphBuilder<'static, SimpleScore>, Sources) {
    let mut b = StreamGraphBuilder::new();
    let s = Sources {
        entities: b.source(),
        groups: b.source(),
        extras: b.source(),
    };
    (b, s)
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    sources: Sources,
    out: StreamPort<I, Match>,
) -> Network {
    let data = b.dataset(&out).unwrap();
    b.score("testConstraint", &out, |m| SimpleScore::new(-m.penalty))
        .unwrap();
    Network {
        graph: b.build(),
        sources,
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
fn fixture(values: usize, groups: usize, n: usize) -> Vec<Entity> {
    (0..n)
        .map(|id| Entity {
            id,
            group: id % groups,
            value: id % values,
            code: format!("Generated Entity {id}"),
            long: [11, 23, 37][id % 3],
            integer: 1,
        })
        .collect()
}
fn load(h: &mut Network, es: &[Entity], groups: usize, extras: usize) {
    let mut tx = h.graph.transaction();
    for id in 0..groups {
        tx.apply(
            &h.sources.groups,
            TupleChange::put(
                id,
                Group {
                    id,
                    code: format!("Generated EntityGroup {id}"),
                },
            ),
        )
        .unwrap();
    }
    for id in 0..extras {
        tx.apply(&h.sources.extras, TupleChange::put(id, id))
            .unwrap();
    }
    for e in es {
        tx.apply(&h.sources.entities, TupleChange::put(e.id, e.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
}
fn check(h: &Network, mut expected: Vec<Match>) {
    expected.sort();
    assert_eq!((h.read)(&h.graph), expected);
    assert_eq!(
        h.graph.score(),
        SimpleScore::new(-expected.iter().map(|m| m.penalty).sum::<i64>())
    );
}
fn put(h: &mut Network, e: Entity) {
    let mut tx = h.graph.transaction();
    tx.apply(&h.sources.entities, TupleChange::put(e.id, e))
        .unwrap();
    tx.commit().unwrap();
}
fn remove(h: &mut Network, id: usize) {
    let mut tx = h.graph.transaction();
    tx.apply(&h.sources.entities, TupleChange::retract(id))
        .unwrap();
    tx.commit().unwrap();
}
fn project(es: &[Entity], groups: &[Group], input: usize, output: usize) -> Match {
    use Field::*;
    let fields = match input {
        1 => {
            let e = &es[0];
            let mut r = vec![Group(e.group), Value(e.value)];
            if output >= 3 {
                r.push(Text(e.code.clone()));
            }
            if output == 4 {
                r.push(Long(e.long));
            }
            r
        }
        2 => {
            let mut r = vec![Group(es[0].group), Group(es[1].group)];
            if output == 3 {
                r.push(Long(es[0].long + es[1].long));
            }
            if output == 4 {
                r.extend([Value(es[0].value), Value(es[1].value)]);
            }
            r
        }
        3 => {
            let mut r = vec![Group(es[0].group), Group(es[1].group)];
            if output >= 3 {
                r.push(Group(es[2].group));
            }
            if output == 4 {
                r.push(Long(es.iter().map(|e| e.long).sum()));
            }
            r
        }
        4 => {
            if output == 4 {
                vec![
                    Text(es[0].code.clone()),
                    Text(es[1].code.clone()),
                    Text(groups[0].code.clone()),
                    Text(groups[1].code.clone()),
                ]
            } else {
                let mut r = vec![Entity(es[0].id), Entity(es[1].id)];
                if output == 3 {
                    r.push(Long(es[0].long + es[1].long));
                }
                r
            }
        }
        _ => unreachable!(),
    };
    one(fields)
}
fn mapping(input: usize, output: usize) -> Network {
    let (mut b, s) = builder();
    if input == 1 {
        let out = b
            .map(s.entities.port(), move |e| {
                project(std::slice::from_ref(e), &[], 1, output)
            })
            .unwrap();
        return finish(b, s, out);
    }
    let pairs = b.unique_pairs(s.entities.port()).unwrap();
    if input == 2 {
        let out = b
            .map(&pairs, move |(a, c)| {
                project(&[a.clone(), c.clone()], &[], 2, output)
            })
            .unwrap();
        return finish(b, s, out);
    }
    if input == 3 {
        let joined = b
            .join_equal(&pairs, s.entities.port(), |_| (), |_| ())
            .unwrap();
        let filtered = b
            .filter(&joined, |((a, c), d)| a.id != d.id && c.id != d.id)
            .unwrap();
        let out = b
            .map(&filtered, move |((a, c), d)| {
                project(&[a.clone(), c.clone(), d.clone()], &[], 3, output)
            })
            .unwrap();
        return finish(b, s, out);
    }
    let joined = b
        .join_equal(&pairs, s.groups.port(), |(a, _)| a.group, |g| g.id)
        .unwrap();
    let joined = b
        .join_equal(&joined, s.groups.port(), |((_, c), _)| c.group, |g| g.id)
        .unwrap();
    let out = b
        .map(&joined, move |(((a, c), g), other)| {
            project(
                &[a.clone(), c.clone()],
                &[g.clone(), other.clone()],
                4,
                output,
            )
        })
        .unwrap();
    finish(b, s, out)
}
fn mapping_case(input: usize, output: usize) {
    use Field::*;
    let (values, groups, n) = match input {
        1 => (2, 2, 2),
        2 => (2, 2, 3),
        3 => (1, 3, 3),
        _ => (1, 2, 2),
    };
    let mut h = mapping(input, output);
    load(&mut h, &fixture(values, groups, n), groups, 0);
    let expected = match input {
        1 => {
            let mut a = vec![Group(0), Value(0)];
            let mut b = vec![Group(1), Value(1)];
            if output >= 3 {
                a.push(Text("Generated Entity 0".into()));
                b.push(Text("Generated Entity 1".into()));
            }
            if output == 4 {
                a.push(Long(11));
                b.push(Long(23));
            }
            vec![one(a), one(b)]
        }
        2 => {
            let mut a = vec![Group(0), Group(1)];
            let mut b = vec![Group(1), Group(0)];
            let mut c = vec![Group(0), Group(0)];
            if output == 3 {
                a.push(Long(34));
                b.push(Long(60));
                c.push(Long(48));
            }
            if output == 4 {
                a.extend([Value(0), Value(1)]);
                b.extend([Value(1), Value(0)]);
                c.extend([Value(0), Value(0)]);
            }
            vec![one(a), one(b), one(c)]
        }
        3 => {
            if output == 2 {
                vec![
                    one(vec![Group(0), Group(1)]),
                    one(vec![Group(0), Group(2)]),
                    one(vec![Group(1), Group(2)]),
                ]
            } else {
                [[0, 1, 2], [0, 2, 1], [1, 2, 0]]
                    .into_iter()
                    .map(|ids| {
                        let mut r = ids.into_iter().map(Group).collect::<Vec<_>>();
                        if output == 4 {
                            r.push(Long(71));
                        }
                        one(r)
                    })
                    .collect()
            }
        }
        _ => vec![one(if output == 4 {
            vec![
                Text("Generated Entity 0".into()),
                Text("Generated Entity 1".into()),
                Text("Generated EntityGroup 0".into()),
                Text("Generated EntityGroup 1".into()),
            ]
        } else {
            let mut r = vec![Entity(0), Entity(1)];
            if output == 3 {
                r.push(Long(34));
            }
            r
        })],
    };
    check(&h, expected.clone());
    remove(&mut h, 0);
    check(
        &h,
        if input <= 2 {
            vec![expected[1].clone()]
        } else {
            vec![]
        },
    );
    h.graph.assert_consistent().unwrap();
}
#[test]
fn uni_map_to_bi() {
    mapping_case(1, 2);
}
#[test]
fn uni_map_to_tri() {
    mapping_case(1, 3);
}
#[test]
fn uni_map_to_quad() {
    mapping_case(1, 4);
}
#[test]
fn bi_map_to_bi() {
    mapping_case(2, 2);
}
#[test]
fn bi_map_to_tri() {
    mapping_case(2, 3);
}
#[test]
fn bi_map_to_quad() {
    mapping_case(2, 4);
}
#[test]
fn tri_map_to_bi() {
    mapping_case(3, 2);
}
#[test]
fn tri_map_to_tri() {
    mapping_case(3, 3);
}
#[test]
fn tri_map_to_quad() {
    mapping_case(3, 4);
}
#[test]
fn quad_map_to_bi() {
    mapping_case(4, 2);
}
#[test]
fn quad_map_to_tri() {
    mapping_case(4, 3);
}
#[test]
fn quad_map_to_quad() {
    mapping_case(4, 4);
}
fn finish_counts<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    s: Sources,
    counts: StreamPort<I, Vec<usize>>,
) -> Network {
    let joined = b
        .join_equal(&counts, s.extras.port(), |_| (), |_| ())
        .unwrap();
    let out = b
        .map(&joined, |(counts, extra)| {
            let mut fields: Vec<_> = counts.iter().map(|&n| Field::Long(n as i64)).collect();
            fields.push(Field::Extra(*extra));
            one(fields)
        })
        .unwrap();
    finish(b, s, out)
}
fn joined_counts(arity: usize) -> Network {
    let (mut b, s) = builder();
    match arity {
        1 => {
            let c = b
                .collect::<_, _, _, DistinctCountCollector<usize>>(s.entities.port(), |e| e.value)
                .unwrap();
            let out = b.map(&c, |n| vec![*n]).unwrap();
            finish_counts(b, s, out)
        }
        2 => {
            let c = b
                .collect::<_, _, _, (DistinctCountCollector<usize>, DistinctCountCollector<usize>)>(
                    s.entities.port(),
                    |e| (e.value, e.value),
                )
                .unwrap();
            let out = b.map(&c, |(a, c)| vec![*a, *c]).unwrap();
            finish_counts(b, s, out)
        }
        _ => {
            let c = b
                .collect::<_, _, _, (
                    DistinctCountCollector<usize>,
                    DistinctCountCollector<usize>,
                    DistinctCountCollector<usize>,
                )>(s.entities.port(), |e| (e.value, e.value, e.value))
                .unwrap();
            let out = b.map(&c, |(a, c, d)| vec![*a, *c, *d]).unwrap();
            finish_counts(b, s, out)
        }
    }
}
fn join_case(arity: usize) {
    let mut es = fixture(1, 1, 2);
    for e in &mut es {
        e.code = format!("MyEntity {}", e.id + 1);
    }
    let mut h = joined_counts(arity);
    load(&mut h, &es, 1, 2);
    let expected = |n| {
        (0..2)
            .map(|extra| {
                let mut fields = vec![Field::Long(n); arity];
                fields.push(Field::Extra(extra));
                one(fields)
            })
            .collect()
    };
    check(&h, expected(1));
    es[1].value = 1;
    put(&mut h, es[1].clone());
    check(&h, expected(2));
    remove(&mut h, 1);
    check(&h, expected(1));
}
#[test]
fn uni_join_after_group_by() {
    join_case(1);
}
#[test]
fn bi_join_after_group_by() {
    join_case(2);
}
#[test]
fn tri_join_after_group_by() {
    join_case(3);
}
fn filtered_grouping(shape: usize) -> Network {
    let (mut b, s) = builder();
    if shape == 3 {
        let grouped = b
            .group_collect_with::<_, _, _, _, CountCollector>(s.entities.port(), |e| {
                (e.integer, ())
            })
            .unwrap();
        let out = b
            .map(&grouped, |(v, n)| {
                matched(*n as i64, vec![Field::Integer(*v), Field::Long(*n as i64)])
            })
            .unwrap();
        return finish(b, s, out);
    }
    if shape == 1 {
        let grouped = b
            .group_collect_with::<_, _, _, _, CountCollector>(s.entities.port(), |e| (e.group, ()))
            .unwrap();
        let filtered = b.filter(&grouped, |(_, n)| *n > 1).unwrap();
        let out = b
            .map(&filtered, |(g, n)| {
                one(vec![Field::Group(*g), Field::Long(*n as i64)])
            })
            .unwrap();
        return finish(b, s, out);
    }
    let keys = b.map(s.entities.port(), |e| e.group).unwrap();
    let grouped = b.distinct(&keys).unwrap();
    if shape == 0 {
        let filtered = b.filter(&grouped, |g| *g == 1).unwrap();
        let out = b.map(&filtered, |g| one(vec![Field::Group(*g)])).unwrap();
        return finish(b, s, out);
    }
    let joined = b
        .join_equal(&grouped, s.entities.port(), |g| *g, |e| e.group)
        .unwrap();
    let filtered = b.filter(&joined, |(g, _)| *g == 1).unwrap();
    let out = b
        .map(&filtered, |(g, e)| {
            one(vec![Field::Group(*g), Field::Entity(e.id)])
        })
        .unwrap();
    finish(b, s, out)
}
fn group_case(shape: usize) {
    let mut es = fixture(5, 1, 7);
    for (id, group, integer) in [(7, 1, 1), (8, 1, 2), (9, 0, 3)] {
        es.push(self::Entity {
            id,
            group,
            integer,
            value: 0,
            code: format!("MyEntity {}", id - 6),
            long: 11,
        });
    }
    let mut h = filtered_grouping(shape);
    load(&mut h, &es, 2, 0);
    use Field::*;
    let expected = match shape {
        0 => vec![one(vec![Group(1)])],
        1 => vec![one(vec![Group(1), Long(2)]), one(vec![Group(0), Long(8)])],
        2 => vec![
            one(vec![Group(1), Entity(7)]),
            one(vec![Group(1), Entity(8)]),
        ],
        _ => vec![
            matched(8, vec![Integer(1), Long(8)]),
            matched(1, vec![Integer(2), Long(1)]),
            matched(1, vec![Integer(3), Long(1)]),
        ],
    };
    check(&h, expected.clone());
    if shape == 3 {
        remove(&mut h, 9);
        check(&h, expected[..2].to_vec());
    }
}
#[test]
fn group_by_1_mapping_0_collect_filtered() {
    group_case(0);
}
#[test]
fn group_by_1_mapping_1_collect_filtered() {
    group_case(1);
}
#[test]
fn group_by_joined_and_filtered() {
    group_case(2);
}
#[test]
fn group_by_1_mapping_1_collector_grouping_on_primitives() {
    group_case(3);
}
#[test]
fn uni_flatten_last_with_duplicates() {
    let (mut b, s) = builder();
    let first = Rc::new((0usize, 1i32));
    let duplicate = Rc::new((0usize, 1i32));
    let second = Rc::new((1usize, 2i32));
    assert!(!Rc::ptr_eq(&first, &duplicate));
    let (a, c) = (first.clone(), second.clone());
    let expanded = b
        .flat_map_canonical(
            s.entities.port(),
            move |e| {
                if e.value == 0 {
                    vec![a.clone(), duplicate.clone(), c.clone()]
                } else {
                    vec![a.clone(), c.clone()]
                }
            },
            |p| **p,
        )
        .unwrap();
    let filtered = b
        .filter(&expanded, move |p| {
            Rc::ptr_eq(p, &first) || Rc::ptr_eq(p, &second)
        })
        .unwrap();
    let out = b
        .map(&filtered, |p| one(vec![Field::Pair(p.0, p.1)]))
        .unwrap();
    let mut h = finish(b, s, out);
    let mut es = fixture(2, 2, 2);
    load(&mut h, &es, 2, 0);
    let first = one(vec![Field::Pair(0, 1)]);
    let second = one(vec![Field::Pair(1, 2)]);
    check(
        &h,
        vec![
            first.clone(),
            first.clone(),
            second.clone(),
            first.clone(),
            second.clone(),
        ],
    );
    es[0].value = 1;
    put(&mut h, es[0].clone());
    check(
        &h,
        vec![first.clone(), second.clone(), first.clone(), second.clone()],
    );
    remove(&mut h, 0);
    check(&h, vec![first, second]);
}
#[test]
fn bi_flatten_last_with_duplicates() {
    let (mut b, s) = builder();
    let pairs = b.unique_pairs(s.entities.port()).unwrap();
    let expanded = b
        .flat_map(&pairs, |(a, c)| {
            [c.group, 0, 1].map(|g| one(vec![Field::Entity(a.id), Field::Group(g)]))
        })
        .unwrap();
    let mut h = finish(b, s, expanded);
    load(&mut h, &fixture(1, 2, 3), 2, 0);
    let row = |e, g| one(vec![Field::Entity(e), Field::Group(g)]);
    // Preserve literal source lists. Its helper checks support in both directions
    // plus total penalty, and does not count each distinct justification's repeats.
    let source_check = |h: &Network, expected: Vec<Match>| {
        let actual = (h.read)(&h.graph);
        assert_eq!(actual.len(), expected.len());
        for e in &expected {
            assert!(actual.contains(e));
        }
        for a in &actual {
            assert!(expected.contains(a));
        }
        assert_eq!(h.graph.score(), SimpleScore::new(-(expected.len() as i64)));
    };
    source_check(
        &h,
        vec![
            row(0, 0),
            row(0, 0),
            row(0, 1),
            row(1, 1),
            row(1, 0),
            row(1, 1),
            row(0, 0),
            row(0, 0),
            row(0, 1),
        ],
    );
    // Additional exact native expansion evidence: 3/3 occurrences for entity0,
    // and 2/1 for entity1, as independently enumerated from the actual last field.
    check(
        &h,
        vec![
            row(0, 0),
            row(0, 0),
            row(0, 0),
            row(0, 1),
            row(0, 1),
            row(0, 1),
            row(1, 0),
            row(1, 0),
            row(1, 1),
        ],
    );
    remove(&mut h, 0);
    source_check(&h, vec![row(1, 1), row(1, 0), row(1, 1)]);
    check(&h, vec![row(1, 0), row(1, 0), row(1, 1)]);
}
