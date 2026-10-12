// Complete Community forEach source-method adaptations; Apache-2.0.
#[path = "fixtures/initialized_solution_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
#[test]
fn for_each_polymorphism() {
    let s = polymorphic_solution();
    let layout = polymorphic_layout();
    let bound = bind_polymorphic(layout.clone(), &s);
    let weights = SolutionWeightAccessor::new(layout.clone(), |s: &ExtendedSolution| {
        s.base.weights.clone()
    })
    .unwrap()
    .read(&s);
    let mut registry = SolutionStreamRegistry::new(layout);
    registry
        .project::<ChildEntity, BaseEntity>(|e| &e.base)
        .unwrap();
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let base = registry.source::<BaseEntity, _>(&mut b).unwrap();
    let child = registry.source::<ChildEntity, _>(&mut b).unwrap();
    let base_constraint = b
        .score_weighted(
            "superclassConstraint",
            base.port(),
            SimpleScore::new(-1),
            |_| 1,
        )
        .unwrap();
    let child_constraint = b
        .score_weighted(
            "subclassConstraint",
            child.port(),
            SimpleScore::new(-1),
            |_| 1,
        )
        .unwrap();
    let mut graph = b.build_with_weights(weights.weights()).unwrap();
    let assignment = bound.problem().initial_assignment();
    base.refresh_membership(
        &mut graph,
        base.initialized_basic(&bound, &assignment).unwrap(),
    )
    .unwrap();
    child
        .refresh_membership(
            &mut graph,
            child.initialized_basic(&bound, &assignment).unwrap(),
        )
        .unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-5));
    let parents = graph.constraint_matches(&base_constraint).unwrap();
    let children = graph.constraint_matches(&child_constraint).unwrap();
    assert_eq!(parents.len(), 3);
    assert_eq!(children.len(), 2);
    for object in &s.base.entity_list {
        let (actual, name, is_child): (Arc<dyn std::any::Any + Send + Sync>, &str, bool) =
            match object {
                EntityObject::Base(e) => (e.clone(), e.code.as_str(), false),
                EntityObject::Child(e) => (e.clone(), e.base.code.as_str(), true),
            };
        let key = Arc::as_ptr(&actual) as *const () as usize;
        let (row, score) = parents.get(&key).unwrap();
        assert!(Arc::ptr_eq(row.allocation(), &actual));
        assert_eq!(row.get().code, name);
        assert_eq!(*score, SimpleScore::new(-1));
        if is_child {
            let (row, score) = children.get(&key).unwrap();
            assert!(Arc::ptr_eq(row.allocation(), &actual));
            assert_eq!(row.get().base.code, name);
            assert_eq!(*score, SimpleScore::new(-1));
        } else {
            assert!(!children.contains_key(&key));
        }
    }
    graph.assert_consistent().unwrap();
}
#[test]
fn for_each_list_var_not_allows_unassigned_values_no_inverse_var() {
    let s = no_inverse_solution();
    let layout = no_inverse_layout();
    let bound = bind_no_inverse(layout.clone(), &s);
    let registry = SolutionStreamRegistry::new(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<NoInverseValue, _>(&mut b).unwrap();
    let constraint = b
        .score("TEST", source.port(), |_| SimpleScore::new(-1))
        .unwrap();
    let mut graph = b.build();
    let mut assignment = bound.problem().initial_assignment();
    let check = |graph: &StreamGraph<'_, SimpleScore>, expected: &[&Arc<NoInverseValue>]| {
        assert_eq!(graph.score(), SimpleScore::new(-(expected.len() as i64)));
        let matches = graph.constraint_matches(&constraint).unwrap();
        assert_eq!(matches.len(), expected.len());
        for object in expected {
            let key = Arc::as_ptr(object) as usize;
            let (row, score) = matches.get(&key).unwrap();
            assert!(std::ptr::eq(row.get(), object.as_ref()));
            assert_eq!(*score, SimpleScore::new(-1));
        }
    };
    source
        .refresh_membership(
            &mut graph,
            source.initialized_list_values(&bound, &assignment).unwrap(),
        )
        .unwrap();
    check(&graph, &[&s.value_list[0]]);
    bound
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
    source
        .refresh_membership(
            &mut graph,
            source.initialized_list_values(&bound, &assignment).unwrap(),
        )
        .unwrap();
    check(&graph, &[&s.value_list[0], &s.value_list[1]]);
    bound
        .problem()
        .apply(
            &mut assignment,
            &ListMove::Unassign {
                entity: 0,
                start: 0,
                end: 1,
            },
        )
        .unwrap()
        .commit();
    source
        .refresh_membership(
            &mut graph,
            source.initialized_list_values(&bound, &assignment).unwrap(),
        )
        .unwrap();
    check(&graph, &[&s.value_list[1]]);
    graph.assert_consistent().unwrap();
}
