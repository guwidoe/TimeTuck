// Adapted from pinned Community Abstract{Uni,Bi,Tri,Quad}ConstraintStreamPrecomputeTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::Cell, rc::Rc};
use timetuck::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Fact {
    id: usize,
    group: Option<usize>,
    property: i32,
}
#[derive(Clone)]
struct Live {
    fact: Fact,
    assigned: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Field {
    Entity(Fact),
    Group(usize),
    Value(usize),
    Count(usize),
    Holder(Rc<i32>),
    Null,
}
type Row = Vec<Field>;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Op {
    Exists,
    Absent,
    Group,
    Flatten,
    FlattenNew,
    FlattenLast,
    FlattenLastNew,
    Map,
    Concat,
    Distinct,
    Complement,
}
#[derive(Clone, Copy)]
struct Case {
    arity: usize,
    op: Op,
}
impl Case {
    fn new_instances(self) -> bool {
        matches!(self.op, Op::FlattenNew | Op::FlattenLastNew)
    }
    fn facts(self) -> Vec<Fact> {
        (0..if matches!(self.op, Op::Map | Op::Distinct | Op::Complement) {
            3
        } else {
            2
        })
            .map(|id| Fact {
                id,
                group: (id > 0 && !self.new_instances()).then_some(0),
                property: if self.new_instances() {
                    id as i32 + 1
                } else {
                    0
                },
            })
            .collect()
    }
    fn expected(self) -> Vec<Row> {
        let entities: Vec<_> = self.facts().into_iter().map(Field::Entity).collect();
        let values = |first: Field, fill: Field| {
            let mut row = vec![first];
            row.extend(std::iter::repeat_n(fill, self.arity - 1));
            row
        };
        match self.op {
            Op::Exists => vec![values(entities[1].clone(), Field::Value(0))],
            Op::Absent => vec![values(entities[0].clone(), Field::Value(0))],
            Op::Group => vec![values(Field::Group(0), Field::Count(1))],
            Op::Map => vec![values(Field::Group(0), Field::Value(0)); 2],
            Op::Distinct => vec![values(Field::Group(0), Field::Value(0))],
            Op::Complement => vec![
                values(entities[1].clone(), Field::Value(0)),
                values(entities[2].clone(), Field::Value(0)),
                values(entities[0].clone(), Field::Null),
            ],
            Op::Concat | Op::FlattenLast => entities
                .into_iter()
                .map(|e| values(e, Field::Value(0)))
                .collect(),
            Op::FlattenLastNew => (1..=2)
                .map(|n| values(Field::Holder(Rc::new(n)), Field::Value(0)))
                .collect(),
            Op::Flatten | Op::FlattenNew => entities
                .iter()
                .enumerate()
                .map(|(id, e)| {
                    // Bi and Tri literally expect entityWithoutGroup in the second field.
                    // Their upstream match helper ignores duplicate-field multiplicity.
                    let second = if self.op == Op::FlattenNew {
                        Field::Holder(Rc::new(id as i32 + 1))
                    } else if self.arity == 1 {
                        e.clone()
                    } else {
                        entities[0].clone()
                    };
                    let mut row = vec![e.clone(), second];
                    row.extend(std::iter::repeat_n(
                        Field::Value(0),
                        self.arity.saturating_sub(2),
                    ));
                    row
                })
                .collect(),
        }
    }
}
fn fact(row: &Row) -> &Fact {
    let Field::Entity(f) = &row[0] else {
        panic!("entity expected")
    };
    f
}
struct Sources {
    entity: StreamSource<usize, Live>,
    value: StreamSource<usize, Field>,
    group: StreamSource<usize, Option<usize>>,
}
fn assert_matches<I: Ord + Clone + 'static>(
    graph: &StreamGraph<'_, SimpleScore>,
    data: &StreamDataset<I, Row>,
    expected: &[Row],
) {
    let actual: Vec<_> = graph
        .dataset_rows(data)
        .unwrap()
        .iter()
        .map(|(_, row)| row.clone())
        .collect();
    // AssertableMatch.isEqualTo: score, field count, expected.containsAll(actual).
    // assertScore checks both missing and excess matches, then the total score.
    let matches = |e: &Row, a: &Row| e.len() == a.len() && a.iter().all(|v| e.contains(v));
    assert!(
        expected
            .iter()
            .all(|e| actual.iter().any(|a| matches(e, a))),
        "missing: expected {expected:?}, actual {actual:?}"
    );
    assert!(
        actual
            .iter()
            .all(|a| expected.iter().any(|e| matches(e, a))),
        "excess: expected {expected:?}, actual {actual:?}"
    );
    assert_eq!(graph.score(), SimpleScore::new(-(expected.len() as i64)));
    assert_eq!(actual.len(), expected.len());
}
fn finish<'a, I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'a, SimpleScore>,
    s: Sources,
    prefix: StreamPort<I, Row>,
    case: Case,
    calls: &'a Cell<usize>,
) {
    let cached = b.dataset(&prefix).unwrap();
    let assigned = b.filter(s.entity.port(), |e| e.assigned).unwrap();
    let visible = b
        .existence_equal(&prefix, &assigned, ExistenceMode::Exists, |_| (), |_| ())
        .unwrap();
    let data = b.dataset(&visible).unwrap();
    b.score("precomputed", &visible, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut entities: Vec<_> = case
        .facts()
        .into_iter()
        .map(|fact| Live {
            fact,
            assigned: false,
        })
        .collect();
    let mut tx = g.transaction();
    tx.apply(&s.value, TupleChange::put(0, Field::Value(0)))
        .unwrap();
    if !case.new_instances() {
        tx.apply(&s.group, TupleChange::put(0, Some(0))).unwrap();
    }
    for e in &entities {
        tx.apply(&s.entity, TupleChange::put(e.fact.id, e.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    assert_matches(&g, &data, &[]);
    let before: Vec<_> = g
        .dataset_rows(&cached)
        .unwrap()
        .iter()
        .map(|(_, r)| r.clone())
        .collect();
    let computed = calls.get();
    for e in &mut entities {
        e.assigned = true;
        let mut tx = g.transaction();
        tx.apply(&s.entity, TupleChange::put(e.fact.id, e.clone()))
            .unwrap();
        tx.commit().unwrap();
    }
    let expected = case.expected();
    assert_matches(&g, &data, &expected);
    assert_eq!(calls.get(), computed);
    // Additional native evidence: assignment changes retain allocated prefix values.
    let after: Vec<_> = g
        .dataset_rows(&cached)
        .unwrap()
        .iter()
        .map(|(_, r)| r.clone())
        .collect();
    assert_eq!(before, after);
    for (a, b) in before.iter().flatten().zip(after.iter().flatten()) {
        if let (Field::Holder(a), Field::Holder(b)) = (a, b) {
            assert!(Rc::ptr_eq(a, b));
        }
    }
    // Deactivate and reactivate the live existence gate without rebuilding facts.
    let mut tx = g.transaction();
    for e in &mut entities {
        e.assigned = false;
        tx.apply(&s.entity, TupleChange::put(e.fact.id, e.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    assert_matches(&g, &data, &[]);
    entities[0].assigned = true;
    let mut tx = g.transaction();
    tx.apply(&s.entity, TupleChange::put(0, entities[0].clone()))
        .unwrap();
    tx.commit().unwrap();
    assert_matches(&g, &data, &expected);
    assert_eq!(calls.get(), computed);
}
fn operate<'a, I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'a, SimpleScore>,
    s: Sources,
    input: StreamPort<I, Row>,
    universe: StreamPort<usize, Row>,
    case: Case,
    calls: &'a Cell<usize>,
) {
    match case.op {
        Op::Exists | Op::Absent => {
            let out = b
                .existence_equal(
                    &input,
                    s.group.port(),
                    if case.op == Op::Exists {
                        ExistenceMode::Exists
                    } else {
                        ExistenceMode::NotExists
                    },
                    |r| fact(r).group,
                    |g| *g,
                )
                .unwrap();
            finish(b, s, out, case, calls);
        }
        Op::Map | Op::Distinct => {
            let filtered = b.filter(&input, |r| fact(r).group.is_some()).unwrap();
            let out = b
                .map(&filtered, |r| {
                    let mut r = r.clone();
                    r[0] = Field::Group(fact(&r).group.unwrap());
                    r
                })
                .unwrap();
            if case.op == Op::Distinct {
                let out = b.distinct(&out).unwrap();
                finish(b, s, out, case, calls);
            } else {
                finish(b, s, out, case, calls);
            }
        }
        Op::Concat => {
            let a = b.filter(&input, |r| fact(r).group.is_none()).unwrap();
            let c = b.filter(&input, |r| fact(r).group.is_some()).unwrap();
            let out = b.concat(&a, &c).unwrap();
            finish(b, s, out, case, calls);
        }
        Op::Complement => {
            let filtered = b.filter(&input, |r| fact(r).group.is_some()).unwrap();
            let out = b
                .complement(
                    &filtered,
                    &universe,
                    |r| fact(r).id,
                    |r| fact(r).id,
                    move |r| {
                        let mut r = r.clone();
                        r.extend(std::iter::repeat_n(Field::Null, case.arity - 1));
                        r
                    },
                )
                .unwrap();
            if case.arity == 3 {
                let out = b.distinct(&out).unwrap();
                finish(b, s, out, case, calls);
            } else {
                finish(b, s, out, case, calls);
            }
        }
        _ => finish(b, s, input, case, calls),
    }
}
fn with_values<'a, I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'a, SimpleScore>,
    s: Sources,
    input: StreamPort<I, Row>,
    universe: StreamPort<usize, Row>,
    case: Case,
    calls: &'a Cell<usize>,
    count: usize,
) {
    macro_rules! join {
        ($p:expr) => {{
            let pair = b.join_equal($p, s.value.port(), |_| (), |_| ()).unwrap();
            b.map(&pair, |(row, v)| {
                let mut row = row.clone();
                row.push(v.clone());
                row
            })
            .unwrap()
        }};
    }
    if count == 0 {
        return operate(b, s, input, universe, case, calls);
    }
    let bi = join!(&input);
    if count == 1 {
        return operate(b, s, bi, universe, case, calls);
    }
    let tri = join!(&bi);
    if count == 2 {
        return operate(b, s, tri, universe, case, calls);
    }
    let quad = join!(&tri);
    operate(b, s, quad, universe, case, calls);
}
fn run(arity: usize, op: Op) {
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let s = Sources {
        entity: b.source(),
        value: b.source(),
        group: b.source(),
    };
    let facts = b
        .precompute(
            s.entity.port(),
            |e| e.fact.clone(),
            |e| {
                calls.set(calls.get() + 1);
                vec![Field::Entity(e.fact.clone())]
            },
        )
        .unwrap();
    let case = Case { arity, op };
    match op {
        Op::Group => {
            let filtered = b.filter(&facts, |r| fact(r).group.is_some()).unwrap();
            match arity {
                1 => {
                    let keys = b
                        .map(&filtered, |r| vec![Field::Group(fact(r).group.unwrap())])
                        .unwrap();
                    let out = b.distinct(&keys).unwrap();
                    finish(b, s, out, case, &calls);
                }
                2 => {
                    let grouped = b
                        .group_collect_with::<_, _, _, _, CountCollector>(&filtered, |r| {
                            (fact(r).group.unwrap(), ())
                        })
                        .unwrap();
                    let out = b
                        .map(&grouped, |(g, n)| vec![Field::Group(*g), Field::Count(*n)])
                        .unwrap();
                    finish(b, s, out, case, &calls);
                }
                3 => {
                    let grouped = b
                        .group_collect_with::<_, _, _, _, (CountCollector, CountCollector)>(
                            &filtered,
                            |r| (fact(r).group.unwrap(), ((), ())),
                        )
                        .unwrap();
                    let out = b
                        .map(&grouped, |(g, (a, c))| {
                            vec![Field::Group(*g), Field::Count(*a), Field::Count(*c)]
                        })
                        .unwrap();
                    finish(b, s, out, case, &calls);
                }
                4 => {
                    let grouped = b.group_collect_with::<_,_,_,_,(CountCollector,CountCollector,CountCollector)>(&filtered,|r| (fact(r).group.unwrap(),((),(),()))).unwrap();
                    let out = b
                        .map(&grouped, |(g, (a, c, d))| {
                            vec![
                                Field::Group(*g),
                                Field::Count(*a),
                                Field::Count(*c),
                                Field::Count(*d),
                            ]
                        })
                        .unwrap();
                    finish(b, s, out, case, &calls);
                }
                _ => unreachable!(),
            }
        }
        Op::Flatten | Op::FlattenNew => {
            let out = b
                .flat_map(&facts, move |r| {
                    let mut out = r.clone();
                    out.push(if op == Op::FlattenNew {
                        Field::Holder(Rc::new(fact(r).property))
                    } else {
                        r[0].clone()
                    });
                    vec![out]
                })
                .unwrap();
            with_values(b, s, out, facts, case, &calls, arity.saturating_sub(2));
        }
        Op::FlattenLast | Op::FlattenLastNew => {
            let list = b.collect_list(&facts, |r| fact(r).clone()).unwrap();
            let out = b
                .flat_map(&list, move |list| {
                    list.iter()
                        .map(|f| {
                            vec![if op == Op::FlattenLastNew {
                                Field::Holder(Rc::new(f.property))
                            } else {
                                Field::Entity(f.clone())
                            }]
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap();
            with_values(b, s, out, facts, case, &calls, arity - 1);
        }
        _ => with_values(b, s, facts.clone(), facts, case, &calls, arity - 1),
    }
}

#[test]
fn uni_if_exists() {
    run(1, Op::Exists);
}

#[test]
fn uni_if_not_exists() {
    run(1, Op::Absent);
}

#[test]
fn uni_group_by() {
    run(1, Op::Group);
}

#[test]
fn uni_flatten() {
    run(1, Op::Flatten);
}

#[test]
fn uni_flatten_new_instances() {
    run(1, Op::FlattenNew);
}

#[test]
fn uni_flatten_last() {
    run(1, Op::FlattenLast);
}

#[test]
fn uni_flatten_last_new_instances() {
    run(1, Op::FlattenLastNew);
}

#[test]
fn uni_map() {
    run(1, Op::Map);
}

#[test]
fn uni_concat() {
    run(1, Op::Concat);
}

#[test]
fn uni_distinct() {
    run(1, Op::Distinct);
}

#[test]
fn uni_complement() {
    run(1, Op::Complement);
}

#[test]
fn bi_if_exists() {
    run(2, Op::Exists);
}

#[test]
fn bi_if_not_exists() {
    run(2, Op::Absent);
}

#[test]
fn bi_group_by() {
    run(2, Op::Group);
}

#[test]
fn bi_flatten() {
    run(2, Op::Flatten);
}

#[test]
fn bi_flatten_new_instances() {
    run(2, Op::FlattenNew);
}

#[test]
fn bi_flatten_last() {
    run(2, Op::FlattenLast);
}

#[test]
fn bi_flatten_last_new_instances() {
    run(2, Op::FlattenLastNew);
}

#[test]
fn bi_map() {
    run(2, Op::Map);
}

#[test]
fn bi_concat() {
    run(2, Op::Concat);
}

#[test]
fn bi_distinct() {
    run(2, Op::Distinct);
}

#[test]
fn bi_complement() {
    run(2, Op::Complement);
}

#[test]
fn tri_if_exists() {
    run(3, Op::Exists);
}

#[test]
fn tri_if_not_exists() {
    run(3, Op::Absent);
}

#[test]
fn tri_group_by() {
    run(3, Op::Group);
}

#[test]
fn tri_flatten() {
    run(3, Op::Flatten);
}

#[test]
fn tri_flatten_new_instances() {
    run(3, Op::FlattenNew);
}

#[test]
fn tri_flatten_last() {
    run(3, Op::FlattenLast);
}

#[test]
fn tri_flatten_last_new_instances() {
    run(3, Op::FlattenLastNew);
}

#[test]
fn tri_map() {
    run(3, Op::Map);
}

#[test]
fn tri_concat() {
    run(3, Op::Concat);
}

#[test]
fn tri_distinct() {
    run(3, Op::Distinct);
}

#[test]
fn tri_complement() {
    run(3, Op::Complement);
}

#[test]
fn quad_if_exists() {
    run(4, Op::Exists);
}

#[test]
fn quad_if_not_exists() {
    run(4, Op::Absent);
}

#[test]
fn quad_group_by() {
    run(4, Op::Group);
}

#[test]
fn quad_flatten_last() {
    run(4, Op::FlattenLast);
}

#[test]
fn quad_flatten_last_new_instances() {
    run(4, Op::FlattenLastNew);
}

#[test]
fn quad_map() {
    run(4, Op::Map);
}

#[test]
fn quad_concat() {
    run(4, Op::Concat);
}

#[test]
fn quad_distinct() {
    run(4, Op::Distinct);
}

#[test]
fn quad_complement() {
    run(4, Op::Complement);
}

// Original runtime regression; not credited as an upstream method.
#[test]
fn fact_edits_update_cached_aggregates_and_padding_with_rollback() {
    use std::collections::{BTreeMap, BTreeSet};
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Live>();
    let groups = b.source::<usize, usize>();
    let cached = b
        .precompute(source.port(), |e| e.fact.clone(), |e| e.fact.clone())
        .unwrap();
    let grouped = b
        .group_collect_with::<_, _, _, _, CountCollector>(&cached, |f| (f.group, ()))
        .unwrap();
    let counts = b.dataset(&grouped).unwrap();
    let list = b.collect_list(&cached, |f| f.clone()).unwrap();
    let flat = b
        .flat_map(&list, |list| {
            list.iter()
                .map(|f| (f.id, Rc::new(f.property)))
                .collect::<Vec<_>>()
        })
        .unwrap();
    let flattened = b.dataset(&flat).unwrap();
    let present = b
        .existence_equal(
            &cached,
            groups.port(),
            ExistenceMode::Exists,
            |f| f.group,
            |g| Some(*g),
        )
        .unwrap();
    let populated = b.map(&present, |f| (f.id, Some(f.property))).unwrap();
    let padded = b
        .complement(&populated, &cached, |r| r.0, |f| f.id, |f| (f.id, None))
        .unwrap();
    let assigned = b.filter(source.port(), |e| e.assigned).unwrap();
    let live = b
        .existence_equal(&padded, &assigned, ExistenceMode::Exists, |_| (), |_| ())
        .unwrap();
    let output = b.dataset(&live).unwrap();
    b.score("padding", &live, |_| SimpleScore::new(-1)).unwrap();
    let mut g = b.build();
    let mut model = BTreeMap::<usize, Live>::new();
    let mut group_model = BTreeSet::new();
    let verify = |g: &StreamGraph<'_, SimpleScore>,
                  model: &BTreeMap<usize, Live>,
                  group_model: &BTreeSet<usize>| {
        let mut expected_counts = BTreeMap::new();
        for e in model.values() {
            *expected_counts.entry(e.fact.group).or_insert(0usize) += 1;
        }
        assert_eq!(
            g.dataset_rows(&counts)
                .unwrap()
                .iter()
                .map(|(_, (k, n))| (*k, *n))
                .collect::<BTreeMap<_, _>>(),
            expected_counts
        );
        assert_eq!(
            g.dataset_rows(&flattened)
                .unwrap()
                .iter()
                .map(|(_, (id, n))| (*id, **n))
                .collect::<Vec<_>>(),
            model
                .values()
                .map(|e| (e.fact.id, e.fact.property))
                .collect::<Vec<_>>()
        );
        let visible = model.values().any(|e| e.assigned);
        let expected: BTreeMap<_, _> = model
            .values()
            .filter(|_| visible)
            .map(|e| {
                (
                    e.fact.id,
                    e.fact
                        .group
                        .filter(|g| group_model.contains(g))
                        .map(|_| e.fact.property),
                )
            })
            .collect();
        assert_eq!(
            g.dataset_rows(&output)
                .unwrap()
                .iter()
                .map(|(_, r)| *r)
                .collect::<BTreeMap<_, _>>(),
            expected
        );
        assert_eq!(g.score(), SimpleScore::new(-(expected.len() as i64)));
    };
    for step in 0..600 {
        let mut next = model.clone();
        let mut next_groups = group_model.clone();
        let id = (step * 7) % 11;
        let mut tx = g.transaction();
        match step % 6 {
            0 => {
                let group = (step / 6) % 4;
                next_groups.insert(group);
                tx.apply(&groups, TupleChange::put(group, group)).unwrap();
            }
            1 => {
                let group = (step / 6) % 4;
                next_groups.remove(&group);
                tx.apply(&groups, TupleChange::retract(group)).unwrap();
            }
            2 => {
                next.remove(&id);
                tx.apply(&source, TupleChange::retract(id)).unwrap();
            }
            3 => {
                let e = next.entry(id).or_insert(Live {
                    fact: Fact {
                        id,
                        group: None,
                        property: 0,
                    },
                    assigned: false,
                });
                e.assigned = !e.assigned;
                tx.apply(&source, TupleChange::put(id, e.clone())).unwrap();
            }
            _ => {
                let e = Live {
                    fact: Fact {
                        id,
                        group: (step % 5 != 0).then_some(step % 4),
                        property: step as i32 - 200,
                    },
                    assigned: step % 3 == 0,
                };
                tx.apply(&source, TupleChange::put(id, e.clone())).unwrap();
                next.insert(id, e);
            }
        }
        verify(tx.graph().unwrap(), &next, &next_groups);
        if step % 7 == 0 {
            drop(tx);
        } else {
            tx.commit().unwrap();
            model = next;
            group_model = next_groups;
        }
        verify(&g, &model, &group_model);
    }
}
