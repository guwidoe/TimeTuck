// Adapted from pinned Community AbstractAdvancedGroupByConstraintStreamTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::collections::{BTreeMap, BTreeSet};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
    code: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum Field {
    Entity(usize),
    Group(usize),
    Text(String),
    Number(i64),
    Pair(String, i64),
    Set(Vec<usize>),
    List(Vec<usize>),
    Map(Vec<(usize, i64)>),
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Match {
    fields: Vec<Field>,
    penalty: i64,
}
fn matched(penalty: i64, mut fields: Vec<Field>) -> Match {
    // Upstream compares justification membership without field ordering.
    // Sorting retains duplicates and strengthens the source containsAll check.
    fields.sort();
    Match { fields, penalty }
}
fn count_match(group: usize, count: usize) -> Match {
    matched(
        count as i64,
        vec![Field::Group(group), Field::Number(count as i64)],
    )
}
fn map_match(pairs: &[(usize, i64)]) -> Match {
    matched(1, vec![Field::Map(pairs.to_vec())])
}
#[derive(Clone, Copy, Debug)]
enum Collection {
    List,
    Map,
    Regroup,
    Double,
}
#[derive(Clone, Copy, Debug)]
enum Shape {
    DownFilter,
    CountFilter,
    SetSum,
    KeySet,
    Tuples(usize, Collection),
    ExistsAfter,
    ExistsBefore,
    PairExists,
    FilteredPairs,
    GroupJoinGroup,
    SharedJoin,
}
type Reader = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Vec<Match>;
struct Network {
    graph: StreamGraph<'static, SimpleScore>,
    entities: StreamSource<usize, Entity>,
    groups: StreamSource<usize, usize>,
    read: Box<Reader>,
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    entities: StreamSource<usize, Entity>,
    groups: StreamSource<usize, usize>,
    out: StreamPort<I, Match>,
) -> Network {
    let data = b.dataset(&out).unwrap();
    b.score("testConstraint", &out, |r| SimpleScore::new(-r.penalty))
        .unwrap();
    Network {
        graph: b.build(),
        entities,
        groups,
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
fn grouped_tuples<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    entities: StreamSource<usize, Entity>,
    groups: StreamSource<usize, usize>,
    rows: StreamPort<I, Vec<Entity>>,
    collection: Collection,
) -> Network {
    if matches!(collection, Collection::Regroup | Collection::Double) {
        let keys = b.map(&rows, |r| r[0].group).unwrap();
        let unique = b.distinct(&keys).unwrap();
        let counted = b
            .group_collect_with::<_, _, _, _, CountCollector>(&unique, |r| (*r, ()))
            .unwrap();
        if matches!(collection, Collection::Double) {
            let again = b
                .group_collect_with::<_, _, _, _, CountCollector>(&counted, |(g, _)| {
                    (format!("Generated EntityGroup {g}"), ())
                })
                .unwrap();
            let out = b
                .map(&again, |(name, n)| {
                    matched(
                        *n as i64,
                        vec![Field::Text(name.clone()), Field::Number(*n as i64)],
                    )
                })
                .unwrap();
            return finish(b, entities, groups, out);
        }
        let out = b.map(&counted, |(g, n)| count_match(*g, *n)).unwrap();
        return finish(b, entities, groups, out);
    }
    let counts = b
        .group_collect_with::<_, _, _, _, CountCollector>(&rows, |r| (r[0].group, ()))
        .unwrap();
    if matches!(collection, Collection::List) {
        let list = b.collect_list(&counts, |(g, _)| *g).unwrap();
        let out = b
            .map(&list, |l| {
                matched(1, vec![Field::List(l.iter().copied().collect())])
            })
            .unwrap();
        return finish(b, entities, groups, out);
    }
    let map = b
        .collect::<_, _, _, MapCollector<usize, i64, IntCollector>>(&counts, |(g, n)| {
            (*g, *n as i64)
        })
        .unwrap();
    let out = b
        .map(&map, |m| {
            matched(
                1,
                vec![Field::Map(m.iter().map(|(&g, s)| (g, s.sum)).collect())],
            )
        })
        .unwrap();
    finish(b, entities, groups, out)
}
fn build(shape: Shape, prune: bool) -> Network {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    if prune {
        b = b.with_activity_pruning();
    }
    let entities = b.source::<usize, Entity>();
    let groups = b.source::<usize, usize>();
    match shape {
        Shape::DownFilter => {
            let counts = b
                .group_collect_with::<_, _, _, _, CountCollector>(entities.port(), |r| {
                    (r.code[0..1].to_owned(), ())
                })
                .unwrap();
            let pairs = b.distinct(&counts).unwrap();
            let filtered = b.filter(&pairs, |(k, _)| k != "G").unwrap();
            let out = b
                .map(&filtered, |(k, n)| {
                    matched(*n as i64, vec![Field::Pair(k.clone(), *n as i64)])
                })
                .unwrap();
            finish(b, entities, groups, out)
        }
        Shape::CountFilter => {
            let count = b
                .collect::<_, _, _, CountCollector>(entities.port(), |_| ())
                .unwrap();
            let filtered = b.filter(&count, |n| *n == 10).unwrap();
            let out = b
                .map(&filtered, |n| {
                    matched(*n as i64, vec![Field::Number(*n as i64)])
                })
                .unwrap();
            finish(b, entities, groups, out)
        }
        Shape::SetSum => {
            let set = b
                .collect::<_, _, _, SetCollector<usize>>(entities.port(), |e| e.id)
                .unwrap();
            let sum = b
                .collect::<_, _, _, IntCollector>(&set, |s| s.len() as i64)
                .unwrap();
            let out = b
                .map(&sum, |s| matched(s.sum, vec![Field::Number(s.sum)]))
                .unwrap();
            finish(b, entities, groups, out)
        }
        Shape::KeySet => {
            let keys = b.map(entities.port(), |e| e.group).unwrap();
            let unique = b.distinct(&keys).unwrap();
            let set = b
                .collect::<_, _, _, SetCollector<usize>>(&unique, |g| *g)
                .unwrap();
            let out = b
                .map(&set, |s| {
                    matched(
                        s.len() as i64,
                        vec![Field::Set(s.keys().copied().collect())],
                    )
                })
                .unwrap();
            finish(b, entities, groups, out)
        }
        Shape::Tuples(arity, collection) => {
            let pairs = b.unique_pairs_equal(entities.port(), |e| e.group).unwrap();
            let pairrows = b.map(&pairs, |(a, c)| vec![a.clone(), c.clone()]).unwrap();
            if arity == 2 {
                return grouped_tuples(b, entities, groups, pairrows, collection);
            }
            let joined = b
                .join_equal(&pairrows, entities.port(), |r| r[0].group, |e| e.group)
                .unwrap();
            let filtered = b
                .filter(&joined, |(r, e)| r.iter().all(|v| v.id != e.id))
                .unwrap();
            let tri = b
                .map(&filtered, |(r, e)| {
                    let mut r = r.clone();
                    r.push(e.clone());
                    r
                })
                .unwrap();
            if arity == 3 {
                return grouped_tuples(b, entities, groups, tri, collection);
            }
            let joined = b
                .join_equal(&tri, entities.port(), |r| r[0].group, |e| e.group)
                .unwrap();
            let filtered = b
                .filter(&joined, |(r, e)| r.iter().all(|v| v.id != e.id))
                .unwrap();
            let quad = b
                .map(&filtered, |(r, e)| {
                    let mut r = r.clone();
                    r.push(e.clone());
                    r
                })
                .unwrap();
            grouped_tuples(b, entities, groups, quad, collection)
        }
        Shape::ExistsAfter => {
            let counts = b
                .group_collect_with::<_, _, _, _, CountCollector>(entities.port(), |e| {
                    (e.group, ())
                })
                .unwrap();
            let visible = b
                .existence_equal(
                    &counts,
                    groups.port(),
                    ExistenceMode::Exists,
                    |(g, _)| *g,
                    |g| *g,
                )
                .unwrap();
            let out = b.map(&visible, |(g, n)| count_match(*g, *n)).unwrap();
            finish(b, entities, groups, out)
        }
        Shape::ExistsBefore => {
            let visible = b
                .existence_equal(
                    entities.port(),
                    groups.port(),
                    ExistenceMode::Exists,
                    |e| e.group,
                    |g| *g,
                )
                .unwrap();
            let counts = b
                .group_collect_with::<_, _, _, _, CountCollector>(&visible, |e| (e.group, ()))
                .unwrap();
            let out = b.map(&counts, |(g, n)| count_match(*g, *n)).unwrap();
            finish(b, entities, groups, out)
        }
        Shape::PairExists => {
            let pairs = b.unique_pairs(entities.port()).unwrap();
            let visible = b
                .existence_equal(
                    &pairs,
                    groups.port(),
                    ExistenceMode::Exists,
                    |(a, _)| a.group,
                    |g| *g,
                )
                .unwrap();
            let counts = b
                .group_collect_with::<_, _, _, _, CountCollector>(&visible, |(a, _)| (a.group, ()))
                .unwrap();
            let out = b.map(&counts, |(g, n)| count_match(*g, *n)).unwrap();
            finish(b, entities, groups, out)
        }
        Shape::FilteredPairs => {
            let pairs = b.unique_pairs_equal(entities.port(), |e| e.group).unwrap();
            let selected = b.filter(&pairs, |(a, _)| !a.code.contains("My")).unwrap();
            let out = b
                .map(&selected, |(a, c)| {
                    matched(1, vec![Field::Entity(a.id), Field::Entity(c.id)])
                })
                .unwrap();
            finish(b, entities, groups, out)
        }
        Shape::GroupJoinGroup => {
            let keys = b.map(entities.port(), |e| (e.group, e.value)).unwrap();
            let unique = b.distinct(&keys).unwrap();
            let joined = b
                .join_equal(&unique, entities.port(), |_| (), |_| ())
                .unwrap();
            let summed = b
                .group_collect_with::<_, _, _, _, IntCollector>(&joined, |((g, _), e)| {
                    ((*g, e.id), 1)
                })
                .unwrap();
            let out = b
                .map(&summed, |((g, id), s)| {
                    matched(
                        1,
                        vec![Field::Group(*g), Field::Entity(*id), Field::Number(s.sum)],
                    )
                })
                .unwrap();
            finish(b, entities, groups, out)
        }
        Shape::SharedJoin => {
            let shared = b.filter(entities.port(), |e| e.group != 0).unwrap();
            let pairs = b
                .join_equal(&shared, &shared, |e| e.group, |e| e.group)
                .unwrap();
            let out = b
                .map(&pairs, |(a, c)| {
                    matched(1, vec![Field::Entity(a.id), Field::Entity(c.id)])
                })
                .unwrap();
            finish(b, entities, groups, out)
        }
    }
}
fn fixture(values: usize, groups: usize, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|id| Entity {
            id,
            group: id % groups,
            value: id % values,
            code: format!("Generated Entity {id}"),
        })
        .collect()
}
fn custom(id: usize, group: usize, number: usize) -> Entity {
    Entity {
        id,
        group,
        value: 0,
        code: format!("MyEntity {number}"),
    }
}
fn load(h: &mut Network, entities: &[Entity], groups: &[usize]) {
    let mut tx = h.graph.transaction();
    for &g in groups {
        tx.apply(&h.groups, TupleChange::put(g, g)).unwrap();
    }
    for e in entities {
        tx.apply(&h.entities, TupleChange::put(e.id, e.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
}
fn check(graph: &StreamGraph<'_, SimpleScore>, read: &Reader, mut expected: Vec<Match>) {
    expected.sort();
    assert_eq!(read(graph), expected);
    assert_eq!(
        graph.score(),
        SimpleScore::new(-expected.iter().map(|r| r.penalty).sum::<i64>())
    );
}
fn assert_rows(h: &Network, expected: Vec<Match>) {
    check(&h.graph, &h.read, expected);
}
fn remove(h: &mut Network, id: usize) {
    let mut tx = h.graph.transaction();
    tx.apply(&h.entities, TupleChange::retract(id)).unwrap();
    tx.commit().unwrap();
}
fn pair_case(arity: usize, collection: Collection) {
    let count = if matches!(collection, Collection::List | Collection::Map) && arity == 2 {
        5
    } else {
        arity * 2
    };
    let es = fixture(
        if matches!(collection, Collection::List | Collection::Map) {
            3
        } else {
            2
        },
        2,
        count,
    );
    let mut h = build(Shape::Tuples(arity, collection), false);
    load(&mut h, &es, &[0, 1]);
    let count = if arity == 4 { 12 } else { 3 };
    let expected = match collection {
        Collection::List => vec![matched(1, vec![Field::List(vec![0, 1])])],
        Collection::Map => vec![map_match(&[
            (0, count),
            (1, if arity == 2 { 1 } else { count }),
        ])],
        Collection::Regroup => vec![count_match(0, 1), count_match(1, 1)],
        Collection::Double => (0..2)
            .map(|g| {
                matched(
                    1,
                    vec![
                        Field::Text(format!("Generated EntityGroup {g}")),
                        Field::Number(1),
                    ],
                )
            })
            .collect(),
    };
    assert_rows(&h, expected);
    remove(&mut h, 0);
    let expected = match collection {
        Collection::List => vec![matched(1, vec![Field::List(vec![0, 1])])],
        Collection::Map => vec![if arity == 2 {
            map_match(&[(0, 1), (1, 1)])
        } else {
            map_match(&[(1, count)])
        }],
        Collection::Regroup => vec![count_match(1, 1)],
        Collection::Double => vec![matched(
            1,
            vec![
                Field::Text("Generated EntityGroup 1".into()),
                Field::Number(1),
            ],
        )],
    };
    assert_rows(&h, expected);
    h.graph.assert_consistent().unwrap();
}
#[test]
fn bi_group_by_recollected_to_list() {
    pair_case(2, Collection::List);
}
#[test]
fn bi_group_by_recollected_to_map() {
    pair_case(2, Collection::Map);
}
#[test]
fn tri_group_by_recollected() {
    pair_case(3, Collection::Map);
}
#[test]
fn quad_group_by_recollected() {
    pair_case(4, Collection::Map);
}
#[test]
fn bi_group_by_regrouped() {
    pair_case(2, Collection::Regroup);
}
#[test]
fn tri_group_by_regrouped() {
    pair_case(3, Collection::Regroup);
}
#[test]
fn quad_group_by_regrouped() {
    pair_case(4, Collection::Regroup);
}
#[test]
fn bi_group_by_regrouped_double() {
    pair_case(2, Collection::Double);
}
#[test]
fn tri_group_by_regrouped_double() {
    pair_case(3, Collection::Double);
}
#[test]
fn quad_group_by_regrouped_double() {
    pair_case(4, Collection::Double);
}
#[test]
fn collected_downgraded_and_filtered() {
    let mut es = fixture(5, 1, 7);
    es.push(custom(7, 0, 1));
    let mut h = build(Shape::DownFilter, false);
    load(&mut h, &es, &[0]);
    assert_rows(&h, vec![matched(1, vec![Field::Pair("M".into(), 1)])]);
    remove(&mut h, 7);
    assert_rows(&h, vec![]);
}
#[test]
fn collected_and_filtered() {
    let mut es = fixture(5, 1, 7);
    es.extend([custom(7, 1, 1), custom(8, 1, 2), custom(9, 0, 3)]);
    let mut h = build(Shape::CountFilter, false);
    load(&mut h, &es, &[0, 1]);
    assert_rows(&h, vec![matched(10, vec![Field::Number(10)])]);
    remove(&mut h, 7);
    remove(&mut h, 8);
    assert_rows(&h, vec![]);
}
#[test]
fn collected_filtered_recollected() {
    let mut h = build(Shape::SetSum, false);
    load(&mut h, &fixture(2, 2, 2), &[0, 1]);
    assert_rows(&h, vec![matched(2, vec![Field::Number(2)])]);
    remove(&mut h, 0);
    assert_rows(&h, vec![matched(1, vec![Field::Number(1)])]);
}
#[test]
fn uni_group_by_recollected() {
    let mut h = build(Shape::KeySet, false);
    load(&mut h, &fixture(2, 2, 2), &[0, 1]);
    assert_rows(&h, vec![matched(2, vec![Field::Set(vec![0, 1])])]);
    remove(&mut h, 0);
    assert_rows(&h, vec![matched(1, vec![Field::Set(vec![1])])]);
}
fn existence_case(shape: Shape) {
    let mut es = fixture(1, 1, 1);
    es.extend([custom(1, 1, 1), custom(2, 1, 2), custom(3, 0, 3)]);
    let mut h = build(shape, false);
    load(&mut h, &es, &[0, 1]);
    let n = if matches!(shape, Shape::PairExists) {
        3
    } else {
        2
    };
    assert_rows(&h, vec![count_match(0, n), count_match(1, n)]);
    let mut tx = h.graph.transaction();
    tx.apply(&h.groups, TupleChange::retract(1)).unwrap();
    tx.commit().unwrap();
    assert_rows(&h, vec![count_match(0, n)]);
}
#[test]
fn exists_after_group_by() {
    existence_case(Shape::ExistsAfter);
}
#[test]
fn group_by_after_exists() {
    existence_case(Shape::ExistsBefore);
}
#[test]
fn group_by_after_exists_bi() {
    existence_case(Shape::PairExists);
}
#[test]
fn filtered_from_unique_pair() {
    let mut es = fixture(1, 1, 1);
    es.extend([custom(1, 1, 1), custom(2, 1, 2), custom(3, 0, 3)]);
    let mut h = build(Shape::FilteredPairs, false);
    load(&mut h, &es, &[0, 1]);
    // Source explicitly expects (MyEntity 3, Generated Entity 0), while its
    // justification helper ignores field order. Preserve that expectation.
    assert_rows(
        &h,
        vec![matched(1, vec![Field::Entity(3), Field::Entity(0)])],
    );
    remove(&mut h, 3);
    assert_rows(&h, vec![]);
}
#[test]
fn group_by_then_join_then_group_by() {
    let _ = build(Shape::GroupJoinGroup, false);
}
#[test]
fn reused_streams_in_join() {
    let mut h = build(Shape::SharedJoin, false);
    load(&mut h, &fixture(1, 2, 4), &[0, 1]);
    assert_rows(
        &h,
        vec![
            matched(1, vec![Field::Entity(1), Field::Entity(1)]),
            matched(1, vec![Field::Entity(1), Field::Entity(3)]),
            matched(1, vec![Field::Entity(3), Field::Entity(1)]),
            matched(1, vec![Field::Entity(3), Field::Entity(3)]),
        ],
    );
}

// Original regression: expected results use direct sets/counts and combination
// counts, without rebuilding the tested pipeline or reading its intermediate data.
fn expected(
    shape: Shape,
    entities: &BTreeMap<usize, Entity>,
    groups: &BTreeSet<usize>,
) -> Vec<Match> {
    let mut counts = BTreeMap::<usize, usize>::new();
    for e in entities.values() {
        *counts.entry(e.group).or_default() += 1;
    }
    match shape {
        Shape::DownFilter => {
            let mut prefixes = BTreeMap::<String, usize>::new();
            for e in entities.values() {
                *prefixes.entry(e.code[0..1].to_owned()).or_default() += 1;
            }
            prefixes
                .into_iter()
                .filter(|(p, _)| p != "G")
                .map(|(p, n)| matched(n as i64, vec![Field::Pair(p, n as i64)]))
                .collect()
        }
        Shape::CountFilter => {
            if entities.len() == 10 {
                vec![matched(10, vec![Field::Number(10)])]
            } else {
                vec![]
            }
        }
        Shape::SetSum => {
            if entities.is_empty() {
                vec![]
            } else {
                vec![matched(
                    entities.len() as i64,
                    vec![Field::Number(entities.len() as i64)],
                )]
            }
        }
        Shape::KeySet => {
            if counts.is_empty() {
                vec![]
            } else {
                vec![matched(
                    counts.len() as i64,
                    vec![Field::Set(counts.keys().copied().collect())],
                )]
            }
        }
        Shape::Tuples(arity, collection) => {
            let counts: Vec<_> = counts
                .into_iter()
                .filter(|(_, n)| *n >= arity)
                .map(|(g, n)| {
                    let mut count = n * (n - 1) / 2;
                    for offset in 2..arity {
                        count *= n - offset;
                    }
                    (g, count as i64)
                })
                .collect();
            match collection {
                Collection::List => {
                    if counts.is_empty() {
                        vec![]
                    } else {
                        vec![matched(
                            1,
                            vec![Field::List(counts.iter().map(|&(g, _)| g).collect())],
                        )]
                    }
                }
                Collection::Map => {
                    if counts.is_empty() {
                        vec![]
                    } else {
                        vec![map_match(&counts)]
                    }
                }
                Collection::Regroup => counts.iter().map(|&(g, _)| count_match(g, 1)).collect(),
                Collection::Double => counts
                    .iter()
                    .map(|&(g, _)| {
                        matched(
                            1,
                            vec![
                                Field::Text(format!("Generated EntityGroup {g}")),
                                Field::Number(1),
                            ],
                        )
                    })
                    .collect(),
            }
        }
        Shape::ExistsAfter | Shape::ExistsBefore => counts
            .into_iter()
            .filter(|(g, _)| groups.contains(g))
            .map(|(g, n)| count_match(g, n))
            .collect(),
        Shape::PairExists => {
            let mut counts = BTreeMap::<usize, usize>::new();
            for a in entities.values() {
                for b in entities.values() {
                    if a.id < b.id && groups.contains(&a.group) {
                        *counts.entry(a.group).or_default() += 1;
                    }
                }
            }
            counts.into_iter().map(|(g, n)| count_match(g, n)).collect()
        }
        Shape::FilteredPairs => {
            let mut out = vec![];
            for a in entities.values() {
                for b in entities.values() {
                    if a.id < b.id && a.group == b.group && !a.code.contains("My") {
                        out.push(matched(1, vec![Field::Entity(a.id), Field::Entity(b.id)]));
                    }
                }
            }
            out
        }
        Shape::GroupJoinGroup => {
            let mut keys = BTreeMap::<usize, BTreeSet<usize>>::new();
            for e in entities.values() {
                keys.entry(e.group).or_default().insert(e.value);
            }
            let mut out = vec![];
            for (g, vs) in keys {
                for e in entities.values() {
                    out.push(matched(
                        1,
                        vec![
                            Field::Group(g),
                            Field::Entity(e.id),
                            Field::Number(vs.len() as i64),
                        ],
                    ));
                }
            }
            out
        }
        Shape::SharedJoin => {
            let mut out = vec![];
            for a in entities.values() {
                for b in entities.values() {
                    if a.group != 0 && a.group == b.group {
                        out.push(matched(1, vec![Field::Entity(a.id), Field::Entity(b.id)]));
                    }
                }
            }
            out
        }
    }
}
#[test]
fn nested_group_updates_and_rollback_match_independent_results() {
    let mut shapes = vec![
        Shape::DownFilter,
        Shape::CountFilter,
        Shape::SetSum,
        Shape::KeySet,
        Shape::ExistsAfter,
        Shape::ExistsBefore,
        Shape::PairExists,
        Shape::FilteredPairs,
        Shape::GroupJoinGroup,
        Shape::SharedJoin,
        Shape::Tuples(2, Collection::List),
    ];
    for arity in 2..=4 {
        for collection in [Collection::Map, Collection::Regroup, Collection::Double] {
            shapes.push(Shape::Tuples(arity, collection));
        }
    }
    assert_eq!(shapes.len(), 20);
    for shape in shapes {
        for prune in [false, true] {
            let initial = fixture(3, 3, 10);
            let mut entities: BTreeMap<_, _> = initial.iter().map(|e| (e.id, e.clone())).collect();
            let mut groups: BTreeSet<_> = (0..3).collect();
            let mut h = build(shape, prune);
            load(&mut h, &initial, &[0, 1, 2]);
            assert_rows(&h, expected(shape, &entities, &groups));
            for step in 0..90 {
                let mut next = entities.clone();
                let mut next_groups = groups.clone();
                let mut tx = h.graph.transaction();
                if step % 30 == 0 {
                    for id in next.keys() {
                        tx.apply(&h.entities, TupleChange::retract(*id)).unwrap();
                    }
                    next.clear();
                } else if step % 5 == 0 {
                    let id = (step / 5) % 3;
                    if next_groups.remove(&id) {
                        tx.apply(&h.groups, TupleChange::retract(id)).unwrap();
                    } else {
                        next_groups.insert(id);
                        tx.apply(&h.groups, TupleChange::put(id, id)).unwrap();
                    }
                } else {
                    let id = step % 12;
                    if step % 4 == 0 {
                        next.remove(&id);
                        tx.apply(&h.entities, TupleChange::retract(id)).unwrap();
                    } else {
                        let e = Entity {
                            id,
                            group: (step / 3) % 3,
                            value: (step / 7) % 4,
                            code: if step % 2 == 0 {
                                format!("MyEntity {id}")
                            } else {
                                format!("Generated Entity {id}")
                            },
                        };
                        next.insert(id, e.clone());
                        tx.apply(&h.entities, TupleChange::put(id, e)).unwrap();
                    }
                }
                check(
                    tx.graph().unwrap(),
                    &h.read,
                    expected(shape, &next, &next_groups),
                );
                if step % 7 == 0 {
                    drop(tx);
                } else {
                    tx.commit().unwrap();
                    entities = next;
                    groups = next_groups;
                }
                assert_rows(&h, expected(shape, &entities, &groups));
            }
            h.graph.assert_consistent().unwrap();
        }
    }
}
