// Adapted from pinned Community BavetRegressionTest.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

type Row = (usize, Option<usize>);
type ReadRows = dyn Fn(&StreamGraph<'_, SimpleScore>) -> Vec<Vec<Row>>;
#[derive(Clone, Copy, Debug)]
enum Shape {
    Join(bool),
    Exists(bool),
    Absent(bool),
    Map,
    Concat,
}
struct Network {
    graph: StreamGraph<'static, SimpleScore>,
    source: PlanningStreamSource<usize>,
    read: Box<ReadRows>,
}
fn finish<I: Ord + Clone + 'static>(
    mut b: StreamGraphBuilder<'static, SimpleScore>,
    source: PlanningStreamSource<usize>,
    port: StreamPort<I, Vec<Row>>,
) -> Network {
    let data = b.dataset(&port).unwrap();
    b.score("regression", &port, |_| SimpleScore::new(-1))
        .unwrap();
    Network {
        graph: b.build(),
        source,
        read: Box::new(move |g| {
            let mut rows: Vec<_> = g
                .dataset_rows(&data)
                .unwrap()
                .iter()
                .map(|(_, row)| row.clone())
                .collect();
            rows.sort();
            rows
        }),
    }
}
fn build(shape: Shape, prune: bool) -> Network {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    if prune {
        b = b.with_activity_pruning();
    }
    let source = b.source::<usize, Row>();
    if matches!(shape, Shape::Map) {
        let mapped = b.map(source.port(), |r| *r).unwrap();
        let assigned = b.filter(&mapped, |r| r.1.is_some()).unwrap();
        let out = b.map(&assigned, |r| vec![*r]).unwrap();
        return finish(b, source, out);
    }
    let assigned = b.filter(source.port(), |r| r.1.is_some()).unwrap();
    if matches!(shape, Shape::Concat) {
        // Value 0 has code A, value 1 has code B.
        let selected = b.filter(&assigned, |r| r.1 == Some(0)).unwrap();
        let concat = b.concat(&selected, &assigned).unwrap();
        let out = b.map(&concat, |r| vec![*r]).unwrap();
        return finish(b, source, out);
    }
    let separate = matches!(
        shape,
        Shape::Join(true) | Shape::Exists(true) | Shape::Absent(true)
    );
    let left = if separate {
        // Explicit including-unassigned/filter path, distinct from right retrieval.
        let explicit = b.filter(source.port(), |r| r.1.is_some()).unwrap();
        assert!(!explicit.same_node(&assigned));
        explicit
    } else {
        assigned.clone()
    };
    if matches!(shape, Shape::Join(_)) {
        let pairs = b.join_equal(&left, &assigned, |_| (), |_| ()).unwrap();
        let guarded = b
            .filter(&pairs, |(a, c)| {
                assert!(a.1.is_some());
                assert!(c.1.is_some());
                true
            })
            .unwrap();
        let out = b.map(&guarded, |(a, c)| vec![*a, *c]).unwrap();
        return finish(b, source, out);
    }
    let mode = if matches!(shape, Shape::Exists(_)) {
        ExistenceMode::Exists
    } else {
        ExistenceMode::NotExists
    };
    let visible = b
        .existence(
            &left,
            &assigned,
            mode,
            |_| (),
            |_| (),
            move |a, c| {
                if separate {
                    assert!(a.1.is_some());
                    assert!(c.1.is_some());
                    a.1 != c.1
                } else if matches!(shape, Shape::Exists(false)) {
                    assert!(a.1.is_some());
                    true
                } else {
                    a.1 != c.1
                }
            },
        )
        .unwrap();
    let out = b.map(&visible, |r| vec![*r]).unwrap();
    finish(b, source, out)
}
fn problem(values: usize, count: usize) -> FiniteProblem<usize> {
    FiniteProblem::new(
        (0..count)
            .map(|_| Variable::new((0..values).collect()))
            .collect(),
    )
    .unwrap()
}
fn change(
    d: &mut StreamScoreDirector<'_, '_, usize, SimpleScore>,
    id: usize,
    value: Option<usize>,
) {
    d.apply(&Move::Change {
        variable: id,
        value,
    })
    .unwrap()
    .commit();
}
fn check(
    assignment: &Assignment<usize>,
    graph: &StreamGraph<'_, SimpleScore>,
    score: SimpleScore,
    read: &ReadRows,
    model: &[Option<usize>],
    ids: &[Vec<usize>],
) {
    assert_eq!(assignment.values(), model);
    let mut expected: Vec<_> = ids
        .iter()
        .map(|row| row.iter().map(|&id| (id, model[id])).collect::<Vec<_>>())
        .collect();
    expected.sort();
    assert_eq!(read(graph), expected);
    let plain = SimpleScore::new(-(ids.len() as i64));
    assert_eq!(graph.score(), plain);
    assert_eq!(
        score,
        plain.with_structural(-(model.iter().filter(|v| v.is_none()).count() as i64))
    );
}
fn observe(
    d: &StreamScoreDirector<'_, '_, usize, SimpleScore>,
    read: &ReadRows,
    model: &[Option<usize>],
    ids: &[Vec<usize>],
) {
    check(d.assignment(), d.graph(), d.score(), read, model, ids);
}
fn null_case(shape: Shape) {
    let p = problem(1, 2);
    let Network {
        graph,
        source,
        read,
    } = build(shape, false);
    let mut d =
        StreamScoreDirector::new(&p, graph, source, Assignment::new(vec![None, Some(0)])).unwrap();
    let matches = |id| match shape {
        Shape::Join(_) => vec![vec![id, id]],
        Shape::Exists(true) => vec![],
        _ => vec![vec![id]],
    };
    observe(&d, &read, &[None, Some(0)], &matches(1));
    change(&mut d, 0, Some(0));
    change(&mut d, 1, None);
    observe(&d, &read, &[Some(0), None], &matches(0));
    change(&mut d, 0, None);
    change(&mut d, 1, Some(0));
    observe(&d, &read, &[None, Some(0)], &matches(1));
    d.assert_consistent().unwrap();
}
#[test]
fn filtering_join_null_conflict() {
    null_case(Shape::Join(false));
}
#[test]
fn filtering_join_null_conflict_different_nodes() {
    null_case(Shape::Join(true));
}
#[test]
fn filtering_if_exists_null_conflict() {
    null_case(Shape::Exists(false));
}
#[test]
fn filtering_if_exists_null_conflict_different_nodes() {
    null_case(Shape::Exists(true));
}
#[test]
fn filtering_if_not_exists_null_conflict() {
    null_case(Shape::Absent(false));
}
#[test]
fn filtering_if_not_exists_null_conflict_different_nodes() {
    null_case(Shape::Absent(true));
}
#[test]
fn map_planning_entity_changes() {
    let p = problem(1, 2);
    let Network {
        graph,
        source,
        read,
    } = build(Shape::Map, false);
    let mut d =
        StreamScoreDirector::new(&p, graph, source, Assignment::new(vec![None, Some(0)])).unwrap();
    observe(&d, &read, &[None, Some(0)], &[vec![1]]);
    change(&mut d, 0, Some(0));
    change(&mut d, 1, None);
    observe(&d, &read, &[Some(0), None], &[vec![0]]);
    change(&mut d, 1, Some(0));
    observe(&d, &read, &[Some(0), Some(0)], &[vec![0], vec![1]]);
    d.assert_consistent().unwrap();
}
#[test]
fn concat_same_tuple_dead_and_alive() {
    let p = problem(2, 2);
    let Network {
        graph,
        source,
        read,
    } = build(Shape::Concat, false);
    let mut d =
        StreamScoreDirector::new(&p, graph, source, Assignment::new(vec![Some(0), Some(1)]))
            .unwrap();
    observe(&d, &read, &[Some(0), Some(1)], &[vec![0], vec![0], vec![1]]);
    change(&mut d, 0, Some(1));
    change(&mut d, 1, Some(0));
    observe(&d, &read, &[Some(1), Some(0)], &[vec![0], vec![1], vec![1]]);
    change(&mut d, 0, Some(0));
    change(&mut d, 1, Some(1));
    // Preserve upstream undo ordering and deliberately omit intermediate observation.
    // Native moves propagate eagerly; Java's deferred event queue is not modeled.
    change(&mut d, 1, Some(0));
    change(&mut d, 0, Some(1));
    observe(&d, &read, &[Some(1), Some(0)], &[vec![0], vec![1], vec![1]]);
    change(&mut d, 1, Some(1));
    change(&mut d, 0, Some(0));
    observe(&d, &read, &[Some(0), Some(1)], &[vec![0], vec![0], vec![1]]);
    d.assert_consistent().unwrap();
}
// Original regression: independently enumerate matches, including the non-empty
// unequal-value branch absent from the original one-value existence fixtures.
fn expected(shape: Shape, values: &[Option<usize>]) -> Vec<Vec<usize>> {
    let assigned: Vec<_> = values
        .iter()
        .enumerate()
        .filter_map(|(id, v)| v.map(|_| id))
        .collect();
    let mut rows = vec![];
    for &a in &assigned {
        match shape {
            Shape::Join(_) => {
                for &b in &assigned {
                    rows.push(vec![a, b]);
                }
            }
            Shape::Exists(false) | Shape::Map => rows.push(vec![a]),
            Shape::Exists(true) => {
                if assigned.iter().any(|&b| values[a] != values[b]) {
                    rows.push(vec![a]);
                }
            }
            Shape::Absent(_) => {
                if assigned.iter().all(|&b| values[a] == values[b]) {
                    rows.push(vec![a]);
                }
            }
            Shape::Concat => {
                rows.push(vec![a]);
                if values[a] == Some(0) {
                    rows.push(vec![a]);
                }
            }
        }
    }
    rows
}
#[test]
fn moves_and_rollback_preserve_current_payloads_and_match_multiplicity() {
    let shapes = [
        Shape::Join(false),
        Shape::Join(true),
        Shape::Exists(false),
        Shape::Exists(true),
        Shape::Absent(false),
        Shape::Absent(true),
        Shape::Map,
        Shape::Concat,
    ];
    for shape in shapes {
        for prune in [false, true] {
            let p = problem(3, 3);
            let Network {
                graph,
                source,
                read,
            } = build(shape, prune);
            let mut model = vec![None; 3];
            let mut d =
                StreamScoreDirector::new(&p, graph, source, p.initial_assignment()).unwrap();
            observe(&d, &read, &model, &expected(shape, &model));
            // Enumerate all 4^3 assignments (unassigned or one of three values),
            // visit each in both update orders, and roll back every fourth trial.
            for n in 0..128 {
                let code = if n < 64 { n } else { 127 - n };
                let next: Vec<_> = (0..3)
                    .map(|i| match (code >> (2 * i)) & 3 {
                        0 => None,
                        v => Some(v - 1),
                    })
                    .collect();
                let mut changes: Vec<_> = next
                    .iter()
                    .enumerate()
                    .map(|(id, &value)| Move::Change {
                        variable: id,
                        value,
                    })
                    .collect();
                if n >= 64 {
                    changes.reverse();
                }
                {
                    let trial = d.apply(&Move::Composite(changes)).unwrap();
                    check(
                        trial.assignment(),
                        trial.graph(),
                        trial.score(),
                        &read,
                        &next,
                        &expected(shape, &next),
                    );
                    if n % 4 != 0 {
                        trial.commit();
                        model = next;
                    }
                }
                observe(&d, &read, &model, &expected(shape, &model));
            }
            d.assert_consistent().unwrap();
        }
    }
}
