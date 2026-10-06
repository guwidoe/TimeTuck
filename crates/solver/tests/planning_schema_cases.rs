// Adapted from PlanningSolutionMetaModelTest and its basic/list fixtures.
// Apache-2.0; see compat/provenance.json. Explicit typed schemas replace annotations.
use timetuck::*;
struct TestdataSolution;
struct TestdataEntity;
struct TestdataValue;
struct TestdataListSolution;
struct TestdataListEntity;
struct TestdataListValue;
fn basic() -> PlanningSchema {
    PlanningSchema::new::<TestdataSolution>(vec![PlanningEntityDefinition::of::<TestdataEntity>(
        vec![PlanningVariableDefinition::basic::<TestdataValue>(
            "value", false,
        )],
    )])
    .unwrap()
}
fn list() -> PlanningSchema {
    PlanningSchema::new::<TestdataListSolution>(vec![
        PlanningEntityDefinition::of::<TestdataListEntity>(vec![
            PlanningVariableDefinition::list::<TestdataListValue>("valueList", false),
        ]),
        PlanningEntityDefinition::of::<TestdataListValue>(vec![
            PlanningVariableDefinition::shadow::<TestdataListEntity>("entity"),
            PlanningVariableDefinition::shadow::<i32>("index"),
        ]),
    ])
    .unwrap()
}
#[test]
fn basic_solution_type() {
    let s = basic();
    assert_eq!(s.solution_type(), PlanningType::of::<TestdataSolution>());
}
#[test]
fn basic_solution_entities() {
    let s = basic();
    assert_eq!(
        s.entities().collect::<Vec<_>>(),
        vec![s.entity::<TestdataEntity>().unwrap()]
    );
}
#[test]
fn basic_solution_genuine_entities() {
    let s = basic();
    assert_eq!(
        s.genuine_entities().collect::<Vec<_>>(),
        vec![s.genuine_entity::<TestdataEntity>().unwrap()]
    );
}
#[test]
fn basic_solution_wrong_entities() {
    let s = basic();
    assert!(
        s.entity::<TestdataListEntity>()
            .unwrap_err()
            .to_string()
            .contains("TestdataListEntity")
    );
    assert!(
        s.entity::<TestdataValue>()
            .unwrap_err()
            .to_string()
            .contains("TestdataValue")
    );
}
#[test]
fn list_solution_type() {
    let s = list();
    assert_eq!(
        s.solution_type(),
        PlanningType::of::<TestdataListSolution>()
    );
}
#[test]
fn list_solution_entities() {
    let s = list();
    assert_eq!(
        s.entities().collect::<Vec<_>>(),
        vec![
            s.entity::<TestdataListEntity>().unwrap(),
            s.entity::<TestdataListValue>().unwrap()
        ]
    );
}
#[test]
fn list_solution_genuine_entities() {
    let s = list();
    assert_eq!(
        s.genuine_entities().collect::<Vec<_>>(),
        vec![s.genuine_entity::<TestdataListEntity>().unwrap()]
    );
}
#[test]
fn list_solution_wrong_entities() {
    let s = list();
    assert!(
        s.entity::<TestdataEntity>()
            .unwrap_err()
            .to_string()
            .contains("TestdataEntity")
    );
    assert!(
        s.entity::<TestdataValue>()
            .unwrap_err()
            .to_string()
            .contains("TestdataValue")
    );
}
#[test]
fn basic_entity_parent() {
    let s = basic();
    let e = s.genuine_entity::<TestdataEntity>().unwrap();
    assert!(std::ptr::eq(e.solution(), &s));
}
#[test]
fn basic_entity_type() {
    let s = basic();
    let e = s.genuine_entity::<TestdataEntity>().unwrap();
    assert_eq!(e.entity_type(), PlanningType::of::<TestdataEntity>());
}
#[test]
fn basic_entity_kind() {
    let s = basic();
    let e = s.genuine_entity::<TestdataEntity>().unwrap();
    assert!(e.is_genuine());
}
#[test]
fn basic_entity_variables() {
    let s = basic();
    let e = s.genuine_entity::<TestdataEntity>().unwrap();
    assert_eq!(
        e.genuine_variables().collect::<Vec<_>>(),
        vec![e.genuine_variable("value").unwrap()]
    );
}
#[test]
fn basic_entity_missing_variable() {
    let s = basic();
    let e = s.genuine_entity::<TestdataEntity>().unwrap();
    assert!(
        e.variable("nonExisting")
            .unwrap_err()
            .to_string()
            .contains("nonExisting")
    );
}
#[test]
fn basic_variable_parent() {
    let s = basic();
    let e = s.genuine_entity::<TestdataEntity>().unwrap();
    let v = e.variable("value").unwrap();
    assert!(std::ptr::eq(v.entity().definition(), e.definition()));
}
#[test]
fn basic_variable_type() {
    let s = basic();
    let e = s.genuine_entity::<TestdataEntity>().unwrap();
    let v = e.variable("value").unwrap();
    assert_eq!(v.value_type(), PlanningType::of::<TestdataValue>());
}
#[test]
fn list_entity_parent() {
    let s = list();
    let e = s.genuine_entity::<TestdataListEntity>().unwrap();
    assert!(std::ptr::eq(e.solution(), &s));
}
#[test]
fn list_entity_type() {
    let s = list();
    let e = s.genuine_entity::<TestdataListEntity>().unwrap();
    assert_eq!(e.entity_type(), PlanningType::of::<TestdataListEntity>());
}
#[test]
fn list_entity_kind() {
    let s = list();
    let e = s.genuine_entity::<TestdataListEntity>().unwrap();
    assert!(e.is_genuine());
}
#[test]
fn list_entity_variables() {
    let s = list();
    let e = s.genuine_entity::<TestdataListEntity>().unwrap();
    assert_eq!(
        e.genuine_variables().collect::<Vec<_>>(),
        vec![e.genuine_variable("valueList").unwrap()]
    );
}
#[test]
fn list_entity_missing_variable() {
    let s = list();
    let e = s.genuine_entity::<TestdataListEntity>().unwrap();
    assert!(
        e.variable("nonExisting")
            .unwrap_err()
            .to_string()
            .contains("nonExisting")
    );
}
#[test]
fn list_variable_parent() {
    let s = list();
    let e = s.genuine_entity::<TestdataListEntity>().unwrap();
    let v = e.variable("valueList").unwrap();
    assert!(std::ptr::eq(v.entity().definition(), e.definition()));
}
#[test]
fn list_variable_type() {
    let s = list();
    let e = s.genuine_entity::<TestdataListEntity>().unwrap();
    let v = e.variable("valueList").unwrap();
    assert_eq!(v.value_type(), PlanningType::of::<TestdataListValue>());
}
#[test]
fn shadow_entity_parent() {
    let s = list();
    let e = s.shadow_entity::<TestdataListValue>().unwrap();
    assert!(std::ptr::eq(e.solution(), &s));
}
#[test]
fn shadow_entity_type() {
    let s = list();
    let e = s.shadow_entity::<TestdataListValue>().unwrap();
    assert_eq!(e.entity_type(), PlanningType::of::<TestdataListValue>());
}
#[test]
fn shadow_entity_kind() {
    let s = list();
    let e = s.shadow_entity::<TestdataListValue>().unwrap();
    assert!(!e.is_genuine());
}
#[test]
fn shadow_entity_variables() {
    let s = list();
    let e = s.shadow_entity::<TestdataListValue>().unwrap();
    assert_eq!(
        e.variables().collect::<Vec<_>>(),
        vec![e.variable("entity").unwrap(), e.variable("index").unwrap()]
    );
}
#[test]
fn shadow_entity_missing_variable() {
    let s = list();
    let e = s.shadow_entity::<TestdataListValue>().unwrap();
    assert!(
        e.variable("nonExisting")
            .unwrap_err()
            .to_string()
            .contains("nonExisting")
    );
}
#[test]
fn shadow_variable_parent() {
    let s = list();
    let e = s.shadow_entity::<TestdataListValue>().unwrap();
    let v = e.variable("entity").unwrap();
    assert!(std::ptr::eq(v.entity().definition(), e.definition()));
}
#[test]
fn shadow_variable_type() {
    let s = list();
    let e = s.shadow_entity::<TestdataListValue>().unwrap();
    let v = e.variable("entity").unwrap();
    assert_eq!(v.value_type(), PlanningType::of::<TestdataListEntity>());
}
