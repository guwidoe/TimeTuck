//! Original executable getter-bank validation and lifetime controls.
#[path = "fixtures/domain_getter_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};
use timetuck::*;
#[test]
fn getters_run_once_per_bind_and_never_during_actual_solver_moves() {
    let s = interface_solution();
    let layout = basic_layout::<InterfaceSolution, InterfaceEntity, Arc<InterfaceValue>>(false);
    let entities = Cell::new(0);
    let ranges = Cell::new(0);
    let mut getters = SolutionBasicAccessors::new(
        layout.clone(),
        vec![SolutionFieldRange::new::<InterfaceEntity>(
            "value",
            vec!["valueRange".into()],
        )],
    )
    .unwrap();
    getters
        .entities("entityList", |s: &InterfaceSolution| {
            entities.set(entities.get() + 1);
            s.0.entities()
                .into_iter()
                .map(|e| {
                    let initial = e.0.value();
                    (e, vec![("value".into(), initial)])
                })
                .collect()
        })
        .unwrap();
    getters
        .range("valueList", |s| {
            ranges.set(ranges.get() + 1);
            s.0.values()
        })
        .unwrap();
    let bound = getters.bind(&s).unwrap();
    assert_eq!((entities.get(), ranges.get()), (1, 1));
    let r = SolutionStreamRegistry::new(layout);
    let result = solve_stream(
        bound.problem(),
        || {
            let mut b = StreamGraphBuilder::<SimpleScore>::new();
            let model = r
                .working_basic_model::<InterfaceEntity, _, _>(&mut b, &bound, true)
                .unwrap();
            interface_provider(&mut b, &model)?;
            let mut g = b.build();
            model.initialize_constants(&mut g)?;
            Ok((g, model.planning_source().clone()))
        },
        SolverConfig {
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!((entities.get(), ranges.get()), (1, 1));
    assert!(
        bound.entities()[0]
            .object_ref::<InterfaceEntity>()
            .unwrap()
            .0
            .value()
            .is_none()
    );
    let again = getters.bind(&s).unwrap();
    assert_eq!((entities.get(), ranges.get()), (2, 2));
    assert!(Arc::ptr_eq(
        bound.entities()[0].object(),
        again.entities()[0].object()
    ));
}
#[test]
fn invalid_or_incomplete_registration_rejects_before_reading_domain_objects() {
    let s = interface_solution();
    let layout = basic_layout::<InterfaceSolution, InterfaceEntity, Arc<InterfaceValue>>(false);
    let calls = Cell::new(0);
    let mut getters = SolutionBasicAccessors::new(
        layout.clone(),
        vec![SolutionFieldRange::new::<InterfaceEntity>(
            "value",
            vec!["valueRange".into()],
        )],
    )
    .unwrap();
    getters
        .entities("entityList", |_: &InterfaceSolution| -> Vec<BasicEntityFields<InterfaceEntity,Arc<InterfaceValue>>> {
            calls.set(calls.get() + 1);
            vec![]
        })
        .unwrap();
    assert!(matches!(
        getters.bind(&s),
        Err(BasicSolutionAccessError::Binding(
            SolutionBindingError::MissingBinding(_)
        ))
    ));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        getters.entities("entityList", |_: &InterfaceSolution| Vec::<
            BasicEntityFields<InterfaceEntity, Arc<InterfaceValue>>,
        >::new()),
        Err(BasicSolutionAccessError::Binding(
            SolutionBindingError::DuplicateBinding(_)
        ))
    ));
    assert!(matches!(
        getters.entities("entityList", |_: &InterfaceSolution| Vec::<
            BasicEntityFields<RecordEntity, Arc<InterfaceValue>>,
        >::new()),
        Err(BasicSolutionAccessError::Binding(
            SolutionBindingError::WrongEntityType { .. }
        ))
    ));
    assert!(matches!(
        getters.range("entityList", |_: &InterfaceSolution| vec![]),
        Err(BasicSolutionAccessError::Binding(
            SolutionBindingError::WrongMemberKind(_)
        ))
    ));
    assert!(matches!(
        SolutionBasicAccessors::<RecordSolution, Arc<InterfaceValue>>::new(layout, vec![]),
        Err(BasicSolutionAccessError::Facts(
            SolutionFactError::SolutionType { .. }
        ))
    ));
    assert_eq!(calls.get(), 0);
}
#[test]
fn failed_getters_publish_no_model_and_record_identity_survives_fact_reads() {
    let s = record_solution();
    let layout = basic_layout::<RecordSolution, RecordEntity, Arc<RecordValue>>(true);
    let getters = record_getters(layout.clone());
    let bound = getters.bind(&s).unwrap();
    for (i, fact) in bound.facts().unwrap().records().iter().enumerate() {
        assert!(Arc::ptr_eq(
            fact.object_ref::<Arc<RecordValue>>().unwrap(),
            &s.values[i]
        ));
    }
    let duplicate = Arc::new(RecordValue {
        code: s.values[0].code.clone(),
    });
    assert_eq!(duplicate, s.values[0]);
    assert!(!Arc::ptr_eq(&duplicate, &s.values[0]));
    let mut fail = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<RecordEntity>(
            "value",
            vec!["valueRange".into()],
        )],
    )
    .unwrap();
    fail.entities(
        "entityList",
        |_: &RecordSolution| -> Vec<BasicEntityFields<RecordEntity, Arc<RecordValue>>> {
            panic!("controlled getter failure")
        },
    )
    .unwrap();
    fail.range("valueList", |s| s.values.clone()).unwrap();
    fail.facts_mut()
        .collection("valueList", |s: &RecordSolution| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = fail.bind(&s);
        }))
        .is_err()
    );
    assert_eq!(getters.bind(&s).unwrap().problem().variables().len(), 7);
}
