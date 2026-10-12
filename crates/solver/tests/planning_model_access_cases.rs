// Complete pinned Community TEST adaptations, Apache-2.0; provenance supplied.
#[path = "fixtures/planning_model_access_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn element_type() {
    let schema = list_schema();
    let access = ListVariableAccess::<ListValue>::new(
        schema
            .entity::<ListEntityType>()
            .unwrap()
            .genuine_variable("valueList")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(access.element_type(), PlanningType::of::<ListValue>());
}
#[test]
fn accepts_value_type() {
    let schema = list_schema();
    let access = ListVariableAccess::<ListValue>::new(
        schema
            .entity::<ListEntityType>()
            .unwrap()
            .genuine_variable("valueList")
            .unwrap(),
    )
    .unwrap();
    assert!(access.accepts_type(PlanningType::of::<ListValue>()));
    assert!(!access.accepts_type(PlanningType::of::<Vec<ListValue>>()));
}
#[test]
fn build_descriptor_with_array_value_range() {
    let schema = list_schema();
    let access = ListVariableAccess::<ListValue>::new(
        schema
            .entity::<ListEntityType>()
            .unwrap()
            .genuine_variable("valueList")
            .unwrap(),
    )
    .unwrap();
    let _binding = access.array_range(|s: &ArraySolution| &s.values);
}
#[test]
fn extract_extended_entities() {
    let solution = extended_solution();
    let entities = extractor().extract(&solution).unwrap();
    let mut expected = vec![
        solution.entity.as_ref().unwrap(),
        &solution.sub_entity.as_ref().unwrap().base,
    ];
    expected.extend(solution.entities.iter());
    expected.extend(solution.sub_entities.iter().map(|e| &e.base));
    expected.extend(
        solution
            .raw
            .iter()
            .map(|e| &e.downcast_ref::<ChildEntity>().unwrap().base),
    );
    expected.extend(
        solution
            .objects
            .iter()
            .filter_map(|e| e.downcast_ref::<BaseEntity>()),
    );
    assert_eq!(entities.len(), 11);
    // Actual identity, including projected child identities, not just code labels.
    assert_eq!(entities.len(), expected.len());
    for e in expected {
        assert_eq!(
            entities
                .iter()
                .filter(|actual| std::ptr::eq(**actual, e))
                .count(),
            1
        );
    }
}
#[test]
fn test_entity_without_variables() {
    struct NoVariableSolution;
    struct NoVariableEntity {
        _code: String,
    }
    let schema = PlanningSchema::new::<NoVariableSolution>(vec![PlanningEntityDefinition::of::<
        NoVariableEntity,
    >(vec![])])
    .unwrap();
    let error = BorrowedEntityExtractor::<NoVariableSolution, NoVariableEntity>::new(&schema)
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("must have at least 1 member with")
    );
}
#[test]
fn group_key_member_for_no_key() {
    assert!(
        resolve_alignment_key::<(), i32>(None, None, "shadow", None)
            .unwrap()
            .is_none()
    );
}
#[test]
fn group_key_member_for_entity_property_for_fact() {
    struct Solution;
    struct Example {
        fact: i32,
    }
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Example>(vec![])])
            .unwrap();
    let mut registry = AlignmentKeyRegistry::new(&schema);
    registry.register("fact", |e: &Example| e.fact).unwrap();
    let member =
        resolve_alignment_key::<Example, i32>(Some(&registry), None, "shadow", Some("fact"))
            .unwrap()
            .unwrap();
    assert_eq!(member.name(), "fact");
    assert_eq!(member.declaring_type(), PlanningType::of::<Example>());
}
#[test]
fn group_key_member_for_entity_property_for_variable() {
    struct Solution;
    struct BadExample {
        variable: i32,
    }
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<BadExample>(vec![
            PlanningVariableDefinition::basic::<i32>("variable", false),
        ])])
        .unwrap();
    let mut registry = AlignmentKeyRegistry::new(&schema);
    registry
        .register("variable", |e: &BadExample| e.variable)
        .unwrap();
    let error = resolve_alignment_key::<BadExample, i32>(
        Some(&registry),
        Some("valueSupplier"),
        "shadow",
        Some("variable"),
    )
    .err()
    .unwrap()
    .to_string();
    for part in [
        "@ShadowSources",
        "annotated supplier method (valueSupplier)",
        "for variable (shadow)",
        &format!("on class ({})", std::any::type_name::<BadExample>()),
        "uses a alignmentKey (variable) that is a variable",
    ] {
        assert!(error.contains(part));
    }
}
