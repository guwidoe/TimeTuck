use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[derive(Default)]
struct Entity {
    value: i64,
    void_calls: usize,
}
#[test]
fn actual_typed_reads_writes_parameters_and_void_execution() {
    use MemberExposure::Public;
    let member: RegisteredMember<Entity, i64, usize> = RegisteredMember::new(
        "NativeEntity",
        Public,
        "value",
        Public,
        "i64",
        RegisteredMemberBody::Field {
            read: Box::new(|e: &Entity| e.value),
            write: Some(Box::new(|e: &mut Entity, v| e.value = v)),
            getter: None,
            setter: None,
        },
    );
    let mut e = Entity::default();
    let bound = member
        .bind(MemberAccessRole::FieldOrGetterWithSetter, None)
        .unwrap();
    for v in [7, -19, 2048] {
        bound.write(&mut e, v).unwrap();
        assert_eq!(bound.read(&e, None).unwrap(), v);
    }
    let readonly = member.bind(MemberAccessRole::FieldOrRead, None).unwrap();
    assert!(readonly.write(&mut e, 99).is_err());
    assert_eq!(e.value, 2048);
    let method: RegisteredMember<Entity, i64, usize> = RegisteredMember::new(
        "NativeEntity",
        Public,
        "readScaled",
        Public,
        "i64",
        RegisteredMemberBody::ReadOne(Box::new(|e: &Entity, p| e.value * (p as i64))),
    );
    let bound = method
        .bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None)
        .unwrap();
    assert_eq!(bound.read(&e, Some(3)).unwrap(), 6144);
    assert!(bound.read(&e, None).is_err());
    let callback: RegisteredMember<Entity, i64, usize> = RegisteredMember::new(
        "NativeEntity",
        Public,
        "update",
        Public,
        "void",
        RegisteredMemberBody::Void(Box::new(|e: &mut Entity| {
            e.void_calls += 1;
            e.value += 1;
        })),
    );
    let bound = callback.bind(MemberAccessRole::Void, None).unwrap();
    bound.invoke_void(&mut e).unwrap();
    assert_eq!(e.void_calls, 1);
    assert_eq!(e.value, 2049);
    assert!(bound.read(&e, None).is_err());
}
#[test]
fn validation_rejects_before_actual_callbacks_and_preserves_entity() {
    use MemberExposure::*;
    let calls = Rc::new(Cell::new(0));
    let capture = calls.clone();
    let read: RegisteredMember<Entity, i64, usize> = RegisteredMember::new(
        "NativeEntity",
        Public,
        "readBoth",
        Public,
        "i64",
        RegisteredMemberBody::ReadTwo(Box::new(move |e: &Entity, a, b| {
            capture.set(capture.get() + 1);
            e.value + (a + b) as i64
        })),
    );
    assert!(
        read.bind(MemberAccessRole::FieldOrReadWithOptionalParameter, None)
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    let read: RegisteredMember<Entity, i64, usize> = RegisteredMember::new(
        "InternalEntity",
        Internal,
        "getValue",
        Public,
        "i64",
        RegisteredMemberBody::Read(Box::new(|e: &Entity| e.value)),
    );
    assert!(
        read.bind(MemberAccessRole::FieldOrGetter, None)
            .unwrap_err_message()
            .contains("declaring class")
    );
    let capture = calls.clone();
    let hidden: RegisteredMember<Entity, i64, usize> = RegisteredMember::new(
        "NativeEntity",
        Public,
        "hidden",
        Internal,
        "i64",
        RegisteredMemberBody::Field {
            read: Box::new(|e: &Entity| e.value),
            write: None,
            getter: None,
            setter: Some((
                Public,
                Box::new(move |e: &mut Entity, v| {
                    capture.set(capture.get() + 1);
                    e.value = v;
                }),
            )),
        },
    );
    assert!(
        hidden
            .bind(MemberAccessRole::FieldOrGetterWithSetter, None)
            .is_err()
    );
    assert_eq!(calls.get(), 0);
    let e = Entity {
        value: 11,
        void_calls: 0,
    };
    assert_eq!(e.value, 11);
    assert_eq!(e.void_calls, 0);
}
trait ErrMessage {
    fn unwrap_err_message(self) -> String;
}
impl<T> ErrMessage for Result<T, MemberAccessError> {
    fn unwrap_err_message(self) -> String {
        match self {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        }
    }
}

#[test]
fn public_readable_field_uses_registered_setter_without_direct_write() {
    let member: RegisteredMember<Entity, i64, usize> = RegisteredMember::new(
        "NativeEntity",
        MemberExposure::Public,
        "value",
        MemberExposure::Public,
        "i64",
        RegisteredMemberBody::Field {
            read: Box::new(|e: &Entity| e.value),
            write: None,
            getter: None,
            setter: Some((
                MemberExposure::Public,
                Box::new(|e: &mut Entity, v| e.value = v),
            )),
        },
    );
    let binding = member
        .bind(MemberAccessRole::FieldOrGetterWithSetter, None)
        .unwrap();
    let mut e = Entity::default();
    binding.write(&mut e, 88).unwrap();
    assert_eq!(binding.read(&e, None).unwrap(), 88);
}
