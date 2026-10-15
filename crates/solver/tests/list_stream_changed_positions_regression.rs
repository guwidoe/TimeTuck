//! Original Eq-only list source-update contracts; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;

// Deliberately neither Hash nor Ord: source identity is the registry occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Value(i64);

fn graph<'a>(
    events: &'a RefCell<Vec<usize>>,
    panic_at: &'a Cell<Option<usize>>,
) -> (StreamGraph<'a, SimpleScore>, ListStreamSources<Value>) {
    let mut builder = StreamGraphBuilder::new();
    let sources = ListStreamSources::<Value>::new(&mut builder);
    builder
        .score("positions", sources.values.port(), |row| {
            events.borrow_mut().push(row.range_index);
            assert_ne!(
                panic_at.get(),
                Some(row.range_index),
                "deliberate source callback panic"
            );
            SimpleScore::new(
                row.position
                    .map_or(0, |p| row.value.0 * (1 + p.entity + p.index) as i64),
            )
        })
        .unwrap();
    (builder.build(), sources)
}

fn check(
    graph: &StreamGraph<'_, SimpleScore>,
    sources: &ListStreamSources<Value>,
    problem: &ListProblem<Value>,
    assignment: &ListAssignment<Value>,
) {
    let rows = graph.source_rows(&sources.values).unwrap();
    assert_eq!(rows.len(), problem.values().len());
    let mut expected_score = 0;
    for (id, value) in problem.values().iter().enumerate() {
        let position = assignment
            .lists()
            .iter()
            .enumerate()
            .find_map(|(entity, list)| {
                list.iter()
                    .position(|v| v == value)
                    .map(|index| ListPosition { entity, index })
            });
        let row = &rows[&id];
        assert_eq!(&row.value, value);
        assert_eq!(row.position, position);
        let (previous, next) = position.map_or((None, None), |p| {
            let list = &assignment.lists()[p.entity];
            (
                p.index.checked_sub(1).map(|i| list[i].clone()),
                list.get(p.index + 1).cloned(),
            )
        });
        assert_eq!(row.previous, previous);
        assert_eq!(row.next, next);
        expected_score += position.map_or(0, |p| value.0 * (1 + p.entity + p.index) as i64);
    }
    assert_eq!(graph.score(), SimpleScore::new(expected_score));
}

fn sparse_registry_case(extra_values: usize) {
    let mut values = vec![Value(900), Value(7), Value(301), Value(55), Value(2)];
    values.extend((0..extra_values).map(|i| Value(50_000 + i as i64)));
    let problem = ListProblem::new(
        values,
        vec![
            ListEntity::new(vec![Value(301), Value(900)]),
            ListEntity::new(vec![Value(2)]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    let events = RefCell::new(vec![]);
    let panic_at = Cell::new(None);
    let (graph, sources) = graph(&events, &panic_at);
    let inspect = sources.clone();
    let original = problem.initial_assignment();
    let mut director =
        ListStreamScoreDirector::new(&problem, graph, sources, original.clone()).unwrap();
    let before = director
        .graph()
        .source_rows(&inspect.values)
        .unwrap()
        .clone();
    events.borrow_mut().clear();
    let remove = ListMove::Unassign {
        entity: 0,
        start: 0,
        end: 2,
    };
    {
        let mut outer = director.edit_session().unwrap();
        outer.apply(&remove).unwrap();
        assert_eq!(*events.borrow(), vec![0, 2]);
        check(
            outer.director().graph(),
            &inspect,
            &problem,
            outer.assignment(),
        );
        let removed = outer
            .director()
            .graph()
            .source_rows(&inspect.values)
            .unwrap()
            .clone();
        {
            let mut inner = outer.nested().unwrap();
            inner
                .apply(&ListMove::Assign {
                    value: Value(7),
                    destination: ListPosition {
                        entity: 0,
                        index: 0,
                    },
                })
                .unwrap();
            check(
                inner.director().graph(),
                &inspect,
                &problem,
                inner.assignment(),
            );
            inner.commit();
        }
        assert!(removed[&1].position.is_none());
        assert!(removed[&0].position.is_none());
    }
    assert_eq!(director.assignment(), &original);
    check(director.graph(), &inspect, &problem, director.assignment());
    assert_eq!(
        before[&0].position,
        Some(ListPosition {
            entity: 0,
            index: 1
        })
    );
    director.apply(&remove).unwrap().commit();
    check(director.graph(), &inspect, &problem, director.assignment());
    assert_eq!(before[&2].next, Some(Value(900)));
}

fn many_touched_rows_case(extra_values: i64) {
    let values: Vec<_> = (0..12 + extra_values)
        .map(|i| Value(100 + i * 17))
        .collect();
    let problem = ListProblem::new(
        values.clone(),
        values[..10]
            .iter()
            .map(|v| ListEntity::new(vec![v.clone()]))
            .collect(),
    )
    .unwrap()
    .allowing_unassigned();
    let events = RefCell::new(vec![]);
    let panic_at = Cell::new(None);
    let (graph, sources) = graph(&events, &panic_at);
    let inspect = sources.clone();
    let original = problem.initial_assignment();
    let mut director =
        ListStreamScoreDirector::new(&problem, graph, sources, original.clone()).unwrap();
    let snapshot = director
        .graph()
        .source_rows(&inspect.values)
        .unwrap()
        .clone();
    let movement = ListMove::Composite(
        (0..10)
            .map(|entity| ListMove::Unassign {
                entity,
                start: 0,
                end: 1,
            })
            .collect(),
    );
    events.borrow_mut().clear();
    panic_at.set(Some(4));
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = director.apply(&movement);
        }))
        .is_err()
    );
    assert_eq!(*events.borrow(), vec![0, 1, 2, 3, 4]);
    panic_at.set(None);
    assert_eq!(director.assignment(), &original);
    check(director.graph(), &inspect, &problem, director.assignment());
    events.borrow_mut().clear();
    {
        let candidate = director.apply(&movement).unwrap();
        assert_eq!(candidate.constraint_evaluations(), 10);
        assert_eq!(*events.borrow(), (0..10).collect::<Vec<_>>());
        check(
            candidate.graph(),
            &inspect,
            &problem,
            candidate.assignment(),
        );
        assert!(candidate.assignment().lists().iter().all(Vec::is_empty));
    }
    assert_eq!(director.assignment(), &original);
    director.apply(&movement).unwrap().commit();
    check(director.graph(), &inspect, &problem, director.assignment());
    assert!(snapshot[&9].position.is_some());
    assert!(snapshot[&10].position.is_none());
    director.assert_consistent().unwrap();
}

#[test]
fn sparse_registry_removed_only_updates_preserve_snapshots_and_nested_rollback() {
    sparse_registry_case(0);
}
#[test]
fn more_than_eight_touched_rows_restore_every_shadow_after_callback_unwind() {
    many_touched_rows_case(0);
}
#[test]
fn sparse_touched_scans_preserve_source_order_metadata_and_panic_rollback() {
    sparse_registry_case(27);
    many_touched_rows_case(20);
}
