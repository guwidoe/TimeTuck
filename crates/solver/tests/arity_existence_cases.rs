// Adapted from pinned AbstractTri/QuadConstraintStreamTest and lavish fixtures.
// Apache-2.0; see compat/provenance.json. Match facts are unordered in source helper.
use timetuck::*;
type Id = (u8, usize);
const E: u8 = 0;
const G: u8 = 1;
const V: u8 = 2;
const VG: u8 = 3;
#[derive(Clone)]
struct Fact {
    kind: u8,
    id: usize,
    group: usize,
    value: usize,
}
fn fact(kind: u8, id: usize, group: usize, value: usize) -> Fact {
    Fact {
        kind,
        id,
        group,
        value,
    }
}
type Source = StreamSource<Id, Fact>;
type Reader = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Vec<Vec<Id>>;
struct Harness {
    g: StreamGraph<'static, SimpleScore>,
    source: Source,
    read: Box<Reader>,
}
impl Harness {
    fn put(&mut self, facts: Vec<Fact>) {
        let mut tx = self.g.transaction();
        for f in facts {
            tx.apply(&self.source, TupleChange::put((f.kind, f.id), f))
                .unwrap();
        }
        tx.commit().unwrap();
    }
    fn remove(&mut self, id: Id) {
        let mut tx = self.g.transaction();
        tx.apply(&self.source, TupleChange::retract(id)).unwrap();
        tx.commit().unwrap();
    }
    fn check(&self, mut expected: Vec<Vec<Id>>) {
        for row in &mut expected {
            row.sort();
        }
        expected.sort();
        assert_eq!((self.read)(&self.g), expected);
        assert_eq!(self.g.score(), SimpleScore::new(-(expected.len() as i64)));
    }
}
#[derive(Clone, Copy)]
struct Case {
    arity: usize,
    indexed: bool,
    filtered: bool,
    absent: bool,
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: Source,
    left: StreamPort<I, Vec<Fact>>,
    right: StreamPort<Id, Fact>,
    case: Case,
) -> Harness {
    let mode = if case.absent {
        ExistenceMode::NotExists
    } else {
        ExistenceMode::Exists
    };
    let visible = b
        .existence(
            &left,
            &right,
            mode,
            move |rows| {
                if case.indexed {
                    rows[usize::from(case.arity == 4 && case.absent && !case.filtered)].group
                } else {
                    0
                }
            },
            move |f| if case.indexed { f.id } else { 0 },
            move |rows, f| {
                if !case.filtered {
                    return true;
                }
                if !case.indexed {
                    return if case.arity == 3 {
                        rows[0].group == f.id && rows[1].group == f.id
                    } else {
                        rows[3].group == f.id
                    };
                }
                if case.arity == 4 && case.absent {
                    !(rows[0].id > 0 && f.id == 1)
                } else {
                    rows[0].id > 0 || f.id == 1
                }
            },
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &visible, |_| SimpleScore::new(-1))
        .unwrap();
    Harness {
        g: b.build(),
        source,
        read: Box::new(move |g| {
            let mut rows: Vec<_> = g
                .constraint_matches(&terminal)
                .unwrap()
                .values()
                .map(|(facts, score)| {
                    assert_eq!(*score, SimpleScore::new(-1));
                    let mut ids: Vec<_> = facts.iter().map(|f| (f.kind, f.id)).collect();
                    ids.sort();
                    ids
                })
                .collect();
            rows.sort();
            rows
        }),
    }
}
fn build(case: Case) -> Harness {
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<Id, Fact>();
    let e = b.filter(source.port(), |f| f.kind == E).unwrap();
    let groups = b.filter(source.port(), |f| f.kind == G).unwrap();
    let values = b.filter(source.port(), |f| f.kind == V).unwrap();
    let value_groups = b.filter(source.port(), |f| f.kind == VG).unwrap();
    let special = !case.indexed && !case.filtered;
    if special && case.arity == 3 {
        let pair = b
            .join_equal(&value_groups, &value_groups, |_| (), |_| ())
            .unwrap();
        let pair = b.filter(&pair, |(a, b)| a.id < b.id).unwrap();
        let triples = b.join_equal(&pair, &groups, |_| (), |_| ()).unwrap();
        let rows = b
            .map(&triples, |((a, b), c)| {
                vec![a.clone(), b.clone(), c.clone()]
            })
            .unwrap();
        return finish(b, source, rows, e, case);
    }
    if special {
        let pair = b.join_equal(&e, &groups, |e| e.group, |g| g.id).unwrap();
        let triple = b
            .join_equal(&pair, &values, |(e, _)| e.value, |v| v.id)
            .unwrap();
        let quad = b
            .join_equal(&triple, &e, |((_, g), _)| g.id, |e| e.group)
            .unwrap();
        let rows = b
            .map(&quad, |(((e, g), v), last)| {
                vec![e.clone(), g.clone(), v.clone(), last.clone()]
            })
            .unwrap();
        return finish(b, source, rows, value_groups, case);
    }
    let pair = b.join_equal(&e, &e, |_| (), |_| ()).unwrap();
    let pair = b.filter(&pair, |(a, b)| a.id < b.id).unwrap();
    if case.arity == 3 {
        let triple = b.join_equal(&pair, &e, |_| (), |_| ()).unwrap();
        let triple = b
            .filter(&triple, |((a, b), c)| c.id != a.id && c.id != b.id)
            .unwrap();
        let rows = b
            .map(&triple, |((a, b), c)| vec![a.clone(), b.clone(), c.clone()])
            .unwrap();
        finish(b, source, rows, groups, case)
    } else {
        let triple = b
            .join_equal(&pair, &groups, |(a, _)| a.group, |g| g.id)
            .unwrap();
        let quad = b
            .join_equal(&triple, &values, |((a, _), _)| a.value, |v| v.id)
            .unwrap();
        let rows = b
            .map(&quad, |(((a, b), g), v)| {
                vec![a.clone(), b.clone(), g.clone(), v.clone()]
            })
            .unwrap();
        finish(
            b,
            source,
            rows,
            if case.indexed { groups } else { value_groups },
            case,
        )
    }
}
fn run(case: Case) {
    let mut h = build(case);
    let special = !case.indexed && !case.filtered;
    let mut facts = vec![
        fact(E, 0, 0, 0),
        fact(G, 0, 0, 0),
        fact(V, 0, 0, 0),
        fact(VG, 0, 0, 0),
    ];
    if special && case.arity == 3 {
        facts.push(fact(VG, 1, 0, 0));
    }
    if !special {
        facts.extend([
            fact(E, 1, 1, 0),
            fact(E, 2, 0, usize::from(case.arity == 4 && !case.indexed)),
            fact(G, 1, 0, 0),
            fact(VG, 1, 0, 0),
        ]);
        let count = if case.arity == 4 && !case.indexed {
            2
        } else {
            5
        };
        facts.extend((1..count).map(|i| fact(V, i, i % 2, 0)));
    }
    h.put(facts);
    let (before, after, removed) = if special {
        let row = if case.arity == 3 {
            vec![(VG, 1), (VG, 0), (G, 0)]
        } else {
            vec![(E, 0), (G, 0), (V, 0), (E, 0)]
        };
        let (before, after) = if case.absent {
            (vec![], vec![row])
        } else {
            (vec![row], vec![])
        };
        (
            before,
            after,
            if case.arity == 3 { (E, 0) } else { (VG, 0) },
        )
    } else if case.arity == 3 {
        // Original assertMatch ignores fact order. All three surviving permutations
        // contain these same facts; their multiplicity is preserved by the score.
        let (yes_before, yes_after) = match (case.indexed, case.filtered) {
            (false, true) => (1, 0),
            (true, false) => (3, 2),
            (true, true) => (1, 0),
            _ => unreachable!(),
        };
        let (n, m) = if case.absent {
            (3 - yes_before, 3 - yes_after)
        } else {
            (yes_before, yes_after)
        };
        (
            vec![vec![(E, 0), (E, 1), (E, 2)]; n],
            vec![vec![(E, 0), (E, 1), (E, 2)]; m],
            (G, usize::from(case.indexed)),
        )
    } else {
        let a = vec![(E, 0), (E, 1), (G, 0), (V, 0)];
        let b = vec![(E, 0), (E, 2), (G, 0), (V, 0)];
        let c = vec![(E, 1), (E, 2), (G, 1), (V, 0)];
        let (before, after) = match (case.indexed, case.filtered, case.absent) {
            (false, true, false) => (vec![a, b, c], vec![]),
            (false, true, true) => (vec![], vec![a, b, c]),
            (true, false, false) => (vec![a.clone(), b.clone(), c], vec![a, b]),
            (true, false, true) => (vec![], vec![a]),
            (true, true, _) => (vec![c], vec![]),
            _ => unreachable!(),
        };
        (before, after, if case.indexed { (G, 1) } else { (VG, 0) })
    };
    h.check(before);
    h.remove(removed);
    h.check(after);
}
mod tri {
    use super::*;
    #[test]
    fn if_exists_0_joiner_0_filter() {
        run(Case {
            arity: 3,
            indexed: false,
            filtered: false,
            absent: false,
        })
    }
    #[test]
    fn if_exists_0_join_1_filter() {
        run(Case {
            arity: 3,
            indexed: false,
            filtered: true,
            absent: false,
        })
    }
    #[test]
    fn if_exists_1_join_0_filter() {
        run(Case {
            arity: 3,
            indexed: true,
            filtered: false,
            absent: false,
        })
    }
    #[test]
    fn if_exists_1_join_1_filter() {
        run(Case {
            arity: 3,
            indexed: true,
            filtered: true,
            absent: false,
        })
    }
    #[test]
    fn if_not_exists_0_joiner_0_filter() {
        run(Case {
            arity: 3,
            indexed: false,
            filtered: false,
            absent: true,
        })
    }
    #[test]
    fn if_not_exists_0_join_1_filter() {
        run(Case {
            arity: 3,
            indexed: false,
            filtered: true,
            absent: true,
        })
    }
    #[test]
    fn if_not_exists_1_join_0_filter() {
        run(Case {
            arity: 3,
            indexed: true,
            filtered: false,
            absent: true,
        })
    }
    #[test]
    fn if_not_exists_1_join_1_filter() {
        run(Case {
            arity: 3,
            indexed: true,
            filtered: true,
            absent: true,
        })
    }
}
mod quad {
    use super::*;
    #[test]
    fn if_exists_0_joiner_0_filter() {
        run(Case {
            arity: 4,
            indexed: false,
            filtered: false,
            absent: false,
        })
    }
    #[test]
    fn if_exists_0_join_1_filter() {
        run(Case {
            arity: 4,
            indexed: false,
            filtered: true,
            absent: false,
        })
    }
    #[test]
    fn if_exists_1_join_0_filter() {
        run(Case {
            arity: 4,
            indexed: true,
            filtered: false,
            absent: false,
        })
    }
    #[test]
    fn if_exists_1_join_1_filter() {
        run(Case {
            arity: 4,
            indexed: true,
            filtered: true,
            absent: false,
        })
    }
    #[test]
    fn if_not_exists_0_joiner_0_filter() {
        run(Case {
            arity: 4,
            indexed: false,
            filtered: false,
            absent: true,
        })
    }
    #[test]
    fn if_not_exists_0_join_1_filter() {
        run(Case {
            arity: 4,
            indexed: false,
            filtered: true,
            absent: true,
        })
    }
    #[test]
    fn if_not_exists_1_join_0_filter() {
        run(Case {
            arity: 4,
            indexed: true,
            filtered: false,
            absent: true,
        })
    }
    #[test]
    fn if_not_exists_1_join_1_filter() {
        run(Case {
            arity: 4,
            indexed: true,
            filtered: true,
            absent: true,
        })
    }
}
