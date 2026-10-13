// Adapted pinned Community SingleCascadingUpdateShadowVariableTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
struct Solution;
struct Route;
struct WrongMethod;
struct InvalidField;
struct StandardCascadeValue;
#[test]
fn required_shadow_variable_dependencies() {
    let s = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Route>(vec![PlanningVariableDefinition::list::<
            StandardCascadeValue,
        >("valueList", false)]),
        PlanningEntityDefinition::of::<WrongMethod>(vec![
            PlanningVariableDefinition::shadow::<Route>("entity"),
            PlanningVariableDefinition::shadow::<WrongMethod>("previous"),
            PlanningVariableDefinition::shadow::<WrongMethod>("next"),
            PlanningVariableDefinition::shadow::<i32>("cascadeValue"),
            PlanningVariableDefinition::shadow::<i32>("cascadeValueReturnType"),
        ]),
        PlanningEntityDefinition::of::<InvalidField>(vec![
            PlanningVariableDefinition::shadow::<Route>("entity"),
            PlanningVariableDefinition::shadow::<InvalidField>("previous"),
            PlanningVariableDefinition::shadow::<InvalidField>("next"),
            PlanningVariableDefinition::shadow::<i32>("cascadeValue"),
        ]),
    ])
    .unwrap();
    let mut registry = CascadeCallbackRegistry::default();
    registry.register(
        PlanningType::of::<WrongMethod>(),
        "updateCascadeValue",
        ListCascadeCallback::new(|_: ListCascadeContext<'_, WrongMethod, i32>| None),
    );
    registry.register_other(
        PlanningType::of::<WrongMethod>(),
        "updateCascadeValueWithReturnType",
        || 1i32,
    );
    let field = s
        .entity::<WrongMethod>()
        .unwrap()
        .variable("cascadeValueReturnType")
        .unwrap();
    let error = registry
        .bind::<WrongMethod, i32>(field, "badUpdateCascadeValueWithReturnType")
        .err()
        .unwrap();
    let message = error.to_string();
    assert!(message.contains(std::any::type_name::<WrongMethod>()));
    assert!(message.contains("annotated property (cascadeValueReturnType)"));
    assert!(message.contains("method \"badUpdateCascadeValueWithReturnType\" cannot be found"));
    registry.register(
        PlanningType::of::<InvalidField>(),
        "updateCascadeValue",
        ListCascadeCallback::new(|_: ListCascadeContext<'_, InvalidField, i32>| None),
    );
    registry.register_data(PlanningType::of::<InvalidField>(), "value", 1i32);
    let field = s
        .entity::<InvalidField>()
        .unwrap()
        .variable("cascadeValue")
        .unwrap();
    let error = registry
        .bind::<InvalidField, i32>(field, "value")
        .err()
        .unwrap();
    let message = error.to_string();
    assert!(message.contains(std::any::type_name::<InvalidField>()));
    assert!(message.contains("annotated property (cascadeValue)"));
    assert!(message.contains("method \"value\" cannot be found"));
}
