//! Original runtime and error controls; no upstream method credit.
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
fn current_basic_slots_drive_membership_without_replaying_unchanged_objects() {
    let s = polymorphic_solution();
    let layout = polymorphic_layout();
    let bound = bind_polymorphic(layout.clone(), &s);
    let mut registry = SolutionStreamRegistry::new(layout);
    registry
        .project::<ChildEntity, BaseEntity>(|c| &c.base)
        .unwrap();
    let calls = Cell::new(0);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<BaseEntity, _>(&mut b).unwrap();
    let constraint = b
        .score("entities", source.port(), |_| {
            calls.set(calls.get() + 1);
            SimpleScore::new(-1)
        })
        .unwrap();
    let mut graph = b.build();
    let mut assignment = bound.problem().initial_assignment();
    source
        .refresh_membership(
            &mut graph,
            source.initialized_basic(&bound, &assignment).unwrap(),
        )
        .unwrap();
    assert_eq!(calls.get(), 3);
    assignment = Assignment::new(vec![None, Some(s.base.value_list[0].clone()), None]);
    source
        .refresh_membership(
            &mut graph,
            source.initialized_basic(&bound, &assignment).unwrap(),
        )
        .unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-1));
    assert_eq!(calls.get(), 3);
    assert_eq!(
        graph
            .constraint_matches(&constraint)
            .unwrap()
            .values()
            .next()
            .unwrap()
            .0
            .get()
            .code,
        "Animal"
    );
    assert_eq!(source.bound(&bound).unwrap().len(), 3);
    assignment = Assignment::new(vec![
        Some(s.base.value_list[1].clone()),
        Some(s.base.value_list[0].clone()),
        None,
    ]);
    let mut rows = source.initialized_basic(&bound, &assignment).unwrap();
    rows.push(rows[0].clone());
    source.refresh_membership(&mut graph, rows).unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-2));
    assert_eq!(calls.get(), 4);
    assert!(matches!(
        source.initialized_basic(&bound, &Assignment::new(vec![])),
        Err(SolutionStreamError::BasicAssignment(_))
    ));
    assert_eq!(graph.score(), SimpleScore::new(-2));
    graph.assert_consistent().unwrap();
}
#[test]
fn concrete_list_bindings_keep_aliases_pins_and_shadow_only_objects() {
    let s = no_inverse_solution();
    let layout = no_inverse_layout();
    let (mut c, r, f) = no_inverse_inputs(&s);
    c[0].entities.push(SolutionListEntityRecord::new(
        s.entity_list[0].clone(),
        vec![("valueList".into(), s.entity_list[0].value_list.clone())],
    ));
    let facts = SolutionFactAccessors::new(layout.clone())
        .unwrap()
        .read(&s)
        .unwrap();
    let bound = layout.bind_list_with_facts(c, r, f, facts).unwrap();
    assert_eq!(bound.collection("entityList"), Some([0, 1, 0].as_slice()));
    assert_eq!(bound.collection("valueList"), Some([2, 3].as_slice()));
    assert_eq!(bound.problem().entities().len(), 2);
    assert_eq!(bound.entities()[0].list_index(), Some(0));
    assert_eq!(bound.entities()[2].list_index(), None);
    assert_eq!(bound.list_owner(), PlanningType::of::<NoInverseOwner>());
    assert_eq!(bound.list_variable(), "valueList");
    assert!(std::ptr::eq(
        bound.entities()[2].object_ref::<NoInverseValue>().unwrap(),
        s.value_list[0].as_ref()
    ));
    let registry = SolutionStreamRegistry::new(layout.clone());
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let owners = registry.source::<NoInverseOwner, _>(&mut b).unwrap();
    let values = registry.source::<NoInverseValue, _>(&mut b).unwrap();
    assert_eq!(owners.list_bound(&bound).unwrap().len(), 2);
    assert_eq!(values.list_bound(&bound).unwrap().len(), 2);
    let (mut c, r, f) = no_inverse_inputs(&s);
    c[0].entities.push(SolutionListEntityRecord::new(
        s.entity_list[0].clone(),
        vec![("valueList".into(), vec![])],
    ));
    let facts = SolutionFactAccessors::new(layout.clone())
        .unwrap()
        .read(&s)
        .unwrap();
    assert!(matches!(
        layout.bind_list_with_facts(c, r, f, facts),
        Err(ListSolutionBindingError::ConflictingRecord(_))
    ));
    let (mut c, r, f) = no_inverse_inputs(&s);
    c[0].entities[0] = SolutionListEntityRecord::new(
        s.entity_list[0].clone(),
        vec![("valueList".into(), s.entity_list[0].value_list.clone())],
    )
    .with_pin_policy(ListPinPolicy {
        fully_pinned: true,
        prefix: 0,
    });
    let facts = SolutionFactAccessors::new(layout.clone())
        .unwrap()
        .read(&s)
        .unwrap();
    let pinned = layout.bind_list_with_facts(c, r, f, facts).unwrap();
    let bad = ListAssignment::new(vec![vec![], vec![s.value_list[0].clone()]]);
    assert!(matches!(
        values.initialized_list_values(&pinned, &bad),
        Err(SolutionStreamError::ListAssignment(_))
    ));
    let (c, r, f) = no_inverse_inputs(&s);
    let foreign = SolutionFactAccessors::new(no_inverse_layout())
        .unwrap()
        .read(&s)
        .unwrap();
    assert!(matches!(
        layout.bind_list_with_facts(c, r, f, foreign),
        Err(ListSolutionBindingError::Binding(
            SolutionBindingError::DifferentFactDescriptor
        ))
    ));
}
#[test]
fn list_membership_uses_canonical_allocations_and_failed_refresh_rolls_back() {
    let s = no_inverse_solution();
    let layout = no_inverse_layout();
    let bound = bind_no_inverse(layout.clone(), &s);
    let registry = SolutionStreamRegistry::new(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<NoInverseValue, _>(&mut b).unwrap();
    let panic_new = Cell::new(false);
    let constraint = b
        .score("values", source.port(), |row| {
            assert!(
                !(panic_new.get() && row.get().code == "v2"),
                "controlled score failure"
            );
            SimpleScore::new(-1)
        })
        .unwrap();
    let mut graph = b.build();
    source
        .refresh_membership(
            &mut graph,
            source
                .initialized_list_values(&bound, &bound.problem().initial_assignment())
                .unwrap(),
        )
        .unwrap();
    let foreign = Arc::new(NoInverseValue {
        code: "v1".into(),
        index: Some(99),
    });
    let equal = ListAssignment::new(vec![vec![foreign], vec![]]);
    let rows = source.initialized_list_values(&bound, &equal).unwrap();
    assert!(std::ptr::eq(rows[0].1.get(), s.value_list[0].as_ref()));
    assert_eq!(rows[0].1.get().index, Some(0));
    let duplicate = ListAssignment::new(vec![
        vec![s.value_list[0].clone()],
        vec![s.value_list[0].clone()],
    ]);
    assert!(matches!(
        source.initialized_list_values(&bound, &duplicate),
        Err(SolutionStreamError::ListAssignment(_))
    ));
    let assignment = ListAssignment::new(vec![vec![], vec![s.value_list[1].clone()]]);
    panic_new.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(|| source
            .refresh_membership(
                &mut graph,
                source.initialized_list_values(&bound, &assignment).unwrap()
            )
            .unwrap()))
        .is_err()
    );
    let matches = graph.constraint_matches(&constraint).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches.values().next().unwrap().0.get().code, "v1");
    assert_eq!(graph.score(), SimpleScore::new(-1));
    panic_new.set(false);
    source
        .refresh_membership(
            &mut graph,
            source.initialized_list_values(&bound, &assignment).unwrap(),
        )
        .unwrap();
    assert_eq!(
        graph
            .constraint_matches(&constraint)
            .unwrap()
            .values()
            .next()
            .unwrap()
            .0
            .get()
            .code,
        "v2"
    );
    graph.assert_consistent().unwrap();
    let mut other = StreamGraphBuilder::<SimpleScore>::new().build();
    assert_eq!(
        source.refresh_membership(&mut other, vec![]).unwrap_err(),
        GraphError::ForeignPort
    );
}
#[test]
fn concrete_bound_list_model_optimizes_in_dense_and_stream_backends() {
    let s = no_inverse_solution();
    let bound = bind_no_inverse(no_inverse_layout(), &s);
    let p = bound.problem();
    let mut network = ListConstraintNetwork::<Arc<NoInverseValue>, SimpleScore>::new(2, 2);
    for entity in 0..2 {
        network
            .add_constraint(
                format!("balance {entity}"),
                vec![ListScope::Entity(entity)],
                |facts| {
                    let ListFact::Entity(list) = facts[0] else {
                        unreachable!()
                    };
                    SimpleScore::new(-((list.len() * list.len()) as i64))
                },
            )
            .unwrap();
    }
    let dense = solve_list(p, &network, SolverPlan::default()).unwrap();
    let stream = solve_list_stream(
        p,
        || {
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::<Arc<NoInverseValue>>::new(&mut b);
            b.score("balance", sources.entities.port(), |(_, list)| {
                SimpleScore::new(-((list.len() * list.len()) as i64))
            })?;
            Ok((b.build(), sources))
        },
        SolverPlan::default(),
    )
    .unwrap();
    for result in [dense, stream] {
        assert!(result.complete);
        assert_eq!(result.score, Some(SimpleScore::new(-2)));
        assert!(result.assignment.lists().iter().all(|list| list.len() == 1));
        for value in result.assignment.lists().iter().flatten() {
            assert!(s.value_list.iter().any(|v| Arc::ptr_eq(v, value)));
        }
    }
}

#[test]
fn indexed_bound_model_preserves_objects_and_live_assignment_selection() {
    let s = no_inverse_solution();
    let layout = no_inverse_layout();
    let indexed = bind_no_inverse(layout.clone(), &s).indexed().unwrap();
    let registry = SolutionStreamRegistry::new(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<NoInverseValue, _>(&mut b).unwrap();
    let mut assignment = indexed.problem().initial_assignment();
    for expected in [1, 2] {
        let rows = source
            .initialized_list_values(&indexed, &assignment)
            .unwrap();
        assert_eq!(rows.len(), expected);
        for (_, row) in rows {
            assert!(
                s.value_list
                    .iter()
                    .any(|v| std::ptr::eq(v.as_ref(), row.get()))
            );
        }
        if expected == 1 {
            indexed
                .problem()
                .apply(
                    &mut assignment,
                    &ListMove::Assign {
                        value: s.value_list[1].clone(),
                        destination: ListPosition {
                            entity: 1,
                            index: 0,
                        },
                    },
                )
                .unwrap()
                .commit();
        }
    }
    assert_eq!(indexed.problem().values().len(), 2);
    assert!(!indexed.problem().allows_unassigned());
}
