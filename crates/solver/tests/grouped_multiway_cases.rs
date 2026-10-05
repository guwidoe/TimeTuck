// Adapted from Timefold Community 2.7.0 AbstractTri/QuadConstraintStreamTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
    long: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Field {
    Group(usize),
    Value(usize),
    Code(usize),
    Number(i64),
    Set(Vec<usize>),
}
use Field::*;
type Fields = Vec<Field>;
type Output = StreamPort<Fields, Fields>;
type Expected = Vec<(Fields, i64)>;
struct Frame {
    a: Entity,
    b: Option<Entity>,
    c: Option<Entity>,
    group: usize,
    value: usize,
}
#[derive(Clone, Copy)]
struct Shape {
    arity: usize,
    keys: usize,
    collectors: usize,
}
impl Shape {
    fn pair(self) -> bool {
        self.keys == 1 && self.collectors >= 2
    }
    fn distinct_three(self) -> bool {
        self.arity == 3 && self.keys >= 3
    }
    fn key(self, r: &Frame) -> Fields {
        if self.pair() {
            return vec![Code(r.a.id)];
        }
        if self.distinct_three() {
            let mut key = vec![
                Group(r.a.group),
                Group(r.b.as_ref().unwrap().group),
                Group(r.c.as_ref().unwrap().group),
            ];
            if self.keys == 4 {
                key.push(Value(r.a.value));
            }
            return key;
        }
        match self.keys {
            1 => vec![Value(r.value)],
            2 => vec![Group(r.group), Value(r.value)],
            3 => vec![
                Group(r.a.group),
                Group(r.b.as_ref().unwrap().group),
                if self.collectors == 1 {
                    Group(r.group)
                } else {
                    Value(r.value)
                },
            ],
            4 => vec![
                Group(r.a.group),
                Group(r.b.as_ref().unwrap().group),
                Group(r.group),
                Value(r.value),
            ],
            _ => unreachable!(),
        }
    }
    fn score(self, row: &[Field]) -> i64 {
        if self.pair() || self.keys >= 3 || self.collectors == 0 {
            return -1;
        }
        -row[self.keys..]
            .iter()
            .map(|f| if let Number(n) = f { *n } else { 0 })
            .sum::<i64>()
    }
}
fn append(mut key: Fields, fields: impl IntoIterator<Item = Field>) -> Fields {
    key.extend(fields);
    key
}
fn group<I: Ord + Clone + 'static, T: Clone + 'static>(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    p: &StreamPort<I, T>,
    frame: impl Fn(&T) -> Frame + 'static,
    s: Shape,
) -> Output {
    let mapped = b
        .map(p, move |r| {
            let f = frame(r);
            (s.key(&f), f.a.id, f.a.long)
        })
        .unwrap();
    match s.collectors {
        0 | 1 => {
            let groups = b
                .group_collect_with::<_, _, _, _, CountCollector>(&mapped, |r| (r.0.clone(), ()))
                .unwrap();
            b.map(&groups, move |(k, n)| {
                if s.collectors == 0 {
                    k.clone()
                } else {
                    append(k.clone(), [Number(*n as i64)])
                }
            })
            .unwrap()
        }
        2 => {
            if s.pair() {
                let groups = b
                    .group_collect_with::<_, _, _, _, (CountCollector, SetCollector<usize>)>(
                        &mapped,
                        |r| (r.0.clone(), ((), r.1)),
                    )
                    .unwrap();
                b.map(&groups, |(k, (n, ids))| {
                    append(
                        k.clone(),
                        [Number(*n as i64), Set(ids.keys().copied().collect())],
                    )
                })
                .unwrap()
            } else {
                // The two source collectors are both count; use two independent
                // counter states, not a duplicated result from one counter.
                let counts = b
                    .group_collect_with::<_, _, _, _, (CountCollector, CountCollector)>(
                        &mapped,
                        |r| (r.0.clone(), ((), ())),
                    )
                    .unwrap();
                b.map(&counts, |(k, (a, b))| {
                    append(k.clone(), [Number(*a as i64), Number(*b as i64)])
                })
                .unwrap()
            }
        }
        3 => {
            let groups=b.group_collect_with::<_,_,_,_,(IntCollector,IntCollector,SetCollector<usize>)>(&mapped,|r|(r.0.clone(),(r.2,r.2,r.1))).unwrap();
            b.map(&groups, |(k, (min, max, ids))| {
                append(
                    k.clone(),
                    [
                        Number(min.min.unwrap()),
                        Number(max.max.unwrap()),
                        Set(ids.keys().copied().collect()),
                    ],
                )
            })
            .unwrap()
        }
        _ => unreachable!(),
    }
}
fn frame(a: &Entity, b: Option<&Entity>, c: Option<&Entity>, group: usize, value: usize) -> Frame {
    Frame {
        a: a.clone(),
        b: b.cloned(),
        c: c.cloned(),
        group,
        value,
    }
}
fn expected(s: Shape, after: bool) -> Expected {
    if s.pair() {
        let mut rows = vec![];
        for (id, count, long) in [(0, 2, i64::MAX), (1, 1, i64::MIN)] {
            if after && id == 0 {
                continue;
            }
            let fields = if s.collectors == 2 {
                vec![Code(id), Number(count), Set(vec![id])]
            } else {
                vec![Code(id), Number(long), Number(long), Set(vec![id])]
            };
            rows.push((fields, -1));
        }
        return rows;
    }
    if s.distinct_three() {
        if after {
            return vec![];
        }
        let mut rows = vec![
            vec![Group(0), Group(1), Group(2)],
            vec![Group(0), Group(2), Group(1)],
            vec![Group(1), Group(2), Group(0)],
        ];
        if s.collectors == 1 {
            for row in &mut rows {
                row.push(Number(1));
            }
        }
        if s.keys == 4 {
            for (row, value) in rows.iter_mut().zip([0, 1, 0]) {
                row.push(Value(value));
            }
        }
        return rows.into_iter().map(|row| (row, -1)).collect();
    }
    let mut rows = vec![];
    for identity in [1, 0] {
        let count = if identity == 1 || after {
            1
        } else if s.arity == 3 {
            2
        } else {
            4
        };
        let mut key = match s.keys {
            1 => vec![Value(identity)],
            2 => vec![Group(identity), Value(identity)],
            3 if s.collectors == 1 => vec![Group(identity), Group(identity), Group(identity)],
            // Preserve the source's repeated-value assertion after removal. Its
            // assertion helper intentionally compares membership, not positions
            // or duplicate multiplicities; see check() below.
            3 if after => vec![Group(identity), Value(identity), Value(identity)],
            3 => vec![Group(identity), Group(identity), Value(identity)],
            4 => vec![
                Group(identity),
                Group(identity),
                Group(identity),
                Value(identity),
            ],
            _ => unreachable!(),
        };
        for _ in 0..s.collectors {
            key.push(Number(count));
        }
        let score = if s.collectors == 0 || s.keys >= 3 {
            -1
        } else {
            -count * s.collectors as i64
        };
        rows.push((key, score));
    }
    rows
}
fn check(
    g: &StreamGraph<'_, SimpleScore>,
    c: &StreamConstraint<Fields, Fields>,
    expected: Expected,
) {
    let actual = g
        .constraint_matches(c)
        .unwrap()
        .values()
        .map(|(r, s)| (r, s.score()))
        .collect::<Vec<_>>();
    // Full source helper semantics: same score and length, and expected facts
    // contain every actual fact; both expected-to-actual and actual-to-expected
    // existence checks. This is deliberately not ordered tuple equality.
    let matches = |(want, score): &(Fields, i64), (row, actual_score): &(&Fields, i64)| {
        score == actual_score && want.len() == row.len() && row.iter().all(|f| want.contains(f))
    };
    for want in &expected {
        assert!(
            actual.iter().any(|got| matches(want, got)),
            "missing {want:?}; actual {actual:?}"
        );
    }
    for got in &actual {
        assert!(
            expected.iter().any(|want| matches(want, got)),
            "excess {got:?}; expected {expected:?}"
        );
    }
    assert_eq!(actual.len(), expected.len());
    let total = SimpleScore::new(expected.iter().map(|(_, score)| score).sum());
    assert_eq!(g.score(), total);
    assert_eq!(
        g.constraint_scores().collect::<Vec<_>>(),
        vec![("testConstraint", total)]
    );
}
fn run(s: Shape) {
    let values = if s.pair() { 1 } else { 2 };
    let groups = if s.distinct_three() { 3 } else { 2 };
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, Entity>();
    let group_source = b.source::<usize, usize>();
    let value_source = b.source::<usize, usize>();
    let output = if s.pair() {
        let pairs = b.unique_pairs(entities.port()).unwrap();
        let tri = b
            .join_equal(&pairs, entities.port(), |(a, _)| a.id, |c| c.id)
            .unwrap();
        if s.arity == 3 {
            group(
                &mut b,
                &tri,
                |((a, b), c)| frame(a, Some(b), Some(c), 0, 0),
                s,
            )
        } else {
            let quad = b
                .join_equal(&tri, entities.port(), |((_, b), _)| b.id, |d| d.id)
                .unwrap();
            group(
                &mut b,
                &quad,
                |(((a, b), c), _)| frame(a, Some(b), Some(c), 0, 0),
                s,
            )
        }
    } else if s.distinct_three() {
        let pairs = b.unique_pairs(entities.port()).unwrap();
        let tri = b
            .join_equal(&pairs, entities.port(), |_| (), |_| ())
            .unwrap();
        let distinct = b
            .filter(&tri, |((a, b), c)| a.id != c.id && b.id != c.id)
            .unwrap();
        group(
            &mut b,
            &distinct,
            |((a, b), c)| frame(a, Some(b), Some(c), 0, 0),
            s,
        )
    } else {
        let eg = b
            .join_equal(entities.port(), group_source.port(), |e| e.group, |g| *g)
            .unwrap();
        let tri = b
            .join_equal(&eg, value_source.port(), |(e, _)| e.value, |v| *v)
            .unwrap();
        if s.arity == 3 {
            group(&mut b, &tri, |((e, g), v)| frame(e, None, None, *g, *v), s)
        } else {
            let quad = b
                .join_equal(&tri, entities.port(), |((_, g), _)| *g, |e| e.group)
                .unwrap();
            group(
                &mut b,
                &quad,
                |(((a, g), v), d)| frame(a, Some(d), None, *g, *v),
                s,
            )
        }
    };
    let terminal = b
        .score("testConstraint", &output, move |row| {
            SimpleScore::new(s.score(row))
        })
        .unwrap();
    let mut graph = b.build();
    let mut tx = graph.transaction();
    for g in 0..groups {
        tx.apply(&group_source, TupleChange::put(g, g)).unwrap();
    }
    for v in 0..values {
        tx.apply(&value_source, TupleChange::put(v, v)).unwrap();
    }
    for id in 0..3 {
        tx.apply(
            &entities,
            TupleChange::put(
                id,
                Entity {
                    id,
                    group: id % groups,
                    value: id % values,
                    long: if id == 0 { i64::MAX } else { i64::MIN },
                },
            ),
        )
        .unwrap();
    }
    tx.commit().unwrap();
    check(&graph, &terminal, expected(s, false));
    if s.keys != 1 || s.collectors != 0 {
        let mut tx = graph.transaction();
        tx.apply(&entities, TupleChange::retract(0)).unwrap();
        tx.commit().unwrap();
        check(&graph, &terminal, expected(s, true));
    }
}

#[test]
fn tri_group_by_1_mapping_0_collector() {
    run(Shape {
        arity: 3,
        keys: 1,
        collectors: 0,
    });
}

#[test]
fn tri_group_by_1_mapping_1_collector() {
    run(Shape {
        arity: 3,
        keys: 1,
        collectors: 1,
    });
}

#[test]
fn tri_group_by_1_mapping_2_collector() {
    run(Shape {
        arity: 3,
        keys: 1,
        collectors: 2,
    });
}

#[test]
fn tri_group_by_1_mapping_3_collector() {
    run(Shape {
        arity: 3,
        keys: 1,
        collectors: 3,
    });
}

#[test]
fn tri_group_by_2_mapping_0_collector() {
    run(Shape {
        arity: 3,
        keys: 2,
        collectors: 0,
    });
}

#[test]
fn tri_group_by_2_mapping_1_collector() {
    run(Shape {
        arity: 3,
        keys: 2,
        collectors: 1,
    });
}

#[test]
fn tri_group_by_2_mapping_2_collector() {
    run(Shape {
        arity: 3,
        keys: 2,
        collectors: 2,
    });
}

#[test]
fn tri_group_by_3_mapping_0_collector() {
    run(Shape {
        arity: 3,
        keys: 3,
        collectors: 0,
    });
}

#[test]
fn tri_group_by_3_mapping_1_collector() {
    run(Shape {
        arity: 3,
        keys: 3,
        collectors: 1,
    });
}

#[test]
fn tri_group_by_4_mapping_0_collector() {
    run(Shape {
        arity: 3,
        keys: 4,
        collectors: 0,
    });
}

#[test]
fn quad_group_by_1_mapping_0_collector() {
    run(Shape {
        arity: 4,
        keys: 1,
        collectors: 0,
    });
}

#[test]
fn quad_group_by_1_mapping_1_collector() {
    run(Shape {
        arity: 4,
        keys: 1,
        collectors: 1,
    });
}

#[test]
fn quad_group_by_1_mapping_2_collector() {
    run(Shape {
        arity: 4,
        keys: 1,
        collectors: 2,
    });
}

#[test]
fn quad_group_by_1_mapping_3_collector() {
    run(Shape {
        arity: 4,
        keys: 1,
        collectors: 3,
    });
}

#[test]
fn quad_group_by_2_mapping_0_collector() {
    run(Shape {
        arity: 4,
        keys: 2,
        collectors: 0,
    });
}

#[test]
fn quad_group_by_2_mapping_1_collector() {
    run(Shape {
        arity: 4,
        keys: 2,
        collectors: 1,
    });
}

#[test]
fn quad_group_by_2_mapping_2_collector() {
    run(Shape {
        arity: 4,
        keys: 2,
        collectors: 2,
    });
}

#[test]
fn quad_group_by_3_mapping_0_collector() {
    run(Shape {
        arity: 4,
        keys: 3,
        collectors: 0,
    });
}

#[test]
fn quad_group_by_3_mapping_1_collector() {
    run(Shape {
        arity: 4,
        keys: 3,
        collectors: 1,
    });
}

#[test]
fn quad_group_by_4_mapping_0_collector() {
    run(Shape {
        arity: 4,
        keys: 4,
        collectors: 0,
    });
}
