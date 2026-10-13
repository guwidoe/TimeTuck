// Adapted complete pinned Gizmo TEST assertions; Apache-2.0, see provenance.
#[path = "fixtures/compiled_member_fixture.rs"]
mod fixture;
use fixture::*;
use std::rc::Rc;
use timetuck::*;
#[test]
fn test_generated_member_accessor_for_method() {
    let accessor = CompiledMemberFactory::default()
        .build(value_registration(), CompiledMemberRole::Read)
        .unwrap();
    assert_eq!(accessor.name(), "value");
    assert_eq!(
        accessor.value_type(),
        PlanningType::of::<Option<Rc<Value>>>()
    );
    assert_eq!(
        accessor.implementation_note(),
        "Compiled Rust function binding"
    );
    assert_eq!(accessor.owner_type(), PlanningType::of::<Entity>());
    assert!(accessor.supports_setter());
    assert!(
        accessor
            .annotations()
            .iter()
            .any(|a| matches!(a, CompiledMemberAnnotation::PlanningVariable { .. }))
    );
    let mut entity = Entity {
        value: Some(Value::new("A")),
    };
    assert_eq!(accessor.read(&entity, None).unwrap(), Some(Value::new("A")));
    accessor.write(&mut entity, Some(Value::new("B"))).unwrap();
    assert_eq!(accessor.read(&entity, None).unwrap(), Some(Value::new("B")));
}
#[test]
fn test_generated_member_accessor_for_method_without_setter() {
    let registration: CompiledMemberRegistration<GizmoEntity, String> =
        CompiledMemberRegistration::new(
            "getId",
            CompiledMemberKind::Method,
            vec![CompiledMemberAnnotation::PlanningId],
            CompiledMemberBody::Read(GizmoEntity::id),
        );
    let accessor = CompiledMemberFactory::default()
        .build(registration, CompiledMemberRole::Read)
        .unwrap();
    assert_eq!(accessor.name(), "id");
    assert_eq!(accessor.value_type(), PlanningType::of::<String>());
    assert_eq!(
        accessor.implementation_note(),
        "Compiled Rust function binding"
    );
    assert_eq!(accessor.owner_type(), PlanningType::of::<GizmoEntity>());
    assert!(!accessor.supports_setter());
    assert!(
        accessor
            .annotations()
            .contains(&CompiledMemberAnnotation::PlanningId)
    );
    assert_eq!(accessor.read(&GizmoEntity::new("A"), None).unwrap(), "A");
}
#[test]
fn test_generated_member_accessor_for_field() {
    let registration: CompiledMemberRegistration<GizmoEntity, Option<Rc<Value>>> =
        CompiledMemberRegistration::new(
            "value",
            CompiledMemberKind::Field,
            variable(),
            CompiledMemberBody::Read(field_value),
        )
        .with_setter(field_set_value);
    let accessor = CompiledMemberFactory::default()
        .build(registration, CompiledMemberRole::Read)
        .unwrap();
    assert_eq!(accessor.name(), "value");
    assert_eq!(
        accessor.value_type(),
        PlanningType::of::<Option<Rc<Value>>>()
    );
    assert_eq!(
        accessor.implementation_note(),
        "Compiled Rust function binding"
    );
    assert_eq!(accessor.owner_type(), PlanningType::of::<GizmoEntity>());
    assert!(accessor.supports_setter());
    assert!(
        accessor
            .annotations()
            .iter()
            .any(|a| matches!(a, CompiledMemberAnnotation::PlanningVariable { .. }))
    );
    let mut entity = GizmoEntity::new("A");
    entity.value = Some(Value::new("A"));
    assert_eq!(accessor.read(&entity, None).unwrap(), Some(Value::new("A")));
    accessor.write(&mut entity, Some(Value::new("B"))).unwrap();
    assert_eq!(accessor.read(&entity, None).unwrap(), Some(Value::new("B")));
}
#[test]
fn test_generated_member_accessor_for_primitive_field() {
    let registration: CompiledMemberRegistration<GizmoEntity, bool> =
        CompiledMemberRegistration::new(
            "isPinned",
            CompiledMemberKind::Field,
            vec![],
            CompiledMemberBody::Read(field_pin),
        )
        .with_setter(GizmoEntity::set_pinned);
    let accessor = CompiledMemberFactory::default()
        .build(registration, CompiledMemberRole::Read)
        .unwrap();
    assert_eq!(accessor.name(), "isPinned");
    assert_eq!(accessor.value_type(), PlanningType::of::<bool>());
    assert_eq!(
        accessor.implementation_note(),
        "Compiled Rust function binding"
    );
    assert_eq!(accessor.owner_type(), PlanningType::of::<GizmoEntity>());
    assert!(accessor.supports_setter());
    let mut entity = GizmoEntity::new("A");
    assert!(!accessor.read(&entity, None).unwrap());
    accessor.write(&mut entity, true).unwrap();
    assert!(accessor.read(&entity, None).unwrap());
}
#[test]
fn test_generated_member_accessor_same_class() {
    let mut factory = CompiledMemberFactory::default();
    let a = factory
        .build(value_registration(), CompiledMemberRole::Read)
        .unwrap();
    let b = factory
        .build(value_registration(), CompiledMemberRole::Read)
        .unwrap();
    assert!(Rc::ptr_eq(a.code_type(), b.code_type()));
    assert_eq!(a.code_type(), b.code_type());
}
#[test]
fn test_generated_member_accessor_return_void() {
    let registration: CompiledMemberRegistration<Entity, ()> = CompiledMemberRegistration::new(
        "updateValue",
        CompiledMemberKind::Method,
        vec![],
        CompiledMemberBody::Unit(Entity::update_value),
    );
    let accessor = CompiledMemberFactory::default()
        .build(registration, CompiledMemberRole::Unit)
        .unwrap();
    let mut entity = Entity {
        value: Some(Value::new("A")),
    };
    accessor.invoke(&mut entity).unwrap();
    assert_eq!(entity.value.unwrap().0, "A/A");
    assert!(!accessor.supports_setter());
    assert_eq!(accessor.owner_type(), PlanningType::of::<Entity>());
    assert_eq!(accessor.generic_type(), None);
    assert_eq!(accessor.value_type(), PlanningType::of::<()>());
    assert_eq!(
        accessor.code_type().value_type(),
        std::any::TypeId::of::<()>()
    );
    assert_eq!(accessor.name(), "updateValue");
    assert_eq!(
        accessor.implementation_note(),
        "Compiled Rust function binding"
    );
}
#[test]
fn test_generated_member_accessor_for_boolean_method() {
    let registration: CompiledMemberRegistration<GizmoEntity, bool> =
        CompiledMemberRegistration::new(
            "isPinned",
            CompiledMemberKind::Method,
            vec![CompiledMemberAnnotation::PlanningPin],
            CompiledMemberBody::Read(GizmoEntity::pinned),
        )
        .with_setter(GizmoEntity::set_pinned);
    let accessor = CompiledMemberFactory::default()
        .build(registration, CompiledMemberRole::Read)
        .unwrap();
    assert_eq!(accessor.name(), "pinned");
    assert_eq!(accessor.value_type(), PlanningType::of::<bool>());
    assert_eq!(
        accessor.implementation_note(),
        "Compiled Rust function binding"
    );
    assert_eq!(accessor.owner_type(), PlanningType::of::<GizmoEntity>());
    assert!(accessor.supports_setter());
    assert!(
        accessor
            .annotations()
            .contains(&CompiledMemberAnnotation::PlanningPin)
    );
    let mut entity = GizmoEntity::new("A");
    assert!(!accessor.read(&entity, None).unwrap());
    accessor.write(&mut entity, true).unwrap();
    assert!(accessor.read(&entity, None).unwrap());
}
#[test]
fn test_method_annotated_entity() {
    let accessor = CompiledMemberFactory::default()
        .build(range_registration(), CompiledMemberRole::ReadWithParameter)
        .unwrap();
    assert_eq!(accessor.name(), "getValueRange");
    assert_eq!(accessor.value_type(), PlanningType::of::<Vec<Rc<Value>>>());
    assert!(
        accessor
            .annotations()
            .iter()
            .any(|a| matches!(a, CompiledMemberAnnotation::ValueRangeProvider { .. }))
    );
    assert_eq!(
        accessor.parameter_type(),
        Some(PlanningType::of::<RangeSolution>())
    );
    let mut entity = RangeEntity {
        values: vec![Value::new("v1"), Value::new("v2")],
    };
    let solution = RangeSolution;
    assert_eq!(
        accessor.read(&entity, Some(&solution)).unwrap(),
        entity.values
    );
    entity.values = vec![Value::new("v2")];
    assert_eq!(
        accessor.read(&entity, Some(&solution)).unwrap(),
        entity.values
    );
}
fn assert_inheritance<P: 'static>(
    name: &str,
    read: fn(&ChildEntity, &P) -> Vec<Rc<Value>>,
    parameter: &mut P,
    change: fn(&mut P, Vec<Rc<Value>>),
) {
    let registration = CompiledMemberRegistration::new(
        name,
        CompiledMemberKind::Method,
        vec![CompiledMemberAnnotation::ValueRangeProvider {
            id: if name == "getOtherValueList" {
                "otherValueRange"
            } else {
                "valueRange"
            }
            .into(),
        }],
        CompiledMemberBody::ReadOne(read),
    );
    let accessor = CompiledMemberFactory::default()
        .build(registration, CompiledMemberRole::ReadWithParameter)
        .unwrap();
    assert_eq!(accessor.name(), name);
    assert_eq!(accessor.value_type(), PlanningType::of::<Vec<Rc<Value>>>());
    assert!(
        accessor
            .annotations()
            .iter()
            .any(|a| matches!(a, CompiledMemberAnnotation::ValueRangeProvider { .. }))
    );
    assert_eq!(accessor.parameter_type(), Some(PlanningType::of::<P>()));
    let entity = ChildEntity::default();
    assert_eq!(
        accessor.read(&entity, Some(parameter)).unwrap(),
        vec![Value::new("v1"), Value::new("v2")]
    );
    change(parameter, vec![Value::new("v2")]);
    assert_eq!(
        accessor.read(&entity, Some(parameter)).unwrap(),
        vec![Value::new("v2")]
    );
}
#[test]
fn test_method_annotated_entity_and_inheritance() {
    let mut solution = ExtendedSolution {
        base: BaseSolution {
            values: vec![Value::new("v1"), Value::new("v2")],
        },
    };
    assert_inheritance("getValueList", inherited_values, &mut solution, |s, v| {
        s.base.values = v
    });
    solution.base.values = vec![Value::new("v1"), Value::new("v2")];
    assert_inheritance(
        "getOtherValueList",
        inherited_other_values,
        &mut solution.base,
        |s, v| s.values = v,
    );
}
#[test]
fn invalid_entity_read_method_with_parameter() {
    let registration = CompiledMemberRegistration::new(
        "getValueRange",
        CompiledMemberKind::Method,
        vec![CompiledMemberAnnotation::ValueRangeProvider {
            id: "valueRange".into(),
        }],
        CompiledMemberBody::ReadTwo(RangeEntity::invalid_values),
    );
    let error = CompiledMemberFactory::default()
        .build(registration, CompiledMemberRole::ReadWithParameter)
        .err()
        .unwrap();
    assert!(
        error
            .to_string()
            .contains("The getterMethod (getValueRange) must have only one parameter")
    );
}
#[test]
fn test_forbidden_entity_read_without_parameter() {
    let accessor = CompiledMemberFactory::default()
        .build(range_registration(), CompiledMemberRole::ReadWithParameter)
        .unwrap();
    assert_eq!(
        accessor
            .read(&RangeEntity::default(), None)
            .unwrap_err()
            .to_string(),
        "Impossible state: the method executeGetter(Object) without parameter is not supported."
    );
    assert_eq!(
        accessor.getter_function().unwrap_err().to_string(),
        "Impossible state: the method getGetterFunction() is not supported."
    );
}
#[test]
fn test_returned_member_accessor() {
    let mut factory = CompiledMemberFactory::default();
    let accessor = factory
        .build(value_registration(), CompiledMemberRole::Read)
        .unwrap();
    let same = factory
        .build(value_registration(), CompiledMemberRole::Read)
        .unwrap();
    assert!(Rc::ptr_eq(&accessor, &same));
    let mut entity = Entity {
        value: Some(Value::new("A")),
    };
    assert_eq!(accessor.read(&entity, None).unwrap(), Some(Value::new("A")));
    assert!(accessor.supports_setter());
    accessor.write(&mut entity, None).unwrap();
    assert_eq!(entity.value, None);
    assert_eq!(accessor.owner_type(), PlanningType::of::<Entity>());
    assert_eq!(
        accessor.generic_type(),
        Some(PlanningType::of::<Option<Rc<Value>>>())
    );
    assert_eq!(
        accessor.value_type(),
        PlanningType::of::<Option<Rc<Value>>>()
    );
    assert_eq!(
        accessor.code_type().value_type(),
        std::any::TypeId::of::<Option<Rc<Value>>>()
    );
    assert_eq!(accessor.name(), "value");
    assert_eq!(
        accessor.implementation_note(),
        "Compiled Rust function binding"
    );
}
