// Complete adapted GenuineVariableDescriptorTest current-field cases, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;
struct Value;
struct Entity {
    code: &'static str,
    value: Option<Arc<Value>>,
}
struct ListEntityData {
    code: &'static str,
    values: Vec<Arc<Value>>,
}
struct Solution;
fn basic(allows_unassigned: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", allows_unassigned),
    ])])
    .unwrap()
}
fn scalar_case(allows_unassigned: bool) {
    let schema = basic(allows_unassigned);
    let descriptor = schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let a = Entity {
        code: "a",
        value: Some(Arc::new(Value)),
    };
    let b = Entity {
        code: "b",
        value: None,
    };
    assert_eq!(a.code, "a");
    assert_eq!(b.code, "b");
    assert!(
        !descriptor
            .is_reinitializable_on(&a, |e| PlanningVariableValue::Basic(e.value.as_ref()))
            .unwrap()
    );
    assert!(
        descriptor
            .is_reinitializable_on(&b, |e| PlanningVariableValue::Basic(e.value.as_ref()))
            .unwrap()
    );
}
#[test]
fn is_reinitializable() {
    scalar_case(false);
}
#[test]
fn is_reinitializable_allows_unassigned() {
    scalar_case(true);
}
#[test]
fn is_reinitializable_list() {
    let schema = PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<
        ListEntityData,
    >(vec![
        PlanningVariableDefinition::list::<Arc<Value>>("values", false),
    ])])
    .unwrap();
    let descriptor = schema
        .genuine_entity::<ListEntityData>()
        .unwrap()
        .genuine_variable("values")
        .unwrap();
    let a = ListEntityData {
        code: "a",
        values: vec![Arc::new(Value)],
    };
    let b = ListEntityData {
        code: "b",
        values: vec![],
    };
    assert_eq!(a.code, "a");
    assert_eq!(b.code, "b");
    assert!(
        !descriptor
            .is_reinitializable_on(&a, |e| PlanningVariableValue::List(&e.values))
            .unwrap()
    );
    assert!(
        !descriptor
            .is_reinitializable_on(&b, |e| PlanningVariableValue::List(&e.values))
            .unwrap()
    );
}
