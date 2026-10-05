// Adapted from Timefold Community 2.7.0 AbstractUni/BiConstraintStreamTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::fmt::Debug;
use timetuck::*;
#[derive(Clone)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
    code: String,
    long: i64,
}
fn generated(values: usize, groups: usize, count: usize) -> Vec<Entity> {
    (0..count)
        .map(|id| Entity {
            id,
            group: id % groups,
            value: id % values,
            code: format!("Generated Entity {id}"),
            long: 0,
        })
        .collect()
}
fn extra(id: usize, group: usize, value: usize, number: usize) -> Entity {
    Entity {
        id,
        group,
        value,
        code: format!("MyEntity {number}"),
        long: 0,
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Field {
    Group(usize),
    Value(usize),
    Entity(usize),
    Code(String),
    Number(i64),
    Set(Vec<usize>),
}
use Field::*;
type Expected = Vec<(Vec<Field>, i64)>;
type Step = (Vec<TupleChange<usize, Entity>>, Expected);
fn unit(rows: Vec<Vec<Field>>) -> Expected {
    rows.into_iter().map(|row| (row, -1)).collect()
}
fn code(id: usize) -> Field {
    Code(format!("Generated Entity {id}"))
}
fn set(s: &SnapshotMap<usize, ()>) -> Field {
    Set(s.keys().copied().collect())
}
fn group<I, T, K, V, C>(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    input: &StreamPort<I, T>,
    mapping: impl Fn(&T) -> (K, V) + 'static,
    output: impl Fn(&(K, C::Output)) -> Vec<Field> + 'static,
) -> StreamPort<K, Vec<Field>>
where
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    K: Ord + Clone + 'static,
    V: Clone + 'static,
    C: ReversibleCollector<V> + 'static,
    C::Output: 'static,
{
    let grouped = b
        .group_collect_with::<I, T, K, V, C>(input, mapping)
        .unwrap();
    b.map(&grouped, output).unwrap()
}
fn run<K: Ord + Clone + Debug + 'static>(
    rows: Vec<Entity>,
    steps: Vec<Step>,
    build: impl FnOnce(
        &mut StreamGraphBuilder<'static, SimpleScore>,
        &StreamSource<usize, Entity>,
    ) -> StreamPort<K, Vec<Field>>,
    weight: fn(&[Field]) -> i64,
) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let output = build(&mut b, &source);
    let terminal = b
        .score("testConstraint", &output, move |row| {
            SimpleScore::new(weight(row))
        })
        .unwrap();
    let mut g = b.build();
    let mut tx = g.transaction();
    for row in rows {
        tx.apply(&source, TupleChange::put(row.id, row)).unwrap();
    }
    tx.commit().unwrap();
    for (changes, mut expected) in steps {
        let mut tx = g.transaction();
        for change in changes {
            tx.apply(&source, change).unwrap();
        }
        tx.commit().unwrap();
        let mut actual: Expected = g
            .constraint_matches(&terminal)
            .unwrap()
            .values()
            .map(|(row, score)| (row.clone(), score.score()))
            .collect();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
        let total = SimpleScore::new(expected.iter().map(|(_, score)| score).sum());
        assert_eq!(g.score(), total);
        assert_eq!(
            g.constraint_scores().collect::<Vec<_>>(),
            vec![("testConstraint", total)]
        );
    }
}
fn one(_: &[Field]) -> i64 {
    -1
}
fn number(row: &[Field], index: usize) -> i64 {
    let Number(n) = row[index] else {
        panic!("expected number")
    };
    n
}
#[test]
fn uni_group_by_1_mapping_2_collector() {
    run(
        generated(1, 2, 3),
        vec![
            (
                vec![],
                unit(vec![
                    vec![Group(0), Number(2), Set(vec![0, 2])],
                    vec![Group(1), Number(1), Set(vec![1])],
                ]),
            ),
            (
                vec![TupleChange::retract(0)],
                unit(vec![
                    vec![Group(0), Number(1), Set(vec![2])],
                    vec![Group(1), Number(1), Set(vec![1])],
                ]),
            ),
        ],
        |b, s| {
            group::<_, _, _, _, (CountCollector, SetCollector<usize>)>(
                b,
                s.port(),
                |e| (e.group, ((), e.id)),
                |(g, (count, ids))| vec![Group(*g), Number(*count as i64), set(ids)],
            )
        },
        one,
    );
}
#[test]
fn uni_group_by_1_mapping_3_collector() {
    run(
        generated(1, 2, 3),
        vec![
            (
                vec![],
                unit(vec![
                    vec![Group(0), Number(2), Number(2), Set(vec![0, 2])],
                    vec![Group(1), Number(1), Number(1), Set(vec![1])],
                ]),
            ),
            (
                vec![TupleChange::retract(0)],
                unit(vec![
                    vec![Group(0), Number(1), Number(1), Set(vec![2])],
                    vec![Group(1), Number(1), Number(1), Set(vec![1])],
                ]),
            ),
        ],
        |b, s| {
            group::<
                _,
                _,
                _,
                _,
                (
                    CountCollector,
                    DistinctCountCollector<usize>,
                    SetCollector<usize>,
                ),
            >(
                b,
                s.port(),
                |e| (e.group, ((), e.id, e.id)),
                |(g, (count, distinct, ids))| {
                    vec![
                        Group(*g),
                        Number(*count as i64),
                        Number(*distinct as i64),
                        set(ids),
                    ]
                },
            )
        },
        one,
    );
}
#[test]
fn uni_group_by_2_mapping_0_collector() {
    let mut rows = generated(5, 1, 7);
    rows.extend([extra(7, 1, 0, 1), extra(8, 1, 0, 2), extra(9, 1, 1, 3)]);
    let mut after = (0..5).map(|v| vec![Group(0), Value(v)]).collect::<Vec<_>>();
    after.push(vec![Group(1), Value(0)]);
    let mut before = after.clone();
    before.push(vec![Group(1), Value(1)]);
    run(
        rows,
        vec![
            (vec![], unit(before)),
            (vec![TupleChange::retract(9)], unit(after.clone())),
            (vec![TupleChange::retract(8)], unit(after)),
        ],
        |b, s| {
            group::<_, _, _, _, CountCollector>(
                b,
                s.port(),
                |e| ((e.group, e.value), ()),
                |((g, v), _)| vec![Group(*g), Value(*v)],
            )
        },
        one,
    );
}
fn two_key_entities() -> Vec<Entity> {
    let mut rows = generated(1, 1, 7);
    rows.extend([extra(7, 1, 1, 1), extra(8, 1, 0, 2), extra(9, 1, 1, 3)]);
    rows
}
#[test]
fn uni_group_by_2_mapping_1_collector() {
    run(
        two_key_entities(),
        vec![
            (
                vec![],
                vec![
                    (vec![Group(0), Value(0), Number(7)], -7),
                    (vec![Group(1), Value(1), Number(2)], -2),
                    (vec![Group(1), Value(0), Number(1)], -1),
                ],
            ),
            (
                vec![TupleChange::put(8, extra(8, 0, 0, 2))],
                vec![
                    (vec![Group(0), Value(0), Number(8)], -8),
                    (vec![Group(1), Value(1), Number(2)], -2),
                ],
            ),
        ],
        |b, s| {
            group::<_, _, _, _, CountCollector>(
                b,
                s.port(),
                |e| ((e.group, e.value), ()),
                |((g, v), n)| vec![Group(*g), Value(*v), Number(*n as i64)],
            )
        },
        |row| -number(row, 2),
    );
}
#[test]
fn uni_group_by_2_mapping_2_collector() {
    run(
        two_key_entities(),
        vec![
            (
                vec![],
                vec![
                    (vec![Group(0), Value(0), Number(7), Number(7)], -7),
                    (vec![Group(1), Value(1), Number(2), Number(2)], -2),
                    (vec![Group(1), Value(0), Number(1), Number(1)], -1),
                ],
            ),
            (
                vec![TupleChange::put(8, extra(8, 0, 0, 2))],
                vec![
                    (vec![Group(0), Value(0), Number(8), Number(8)], -8),
                    (vec![Group(1), Value(1), Number(2), Number(2)], -2),
                ],
            ),
        ],
        |b, s| {
            group::<_, _, _, _, (CountCollector, CountCollector)>(
                b,
                s.port(),
                |e| ((e.group, e.value), ((), ())),
                |((g, v), (a, b))| vec![Group(*g), Value(*v), Number(*a as i64), Number(*b as i64)],
            )
        },
        |row| -number(row, 2),
    );
}
fn three_key_expected(with_set: bool) -> Vec<Vec<Field>> {
    [(0, 0, 0), (1, 1, 1), (2, 0, 2), (3, 1, 0), (4, 0, 1)]
        .into_iter()
        .map(|(id, g, v)| {
            let mut r = vec![Group(g), Value(v), code(id)];
            if with_set {
                r.push(Set(vec![id]));
            }
            r
        })
        .collect()
}
#[test]
fn uni_group_by_3_mapping_0_collector() {
    let rows = three_key_expected(false);
    run(
        generated(3, 2, 5),
        vec![
            (vec![], unit(rows.clone())),
            (vec![TupleChange::retract(0)], unit(rows[1..].to_vec())),
        ],
        |b, s| {
            group::<_, _, _, _, CountCollector>(
                b,
                s.port(),
                |e| ((e.group, e.value, e.code.clone()), ()),
                |((g, v, c), _)| vec![Group(*g), Value(*v), Code(c.clone())],
            )
        },
        one,
    );
}
#[test]
fn uni_group_by_3_mapping_1_collector() {
    let rows = three_key_expected(true);
    run(
        generated(3, 2, 5),
        vec![
            (vec![], unit(rows.clone())),
            (vec![TupleChange::retract(0)], unit(rows[1..].to_vec())),
        ],
        |b, s| {
            group::<_, _, _, _, SetCollector<usize>>(
                b,
                s.port(),
                |e| ((e.group, e.value, e.code.clone()), e.id),
                |((g, v, c), ids)| vec![Group(*g), Value(*v), Code(c.clone()), set(ids)],
            )
        },
        one,
    );
}
#[test]
fn uni_group_by_4_mapping_0_collector() {
    let rows = three_key_expected(false)
        .into_iter()
        .enumerate()
        .map(|(id, mut r)| {
            r.insert(0, Entity(id));
            r
        })
        .collect::<Vec<_>>();
    run(
        generated(3, 2, 5),
        vec![
            (vec![], unit(rows.clone())),
            (vec![TupleChange::retract(0)], unit(rows[1..].to_vec())),
        ],
        |b, s| {
            group::<_, _, _, _, CountCollector>(
                b,
                s.port(),
                |e| ((e.id, e.group, e.value, e.code.clone()), ()),
                |((id, g, v, c), _)| vec![Entity(*id), Group(*g), Value(*v), Code(c.clone())],
            )
        },
        one,
    );
}
#[test]
fn bi_group_by_1_mapping_0_collector() {
    let mut rows = generated(5, 1, 7);
    rows.extend([extra(7, 1, 0, 1), extra(8, 1, 0, 2), extra(9, 0, 0, 3)]);
    run(
        rows,
        vec![
            (vec![], unit(vec![vec![Group(0)], vec![Group(1)]])),
            (
                vec![TupleChange::retract(7), TupleChange::retract(8)],
                unit(vec![vec![Group(0)]]),
            ),
        ],
        |b, s| {
            let p = b.unique_pairs_equal(s.port(), |e| e.group).unwrap();
            group::<_, _, _, _, CountCollector>(
                b,
                &p,
                |(a, _)| (a.group, ()),
                |(g, _)| vec![Group(*g)],
            )
        },
        one,
    );
}
#[test]
fn bi_group_by_1_mapping_1_collector() {
    run(
        generated(5, 3, 7),
        vec![
            (
                vec![],
                unit(vec![vec![code(0), Number(6)], vec![code(1), Number(5)]]),
            ),
            (
                vec![TupleChange::retract(0)],
                unit(vec![vec![code(1), Number(5)]]),
            ),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            let g = group::<_, _, _, _, CountCollector>(
                b,
                &p,
                |(a, _)| (a.code.clone(), ()),
                |(c, n)| vec![Code(c.clone()), Number(*n as i64)],
            );
            b.filter(&g, |r| number(r, 1) > 4).unwrap()
        },
        one,
    );
}
#[test]
fn bi_group_by_1_mapping_2_collector() {
    run(
        generated(1, 2, 3),
        vec![
            (
                vec![],
                unit(vec![
                    vec![code(0), Number(2), Set(vec![0])],
                    vec![code(1), Number(1), Set(vec![1])],
                ]),
            ),
            (
                vec![TupleChange::retract(0)],
                unit(vec![vec![code(1), Number(1), Set(vec![1])]]),
            ),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, (CountCollector, SetCollector<usize>)>(
                b,
                &p,
                |(a, _)| (a.code.clone(), ((), a.id)),
                |(c, (n, ids))| vec![Code(c.clone()), Number(*n as i64), set(ids)],
            )
        },
        one,
    );
}
#[test]
fn bi_group_by_1_mapping_3_collector() {
    let mut rows = generated(1, 2, 3);
    rows[0].long = i64::MAX;
    rows[1].long = i64::MIN;
    run(
        rows,
        vec![
            (
                vec![],
                unit(vec![
                    vec![code(0), Number(i64::MAX), Number(i64::MAX), Set(vec![0])],
                    vec![code(1), Number(i64::MIN), Number(i64::MIN), Set(vec![1])],
                ]),
            ),
            (
                vec![TupleChange::retract(0)],
                unit(vec![vec![
                    code(1),
                    Number(i64::MIN),
                    Number(i64::MIN),
                    Set(vec![1]),
                ]]),
            ),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, (IntCollector, IntCollector, SetCollector<usize>)>(
                b,
                &p,
                |(a, _)| (a.code.clone(), (a.long, a.long, a.id)),
                |(c, (min, max, ids))| {
                    vec![
                        Code(c.clone()),
                        Number(min.min.unwrap()),
                        Number(max.max.unwrap()),
                        set(ids),
                    ]
                },
            )
        },
        one,
    );
}
#[test]
fn bi_group_by_2_mapping_0_collector() {
    run(
        generated(5, 3, 3),
        vec![
            (
                vec![],
                unit(vec![
                    vec![Group(0), Group(1)],
                    vec![Group(0), Group(2)],
                    vec![Group(1), Group(2)],
                ]),
            ),
            (
                vec![TupleChange::retract(0)],
                unit(vec![vec![Group(1), Group(2)]]),
            ),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, CountCollector>(
                b,
                &p,
                |(a, b)| ((a.group, b.group), ()),
                |((a, b), _)| vec![Group(*a), Group(*b)],
            )
        },
        one,
    );
}
fn pair_counts(twice: bool, after: bool) -> Expected {
    let counts = if after {
        vec![(0, 1, 1), (1, 0, 1), (1, 1, 1)]
    } else {
        vec![(0, 0, 1), (0, 1, 3), (1, 0, 1), (1, 1, 1)]
    };
    counts
        .into_iter()
        .map(|(a, b, n)| {
            let mut r = vec![Group(a), Group(b), Number(n)];
            if twice {
                r.push(Number(n));
            }
            (r, -n * if twice { 2 } else { 1 })
        })
        .collect()
}
#[test]
fn bi_group_by_2_mapping_1_collector() {
    run(
        generated(1, 2, 4),
        vec![
            (vec![], pair_counts(false, false)),
            (vec![TupleChange::retract(0)], pair_counts(false, true)),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, CountCollector>(
                b,
                &p,
                |(a, b)| ((a.group, b.group), ()),
                |((a, b), n)| vec![Group(*a), Group(*b), Number(*n as i64)],
            )
        },
        |r| -number(r, 2),
    );
}
#[test]
fn bi_group_by_2_mapping_2_collector() {
    run(
        generated(1, 2, 4),
        vec![
            (vec![], pair_counts(true, false)),
            (vec![TupleChange::retract(0)], pair_counts(true, true)),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, (CountCollector, CountCollector)>(
                b,
                &p,
                |(a, b)| ((a.group, b.group), ((), ())),
                |((a, b), (x, y))| vec![Group(*a), Group(*b), Number(*x as i64), Number(*y as i64)],
            )
        },
        |r| -(number(r, 2) + number(r, 3)),
    );
}
fn pair_three(count: bool) -> Vec<Vec<Field>> {
    [(0, 1, 0), (0, 2, 0), (1, 2, 1)]
        .into_iter()
        .map(|(a, b, v)| {
            let mut r = vec![Group(a), Group(b), Value(v)];
            if count {
                r.push(Number(1));
            }
            r
        })
        .collect()
}
#[test]
fn bi_group_by_3_mapping_0_collector() {
    let rows = pair_three(false);
    run(
        generated(2, 3, 3),
        vec![
            (vec![], unit(rows.clone())),
            (vec![TupleChange::retract(0)], unit(rows[2..].to_vec())),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, CountCollector>(
                b,
                &p,
                |(a, b)| ((a.group, b.group, a.value), ()),
                |((a, b, v), _)| vec![Group(*a), Group(*b), Value(*v)],
            )
        },
        one,
    );
}
#[test]
fn bi_group_by_3_mapping_1_collector() {
    let rows = pair_three(true);
    run(
        generated(2, 3, 3),
        vec![
            (vec![], unit(rows.clone())),
            (vec![TupleChange::retract(0)], unit(rows[2..].to_vec())),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, CountCollector>(
                b,
                &p,
                |(a, b)| ((a.group, b.group, a.value), ()),
                |((a, b, v), n)| vec![Group(*a), Group(*b), Value(*v), Number(*n as i64)],
            )
        },
        one,
    );
}
#[test]
fn bi_group_by_4_mapping_0_collector() {
    let rows = vec![
        vec![Group(0), Group(1), Value(0), Value(1)],
        vec![Group(0), Group(2), Value(0), Value(0)],
        vec![Group(1), Group(2), Value(1), Value(0)],
    ];
    run(
        generated(2, 3, 3),
        vec![
            (vec![], unit(rows.clone())),
            (vec![TupleChange::retract(0)], unit(rows[2..].to_vec())),
        ],
        |b, s| {
            let p = b.unique_pairs(s.port()).unwrap();
            group::<_, _, _, _, CountCollector>(
                b,
                &p,
                |(a, b)| ((a.group, b.group, a.value, b.value), ()),
                |((a, b, v, w), _)| vec![Group(*a), Group(*b), Value(*v), Value(*w)],
            )
        },
        one,
    );
}
