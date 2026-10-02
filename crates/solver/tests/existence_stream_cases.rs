// Adapted from Timefold Community 2.7.0 AbstractUniConstraintStreamTest.
// Apache-2.0; see THIRD_PARTY_NOTICES.md and compat/provenance.json.
// Stable IDs and explicit assigned filtering replace Java fact discovery.
use std::fmt::Debug;
use timetuck::{
    ExistenceMode as Mode, SimpleScore, StreamConstraint, StreamGraph, StreamGraphBuilder,
    TupleChange as Change,
};

fn assert_matches<I: Ord + Clone + Debug + 'static, T: Clone + Debug + PartialEq + 'static>(
    graph: &StreamGraph<'_, SimpleScore>,
    terminal: &StreamConstraint<I, T>,
    expected: Vec<(I, T)>,
) {
    let score = SimpleScore::new(-(expected.len() as i64));
    assert_eq!(graph.score(), score);
    assert_eq!(
        graph.constraint_scores().collect::<Vec<_>>(),
        [("testConstraint", score)]
    );
    assert_eq!(
        graph
            .constraint_matches(terminal)
            .unwrap()
            .iter()
            .map(|(id, (row, s))| (id.clone(), row.clone(), *s))
            .collect::<Vec<_>>(),
        expected
            .into_iter()
            .map(|(id, row)| (id, row, SimpleScore::new(-1)))
            .collect::<Vec<_>>()
    );
}
fn no_joiner(mode: Mode) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let values = b.source::<usize, &'static str>();
    let groups = b.source::<usize, &'static str>();
    let stream = b
        .existence(
            values.port(),
            groups.port(),
            mode,
            |_| (),
            |_| (),
            |_, _| true,
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &stream, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for (id, code) in [(0, "Generated ValueGroup 0"), (1, "MyValueGroup")] {
        t.apply(&values, Change::put(id, code)).unwrap();
    }
    for (id, code) in [(0, "Generated EntityGroup 0"), (1, "MyEntityGroup")] {
        t.apply(&groups, Change::put(id, code)).unwrap();
    }
    t.commit().unwrap();
    let expected = if mode == Mode::Exists {
        vec![(0, "Generated ValueGroup 0"), (1, "MyValueGroup")]
    } else {
        vec![]
    };
    assert_matches(&g, &terminal, expected.clone());
    let mut t = g.transaction();
    t.apply(&groups, Change::retract(1)).unwrap();
    t.commit().unwrap();
    assert_matches(&g, &terminal, expected);
}
#[test]
fn if_exists_0_joiner_0_filter() {
    no_joiner(Mode::Exists);
}
#[test]
fn if_not_exists_0_joiner_0_filter() {
    no_joiner(Mode::NotExists);
}

#[derive(Clone, Debug, PartialEq)]
struct Entity {
    id: usize,
    group: usize,
    value: Option<usize>,
    code: &'static str,
}
fn entities() -> Vec<Entity> {
    [
        (0, 0, "Generated Entity 0"),
        (1, 1, "MyEntity 1"),
        (2, 0, "MyEntity 2"),
    ]
    .into_iter()
    .map(|(id, group, code)| Entity {
        id,
        group,
        code,
        value: Some(0),
    })
    .collect()
}
fn group_case(mode: Mode, variant: usize) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let groups = b.source::<usize, (usize, &'static str)>();
    let stream = b
        .existence(
            source.port(),
            groups.port(),
            mode,
            move |e| if variant == 0 { 0 } else { e.group },
            move |g| if variant == 0 { 0 } else { g.0 },
            move |e, g| match variant {
                0 => e.group == g.0,
                1 => true,
                _ => e.code.contains("MyEntity") || g.1.contains("MyEntity"),
            },
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &stream, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let rows = entities();
    let mut t = g.transaction();
    for e in &rows {
        t.apply(&source, Change::put(e.id, e.clone())).unwrap();
    }
    for row in [(0, "Generated EntityGroup 0"), (1, "MyEntityGroup")] {
        t.apply(&groups, Change::put(row.0, row)).unwrap();
    }
    t.commit().unwrap();
    let before: &[usize] = match (mode, variant) {
        (Mode::Exists, 2) => &[1, 2],
        (Mode::Exists, _) => &[0, 1, 2],
        (Mode::NotExists, 2) => &[0],
        _ => &[],
    };
    assert_matches(
        &g,
        &terminal,
        before.iter().map(|&i| (i, rows[i].clone())).collect(),
    );
    let mut t = g.transaction();
    t.apply(&groups, Change::retract(1)).unwrap();
    t.commit().unwrap();
    let after: &[usize] = match (mode, variant) {
        (Mode::Exists, 2) => &[2],
        (Mode::Exists, _) => &[0, 2],
        (Mode::NotExists, 2) => &[0, 1],
        _ => &[1],
    };
    assert_matches(
        &g,
        &terminal,
        after.iter().map(|&i| (i, rows[i].clone())).collect(),
    );
}
#[test]
fn if_exists_0_join_1_filter() {
    group_case(Mode::Exists, 0);
}
#[test]
fn if_exists_1_join_0_filter() {
    group_case(Mode::Exists, 1);
}
#[test]
fn if_exists_1_join_1_filter() {
    group_case(Mode::Exists, 2);
}
#[test]
fn if_not_exists_0_join_1_filter() {
    group_case(Mode::NotExists, 0);
}
#[test]
fn if_not_exists_1_join_0_filter() {
    group_case(Mode::NotExists, 1);
}
#[test]
fn if_not_exists_1_join_1_filter() {
    group_case(Mode::NotExists, 2);
}
fn other_case(mode: Mode, unassigned: bool) {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let assigned = b.filter(source.port(), |e| e.value.is_some()).unwrap();
    let stream = b
        .existence(
            &assigned,
            &assigned,
            mode,
            |e| e.group,
            |e| e.group,
            |a, b| a.id != b.id,
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &stream, |_| SimpleScore::new(-1))
        .unwrap();
    let mut rows = entities();
    if unassigned {
        if mode == Mode::Exists {
            rows.push(Entity {
                id: 3,
                group: 0,
                value: None,
                code: "Entity with null var",
            });
        } else {
            rows[2].value = None;
            rows[2].code = "Entity with null var";
        }
    }
    let mut g = b.build();
    let mut t = g.transaction();
    for e in &rows {
        t.apply(&source, Change::put(e.id, e.clone())).unwrap();
    }
    t.commit().unwrap();
    let before: &[usize] = match (mode, unassigned) {
        (Mode::Exists, _) => &[0, 2],
        (Mode::NotExists, true) => &[0, 1],
        _ => &[1],
    };
    assert_matches(
        &g,
        &terminal,
        before.iter().map(|&i| (i, rows[i].clone())).collect(),
    );
    rows[2].group = 1;
    let mut t = g.transaction();
    t.apply(&source, Change::put(2, rows[2].clone())).unwrap();
    t.commit().unwrap();
    let after: &[usize] = match (mode, unassigned) {
        (Mode::Exists, _) => &[1, 2],
        (Mode::NotExists, true) => &[0, 1],
        _ => &[0],
    };
    assert_matches(
        &g,
        &terminal,
        after.iter().map(|&i| (i, rows[i].clone())).collect(),
    );
}
#[test]
fn if_exists_other_1_join_0_filter() {
    other_case(Mode::Exists, false);
}
#[test]
fn if_not_exists_other_1_join_0_filter() {
    other_case(Mode::NotExists, false);
}
#[test]
fn if_exists_does_not_include_unassigned() {
    other_case(Mode::Exists, true);
}
#[test]
fn if_not_exists_does_not_include_unassigned() {
    other_case(Mode::NotExists, true);
}
#[test]
fn if_exists_after_group_by() {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let entities = b.source::<usize, i64>();
    let extras = b.source::<usize, &'static str>();
    let group = b
        .group_collect(entities.port(), |&value| ((), value))
        .unwrap();
    let distinct = b.map(&group, |(_, s)| s.distinct_count).unwrap();
    let stream = b
        .existence(
            &distinct,
            extras.port(),
            Mode::Exists,
            |_| (),
            |_| (),
            |_, _| true,
        )
        .unwrap();
    let terminal = b
        .score("testConstraint", &stream, |_| SimpleScore::new(-1))
        .unwrap();
    let mut g = b.build();
    let mut t = g.transaction();
    for id in 0..2 {
        t.apply(&entities, Change::put(id, 1)).unwrap();
    }
    for row in [(0, "MyExtra 1"), (1, "MyExtra 2")] {
        t.apply(&extras, Change::put(row.0, row.1)).unwrap();
    }
    t.commit().unwrap();
    assert_matches(&g, &terminal, vec![((), 1)]);
    let mut t = g.transaction();
    t.apply(&entities, Change::put(1, 2)).unwrap();
    t.commit().unwrap();
    assert_matches(&g, &terminal, vec![((), 2)]);
    let mut t = g.transaction();
    t.apply(&entities, Change::retract(1)).unwrap();
    t.commit().unwrap();
    assert_matches(&g, &terminal, vec![((), 1)]);
}
