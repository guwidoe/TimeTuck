// Adapted from pinned Timefold Community 2.7.0 stream tests; Apache-2.0.
// Test, fixture and assertion-helper provenance: compat/provenance.json.
use std::fmt::Debug;
use timetuck::*;
#[derive(Clone)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
    number: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fact {
    Entity(usize),
    Group(usize),
    Value(usize),
    Text(String),
    Number(i64),
}
use Fact::{Entity as E, Group as G, Number as N, Text as T, Value as V};
type Rows = Vec<Vec<Fact>>;
fn execute<I: Ord + Clone + Debug + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    port: StreamPort<I, Vec<Fact>>,
    dims: (usize, usize, usize),
    expected: [Rows; 2],
) {
    let c = b
        .score("testConstraint", &port, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for id in 0..dims.0 {
        t.apply(
            &source,
            TupleChange::put(
                id,
                Entity {
                    id,
                    group: id % dims.1,
                    value: id % dims.2,
                    number: [17, 311, 90001][id],
                },
            ),
        )
        .unwrap();
    }
    t.commit().unwrap();
    for (step, mut expected) in expected.into_iter().enumerate() {
        if step == 1 {
            let mut t = g.transaction();
            t.apply(&source, TupleChange::retract(0)).unwrap();
            t.commit().unwrap();
        }
        expected.sort();
        let mut actual = g
            .constraint_matches(&c)
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        actual.sort();
        let score = SimpleScore::new(-(expected.len() as i64));
        assert_eq!(
            actual,
            expected
                .into_iter()
                .map(|row| (row, SimpleScore::new(-1)))
                .collect::<Vec<_>>()
        );
        assert_eq!(g.score(), score);
        assert_eq!(
            g.constraint_scores().collect::<Vec<_>>(),
            [("testConstraint", score)]
        );
        g.assert_consistent().unwrap();
    }
}
fn uni(arity: usize, expected: [Rows; 2]) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let port = b
        .map(source.port(), move |e| {
            let mut row = vec![E(e.id), G(e.group)];
            if arity >= 3 {
                row.push(V(e.value));
            }
            if arity == 4 {
                row.push(T(format!("Generated Entity {}", e.id)));
            }
            row
        })
        .unwrap();
    execute(b, source, port, (2, 2, 2), expected);
}
fn bi(arity: usize, expected: [Rows; 2]) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let pairs = b
        .join_equal(source.port(), source.port(), |_| (), |_| ())
        .unwrap();
    let pairs = b.filter(&pairs, |(a, b)| a.id < b.id).unwrap();
    let port = b
        .map(&pairs, move |(a, b)| {
            let mut row = vec![E(a.id), E(b.id), N(a.number + b.number)];
            if arity == 4 {
                row.push(T(format!(
                    "Generated EntityGroup {}Generated EntityGroup {}",
                    a.group, b.group
                )));
            }
            row
        })
        .unwrap();
    execute(b, source, port, (3, 2, 2), expected);
}
#[test]
fn uni_expand_to_bi() {
    uni(
        2,
        [
            vec![vec![E(0), G(0)], vec![E(1), G(1)]],
            vec![vec![E(1), G(1)]],
        ],
    );
}
#[test]
fn uni_expand_to_tri() {
    uni(
        3,
        [
            vec![vec![E(0), G(0), V(0)], vec![E(1), G(1), V(1)]],
            vec![vec![E(1), G(1), V(1)]],
        ],
    );
}
#[test]
fn uni_expand_to_quad() {
    uni(
        4,
        [
            vec![
                vec![E(0), G(0), V(0), T("Generated Entity 0".into())],
                vec![E(1), G(1), V(1), T("Generated Entity 1".into())],
            ],
            vec![vec![E(1), G(1), V(1), T("Generated Entity 1".into())]],
        ],
    );
}
#[test]
fn bi_expand_to_tri() {
    bi(
        3,
        [
            vec![
                vec![E(0), E(1), N(328)],
                vec![E(1), E(2), N(90312)],
                vec![E(0), E(2), N(90018)],
            ],
            vec![vec![E(1), E(2), N(90312)]],
        ],
    );
}
#[test]
fn bi_expand_to_quad() {
    bi(
        4,
        [
            vec![
                vec![
                    E(0),
                    E(1),
                    N(328),
                    T("Generated EntityGroup 0Generated EntityGroup 1".into()),
                ],
                vec![
                    E(1),
                    E(2),
                    N(90312),
                    T("Generated EntityGroup 1Generated EntityGroup 0".into()),
                ],
                vec![
                    E(0),
                    E(2),
                    N(90018),
                    T("Generated EntityGroup 0Generated EntityGroup 0".into()),
                ],
            ],
            vec![vec![
                E(1),
                E(2),
                N(90312),
                T("Generated EntityGroup 1Generated EntityGroup 0".into()),
            ]],
        ],
    );
}
#[test]
fn tri_expand_to_quad() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let pairs = b
        .join_equal(source.port(), source.port(), |_| (), |_| ())
        .unwrap();
    let pairs = b.filter(&pairs, |(a, b)| a.id < b.id).unwrap();
    let triples = b.join_equal(&pairs, source.port(), |_| (), |_| ()).unwrap();
    let triples = b
        .filter(&triples, |((a, b), c)| a.id != c.id && b.id != c.id)
        .unwrap();
    let port = b
        .map(&triples, |((a, b), c)| {
            vec![E(a.id), E(b.id), E(c.id), N(a.number + b.number + c.number)]
        })
        .unwrap();
    execute(
        b,
        source,
        port,
        (3, 3, 1),
        [
            vec![
                vec![E(0), E(1), E(2), N(90329)],
                vec![E(0), E(2), E(1), N(90329)],
                vec![E(1), E(2), E(0), N(90329)],
            ],
            vec![],
        ],
    );
}
