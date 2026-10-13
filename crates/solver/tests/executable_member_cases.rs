// Adapted Community descriptor/factory TEST contracts, Apache-2.0.
use std::{cell::Cell, sync::Arc};
use timetuck::*;
include!("fixtures/executable_member_fixture.rs");
#[test]
fn test_creating_descriptor_for_private_field_with_public_accessors_work() {
    let d = ExecutableMemberDescriptor::private_field_with_accessors(
        "value",
        FieldAnnotatedEntity::get_value,
        FieldAnnotatedEntity::set_value,
        FieldAnnotatedEntity::get_value,
        FieldAnnotatedEntity::set_value,
    )
    .unwrap();
    assert_eq!(d.owner_type(), PlanningType::of::<FieldAnnotatedEntity>());
    assert_eq!(d.name(), "value");
}
#[test]
fn test_that_creating_descriptor_for_private_members_fail() {
    let d = ExecutableMemberDescriptor::method(
        "getPrivateProperty",
        MemberExposure::Internal,
        |s: &VisibilitySolution| s.private_property.clone(),
    );
    assert_eq!(
        d.err().unwrap().0,
        format!(
            "Member (getPrivateProperty) of class ({}) is not public.",
            PlanningType::of::<VisibilitySolution>().name()
        )
    );
}
#[test]
fn test_that_when_is_method_execute_consumer_iff_member_is_method() {
    let calls = Cell::new(0);
    id().when_method(|_| calls.set(calls.get() + 1));
    assert_eq!(calls.get(), 1);
    calls.set(0);
    field().when_method(|_| calls.set(calls.get() + 1));
    assert_eq!(calls.get(), 0);
}
#[test]
fn test_that_when_is_field_execute_consumer_iff_member_is_field() {
    let calls = Cell::new(0);
    field().when_field(|_| calls.set(calls.get() + 1));
    assert_eq!(calls.get(), 1);
    calls.set(0);
    id().when_field(|_| calls.set(calls.get() + 1));
    assert_eq!(calls.get(), 0);
}
#[test]
fn test_that_get_declaring_class_name_is_correct() {
    assert_eq!(field().owner_type(), PlanningType::of::<GizmoEntity>());
    assert_eq!(id().owner_type(), PlanningType::of::<GizmoEntity>());
}
#[test]
fn test_member_descriptor_name_is_correct() {
    assert_eq!(field().name(), "value");
    assert_eq!(id().name(), "id");
    assert_eq!(read_method().name(), "readMethod");
}
#[test]
fn test_member_descriptor_setter_is_correct() {
    assert_eq!(field().setter(), None);
    assert_eq!(id().setter(), None);
    let d = ExecutableMemberDescriptor::method(
        "getValue",
        MemberExposure::Public,
        GizmoEntity::get_value,
    )
    .unwrap()
    .with_method_setter("setValue", GizmoEntity::set_value)
    .unwrap();
    assert_eq!(
        d.setter(),
        Some(&ExecutableMethodIdentity {
            owner: PlanningType::of::<GizmoEntity>(),
            name: "setValue".into(),
            parameter: Some(PlanningType::of::<Arc<TestValue>>()),
            result: PlanningType::of::<()>()
        })
    );
    assert_eq!(read_method().setter(), None);
}
#[test]
fn test_member_descriptor_type_name_is_correct() {
    assert_eq!(field().value_type(), PlanningType::of::<Arc<TestValue>>());
    assert_eq!(id().value_type(), PlanningType::of::<String>());
    let d = ExecutableMemberDescriptor::field(
        "genericField",
        |e: &GizmoEntity| e.generic.clone(),
        |e, v| e.generic = v,
    )
    .unwrap();
    assert_eq!(
        d.value_type(),
        PlanningType::of::<Vec<std::collections::BTreeMap<String, String>>>()
    );
}
#[test]
fn method_return_void() {
    let member = ExecutableVoidMember::build("updateValue", MutableEntity::update_value).unwrap();
    assert_eq!(member.kind(), ExecutableMemberKind::Method);
    assert_eq!(member.name(), "updateValue");
    assert_eq!(member.value_type(), PlanningType::of::<()>());
    assert_eq!(member.generic_type(), PlanningType::of::<()>());
    assert_eq!(
        member.backend(),
        ExecutableMemberBackend::RegisteredRustCallback
    );
    let mut e = MutableEntity {
        value: Arc::new(TestValue("A".into())),
    };
    member.invoke(&mut e).unwrap();
    assert_eq!(e.value.0, "A/A");
}
