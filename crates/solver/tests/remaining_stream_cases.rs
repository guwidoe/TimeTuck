// Complete selected Community stream adaptations; Apache-2.0.
#[path = "fixtures/remaining_stream_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
#[test]
fn bi_joiner_equals_and_sameness() {
    let s = decimal_solution();
    assert_eq!(s.entities[0].decimal, s.entities[1].decimal);
    assert!(!Arc::ptr_eq(&s.entities[0].decimal, &s.entities[1].decimal));
    assert!(Arc::ptr_eq(&s.entities[0].decimal, &s.entities[2].decimal));
    let layout = decimal_layout();
    let bound = decimal_objects(layout.clone(), &s);
    let registry = SolutionStreamRegistry::new(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<DecimalEntity, _>(&mut b).unwrap();
    let joined = b
        .join_equal(
            source.port(),
            source.port(),
            |e| e.get().decimal.clone(),
            |e| e.get().decimal.clone(),
        )
        .unwrap();
    let constraint = b.score("TEST", &joined, |_| SimpleScore::new(-1)).unwrap();
    let mut graph = b.build();
    source
        .refresh_membership(
            &mut graph,
            source
                .initialized_basic(&bound, &bound.problem().initial_assignment())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(graph.score(), SimpleScore::new(-9));
    let matches = graph.constraint_matches(&constraint).unwrap();
    assert_eq!(matches.len(), 9);
    for left in &s.entities {
        for right in &s.entities {
            let key = (Arc::as_ptr(left) as usize, Arc::as_ptr(right) as usize);
            let ((a, b), score) = matches.get(&key).unwrap();
            assert!(std::ptr::eq(a.get(), left.as_ref()));
            assert!(std::ptr::eq(b.get(), right.as_ref()));
            assert_eq!(*score, SimpleScore::new(-1));
        }
    }
    graph.assert_consistent().unwrap();
}
#[test]
fn uni_constraint_provided_from_unknown_package() {
    let mut s = UnnamedSolution::generate();
    s.entity_list.retain(|e| e.code == "Generated Entity 0");
    let layout = UnnamedSolution::layout();
    let bound = s.bind(layout.clone());
    let registry = SolutionStreamRegistry::new(layout);
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = registry.source::<basic::Entity, _>(&mut b).unwrap();
    let constraint = named_provider::define(&mut b, &source);
    let mut graph = b.build();
    source
        .refresh_membership(
            &mut graph,
            source
                .initialized_basic(&bound, &bound.problem().initial_assignment())
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        graph.constraint_name(&constraint).unwrap(),
        "Always penalize"
    );
    assert_eq!(graph.score(), SimpleScore::new(-1));
    let matches = graph.constraint_matches(&constraint).unwrap();
    assert_eq!(matches.len(), 1);
    let key = Arc::as_ptr(&s.entity_list[0]) as usize;
    let (row, score) = matches.get(&key).unwrap();
    assert!(std::ptr::eq(row.get(), s.entity_list[0].as_ref()));
    assert_eq!(*score, SimpleScore::new(-1));
    graph.assert_consistent().unwrap();
}
