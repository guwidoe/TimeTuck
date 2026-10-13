// Adapted pinned Community TEST assertions and immutable helper shapes, Apache-2.0.
// Attribution: compat/provenance.json, THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;
struct TestdataSolution;
struct TestdataRecordSolution;
struct TestdataRecordEntity;
struct TestdataEnumSolution;
struct TestdataEnumEntity;
struct DummyRecordSolution;
struct DummySolutionWithRecordEntity;
struct DummyRecordEntity;
struct TestdataEntity;
fn layout<S: 'static, E: 'static>() -> Arc<ComposedPlanningSolution> {
    let variables = if PlanningType::of::<E>() == PlanningType::of::<TestdataEnumEntity>() {
        vec![]
    } else {
        vec![PlanningVariableDefinition::basic::<String>("code", false)]
    };
    let entities =
        PlanningComposition::new(vec![PlanningComponent::registered::<E>(variables)]).unwrap();
    let solution =
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<S>(vec![
            SolutionMemberDefinition::entity_collection::<E>("entities"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap();
    Arc::new(
        solution
            .compile::<S>(
                &entities,
                &[PlanningType::of::<E>()],
                SolutionCompositionPolicy::default(),
                PlanningCompositionPolicy::default(),
            )
            .unwrap(),
    )
}
fn error<T: 'static>(
    layout: Arc<ComposedPlanningSolution>,
    role: PlanningModelRole,
    shape: ReadOnlyModelShape,
) -> String {
    match ModelMutabilityAccess::read_only(shape, |_: &T| {
        panic!("validation must not read immutable models")
    })
    .bind(layout, role)
    {
        Err(e) => e.to_string(),
        Ok(_) => panic!("immutable working model accepted"),
    }
}
#[test]
fn entity_descriptor_test_immutable_class() {
    assert!(
        error::<TestdataRecordEntity>(
            layout::<TestdataSolution, TestdataRecordEntity>(),
            PlanningModelRole::Entity,
            ReadOnlyModelShape::Record
        )
        .contains("cannot be a record as it needs to be mutable.")
    );
}
#[test]
fn solution_descriptor_test_immutable_class() {
    assert!(
        error::<TestdataRecordSolution>(
            layout::<TestdataRecordSolution, TestdataRecordEntity>(),
            PlanningModelRole::Solution,
            ReadOnlyModelShape::Record
        )
        .contains("cannot be a record as it needs to be mutable.")
    );
    // The source's enum fixture is a mutable solution whose planning entity is enum.
    assert!(
        error::<TestdataEnumEntity>(
            layout::<TestdataEnumSolution, TestdataEnumEntity>(),
            PlanningModelRole::Entity,
            ReadOnlyModelShape::Enum
        )
        .contains("cannot be an enum as it needs to be mutable.")
    );
}
#[test]
fn solver_config_solution_is_a_record() {
    let e = error::<DummyRecordSolution>(
        layout::<DummyRecordSolution, TestdataEntity>(),
        PlanningModelRole::Solution,
        ReadOnlyModelShape::Record,
    );
    assert!(e.contains("DummyRecordSolution"));
    assert!(e.contains("record"));
}
#[test]
fn solver_config_entity_is_a_record() {
    let e = error::<DummyRecordEntity>(
        layout::<DummySolutionWithRecordEntity, DummyRecordEntity>(),
        PlanningModelRole::Entity,
        ReadOnlyModelShape::Record,
    );
    assert!(e.contains("DummyRecordEntity"));
    assert!(e.contains("record"));
}
