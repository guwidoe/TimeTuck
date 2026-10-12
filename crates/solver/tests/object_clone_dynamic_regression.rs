// Original concrete-runtime dispatch and cross-view alias controls.
use std::sync::{Arc, Mutex};
use timetuck::*;
struct Node {
    next: Mutex<Option<ObjectGraphObject>>,
    fact: Mutex<Option<ObjectGraphObject>>,
    shared: Mutex<Option<ObjectGraphObject>>,
    count: i32,
}
fn registry() -> ObjectGraphCloneRegistry<'static> {
    use ObjectCloneFieldPolicy::{Registered, Shared};
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<Node>(
        |s| {
            Ok(Node {
                next: Mutex::new(None),
                fact: Mutex::new(None),
                shared: Mutex::new(None),
                count: s.count,
            })
        },
        |s, c| {
            c.optional_object(&s.next.lock().unwrap(), Registered)?;
            c.optional_object(&s.fact.lock().unwrap(), Registered)?;
            c.optional_object(&s.shared.lock().unwrap(), Shared)
        },
        |s, t, c| {
            *t.next.lock().unwrap() = c.optional_object(&s.next.lock().unwrap(), Registered)?;
            *t.fact.lock().unwrap() = c.optional_object(&s.fact.lock().unwrap(), Registered)?;
            *t.shared.lock().unwrap() = c.optional_object(&s.shared.lock().unwrap(), Shared)?;
            Ok(())
        },
    )
    .unwrap();
    r
}
#[test]
fn erased_cycle_preserves_concrete_types_aliases_and_shared_fields() {
    let a = Arc::new(Node {
        next: Mutex::new(None),
        fact: Mutex::new(Some(Arc::new(String::from("immutable")))),
        shared: Mutex::new(None),
        count: 7,
    });
    *a.next.lock().unwrap() = Some(a.clone());
    *a.shared.lock().unwrap() = Some(a.clone());
    let erased: ObjectGraphObject = a.clone();
    let copied = registry().copy_object(&erased).unwrap();
    let b = copied.root.clone().downcast::<Node>().unwrap();
    assert_eq!(copied.objects.len(), 1);
    assert_eq!(b.count, 7);
    assert!(!Arc::ptr_eq(&a, &b));
    let next = b
        .next
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .downcast::<Node>()
        .unwrap();
    assert!(Arc::ptr_eq(&next, &b));
    assert!(Arc::ptr_eq(&copied.objects.required(&a).unwrap(), &b));
    assert!(Arc::ptr_eq(
        &copied
            .objects
            .required_object(&erased)
            .unwrap()
            .downcast::<Node>()
            .unwrap(),
        &b
    ));
    assert!(Arc::ptr_eq(
        b.shared.lock().unwrap().as_ref().unwrap(),
        &erased
    ));
    assert!(Arc::ptr_eq(
        b.fact.lock().unwrap().as_ref().unwrap(),
        a.fact.lock().unwrap().as_ref().unwrap()
    ));
    let foreign: ObjectGraphObject = Arc::new(Node {
        next: Mutex::new(None),
        fact: Mutex::new(None),
        shared: Mutex::new(None),
        count: 7,
    });
    assert!(matches!(
        copied.objects.required_object(&foreign),
        Err(ObjectGraphCloneError::UndiscoveredReference)
    ));
    a.next.lock().unwrap().take();
    a.shared.lock().unwrap().take();
    b.next.lock().unwrap().take();
    b.shared.lock().unwrap().take();
}
#[test]
fn missing_runtime_factories_and_omitted_registered_edges_fail_without_sharing() {
    use ObjectCloneFieldPolicy::Registered;
    let unknown: ObjectGraphObject = Arc::new(42_i64);
    assert!(
        matches!(registry().copy_object(&unknown),Err(ObjectGraphCloneError::MissingDynamicFactory(id)) if id==std::any::TypeId::of::<i64>())
    );
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<String>(|s| Ok(s.clone()), |_, _| Ok(()), |_, _, _| Ok(()))
        .unwrap();
    r.register::<Node>(
        |s| {
            Ok(Node {
                next: Mutex::new(None),
                fact: Mutex::new(None),
                shared: Mutex::new(None),
                count: s.count,
            })
        },
        |_, _| Ok(()),
        |s, t, c| {
            *t.fact.lock().unwrap() = c.optional_object(&s.fact.lock().unwrap(), Registered)?;
            Ok(())
        },
    )
    .unwrap();
    let source = Arc::new(Node {
        next: Mutex::new(None),
        fact: Mutex::new(Some(Arc::new(String::from("registered")))),
        shared: Mutex::new(None),
        count: 0,
    });
    assert!(matches!(
        r.copy(&source),
        Err(ObjectGraphCloneError::UndiscoveredReference)
    ));
}
#[test]
fn mixed_runtime_types_use_their_own_factories_and_optional_nulls_stay_null() {
    use ObjectCloneFieldPolicy::Required;
    struct Root(Vec<ObjectGraphObject>, Mutex<Vec<ObjectGraphObject>>);
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<String>(|s| Ok(format!("{s}!")), |_, _| Ok(()), |_, _, _| Ok(()))
        .unwrap();
    r.register::<i64>(|s| Ok(s + 1), |_, _| Ok(()), |_, _, _| Ok(()))
        .unwrap();
    r.register::<Root>(
        |s| Ok(Root(s.0.clone(), Mutex::new(vec![]))),
        |s, c| {
            for v in &s.0 {
                c.object(v, Required)?;
            }
            c.optional_object(&None, Required)
        },
        |s, t, c| {
            assert!(c.optional_object(&None, Required)?.is_none());
            *t.1.lock().unwrap() =
                s.0.iter()
                    .map(|v| c.object(v, Required))
                    .collect::<Result<_, _>>()?;
            Ok(())
        },
    )
    .unwrap();
    let text: ObjectGraphObject = Arc::new(String::from("a"));
    let integer: ObjectGraphObject = Arc::new(3_i64);
    let source = Arc::new(Root(vec![text.clone(), integer, text], Mutex::new(vec![])));
    let copied = r.copy(&source).unwrap();
    let out = copied.root.1.lock().unwrap();
    assert_eq!(out[0].downcast_ref::<String>().unwrap(), "a!");
    assert_eq!(*out[1].downcast_ref::<i64>().unwrap(), 4);
    assert!(Arc::ptr_eq(&out[0], &out[2]));
}

#[test]
fn null_deep_fields_require_a_type_policy_and_succeed_after_factory_registration() {
    struct Owner;
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<Owner>(
        |_| Ok(Owner),
        |_, c| c.required_optional_field::<Owner, String>("empty", &None),
        |_, _, _| Ok(()),
    )
    .unwrap();
    let source = Arc::new(Owner);
    assert!(
        matches!(r.copy(&source),Err(ObjectGraphCloneError::RequiredFieldWithoutFactory {owner,field,target}) if owner.is::<Owner>() && field=="empty" && target.is::<String>())
    );
    r.register::<String>(|s| Ok(s.clone()), |_, _| Ok(()), |_, _, _| Ok(()))
        .unwrap();
    assert_eq!(r.copy(&source).unwrap().objects.len(), 1);
}
