// Adapted from pinned Timefold Community 2.7.0 global groupBy tests.
// Apache-2.0. Test/fixture/helper provenance: compat/provenance.json.
use timetuck::*;
#[derive(Clone, Debug)]
struct Entity {
    id: usize,
    group: usize,
    value: usize,
    number: i64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct ResultRow {
    count: usize,
    distinct: Option<usize>,
    min: Option<i64>,
    max: Option<i64>,
    set: Option<Vec<usize>>,
}
fn expected(arity: usize, fields: usize, after: bool) -> ResultRow {
    let count = if fields == 1 {
        match (arity, after) {
            (1, false) => 10,
            (1, true) => 9,
            (2, false) | (3, false) => 3,
            (2, true) => 1,
            (3, true) => 2,
            (4, false) => 5,
            (4, true) => 2,
            _ => unreachable!(),
        }
    } else if arity == 1 {
        if after { 2 } else { 3 }
    } else if after {
        1
    } else {
        3
    };
    let distinct = if fields == 2 {
        Some(if arity == 1 {
            count
        } else if after {
            1
        } else {
            2
        })
    } else {
        None
    };
    let min = (fields >= 3).then_some(if after { 1 } else { 0 });
    let max = (fields >= 3).then_some(if arity == 1 { 2 } else { 1 });
    let set = (fields == 4).then(|| {
        if arity == 1 {
            if after { vec![1, 2] } else { vec![0, 1, 2] }
        } else if after {
            vec![1]
        } else {
            vec![0, 1]
        }
    });
    ResultRow {
        count,
        distinct,
        min,
        max,
        set,
    }
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    facts: (StreamSource<usize, usize>, StreamSource<usize, usize>),
    input: StreamPort<I, (usize, i64)>,
    rows: Vec<Entity>,
    dims: (usize, usize),
    config: (usize, usize),
) {
    let (arity, fields) = config;
    let summary = b
        .collect::<_, _, _, PairCollector<IntCollector, SetCollector<usize>>>(
            &input,
            |&(id, number)| (number, id),
        )
        .unwrap();
    let projected = b
        .map(&summary, move |(stats, ids)| ResultRow {
            count: stats.count,
            distinct: (fields == 2).then_some(ids.len()),
            min: if fields >= 3 { stats.min } else { None },
            max: if fields >= 3 { stats.max } else { None },
            set: (fields == 4).then(|| ids.keys().copied().collect()),
        })
        .unwrap();
    let terminal = b
        .score("testConstraint", &projected, move |r| {
            SimpleScore::new(if fields == 1 { -(r.count as i64) } else { -1 })
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for id in 0..dims.0 {
        t.apply(&facts.0, TupleChange::put(id, id)).unwrap();
    }
    for id in 0..dims.1 {
        t.apply(&facts.1, TupleChange::put(id, id)).unwrap();
    }
    for row in rows {
        t.apply(&source, TupleChange::put(row.id, row)).unwrap();
    }
    t.commit().unwrap();
    let assert_state = |g: &StreamGraph<'_, SimpleScore>, after| {
        let row = expected(arity, fields, after);
        let score = SimpleScore::new(if fields == 1 { -(row.count as i64) } else { -1 });
        assert_eq!(g.score(), score);
        assert_eq!(
            g.constraint_scores().collect::<Vec<_>>(),
            [("testConstraint", score)]
        );
        assert_eq!(
            g.constraint_matches(&terminal)
                .unwrap()
                .iter()
                .map(|(id, v)| (*id, v.clone()))
                .collect::<Vec<_>>(),
            vec![((), (row, score))]
        );
    };
    assert_state(&g, false);
    let mut t = g.transaction();
    t.apply(
        &source,
        TupleChange::retract(if arity == 1 && fields == 1 { 9 } else { 0 }),
    )
    .unwrap();
    t.commit().unwrap();
    assert_state(&g, true);
}
fn run(arity: usize, fields: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let groups = b.source::<usize, usize>();
    let values = b.source::<usize, usize>();
    let n = if arity == 1 && fields == 1 { 10 } else { 3 };
    let dims = (
        2,
        if fields == 1 && arity <= 2 {
            5
        } else if fields == 1 {
            2
        } else {
            1
        },
    );
    let rows = (0..n)
        .map(|id| Entity {
            id,
            group: if arity == 1 && fields == 1 {
                usize::from(id == 7 || id == 8)
            } else {
                id % 2
            },
            value: if id >= 7 { 0 } else { id % dims.1 },
            number: if fields >= 3 { id as i64 } else { 0 },
        })
        .collect();
    if arity == 1 {
        let input = b.map(source.port(), |e| (e.id, e.number)).unwrap();
        return finish(
            b,
            source,
            (groups, values),
            input,
            rows,
            dims,
            (arity, fields),
        );
    }
    if arity >= 3 && fields == 1 {
        let pair = b
            .join_equal(source.port(), groups.port(), |e| e.group, |g| *g)
            .unwrap();
        let triple = b
            .join_equal(&pair, values.port(), |(e, _)| e.value, |v| *v)
            .unwrap();
        if arity == 3 {
            let input = b.map(&triple, |((e, _), _)| (e.id, e.number)).unwrap();
            return finish(
                b,
                source,
                (groups, values),
                input,
                rows,
                dims,
                (arity, fields),
            );
        }
        let quad = b
            .join_equal(&triple, source.port(), |((_, g), _)| *g, |e| e.group)
            .unwrap();
        let input = b.map(&quad, |(((e, _), _), _)| (e.id, e.number)).unwrap();
        return finish(
            b,
            source,
            (groups, values),
            input,
            rows,
            dims,
            (arity, fields),
        );
    }
    let pair = b
        .join_equal(source.port(), source.port(), |_| (), |_| ())
        .unwrap();
    let pair = b.filter(&pair, |(a, b)| a.id < b.id).unwrap();
    if arity == 2 {
        let input = b.map(&pair, |(a, _)| (a.id, a.number)).unwrap();
        return finish(
            b,
            source,
            (groups, values),
            input,
            rows,
            dims,
            (arity, fields),
        );
    }
    let triple = b
        .join_equal(&pair, source.port(), |(a, _)| a.id, |e| e.id)
        .unwrap();
    if arity == 3 {
        let input = b.map(&triple, |((a, _), _)| (a.id, a.number)).unwrap();
        return finish(
            b,
            source,
            (groups, values),
            input,
            rows,
            dims,
            (arity, fields),
        );
    }
    let quad = b
        .join_equal(&triple, source.port(), |((_, b), _)| b.id, |e| e.id)
        .unwrap();
    let input = b.map(&quad, |(((a, _), _), _)| (a.id, a.number)).unwrap();
    finish(
        b,
        source,
        (groups, values),
        input,
        rows,
        dims,
        (arity, fields),
    );
}

#[test]
fn uni_group_by_0_mapping_1_collector() {
    run(1, 1);
}

#[test]
fn uni_group_by_0_mapping_2_collector() {
    run(1, 2);
}

#[test]
fn uni_group_by_0_mapping_3_collector() {
    run(1, 3);
}

#[test]
fn uni_group_by_0_mapping_4_collector() {
    run(1, 4);
}

#[test]
fn bi_group_by_0_mapping_1_collector() {
    run(2, 1);
}

#[test]
fn bi_group_by_0_mapping_2_collector() {
    run(2, 2);
}

#[test]
fn bi_group_by_0_mapping_3_collector() {
    run(2, 3);
}

#[test]
fn bi_group_by_0_mapping_4_collector() {
    run(2, 4);
}

#[test]
fn tri_group_by_0_mapping_1_collector() {
    run(3, 1);
}

#[test]
fn tri_group_by_0_mapping_2_collector() {
    run(3, 2);
}

#[test]
fn tri_group_by_0_mapping_3_collector() {
    run(3, 3);
}

#[test]
fn tri_group_by_0_mapping_4_collector() {
    run(3, 4);
}

#[test]
fn quad_group_by_0_mapping_1_collector() {
    run(4, 1);
}

#[test]
fn quad_group_by_0_mapping_2_collector() {
    run(4, 2);
}

#[test]
fn quad_group_by_0_mapping_3_collector() {
    run(4, 3);
}

#[test]
fn quad_group_by_0_mapping_4_collector() {
    run(4, 4);
}
