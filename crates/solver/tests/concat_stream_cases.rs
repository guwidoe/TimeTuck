// Adapted from pinned Timefold Community 2.7.0 stream tests; Apache-2.0.
// Test and fixture attribution: compat/provenance.json.
use std::fmt::Debug;
use timetuck::*;
#[derive(Clone)]
struct Entity {
    id: usize,
    value: usize,
}
type Row = Vec<Option<usize>>;
type Rows = Vec<Row>;
fn assert_matches<I: Ord + Debug + 'static>(
    g: &StreamGraph<'_, SimpleScore>,
    c: &StreamConstraint<I, Row>,
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
            .map(|r| (r, SimpleScore::new(-1)))
            .collect::<Vec<_>>()
    );
    let score = SimpleScore::new(-(expected.len() as i64));
    assert_eq!(g.score(), score);
    assert_eq!(
        g.constraint_scores().collect::<Vec<_>>(),
        [("testConstraint", score)]
    );
}
fn first(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    source: &StreamSource<usize, Entity>,
    value: usize,
) -> StreamPort<usize, Vec<Entity>> {
    let filtered = b.filter(source.port(), move |e| e.value == value).unwrap();
    b.map(&filtered, |e| vec![e.clone()]).unwrap()
}
fn append<I: Ord + Clone + 'static>(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    source: &StreamSource<usize, Entity>,
    input: &StreamPort<I, Vec<Entity>>,
    value: usize,
) -> StreamPort<(I, usize), Vec<Entity>> {
    let filtered = b.filter(source.port(), move |e| e.value == value).unwrap();
    let joined = b.join_equal(input, &filtered, |_| (), |_| ()).unwrap();
    b.map(&joined, |(prefix, last)| {
        let mut rows = prefix.clone();
        rows.push(last.clone());
        rows
    })
    .unwrap()
}
macro_rules! branch {
    ($b:ident,$s:ident,$v:expr,1) => {
        first(&mut $b, &$s, $v[0])
    };
    ($b:ident,$s:ident,$v:expr,2) => {{
        let p = branch!($b, $s, $v, 1);
        append(&mut $b, &$s, &p, $v[1])
    }};
    ($b:ident,$s:ident,$v:expr,3) => {{
        let p = branch!($b, $s, $v, 2);
        append(&mut $b, &$s, &p, $v[2])
    }};
    ($b:ident,$s:ident,$v:expr,4) => {{
        let p = branch!($b, $s, $v, 3);
        append(&mut $b, &$s, &p, $v[3])
    }};
}
fn execute<I: Ord + Clone + Debug + 'static>(
    mut g: StreamGraph<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    c: StreamConstraint<I, Row>,
    expected: (Rows, Rows),
) {
    let mut t = g.transaction();
    for id in 0..3 {
        t.apply(&source, TupleChange::put(id, Entity { id, value: id }))
            .unwrap();
    }
    t.commit().unwrap();
    assert_matches(&g, &c, expected.0);
    // Preserve source notification order: entity3 -> value2, entity2 -> value3.
    let mut t = g.transaction();
    t.apply(&source, TupleChange::put(2, Entity { id: 2, value: 1 }))
        .unwrap();
    t.apply(&source, TupleChange::put(1, Entity { id: 1, value: 2 }))
        .unwrap();
    t.commit().unwrap();
    assert_matches(&g, &c, expected.1);
    g.assert_consistent().unwrap();
}
fn finish<L: Ord + Clone + Debug + 'static, R: Ord + Clone + Debug + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    left: StreamPort<L, Vec<Entity>>,
    right: StreamPort<R, Vec<Entity>>,
    arity: usize,
    distinct: bool,
    expected: (Rows, Rows),
) {
    let padding = move |row: &Vec<Entity>| {
        let mut ids = row.iter().map(|e| Some(e.id)).collect::<Vec<_>>();
        ids.resize(arity, None);
        ids
    };
    let left = b.map(&left, padding).unwrap();
    let right = b.map(&right, padding).unwrap();
    let combined = b.concat(&left, &right).unwrap();
    if distinct {
        let port = b.distinct(&combined).unwrap();
        let c = b
            .score("testConstraint", &port, |_| SimpleScore::new(-1))
            .unwrap();
        execute(b.build(), source, c, expected);
    } else {
        let c = b
            .score("testConstraint", &combined, |_| SimpleScore::new(-1))
            .unwrap();
        execute(b.build(), source, c, expected);
    }
}
macro_rules! case {
    ($name:ident,$l:tt,$r:tt,$duplicates:expr,$distinct:expr,$before:expr,$after:expr) => {
        #[test]
        fn $name() {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let s = b.source::<usize, Entity>();
            let left = branch!(b, s, [0, 1, 2, 0], $l);
            let right_values = if $duplicates {
                [0, 1, 2, 0]
            } else {
                [1, 2, 0, 1]
            };
            let right = branch!(b, s, right_values, $r);
            finish(
                b,
                s,
                left,
                right,
                usize::max($l, $r),
                $distinct,
                ($before, $after),
            );
        }
    };
}
case!(
    uni_concat_uni_without_value_duplicates,
    1,
    1,
    false,
    false,
    vec![vec![Some(0)], vec![Some(1)]],
    vec![vec![Some(0)], vec![Some(2)]]
);
case!(
    uni_concat_uni_with_value_duplicates,
    1,
    1,
    true,
    false,
    vec![vec![Some(0)], vec![Some(0)]],
    vec![vec![Some(0)], vec![Some(0)]]
);
case!(
    uni_concat_and_distinct_uni_without_value_duplicates,
    1,
    1,
    false,
    true,
    vec![vec![Some(0)], vec![Some(1)]],
    vec![vec![Some(0)], vec![Some(2)]]
);
case!(
    uni_concat_and_distinct_uni_with_value_duplicates,
    1,
    1,
    true,
    true,
    vec![vec![Some(0)]],
    vec![vec![Some(0)]]
);
case!(
    uni_concat_bi_without_value_duplicates,
    1,
    2,
    false,
    false,
    vec![vec![Some(0), None], vec![Some(1), Some(2)]],
    vec![vec![Some(0), None], vec![Some(2), Some(1)]]
);
case!(
    uni_concat_and_distinct_bi_without_value_duplicates,
    1,
    2,
    false,
    true,
    vec![vec![Some(0), None], vec![Some(1), Some(2)]],
    vec![vec![Some(0), None], vec![Some(2), Some(1)]]
);
case!(
    uni_concat_tri_without_value_duplicates,
    1,
    3,
    false,
    false,
    vec![vec![Some(0), None, None], vec![Some(1), Some(2), Some(0)]],
    vec![vec![Some(0), None, None], vec![Some(2), Some(1), Some(0)]]
);
case!(
    uni_concat_and_distinct_tri_without_value_duplicates,
    1,
    3,
    false,
    true,
    vec![vec![Some(0), None, None], vec![Some(1), Some(2), Some(0)]],
    vec![vec![Some(0), None, None], vec![Some(2), Some(1), Some(0)]]
);
case!(
    uni_concat_quad_without_value_duplicates,
    1,
    4,
    false,
    false,
    vec![
        vec![Some(0), None, None, None],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), None, None, None],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    uni_concat_and_distinct_quad_without_value_duplicates,
    1,
    4,
    false,
    true,
    vec![
        vec![Some(0), None, None, None],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), None, None, None],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    bi_concat_uni_without_value_duplicates,
    2,
    1,
    false,
    false,
    vec![vec![Some(0), Some(1)], vec![Some(1), None]],
    vec![vec![Some(0), Some(2)], vec![Some(2), None]]
);
case!(
    bi_concat_and_distinct_uni_without_value_duplicates,
    2,
    1,
    false,
    true,
    vec![vec![Some(0), Some(1)], vec![Some(1), None]],
    vec![vec![Some(0), Some(2)], vec![Some(2), None]]
);
case!(
    bi_concat_bi_without_value_duplicates,
    2,
    2,
    false,
    false,
    vec![vec![Some(0), Some(1)], vec![Some(1), Some(2)]],
    vec![vec![Some(0), Some(2)], vec![Some(2), Some(1)]]
);
case!(
    bi_concat_bi_with_value_duplicates,
    2,
    2,
    true,
    false,
    vec![vec![Some(0), Some(1)], vec![Some(0), Some(1)]],
    vec![vec![Some(0), Some(2)], vec![Some(0), Some(2)]]
);
case!(
    bi_concat_and_distinct_bi_without_value_duplicates,
    2,
    2,
    false,
    true,
    vec![vec![Some(0), Some(1)], vec![Some(1), Some(2)]],
    vec![vec![Some(0), Some(2)], vec![Some(2), Some(1)]]
);
case!(
    bi_concat_and_distinct_bi_with_value_duplicates,
    2,
    2,
    true,
    true,
    vec![vec![Some(0), Some(1)]],
    vec![vec![Some(0), Some(2)]]
);
case!(
    bi_concat_tri_without_value_duplicates,
    2,
    3,
    false,
    false,
    vec![
        vec![Some(0), Some(1), None],
        vec![Some(1), Some(2), Some(0)]
    ],
    vec![
        vec![Some(0), Some(2), None],
        vec![Some(2), Some(1), Some(0)]
    ]
);
case!(
    bi_concat_and_distinct_tri_without_value_duplicates,
    2,
    3,
    false,
    true,
    vec![
        vec![Some(0), Some(1), None],
        vec![Some(1), Some(2), Some(0)]
    ],
    vec![
        vec![Some(0), Some(2), None],
        vec![Some(2), Some(1), Some(0)]
    ]
);
case!(
    bi_concat_quad_without_value_duplicates,
    2,
    4,
    false,
    false,
    vec![
        vec![Some(0), Some(1), None, None],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), Some(2), None, None],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    bi_concat_and_distinct_quad_without_value_duplicates,
    2,
    4,
    false,
    true,
    vec![
        vec![Some(0), Some(1), None, None],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), Some(2), None, None],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    tri_concat_uni_without_value_duplicates,
    3,
    1,
    false,
    false,
    vec![vec![Some(0), Some(1), Some(2)], vec![Some(1), None, None]],
    vec![vec![Some(0), Some(2), Some(1)], vec![Some(2), None, None]]
);
case!(
    tri_concat_and_distinct_uni_without_value_duplicates,
    3,
    1,
    false,
    true,
    vec![vec![Some(0), Some(1), Some(2)], vec![Some(1), None, None]],
    vec![vec![Some(0), Some(2), Some(1)], vec![Some(2), None, None]]
);
case!(
    tri_concat_bi_without_value_duplicates,
    3,
    2,
    false,
    false,
    vec![
        vec![Some(0), Some(1), Some(2)],
        vec![Some(1), Some(2), None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1)],
        vec![Some(2), Some(1), None]
    ]
);
case!(
    tri_concat_and_distinct_bi_without_value_duplicates,
    3,
    2,
    false,
    true,
    vec![
        vec![Some(0), Some(1), Some(2)],
        vec![Some(1), Some(2), None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1)],
        vec![Some(2), Some(1), None]
    ]
);
case!(
    tri_concat_tri_without_value_duplicates,
    3,
    3,
    false,
    false,
    vec![
        vec![Some(0), Some(1), Some(2)],
        vec![Some(1), Some(2), Some(0)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1)],
        vec![Some(2), Some(1), Some(0)]
    ]
);
case!(
    tri_concat_tri_with_value_duplicates,
    3,
    3,
    true,
    false,
    vec![
        vec![Some(0), Some(1), Some(2)],
        vec![Some(0), Some(1), Some(2)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1)],
        vec![Some(0), Some(2), Some(1)]
    ]
);
case!(
    tri_concat_and_distinct_tri_without_value_duplicates,
    3,
    3,
    false,
    true,
    vec![
        vec![Some(0), Some(1), Some(2)],
        vec![Some(1), Some(2), Some(0)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1)],
        vec![Some(2), Some(1), Some(0)]
    ]
);
case!(
    tri_concat_and_distinct_tri_with_value_duplicates,
    3,
    3,
    true,
    true,
    vec![vec![Some(0), Some(1), Some(2)]],
    vec![vec![Some(0), Some(2), Some(1)]]
);
case!(
    tri_concat_quad_without_value_duplicates,
    3,
    4,
    false,
    false,
    vec![
        vec![Some(0), Some(1), Some(2), None],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), None],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    tri_concat_and_distinct_quad_without_value_duplicates,
    3,
    4,
    false,
    true,
    vec![
        vec![Some(0), Some(1), Some(2), None],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), None],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    quad_concat_uni_without_value_duplicates,
    4,
    1,
    false,
    false,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), None, None, None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), None, None, None]
    ]
);
case!(
    quad_concat_and_distinct_uni_without_value_duplicates,
    4,
    1,
    false,
    true,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), None, None, None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), None, None, None]
    ]
);
case!(
    quad_concat_bi_without_value_duplicates,
    4,
    2,
    false,
    false,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), Some(2), None, None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), Some(1), None, None]
    ]
);
case!(
    quad_concat_and_distinct_bi_without_value_duplicates,
    4,
    2,
    false,
    true,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), Some(2), None, None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), Some(1), None, None]
    ]
);
case!(
    quad_concat_tri_without_value_duplicates,
    4,
    3,
    false,
    false,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), Some(2), Some(0), None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), Some(1), Some(0), None]
    ]
);
case!(
    quad_concat_and_distinct_tri_without_value_duplicates,
    4,
    3,
    false,
    true,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), Some(2), Some(0), None]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), Some(1), Some(0), None]
    ]
);
case!(
    quad_concat_quad_without_value_duplicates,
    4,
    4,
    false,
    false,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    quad_concat_quad_with_value_duplicates,
    4,
    4,
    true,
    false,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(0), Some(1), Some(2), Some(0)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(0), Some(2), Some(1), Some(0)]
    ]
);
case!(
    quad_concat_and_distinct_quad_without_value_duplicates,
    4,
    4,
    false,
    true,
    vec![
        vec![Some(0), Some(1), Some(2), Some(0)],
        vec![Some(1), Some(2), Some(0), Some(1)]
    ],
    vec![
        vec![Some(0), Some(2), Some(1), Some(0)],
        vec![Some(2), Some(1), Some(0), Some(2)]
    ]
);
case!(
    quad_concat_and_distinct_quad_with_value_duplicates,
    4,
    4,
    true,
    true,
    vec![vec![Some(0), Some(1), Some(2), Some(0)]],
    vec![vec![Some(0), Some(2), Some(1), Some(0)]]
);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Value(usize),
    Code(String),
}
use Key::{Code as C, Value as V};
fn grouped<I: Ord + Clone + 'static>(
    b: &mut StreamGraphBuilder<'static, SimpleScore>,
    input: &StreamPort<I, Vec<Entity>>,
    arity: usize,
) -> StreamPort<Vec<Key>, (Vec<Key>, usize)> {
    b.group_collect_with::<_, _, _, (), CountCollector>(input, move |rows| {
        let key = if arity == 4 {
            let codes = ["Generated Value 0", "MyValue 2", "MyValue 3"];
            vec![
                V(rows[0].value),
                V(rows[1].value),
                C(format!("{}{}", codes[rows[2].value], codes[rows[3].value])),
            ]
        } else {
            rows.iter().map(|e| V(e.value)).collect()
        };
        (key, ())
    })
    .unwrap()
}
fn grouped_execute(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: StreamSource<usize, Entity>,
    left: StreamPort<Vec<Key>, (Vec<Key>, usize)>,
    right: StreamPort<Vec<Key>, (Vec<Key>, usize)>,
    expected: [Vec<(Vec<Key>, usize)>; 3],
) {
    let port = b.concat(&left, &right).unwrap();
    let c = b
        .score("testConstraint", &port, |(_, count)| {
            SimpleScore::new(-(*count as i64))
        })
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for id in 0..3 {
        t.apply(&source, TupleChange::put(id, Entity { id, value: id }))
            .unwrap();
    }
    t.commit().unwrap();
    for (step, mut rows) in expected.into_iter().enumerate() {
        if step != 0 {
            let mut t = g.transaction();
            let changes = if step == 1 {
                vec![(2, 1)]
            } else {
                vec![(0, 2), (2, 0)]
            };
            for (id, value) in changes {
                t.apply(&source, TupleChange::put(id, Entity { id, value }))
                    .unwrap();
            }
            t.commit().unwrap();
        }
        rows.sort();
        let mut actual = g
            .constraint_matches(&c)
            .unwrap()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        actual.sort();
        assert_eq!(
            actual,
            rows.iter()
                .cloned()
                .map(|r| {
                    let weight = SimpleScore::new(-(r.1 as i64));
                    (r, weight)
                })
                .collect::<Vec<_>>()
        );
        let score = SimpleScore::new(-rows.iter().map(|r| r.1 as i64).sum::<i64>());
        assert_eq!(g.score(), score);
        assert_eq!(
            g.constraint_scores().collect::<Vec<_>>(),
            [("testConstraint", score)]
        );
        g.assert_consistent().unwrap();
    }
}
macro_rules! group_case {
    ($name:ident,$arity:tt,$right:expr,$expected:expr) => {
        #[test]
        fn $name() {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let source = b.source::<usize, Entity>();
            let left = branch!(b, source, [0, 1, 2, 0], $arity);
            let right = branch!(b, source, $right, $arity);
            let left = grouped(&mut b, &left, $arity);
            let right = grouped(&mut b, &right, $arity);
            grouped_execute(b, source, left, right, $expected);
        }
    };
}
group_case!(
    uni_concat_after_group_by,
    1,
    [1, 2, 0, 1],
    [
        vec![(vec![V(0)], 1), (vec![V(1)], 1)],
        vec![(vec![V(0)], 1), (vec![V(1)], 2)],
        vec![(vec![V(0)], 1), (vec![V(1)], 1)]
    ]
);
group_case!(
    bi_concat_after_group_by,
    2,
    [1, 2, 0, 1],
    [
        vec![(vec![V(0), V(1)], 1), (vec![V(1), V(2)], 1)],
        vec![(vec![V(0), V(1)], 2)],
        vec![(vec![V(0), V(1)], 1), (vec![V(1), V(2)], 1)]
    ]
);
group_case!(
    tri_concat_after_group_by,
    3,
    [2, 1, 0, 2],
    [
        vec![(vec![V(0), V(1), V(2)], 1), (vec![V(2), V(1), V(0)], 1)],
        vec![],
        vec![(vec![V(0), V(1), V(2)], 1), (vec![V(2), V(1), V(0)], 1)]
    ]
);
group_case!(
    quad_concat_after_group_by,
    4,
    [2, 1, 0, 2],
    [
        vec![
            (vec![V(0), V(1), C("MyValue 3Generated Value 0".into())], 1),
            (vec![V(2), V(1), C("Generated Value 0MyValue 3".into())], 1)
        ],
        vec![],
        vec![
            (vec![V(0), V(1), C("MyValue 3Generated Value 0".into())], 1),
            (vec![V(2), V(1), C("Generated Value 0MyValue 3".into())], 1)
        ]
    ]
);
