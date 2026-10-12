//! Original current planning-field feed controls; no upstream method credit.
#[path = "fixtures/initialized_solution_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use timetuck::*;
#[test]
fn current_fields_change_scores_without_mutating_or_cloning_objects() {
    let s = polymorphic_solution();
    let layout = polymorphic_layout();
    let bound = bind_polymorphic(layout.clone(), &s);
    let mut registry = SolutionStreamRegistry::new(layout);
    registry
        .project::<ChildEntity, BaseEntity>(|c| &c.base)
        .unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry
        .working_basic_source::<BaseEntity, Arc<Value>, _>(&mut b)
        .unwrap();
    let constraint = b
        .score("current", source.port(), |row| {
            calls.set(calls.get() + 1);
            SimpleScore::new(
                row.field("value")
                    .unwrap()
                    .as_ref()
                    .map_or(0, |v| if v.code == "v2" { 7 } else { 1 }),
            )
        })
        .unwrap();
    let mut graph = b.build();
    let initial = bound.problem().initial_assignment();
    source
        .refresh(&mut graph, source.rows(&bound, &initial, true).unwrap())
        .unwrap();
    assert_eq!(graph.score(), SimpleScore::new(3));
    assert_eq!(calls.get(), 3);
    let old = graph
        .explain_constraint_default(&constraint, |row| {
            vec![(
                row.object().code.clone(),
                row.field("value").unwrap().as_ref().map(|v| v.code.clone()),
            )]
        })
        .unwrap();
    let current = Assignment::new(vec![
        Some(s.base.value_list[1].clone()),
        Some(s.base.value_list[0].clone()),
        Some(s.base.value_list[0].clone()),
    ]);
    source
        .update_entity(&mut graph, &bound, &current, 0, true)
        .unwrap();
    assert_eq!(graph.score(), SimpleScore::new(9));
    assert_eq!(calls.get(), 4);
    let key = Arc::as_ptr(bound.entities()[0].object()) as *const () as usize;
    let row = &graph.constraint_matches(&constraint).unwrap()[&key].0;
    assert_eq!(row.field("value").unwrap().as_ref().unwrap().code, "v2");
    assert_eq!(row.object().value.as_ref().unwrap().code, "v1");
    assert!(Arc::ptr_eq(row.allocation(), bound.entities()[0].object()));
    assert_eq!(row.fields().count(), 1);
    assert!(row.field("typo").is_none());
    assert_eq!(
        old.matches[&key].justification.facts[0].1.as_deref(),
        Some("v1")
    );
    let null = Assignment::new(vec![
        None,
        Some(s.base.value_list[0].clone()),
        Some(s.base.value_list[0].clone()),
    ]);
    source
        .update_entity(&mut graph, &bound, &null, 0, true)
        .unwrap();
    assert_eq!(graph.score(), SimpleScore::new(2));
    source
        .update_entity(&mut graph, &bound, &null, 0, false)
        .unwrap();
    assert_eq!(graph.constraint_matches(&constraint).unwrap().len(), 3);
    assert_eq!(
        graph.constraint_matches(&constraint).unwrap()[&key]
            .0
            .field("value"),
        Some(&None)
    );
    graph.assert_consistent().unwrap();
}
#[test]
fn failed_updates_and_foreign_inputs_preserve_graph_and_field_snapshots() {
    let s = polymorphic_solution();
    let layout = polymorphic_layout();
    let bound = bind_polymorphic(layout.clone(), &s);
    let mut registry = SolutionStreamRegistry::new(layout);
    registry
        .project::<ChildEntity, BaseEntity>(|c| &c.base)
        .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    assert!(matches!(
        registry.working_basic_source::<BaseEntity, i32, _>(&mut b),
        Err(SolutionStreamError::WrongWorkingValueType { .. })
    ));
    let facts_source = registry
        .working_basic_source::<Arc<Value>, Arc<Value>, _>(&mut b)
        .unwrap();
    let source = registry
        .working_basic_source::<BaseEntity, Arc<Value>, _>(&mut b)
        .unwrap();
    let panic_value = Cell::new(false);
    let constraint = b
        .score("current", source.port(), |row| {
            let v = row.field("value").unwrap().as_ref().unwrap();
            assert!(!(panic_value.get() && v.code == "v2"), "controlled failure");
            SimpleScore::new(if v.code == "v2" { 7 } else { 1 })
        })
        .unwrap();
    let mut graph = b.build();
    let assignment = bound.problem().initial_assignment();
    source
        .refresh(&mut graph, source.rows(&bound, &assignment, true).unwrap())
        .unwrap();
    let current = Assignment::new(vec![
        Some(s.base.value_list[1].clone()),
        Some(s.base.value_list[0].clone()),
        Some(s.base.value_list[0].clone()),
    ]);
    panic_value.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| source
            .update_entity(&mut graph, &bound, &current, 0, true)
            .unwrap()))
        .is_err()
    );
    assert_eq!(graph.score(), SimpleScore::new(3));
    let key = Arc::as_ptr(bound.entities()[0].object()) as *const () as usize;
    assert_eq!(
        graph.constraint_matches(&constraint).unwrap()[&key]
            .0
            .field("value")
            .unwrap()
            .as_ref()
            .unwrap()
            .code,
        "v1"
    );
    panic_value.set(false);
    graph.assert_consistent().unwrap();
    assert!(matches!(
        source.rows(&bound, &Assignment::new(vec![]), true),
        Err(SolutionStreamError::BasicAssignment(_))
    ));
    assert!(matches!(
        source.update_entity(&mut graph, &bound, &assignment, 99, true),
        Err(WorkingBasicUpdateError::Source(
            SolutionStreamError::UnknownEntityRow(99)
        ))
    ));
    let foreign = bind_polymorphic(polymorphic_layout(), &s);
    assert!(matches!(
        source.rows(&foreign, &assignment, true),
        Err(SolutionStreamError::DifferentDescriptor)
    ));
    let mut other = StreamGraphBuilder::<SimpleScore>::new().build();
    assert_eq!(
        facts_source
            .update_entity(&mut other, &bound, &assignment, 0, true)
            .unwrap_err(),
        WorkingBasicUpdateError::Graph(GraphError::ForeignPort)
    );
    assert_eq!(
        source
            .update_entity(&mut other, &bound, &assignment, 0, true)
            .unwrap_err(),
        WorkingBasicUpdateError::Graph(GraphError::ForeignPort)
    );
}
#[test]
fn equal_planning_identity_with_new_payload_is_published_on_refresh() {
    let s = polymorphic_solution();
    let layout = polymorphic_layout();
    let bound = bind_polymorphic(layout.clone(), &s);
    let mut registry = SolutionStreamRegistry::new(layout);
    registry
        .project::<ChildEntity, BaseEntity>(|c| &c.base)
        .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry
        .working_basic_source::<BaseEntity, Arc<Value>, _>(&mut b)
        .unwrap();
    let constraint = b
        .score("codes", source.port(), |_| SimpleScore::new(1))
        .unwrap();
    let mut graph = b.build();
    let initial = bound.problem().initial_assignment();
    source
        .refresh(&mut graph, source.rows(&bound, &initial, true).unwrap())
        .unwrap();
    let equal = Arc::new(Value { code: "v1".into() });
    assert_eq!(equal, s.base.value_list[0]);
    assert!(!Arc::ptr_eq(&equal, &s.base.value_list[0]));
    let current = Assignment::new(vec![
        Some(equal.clone()),
        Some(s.base.value_list[0].clone()),
        Some(s.base.value_list[0].clone()),
    ]);
    source
        .refresh(&mut graph, source.rows(&bound, &current, true).unwrap())
        .unwrap();
    let key = Arc::as_ptr(bound.entities()[0].object()) as *const () as usize;
    assert!(Arc::ptr_eq(
        graph.constraint_matches(&constraint).unwrap()[&key]
            .0
            .field("value")
            .unwrap()
            .as_ref()
            .unwrap(),
        &equal
    ));
    assert_eq!(graph.score(), SimpleScore::new(3));
}
