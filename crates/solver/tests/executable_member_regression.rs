use std::{cell::Cell, rc::Rc, sync::Arc};
use timetuck::*;
struct Entity {
    id: String,
    value: Arc<String>,
}
#[test]
fn executable_private_field_routes_and_typed_method_writes_read_current_values() {
    let gets = Rc::new(Cell::new(0));
    let sets = Rc::new(Cell::new(0));
    let g = gets.clone();
    let s = sets.clone();
    let d = ExecutableMemberDescriptor::private_field_with_accessors(
        "value",
        |_: &Entity| panic!("private direct read"),
        |_, _| panic!("private direct write"),
        move |e| {
            g.set(g.get() + 1);
            e.value.clone()
        },
        move |e, v| {
            s.set(s.get() + 1);
            e.value = v
        },
    )
    .unwrap();
    let mut e = Entity {
        id: "old".into(),
        value: Arc::new("first".into()),
    };
    let value: Arc<String> = Arc::new("second".into());
    d.write(&mut e, value.clone()).unwrap();
    assert!(Arc::ptr_eq(&d.read(&e).unwrap(), &value));
    assert_eq!((gets.get(), sets.get()), (1, 1));
    assert!(d.setter().is_none());
    let getter =
        ExecutableMemberDescriptor::method("getId", MemberExposure::Public, |e: &Entity| {
            e.id.clone()
        })
        .unwrap()
        .with_method_setter("setId", |e, v| e.id = v)
        .unwrap();
    getter.write(&mut e, "new".into()).unwrap();
    assert_eq!(getter.read(&e).unwrap(), "new");
    let readonly =
        ExecutableMemberDescriptor::method("getId", MemberExposure::Public, |e: &Entity| {
            e.id.clone()
        })
        .unwrap();
    assert!(readonly.write(&mut e, "bad".into()).is_err());
    assert_eq!(e.id, "new");
}
#[test]
fn consumers_receive_live_type_metadata_and_private_callbacks_never_execute() {
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    assert!(
        ExecutableMemberDescriptor::method(
            "private",
            MemberExposure::Internal,
            move |_: &Entity| {
                c.set(c.get() + 1);
                String::new()
            }
        )
        .is_err()
    );
    assert_eq!(calls.get(), 0);
    let d = ExecutableMemberDescriptor::method("getId", MemberExposure::Public, |e: &Entity| {
        e.id.clone()
    })
    .unwrap();
    let e = Entity {
        id: "payload".into(),
        value: Arc::new("v".into()),
    };
    d.when_method(|m| {
        assert!(m.owner.is::<Entity>());
        assert!(m.result.is::<String>());
        assert_eq!(m.name, "getId");
        assert_eq!(m.parameter, None);
        assert_eq!(d.read(&e).unwrap(), "payload");
    });
    let field = ExecutableMemberDescriptor::field(
        "value",
        |e: &Entity| e.value.clone(),
        |e, v| e.value = v,
    )
    .unwrap();
    field.when_field(|m| {
        assert!(m.owner.is::<Entity>());
        assert!(m.value.is::<Arc<String>>());
        assert_eq!(m.name, "value");
    });
}
#[test]
fn void_actions_repeat_actual_mutation_and_validate_names() {
    assert!(ExecutableVoidMember::<Entity>::build("", |_| {}).is_err());
    let action = ExecutableVoidMember::build("advance", |e: &mut Entity| e.id.push('!')).unwrap();
    let mut e = Entity {
        id: "start".into(),
        value: Arc::new("unchanged".into()),
    };
    let old = e.value.clone();
    for _ in 0..3 {
        action.invoke(&mut e).unwrap();
    }
    assert_eq!(e.id, "start!!!");
    assert!(Arc::ptr_eq(&old, &e.value));
}
