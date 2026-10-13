// Adapted pinned Community reflection accessor TESTS; Apache-2.0.
use std::sync::Arc;
use timetuck::*;
include!("fixtures/typed_accessor_fixture.rs");
#[test]
fn field_annotated_entity() {
    let accessor = TypedPropertyAccessor::field(
        "value",
        planning(),
        |e: &FieldEntity| e.value.clone(),
        |e: &mut FieldEntity, v| e.value = v,
    );
    assert_eq!(accessor.name(), "value");
    assert_eq!(accessor.value_type(), PlanningType::of::<Value>());
    assert!(matches!(
        &accessor.annotations()[0],
        MemberAnnotation::PlanningVariable { .. }
    ));
    let v1 = Arc::new(Value("v1"));
    let v2 = Arc::new(Value("v2"));
    let mut e1 = FieldEntity { value: v1.clone() };
    assert!(Arc::ptr_eq(&accessor.get(&e1).unwrap(), &v1));
    accessor.set(&mut e1, v2.clone()).unwrap();
    assert!(Arc::ptr_eq(&e1.get_value(), &v2));
}
#[test]
fn bean_method_annotated_entity() {
    let accessor = TypedPropertyAccessor::bean(
        "value",
        "getValue",
        planning(),
        Entity::get_value,
        Entity::set_value,
    )
    .unwrap();
    assert_eq!(accessor.name(), "value");
    assert_eq!(accessor.value_type(), PlanningType::of::<Value>());
    assert!(matches!(
        &accessor.annotations()[0],
        MemberAnnotation::PlanningVariable { .. }
    ));
    let v1 = Arc::new(Value("v1"));
    let v2 = Arc::new(Value("v2"));
    let mut e1 = Entity { value: v1.clone() };
    assert!(Arc::ptr_eq(&accessor.get(&e1).unwrap(), &v1));
    accessor.set(&mut e1, v2.clone()).unwrap();
    assert!(Arc::ptr_eq(&e1.get_value(), &v2));
}
#[test]
fn extended_method_annotated_entity() {
    let accessor = TypedParameterAccessor::new(
        "getValueRange",
        range("valueRange"),
        ParameterEntity::get_value_range,
    );
    assert_eq!(accessor.name(), "getValueRange");
    assert_eq!(
        accessor.value_type(),
        PlanningType::of::<Option<Vec<Arc<Value>>>>()
    );
    assert!(matches!(
        &accessor.annotations()[0],
        MemberAnnotation::ValueRangeProvider { .. }
    ));
    assert_eq!(
        accessor.parameter_type(),
        PlanningType::of::<ParameterSolution>()
    );
    let v1 = Arc::new(Value("v1"));
    let v2 = Arc::new(Value("v2"));
    let mut e1 = ParameterEntity {
        values: Some(vec![v1.clone(), v2.clone()]),
        _value: Some(v1.clone()),
    };
    let s1 = ParameterSolution;
    assert_eq!(
        accessor.get(&e1, Some(&s1)).unwrap(),
        Some(vec![v1, v2.clone()])
    );
    e1.values = Some(vec![v2.clone()]);
    assert_eq!(accessor.get(&e1, Some(&s1)).unwrap(), Some(vec![v2]));
}
#[test]
fn method_annotated_entity_and_inheritance() {
    let member = TypedParameterAccessor::new(
        "getValueList",
        range("valueRange"),
        |e: &ChildEntity, s: &ExtendedSolution| e.base.get_value_list(s),
    );
    let other = TypedParameterAccessor::new(
        "getOtherValueList",
        range("otherValueRange"),
        |e: &ChildEntity, s: &BaseSolution| e.base.get_other_value_list(s),
    );
    assert_inherited(&member, "getValueList", |s| s);
    assert_inherited(&other, "getOtherValueList", |s| &s.base);
}
fn assert_inherited<S: 'static>(
    member: &TypedParameterAccessor<ChildEntity, S, Vec<Arc<Value>>>,
    name: &str,
    project: impl Fn(&ExtendedSolution) -> &S,
) {
    assert_eq!(member.name(), name);
    assert_eq!(member.value_type(), PlanningType::of::<Vec<Arc<Value>>>());
    assert!(matches!(
        &member.annotations()[0],
        MemberAnnotation::ValueRangeProvider { .. }
    ));
    assert_eq!(member.parameter_type(), PlanningType::of::<S>());
    let v1 = Arc::new(Value("v1"));
    let v2 = Arc::new(Value("v2"));
    let e = ChildEntity {
        base: BaseEntity { _value: v1.clone() },
    };
    let mut s = ExtendedSolution {
        base: BaseSolution {
            values: vec![v1.clone(), v2.clone()],
        },
    };
    assert_eq!(
        member.get(&e, Some(project(&s))).unwrap(),
        vec![v1, v2.clone()]
    );
    s.base.values = vec![v2.clone()];
    assert_eq!(member.get(&e, Some(project(&s))).unwrap(), vec![v2]);
}
#[test]
fn forbidden_entity_read_without_parameter() {
    let member = TypedParameterAccessor::new(
        "getValueRange",
        range("valueRange"),
        ParameterEntity::get_value_range,
    );
    let e = ParameterEntity {
        values: None,
        _value: None,
    };
    assert!(member.get(&e, None).unwrap_err().to_string().contains(
        "Impossible state: the method executeGetter(Object) without parameter is not supported."
    ));
    assert!(
        member
            .getter_function()
            .unwrap_err()
            .to_string()
            .contains("Impossible state: the method getGetterFunction() is not supported.")
    );
}
