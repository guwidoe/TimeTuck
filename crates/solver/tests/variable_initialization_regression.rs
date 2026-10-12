// Original type/kind guards, getter side effects and borrow-only value controls.
use std::cell::Cell;
use timetuck::*;
struct Value; // Deliberately neither Eq nor Clone.
struct Owner {
    value: Option<Value>,
    list: Vec<Value>,
}
struct Foreign;
struct Solution;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Owner>(vec![
        PlanningVariableDefinition::basic::<Value>("value", true),
        PlanningVariableDefinition::list::<Value>("list", true),
        PlanningVariableDefinition::shadow::<Value>("shadow"),
    ])])
    .unwrap()
}
#[test]
fn wrong_entities_and_value_types_reject_before_getters_and_shadow_fields_reject() {
    let s = schema();
    let e = s.genuine_entity::<Owner>().unwrap();
    let scalar = e.genuine_variable("value").unwrap();
    let calls = Cell::new(0);
    assert!(matches!(
        scalar.is_reinitializable_on(&Foreign, |_| {
            calls.set(calls.get() + 1);
            PlanningVariableValue::<Value>::Basic(None)
        }),
        Err(PlanningSchemaError::UnknownEntity(_))
    ));
    let source = Owner {
        value: None,
        list: vec![],
    };
    assert!(matches!(
        scalar.is_reinitializable_on(&source, |_| {
            calls.set(calls.get() + 1);
            PlanningVariableValue::<i64>::Basic(None)
        }),
        Err(PlanningSchemaError::WrongValueType { .. })
    ));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        e.variable("shadow")
            .unwrap()
            .is_reinitializable(PlanningVariableValue::<Value>::Basic(None)),
        Err(PlanningSchemaError::WrongVariableKind { .. })
    ));
}
#[test]
fn field_shape_guards_and_live_borrowed_values_do_not_require_clone_or_equality() {
    let s = schema();
    let e = s.genuine_entity::<Owner>().unwrap();
    let scalar = e.genuine_variable("value").unwrap();
    let list = e.genuine_variable("list").unwrap();
    assert!(matches!(
        scalar.is_reinitializable(PlanningVariableValue::<Value>::List(&[])),
        Err(PlanningSchemaError::WrongVariableKind { .. })
    ));
    assert!(matches!(
        list.is_reinitializable(PlanningVariableValue::<Value>::Basic(None)),
        Err(PlanningSchemaError::WrongVariableKind { .. })
    ));
    let mut owner = Owner {
        value: None,
        list: vec![],
    };
    let calls = Cell::new(0);
    assert!(
        scalar
            .is_reinitializable_on(&owner, |e| {
                calls.set(calls.get() + 1);
                PlanningVariableValue::Basic(e.value.as_ref())
            })
            .unwrap()
    );
    owner.value = Some(Value);
    assert!(
        !scalar
            .is_reinitializable_on(&owner, |e| {
                calls.set(calls.get() + 1);
                PlanningVariableValue::Basic(e.value.as_ref())
            })
            .unwrap()
    );
    assert_eq!(calls.get(), 2);
    assert!(
        !list
            .is_reinitializable_on(&owner, |e| PlanningVariableValue::List(&e.list))
            .unwrap()
    );
    owner.list.push(Value);
    assert!(
        !list
            .is_reinitializable_on(&owner, |e| PlanningVariableValue::List(&e.list))
            .unwrap()
    );
}
