// Adapted pinned Community MemberAccessorFactoryTest; Apache-2.0.
use std::sync::Arc;
use timetuck::*;
include!("fixtures/member_factory_fixture.rs");
#[test]
fn field_annotated_entity_with_public_getter() {
    let a = NativeMemberAccessorFactory::build(
        MemberFactoryRegistration::field_with_public_accessors(
            "FieldEntity",
            "value",
            |e: &FieldEntity| e.value.clone(),
            |e: &mut FieldEntity, v| e.value = v,
            FieldEntity::get_value,
            FieldEntity::set_value,
        ),
        MemberAccessRole::FieldOrGetterWithSetter,
        Some("PlanningVariable"),
    )
    .unwrap();
    assert_eq!(a.kind(), FactoryAccessorKind::BeanProperty);
    assert_eq!(a.name(), "value");
    assert_eq!(a.value_type(), PlanningType::of::<Value>());
    let v1 = Arc::new(Value("v1".into()));
    let v2 = Arc::new(Value("v2".into()));
    let mut e = FieldEntity { value: v1.clone() };
    assert!(Arc::ptr_eq(&a.get(&e).unwrap(), &v1));
    a.set(&mut e, v2.clone()).unwrap();
    assert!(Arc::ptr_eq(&e.get_value(), &v2));
}
#[test]
fn private_field() {
    let r = MemberFactoryRegistration::field(
        CLASS,
        "privateField",
        MemberExposure::Internal,
        |s: &VisibilitySolution| s.private_field.clone(),
        |s: &mut VisibilitySolution, v| s.private_field = v,
    );
    let error = NativeMemberAccessorFactory::build(
        r,
        MemberAccessRole::FieldOrGetterWithSetter,
        Some("ProblemFactProperty"),
    )
    .err()
    .unwrap()
    .to_string();
    for fragment in [
        "The @ProblemFactProperty annotated field",
        "privateField",
        CLASS,
        "is not public and does not have a public getter method",
    ] {
        assert!(error.contains(fragment));
    }
}
#[test]
fn public_field() {
    let a = NativeMemberAccessorFactory::build(
        MemberFactoryRegistration::field(
            CLASS,
            "publicField",
            MemberExposure::Public,
            |s: &VisibilitySolution| s.public_field.clone(),
            |s: &mut VisibilitySolution, v| s.public_field = v,
        ),
        MemberAccessRole::FieldOrGetterWithSetter,
        Some("ProblemFactProperty"),
    )
    .unwrap();
    assert_eq!(a.kind(), FactoryAccessorKind::Field);
    assert_eq!(a.name(), "publicField");
    assert_eq!(a.value_type(), PlanningType::of::<String>());
    let mut s = VisibilitySolution::new(["n/a", "firstValue", "n/a", "n/a", "n/a", "n/a"]);
    assert_eq!(a.get(&s).unwrap(), "firstValue");
    a.set(&mut s, "secondValue".into()).unwrap();
    assert_eq!(a.get(&s).unwrap(), "secondValue");
}
#[test]
fn public_property() {
    let a = NativeMemberAccessorFactory::build(
        property(),
        MemberAccessRole::FieldOrGetterWithSetter,
        Some("ProblemFactProperty"),
    )
    .unwrap();
    assert_eq!(a.kind(), FactoryAccessorKind::BeanProperty);
    assert_eq!(a.name(), "publicProperty");
    assert_eq!(a.value_type(), PlanningType::of::<String>());
    let mut s = VisibilitySolution::new(["n/a", "n/a", "n/a", "n/a", "n/a", "firstValue"]);
    assert_eq!(a.get(&s).unwrap(), "firstValue");
    a.set(&mut s, "secondValue".into()).unwrap();
    assert_eq!(a.get(&s).unwrap(), "secondValue");
}
#[test]
fn should_use_generated_member_accessor_if_exists() {
    let supplied = NativeMemberAccessorFactory::build(
        property(),
        MemberAccessRole::FieldOrGetterWithSetter,
        Some("ProblemFactProperty"),
    )
    .unwrap();
    let mut factory = NativeMemberAccessorFactory::new();
    factory.seed(supplied.clone());
    let actual = factory
        .build_and_cache(
            property(),
            MemberAccessRole::FieldOrGetterWithSetter,
            Some("ProblemFactProperty"),
        )
        .unwrap();
    assert!(Arc::ptr_eq(&actual, &supplied));
    let mut s = VisibilitySolution::new(["n/a", "n/a", "n/a", "n/a", "n/a", "firstValue"]);
    actual.set(&mut s, "provided action".into()).unwrap();
    assert_eq!(actual.get(&s).unwrap(), "provided action");
}
