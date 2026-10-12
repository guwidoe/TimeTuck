// Adapted Community SolverConfigTest, Apache-2.0; see provenance and notices.
#[path = "fixtures/domain_getter_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
#[test]
fn domain_classes_are_interfaces() {
    let solution = interface_solution();
    let layout = basic_layout::<InterfaceSolution, InterfaceEntity, Arc<InterfaceValue>>(false);
    let getters = interface_getters(layout.clone());
    let bound = getters.bind(&solution).unwrap();
    assert_eq!(solution.0.entities().len(), 1);
    assert_eq!(solution.0.values().len(), 1);
    assert_eq!(solution.0.score(), None);
    let registry = SolutionStreamRegistry::new(layout);
    let result = solve_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = registry
                .working_basic_model::<InterfaceEntity, _, _>(&mut b, &bound, true)
                .unwrap();
            interface_provider(&mut b, &model)?;
            let mut graph = b.build();
            model.initialize_constants(&mut graph)?;
            Ok((graph, model.planning_source().clone()))
        },
        SolverConfig {
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(-1)));
    assert_eq!(result.phases.len(), 1);
    let value = result.assignment.values()[0].as_ref().unwrap();
    assert!(Arc::ptr_eq(value, &solution.0.values()[0]));
    assert_eq!(
        bound.entities()[0]
            .object_ref::<InterfaceEntity>()
            .unwrap()
            .0
            .id(),
        "entity"
    );
}
#[test]
fn variable_with_planning_id_is_a_record() {
    let solution = record_solution();
    let layout = basic_layout::<RecordSolution, RecordEntity, Arc<RecordValue>>(true);
    let getters = record_getters(layout);
    let bound = getters.bind(&solution).unwrap();
    assert_eq!(solution.code, "Generated Solution 0");
    assert_eq!(solution.values.len(), 5);
    assert_eq!(solution.entities.len(), 7);
    assert_eq!(solution.score, None);
    assert_eq!(bound.facts().unwrap().records().len(), 5);
    let mut network = ConstraintNetwork::new(7);
    network
        .add_constraint("entity count", (0..7).collect(), |_| {
            record_calculator(&solution)
        })
        .unwrap();
    let result = solve(
        bound.problem(),
        &network,
        SolverConfig {
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(7)));
    assert_eq!(result.phases.len(), 1);
    for (i, value) in result.assignment.values().iter().enumerate() {
        assert!(Arc::ptr_eq(
            value.as_ref().unwrap(),
            &solution.values[i % 5]
        ));
        assert_eq!(solution.entities[i].code, format!("Generated Entity {i}"));
        assert_eq!(
            value.as_ref().unwrap().code,
            format!("Generated Value {}", i % 5)
        );
    }
}
