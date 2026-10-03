// Adapted from pinned Timefold Community 2.7.0 stream tests; Apache-2.0.
// See compat/provenance.json for test, fixture and assertion-helper provenance.
use timetuck::*;
#[derive(Clone)]
struct Entity {
    id: usize,
    code: &'static str,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fact {
    Entity(usize),
    Number(i32),
}
use Fact::{Entity as E, Number as N};
fn run(arity: usize, expected: [Vec<Vec<Fact>>; 2]) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let expanded = b
        .map(source.port(), move |e| {
            let mut row = vec![E(e.id)];
            if arity > 1 {
                let index = e.code[e.code.len() - 1..].parse::<i32>().unwrap();
                for offset in 0..arity - 1 {
                    row.push(N(index + offset as i32));
                }
            }
            row
        })
        .unwrap();
    let filtered = if arity == 1 {
        let selected = b
            .filter(source.port(), |e| e.code == "Generated Value 0")
            .unwrap();
        b.map(&selected, |e| vec![E(e.id)]).unwrap()
    } else {
        b.filter(&expanded, |row| row[1] == N(0)).unwrap()
    };
    let completed = b
        .complement(
            &filtered,
            source.port(),
            |row| match row[0] {
                E(id) => id,
                _ => unreachable!(),
            },
            |e| e.id,
            move |e| {
                let mut row = vec![E(e.id)];
                row.extend([i32::MAX, -1, 0].into_iter().take(arity - 1).map(N));
                row
            },
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &completed, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, code) in [(0, "Generated Value 0"), (1, "MyValue 2"), (2, "MyValue 2")] {
        t.apply(&source, TupleChange::put(id, Entity { id, code }))
            .unwrap();
    }
    t.commit().unwrap();
    for (step, mut expected) in expected.into_iter().enumerate() {
        if step == 1 {
            let mut t = g.transaction();
            t.apply(
                &source,
                TupleChange::put(
                    1,
                    Entity {
                        id: 1,
                        code: "Generated Value 0",
                    },
                ),
            )
            .unwrap();
            t.commit().unwrap();
        }
        expected.sort();
        let mut actual = g
            .constraint_matches(&terminal)
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        actual.sort();
        assert_eq!(
            actual,
            expected
                .into_iter()
                .map(|row| (row, SimpleScore::new(-1)))
                .collect::<Vec<_>>()
        );
        assert_eq!(g.score(), SimpleScore::new(-3));
        assert_eq!(
            g.constraint_scores().collect::<Vec<_>>(),
            [("testConstraint", SimpleScore::new(-3))]
        );
        g.assert_consistent().unwrap();
    }
}
#[test]
fn uni_complement() {
    run(
        1,
        [
            vec![vec![E(0)], vec![E(1)], vec![E(2)]],
            vec![vec![E(0)], vec![E(1)], vec![E(2)]],
        ],
    );
}
#[test]
fn bi_complement() {
    run(
        2,
        [
            vec![
                vec![E(0), N(0)],
                vec![E(1), N(i32::MAX)],
                vec![E(2), N(i32::MAX)],
            ],
            vec![vec![E(0), N(0)], vec![E(1), N(0)], vec![E(2), N(i32::MAX)]],
        ],
    );
}
#[test]
fn tri_complement() {
    run(
        3,
        [
            vec![
                vec![E(0), N(0), N(1)],
                vec![E(1), N(i32::MAX), N(-1)],
                vec![E(2), N(i32::MAX), N(-1)],
            ],
            vec![
                vec![E(0), N(0), N(1)],
                vec![E(1), N(0), N(1)],
                vec![E(2), N(i32::MAX), N(-1)],
            ],
        ],
    );
}
#[test]
fn quad_complement() {
    run(
        4,
        [
            vec![
                vec![E(0), N(0), N(1), N(2)],
                vec![E(1), N(i32::MAX), N(-1), N(0)],
                vec![E(2), N(i32::MAX), N(-1), N(0)],
            ],
            vec![
                vec![E(0), N(0), N(1), N(2)],
                vec![E(1), N(0), N(1), N(2)],
                vec![E(2), N(i32::MAX), N(-1), N(0)],
            ],
        ],
    );
}
