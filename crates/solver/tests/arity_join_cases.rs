// Adapted from pinned AbstractBi/Tri/QuadConstraintStreamTest and lavish fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[derive(Clone)]
struct Row {
    id: usize,
    value: usize,
    group: usize,
    text: Option<&'static str>,
    number: i64,
}
fn row(id: usize, value: usize) -> Row {
    Row {
        id,
        value,
        group: 1,
        text: Some(""),
        number: 1,
    }
}
type Source = StreamSource<usize, Row>;
type Reader = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Vec<Vec<usize>>;
struct Harness {
    graph: StreamGraph<'static, SimpleScore>,
    e: Source,
    v: Source,
    x: Source,
    read: Box<Reader>,
}
impl Harness {
    fn put(&mut self, kind: char, rows: Vec<Row>) {
        let source = match kind {
            'e' => &self.e,
            'v' => &self.v,
            'x' => &self.x,
            _ => unreachable!(),
        };
        let mut tx = self.graph.transaction();
        for row in rows {
            tx.apply(source, TupleChange::put(row.id, row)).unwrap();
        }
        tx.commit().unwrap();
    }
    fn remove(&mut self, id: usize) {
        let mut tx = self.graph.transaction();
        tx.apply(&self.e, TupleChange::retract(id)).unwrap();
        tx.commit().unwrap();
    }
    fn check(&self, expected: &[&[usize]]) {
        let mut expected: Vec<Vec<usize>> = expected.iter().map(|r| r.to_vec()).collect();
        expected.sort();
        assert_eq!((self.read)(&self.graph), expected);
        assert_eq!(
            self.graph.score(),
            SimpleScore::new(-(expected.len() as i64))
        );
    }
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    sources: (Source, Source, Source),
    port: StreamPort<I, Vec<Row>>,
) -> Harness {
    let terminal = b
        .score("testConstraint", &port, |_| SimpleScore::new(-1))
        .unwrap();
    Harness {
        graph: b.build(),
        e: sources.0,
        v: sources.1,
        x: sources.2,
        read: Box::new(move |g| {
            let mut out: Vec<_> = g
                .constraint_matches(&terminal)
                .unwrap()
                .values()
                .map(|(rows, score)| {
                    assert_eq!(*score, SimpleScore::new(-1));
                    rows.iter().map(|r| r.id).collect::<Vec<_>>()
                })
                .collect();
            out.sort();
            out
        }),
    }
}
fn sources(b: &mut StreamGraphBuilder<'static, SimpleScore>) -> (Source, Source, Source) {
    (b.source(), b.source(), b.source())
}
fn extra_filter<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    s: (Source, Source, Source),
    port: StreamPort<I, Vec<Row>>,
) -> Harness {
    let joined = b.join_equal(&port, s.2.port(), |_| (), |_| ()).unwrap();
    let joined = b
        .map(&joined, |(rows, x)| {
            let mut out = rows.clone();
            out.push(x.clone());
            out
        })
        .unwrap();
    let selected = b
        .filter(&joined, |rows| {
            rows[rows.len() - 2].id == 1 && rows.last().unwrap().id == 1
        })
        .unwrap();
    finish(b, s, selected)
}
fn filter_entity(arity: usize) {
    let mut b = StreamGraphBuilder::new();
    let s = sources(&mut b);
    let mut h = if arity == 4 {
        let pair = b
            .join_equal(s.0.port(), s.0.port(), |_| (), |_| ())
            .unwrap();
        let pair = b.filter(&pair, |(a, b)| a.id < b.id).unwrap();
        let joined = b
            .join_equal(&pair, s.1.port(), |(a, _)| a.value, |v| v.id)
            .unwrap();
        let joined = b
            .map(&joined, |((a, b), v)| vec![a.clone(), b.clone(), v.clone()])
            .unwrap();
        extra_filter(b, s, joined)
    } else {
        let joined = b
            .join_equal(s.0.port(), s.1.port(), |e| e.value, |v| v.id)
            .unwrap();
        let joined = b.map(&joined, |(e, v)| vec![e.clone(), v.clone()]).unwrap();
        if arity == 3 {
            extra_filter(b, s, joined)
        } else {
            let selected = b.filter(&joined, |rows| rows[1].id == 1).unwrap();
            finish(b, s, selected)
        }
    };
    h.put('v', vec![row(1, 0), row(2, 0)]);
    h.put('x', vec![row(1, 0), row(2, 0)]);
    h.put('e', vec![row(1, 1), row(2, 2), row(3, 1)]);
    match arity {
        2 => h.check(&[&[1, 1], &[3, 1]]),
        3 => h.check(&[&[1, 1, 1], &[3, 1, 1]]),
        4 => h.check(&[&[1, 2, 1, 1], &[1, 3, 1, 1]]),
        _ => unreachable!(),
    }
    h.put('e', vec![row(3, 2)]);
    match arity {
        2 => h.check(&[&[1, 1]]),
        3 => h.check(&[&[1, 1, 1]]),
        4 => h.check(&[&[1, 2, 1, 1], &[1, 3, 1, 1]]),
        _ => unreachable!(),
    }
    h.put('e', vec![row(2, 1)]);
    match arity {
        2 => h.check(&[&[1, 1], &[2, 1]]),
        3 => h.check(&[&[1, 1, 1], &[2, 1, 1]]),
        4 => h.check(&[&[1, 2, 1, 1], &[1, 3, 1, 1], &[2, 3, 1, 1]]),
        _ => unreachable!(),
    }
}
fn consecutive_finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    s: (Source, Source, Source),
    port: StreamPort<I, Vec<Row>>,
    repeat_first: bool,
) -> Harness {
    let first = b.filter(&port, |r| r[0].id != 1).unwrap();
    let first = if repeat_first {
        b.filter(&first, |r| r[0].id != 1).unwrap()
    } else {
        first
    };
    let second = b.filter(&first, |r| r[0].id != 2).unwrap();
    let third = b.filter(&second, |r| r[0].id != 3).unwrap();
    finish(b, s, third)
}
fn filter_consecutive(arity: usize) {
    let mut b = StreamGraphBuilder::new();
    let s = sources(&mut b);
    let pair = b
        .join_equal(s.0.port(), s.0.port(), |_| (), |_| ())
        .unwrap();
    let pair = b.filter(&pair, |(a, b)| a.id < b.id).unwrap();
    let mut h = if arity == 2 {
        let rows = b.map(&pair, |(a, b)| vec![a.clone(), b.clone()]).unwrap();
        consecutive_finish(b, s, rows, false)
    } else {
        let triple = b
            .join_equal(&pair, s.0.port(), |(a, _)| a.id, |c| c.id)
            .unwrap();
        if arity == 3 {
            let rows = b
                .map(&triple, |((a, b), c)| vec![a.clone(), b.clone(), c.clone()])
                .unwrap();
            consecutive_finish(b, s, rows, true)
        } else {
            let quad = b
                .join_equal(&triple, s.0.port(), |((a, _), _)| a.id, |d| d.id)
                .unwrap();
            let rows = b
                .map(&quad, |(((a, b), c), d)| {
                    vec![a.clone(), b.clone(), c.clone(), d.clone()]
                })
                .unwrap();
            consecutive_finish(b, s, rows, false)
        }
    };
    h.put('e', (1..=5).map(|id| row(id, id)).collect());
    match arity {
        2 => h.check(&[&[4, 5]]),
        3 => h.check(&[&[4, 5, 4]]),
        4 => h.check(&[&[4, 5, 4, 4]]),
        _ => unreachable!(),
    }
    h.remove(4);
    h.check(&[]);
}
fn extra_join<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    s: (Source, Source, Source),
    port: StreamPort<I, Vec<Row>>,
    equal: usize,
) -> Harness {
    let joined = b
        .join_equal(
            &port,
            s.2.port(),
            move |rows| {
                (
                    if equal > 0 { rows[0].text } else { None },
                    if equal > 1 { rows[0].number } else { 0 },
                )
            },
            move |x| {
                (
                    if equal > 0 { x.text } else { None },
                    if equal > 1 { x.number } else { 0 },
                )
            },
        )
        .unwrap();
    let rows = b
        .map(&joined, |(rows, x)| {
            let mut out = rows.clone();
            out.push(x.clone());
            out
        })
        .unwrap();
    finish(b, s, rows)
}
fn join_fixture(arity: usize, equal: usize, filtering: bool) -> Harness {
    let mut b = StreamGraphBuilder::new();
    let s = sources(&mut b);
    let mut h = if arity == 2 {
        let pair = b
            .join_equal(
                s.0.port(),
                s.1.port(),
                move |e| if filtering { 0 } else { e.value },
                move |v| if filtering { 0 } else { v.id },
            )
            .unwrap();
        let pair = if filtering {
            b.filter(&pair, |(e, v)| e.value == v.id).unwrap()
        } else {
            pair
        };
        let rows = b.map(&pair, |(e, v)| vec![e.clone(), v.clone()]).unwrap();
        extra_join(b, s, rows, equal)
    } else {
        let pair = b
            .join_equal(s.0.port(), s.0.port(), |_| (), |_| ())
            .unwrap();
        let pair = b.filter(&pair, |(a, b)| a.id < b.id).unwrap();
        if equal == 2 {
            let triple = b
                .join_equal(&pair, s.1.port(), |(a, _)| a.value, |v| v.id)
                .unwrap();
            let rows = b
                .map(&triple, |((a, b), v)| vec![a.clone(), b.clone(), v.clone()])
                .unwrap();
            extra_join(b, s, rows, equal)
        } else {
            // Source key picks the third identity from the original three entities.
            let triple = b
                .join_equal(
                    &pair,
                    s.0.port(),
                    |(a, b)| (1..=3).find(|id| *id != a.id && *id != b.id).unwrap(),
                    |c| c.id,
                )
                .unwrap();
            let rows = b
                .map(&triple, |((a, b), c)| vec![a.clone(), b.clone(), c.clone()])
                .unwrap();
            extra_join(b, s, rows, equal)
        }
    };
    let mut e1 = row(1, 1);
    e1.text = Some("MyString");
    e1.number = 7;
    let mut e2 = row(2, 2);
    e2.text = None;
    e2.number = 8;
    let mut e3 = row(3, 2);
    e3.text = None;
    let mut x1 = row(1, 0);
    x1.text = Some("MyString");
    x1.number = 8;
    let mut x2 = row(2, 0);
    x2.text = None;
    x2.number = 7;
    let mut x3 = row(3, 0);
    x3.text = Some("MyString");
    x3.number = 7;
    h.put('v', vec![row(1, 0), row(2, 0)]);
    h.put('x', vec![x1, x2, x3]);
    h.put(
        'e',
        if arity == 3 && equal < 2 {
            vec![e1, e2, e3]
        } else {
            vec![e1, e2]
        },
    );
    h
}
fn bi_join(equal: usize, filtering: bool) {
    let mut h = join_fixture(2, equal, filtering);
    let expected: Vec<&[usize]> = match equal {
        0 => vec![
            &[1, 1, 1],
            &[1, 1, 2],
            &[1, 1, 3],
            &[2, 2, 1],
            &[2, 2, 2],
            &[2, 2, 3],
        ],
        1 => vec![&[1, 1, 1], &[1, 1, 3], &[2, 2, 2]],
        2 => vec![&[1, 1, 3]],
        _ => unreachable!(),
    };
    h.check(&expected);
    h.remove(2);
    let retained: Vec<_> = expected.into_iter().filter(|r| r[0] == 1).collect();
    h.check(&retained);
    h.remove(1);
    h.check(&[]);
}
fn tri_join(equal: usize) {
    let mut h = join_fixture(3, equal, false);
    match equal {
        0 => h.check(&[
            &[1, 2, 3, 1],
            &[1, 2, 3, 2],
            &[1, 2, 3, 3],
            &[1, 3, 2, 1],
            &[1, 3, 2, 2],
            &[1, 3, 2, 3],
            &[2, 3, 1, 1],
            &[2, 3, 1, 2],
            &[2, 3, 1, 3],
        ]),
        1 => h.check(&[
            &[1, 2, 3, 1],
            &[1, 2, 3, 3],
            &[1, 3, 2, 1],
            &[1, 3, 2, 3],
            &[2, 3, 1, 2],
        ]),
        2 => h.check(&[&[1, 2, 1, 3]]),
        _ => unreachable!(),
    };
    h.remove(if equal == 0 { 2 } else { 1 });
    h.check(&[]);
}
fn mixed_join() {
    let mut b = StreamGraphBuilder::new();
    let s = sources(&mut b);
    let pair = b
        .join_equal(s.0.port(), s.1.port(), |e| e.value, |v| v.id)
        .unwrap();
    let pair = b.filter(&pair, |(_, v)| v.id == 1).unwrap();
    let rows = b.map(&pair, |(e, v)| vec![e.clone(), v.clone()]).unwrap();
    let mut h = finish(b, s, rows);
    h.put('v', vec![row(1, 0), row(2, 0)]);
    h.put('e', vec![row(1, 1), row(2, 2)]);
    h.check(&[&[1, 1]]);
    h.remove(2);
    h.check(&[&[1, 1]]);
    h.remove(1);
    h.check(&[]);
}
fn unchanged_key(right: bool, indexed: bool) {
    let mut b = StreamGraphBuilder::new();
    let s = sources(&mut b);
    let pair = b
        .join_equal(
            s.0.port(),
            s.0.port(),
            move |e| if indexed { e.group } else { 0 },
            move |e| if indexed { e.group } else { 0 },
        )
        .unwrap();
    let pair = b
        .filter(&pair, move |(a, b)| {
            a.id != b.id && if right { b.value == 1 } else { a.value == 1 }
        })
        .unwrap();
    let rows = b.map(&pair, |(a, b)| vec![a.clone(), b.clone()]).unwrap();
    let mut h = finish(b, s, rows);
    let (initial, expected, after, changed) = if !indexed {
        (
            vec![row(1, 1), row(2, 2), row(3, 1)],
            vec![vec![1, 2], vec![1, 3], vec![3, 1], vec![3, 2]],
            vec![vec![3, 1], vec![3, 2]],
            1,
        )
    } else if right {
        let mut c = row(4, 2);
        c.group = 2;
        let mut d = row(5, 1);
        d.group = 2;
        (
            vec![row(1, 2), row(2, 2), row(3, 1), c, d],
            vec![vec![1, 3], vec![2, 3], vec![4, 5]],
            vec![vec![4, 5]],
            3,
        )
    } else {
        let mut c = row(4, 1);
        c.group = 2;
        let mut d = row(5, 2);
        d.group = 2;
        (
            vec![row(1, 1), row(2, 2), row(3, 2), c, d],
            vec![vec![1, 2], vec![1, 3], vec![4, 5]],
            vec![vec![4, 5]],
            1,
        )
    };
    h.put('e', initial);
    h.check(&expected.iter().map(Vec::as_slice).collect::<Vec<_>>());
    h.put('e', vec![row(changed, 2)]);
    h.check(&after.iter().map(Vec::as_slice).collect::<Vec<_>>());
    h.put('e', vec![row(changed, 1)]);
    h.check(&expected.iter().map(Vec::as_slice).collect::<Vec<_>>());
}
mod bi {
    use super::*;
    #[test]
    fn filter_entity() {
        super::filter_entity(2)
    }
    #[test]
    fn filter_consecutive() {
        super::filter_consecutive(2)
    }
    #[test]
    fn join_0() {
        bi_join(0, false)
    }
    #[test]
    fn join_1_equal() {
        bi_join(1, false)
    }
    #[test]
    fn join_1_filtering() {
        bi_join(1, true)
    }
    #[test]
    fn join_2_equal() {
        bi_join(2, false)
    }
    #[test]
    fn join_mixed_equals_and_filtering() {
        mixed_join()
    }
    #[test]
    fn join_filtering_update_left_unchanged_key() {
        unchanged_key(false, true)
    }
    #[test]
    fn join_filtering_update_right_unchanged_key() {
        unchanged_key(true, true)
    }
    #[test]
    fn join_1_filtering_update_unchanged_key() {
        unchanged_key(false, false)
    }
}
mod tri {
    use super::*;
    #[test]
    fn filter_entity() {
        super::filter_entity(3)
    }
    #[test]
    fn filter_consecutive() {
        super::filter_consecutive(3)
    }
    #[test]
    fn join_0() {
        tri_join(0)
    }
    #[test]
    fn join_1_equal() {
        tri_join(1)
    }
    #[test]
    fn join_2_equal() {
        tri_join(2)
    }
}
mod quad {
    #[test]
    fn filter_entity() {
        super::filter_entity(4)
    }
    #[test]
    fn filter_consecutive() {
        super::filter_consecutive(4)
    }
}
