// Adapted from pinned Timefold Community 2.7.0 stream tests; Apache-2.0.
// See compat/provenance.json for all test, fixture and assertion provenance.
use std::fmt::Debug;
use timetuck::{
    SimpleScore, StreamConstraint, StreamGraph, StreamGraphBuilder, StreamPort, StreamSource,
    TupleChange as Change,
};
#[derive(Clone)]
struct Entity {
    id: usize,
    group: usize,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fact {
    E(usize),
    G(usize),
}
use Fact::{E, G};
type Rows = Vec<Vec<Fact>>;
fn expected(arity: usize, duplicates: bool, distinct: bool) -> (Rows, Rows) {
    // Exact source assertion multisets, with zero-based stable fact IDs.
    match (arity, duplicates, distinct) {
        (1, true, true) => (vec![vec![G(0)], vec![G(1)]], vec![vec![G(0)], vec![G(1)]]),
        (1, false, _) => (vec![vec![G(0)], vec![G(1)]], vec![vec![G(1)]]),
        (2, false, _) => (
            vec![vec![E(1), G(0)], vec![E(0), G(1)], vec![E(0), G(0)]],
            vec![vec![E(1), G(0)]],
        ),
        (2, true, true) => (
            vec![
                vec![E(0), G(0)],
                vec![E(0), G(1)],
                vec![E(1), G(1)],
                vec![E(1), G(0)],
            ],
            vec![vec![E(1), G(0)], vec![E(1), G(1)]],
        ),
        (3, true, false) => (
            vec![
                vec![E(0), E(1), G(0)],
                vec![E(0), E(1), G(0)],
                vec![E(0), E(1), G(1)],
                vec![E(0), E(2), G(0)],
                vec![E(0), E(2), G(1)],
                vec![E(0), E(2), G(1)],
                vec![E(1), E(2), G(0)],
                vec![E(1), E(2), G(0)],
                vec![E(1), E(2), G(1)],
            ],
            vec![],
        ),
        (3, _, _) => (
            vec![
                vec![E(0), E(1), G(0)],
                vec![E(0), E(1), G(1)],
                vec![E(0), E(2), G(0)],
                vec![E(0), E(2), G(1)],
                vec![E(1), E(2), G(0)],
                vec![E(1), E(2), G(1)],
            ],
            vec![],
        ),
        (4, true, false) => (
            vec![
                vec![E(0), E(1), E(2), G(0)],
                vec![E(0), E(1), E(2), G(0)],
                vec![E(0), E(1), E(2), G(1)],
                vec![E(0), E(2), E(1), G(0)],
                vec![E(0), E(2), E(1), G(0)],
                vec![E(0), E(2), E(1), G(1)],
                vec![E(1), E(2), E(0), G(0)],
                vec![E(1), E(2), E(0), G(0)],
                vec![E(1), E(2), E(0), G(1)],
            ],
            vec![],
        ),
        (4, _, _) => (
            vec![
                vec![E(0), E(1), E(2), G(0)],
                vec![E(0), E(1), E(2), G(1)],
                vec![E(0), E(2), E(1), G(0)],
                vec![E(0), E(2), E(1), G(1)],
                vec![E(1), E(2), E(0), G(0)],
                vec![E(1), E(2), E(0), G(1)],
            ],
            vec![],
        ),
        _ => panic!("unadapted case"),
    }
}
fn assert_matches<I: Ord + Clone + Debug + 'static>(
    g: &StreamGraph<'_, SimpleScore>,
    c: &StreamConstraint<I, Vec<Fact>>,
    mut expected: Rows,
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
            .map(|row| (row, SimpleScore::new(-1)))
            .collect::<Vec<_>>()
    );
    let score = SimpleScore::new(-(expected.len() as i64));
    assert_eq!(g.score(), score);
    assert_eq!(
        g.constraint_scores().collect::<Vec<_>>(),
        [("testConstraint", score)]
    );
}
fn execute<I: Ord + Clone + Debug + 'static>(
    mut g: StreamGraph<'_, SimpleScore>,
    source: StreamSource<usize, Entity>,
    terminal: StreamConstraint<I, Vec<Fact>>,
    count: usize,
    expected: (Rows, Rows),
) {
    let mut t = g.transaction();
    for id in 0..count {
        t.apply(&source, Change::put(id, Entity { id, group: id % 2 }))
            .unwrap();
    }
    t.commit().unwrap();
    assert_matches(&g, &terminal, expected.0);
    let mut t = g.transaction();
    t.apply(&source, Change::retract(0)).unwrap();
    t.commit().unwrap();
    assert_matches(&g, &terminal, expected.1);
    g.assert_consistent().unwrap();
}
fn finish<I: Ord + Clone + Debug + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    port: StreamPort<I, Vec<Fact>>,
    config: (usize, bool, bool),
) {
    let (arity, duplicates, distinct) = config;
    let expected = expected(arity, duplicates, distinct);
    let count = if arity == 1 { 2 } else { 3 };
    if distinct {
        let port = b.distinct(&port).unwrap();
        let c = b
            .score("testConstraint", &port, |_| SimpleScore::new(-1))
            .unwrap();
        execute(b.build(), source, c, count, expected);
    } else {
        let c = b
            .score("testConstraint", &port, |_| SimpleScore::new(-1))
            .unwrap();
        execute(b.build(), source, c, count, expected);
    }
}
fn run(arity: usize, duplicates: bool, distinct: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let config = (arity, duplicates, distinct);
    if arity == 1 {
        let port = b
            .flat_map(source.port(), move |e| {
                if duplicates {
                    vec![vec![G(0)], vec![G(0)], vec![G(1)]]
                } else {
                    vec![vec![G(e.group)]]
                }
            })
            .unwrap();
        return finish(b, source, port, config);
    }
    let pairs = b
        .join_equal(source.port(), source.port(), |_| (), |_| ())
        .unwrap();
    let pairs = b.filter(&pairs, |(a, b)| a.id < b.id).unwrap();
    if arity == 2 {
        let port = b
            .flat_map(&pairs, move |(a, b)| {
                let groups = if duplicates {
                    vec![b.group, 0, 1]
                } else {
                    vec![b.group]
                };
                groups
                    .into_iter()
                    .map(|g| vec![E(a.id), G(g)])
                    .collect::<Vec<_>>()
            })
            .unwrap();
        return finish(b, source, port, config);
    }
    let triple = b.join_equal(&pairs, source.port(), |_| (), |_| ()).unwrap();
    let triple = b
        .filter(&triple, |((a, b), c)| a.id != c.id && b.id != c.id)
        .unwrap();
    if arity == 3 {
        let port = b
            .flat_map(&triple, move |((a, b), c)| {
                let groups = if duplicates {
                    vec![c.group, 0, 1]
                } else {
                    vec![c.group, 1 - c.group]
                };
                groups
                    .into_iter()
                    .map(|g| vec![E(a.id), E(b.id), G(g)])
                    .collect::<Vec<_>>()
            })
            .unwrap();
        return finish(b, source, port, config);
    }
    let quad = b
        .join_equal(&triple, source.port(), |_| (), |_| ())
        .unwrap();
    let quad = b
        .filter(&quad, |(((a, b), _), d)| a.id != d.id && b.id != d.id)
        .unwrap();
    let port = b
        .flat_map(&quad, move |(((a, b), c), _)| {
            let groups = if duplicates {
                vec![0, 0, 1]
            } else {
                vec![0, 1]
            };
            groups
                .into_iter()
                .map(|g| vec![E(a.id), E(b.id), E(c.id), G(g)])
                .collect::<Vec<_>>()
        })
        .unwrap();
    finish(b, source, port, config);
}
#[test]
fn uni_flatten() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let port = b
        .flat_map(source.port(), |e| {
            [
                vec![E(e.id), G(0)],
                vec![E(e.id), G(0)],
                vec![E(e.id), G(1)],
            ]
        })
        .unwrap();
    let port = b.distinct(&port).unwrap();
    let c = b
        .score("testConstraint", &port, |_| SimpleScore::new(-1))
        .unwrap();
    execute(
        b.build(),
        source,
        c,
        2,
        (
            vec![
                vec![E(0), G(0)],
                vec![E(0), G(1)],
                vec![E(1), G(0)],
                vec![E(1), G(1)],
            ],
            vec![vec![E(1), G(0)], vec![E(1), G(1)]],
        ),
    );
}
#[test]
fn uni_flatten_last_without_duplicates() {
    run(1, false, false);
}
#[test]
fn uni_flatten_last_and_distinct_with_duplicates() {
    run(1, true, true);
}
#[test]
fn uni_flatten_last_and_distinct_without_duplicates() {
    run(1, false, true);
}
#[test]
fn bi_flatten_last_without_duplicates() {
    run(2, false, false);
}
#[test]
fn bi_flatten_last_and_distinct_with_duplicates() {
    run(2, true, true);
}
#[test]
fn bi_flatten_last_and_distinct_without_duplicates() {
    run(2, false, true);
}
#[test]
fn tri_flatten_last_with_duplicates() {
    run(3, true, false);
}
#[test]
fn tri_flatten_last_without_duplicates() {
    run(3, false, false);
}
#[test]
fn tri_flatten_last_and_distinct_with_duplicates() {
    run(3, true, true);
}
#[test]
fn tri_flatten_last_and_distinct_without_duplicates() {
    run(3, false, true);
}
#[test]
fn quad_flatten_last_with_duplicates() {
    run(4, true, false);
}
#[test]
fn quad_flatten_last_without_duplicates() {
    run(4, false, false);
}
#[test]
fn quad_flatten_last_and_distinct_with_duplicates() {
    run(4, true, true);
}
#[test]
fn quad_flatten_last_and_distinct_without_duplicates() {
    run(4, false, true);
}
