use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;
struct Entity {
    value: String,
}
#[test]
fn selected_getters_and_setters_execute_without_direct_field_callbacks() {
    let calls = Rc::new(Cell::new(0));
    let get = calls.clone();
    let set = calls.clone();
    let registration = MemberFactoryRegistration::field_with_public_accessors(
        "NativeEntity",
        "value",
        |_: &Entity| panic!("private direct read"),
        |_: &mut Entity, _| panic!("private direct write"),
        move |e: &Entity| {
            get.set(get.get() + 1);
            e.value.clone()
        },
        move |e: &mut Entity, v| {
            set.set(set.get() + 1);
            e.value = v;
        },
    );
    let a = NativeMemberAccessorFactory::build(
        registration,
        MemberAccessRole::FieldOrGetterWithSetter,
        None,
    )
    .unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(a.kind(), FactoryAccessorKind::BeanProperty);
    for value in ["one", "two", "three"] {
        let mut e = Entity {
            value: "original".into(),
        };
        a.set(&mut e, value.into()).unwrap();
        assert_eq!(a.get(&e).unwrap(), value);
    }
    assert_eq!(calls.get(), 6);
    let capture = calls.clone();
    let rejected = MemberFactoryRegistration::field(
        "NativeEntity",
        "hidden",
        MemberExposure::Internal,
        move |e: &Entity| {
            capture.set(capture.get() + 1);
            e.value.clone()
        },
        |e: &mut Entity, v| e.value = v,
    );
    assert!(
        NativeMemberAccessorFactory::build(
            rejected,
            MemberAccessRole::FieldOrGetterWithSetter,
            Some("NativeProperty")
        )
        .is_err()
    );
    assert_eq!(calls.get(), 6);
}
#[test]
fn cache_keys_preserve_real_actions_and_separate_access_roles() {
    let registration = || {
        MemberFactoryRegistration::field(
            "NativeEntity",
            "value",
            MemberExposure::Public,
            |e: &Entity| e.value.clone(),
            |e: &mut Entity, v| e.value = v,
        )
    };
    let mut factory = NativeMemberAccessorFactory::default();
    let supplied = NativeMemberAccessorFactory::build(
        registration(),
        MemberAccessRole::FieldOrGetterWithSetter,
        None,
    )
    .unwrap();
    factory.seed(supplied.clone());
    let active = factory
        .build_and_cache(
            registration(),
            MemberAccessRole::FieldOrGetterWithSetter,
            None,
        )
        .unwrap();
    assert!(Arc::ptr_eq(&active, &supplied));
    let readonly = factory
        .build_and_cache(registration(), MemberAccessRole::FieldOrRead, None)
        .unwrap();
    assert!(!Arc::ptr_eq(&readonly, &supplied));
    assert_ne!(active.key(), readonly.key());
    let mut e = Entity {
        value: "before".into(),
    };
    active.set(&mut e, "after".into()).unwrap();
    assert_eq!(readonly.get(&e).unwrap(), "after");
    assert!(readonly.set(&mut e, "denied".into()).is_err());
    assert_eq!(e.value, "after");
    let cached = factory
        .build_and_cache(registration(), MemberAccessRole::FieldOrRead, None)
        .unwrap();
    assert!(Arc::ptr_eq(&cached, &readonly));
}
