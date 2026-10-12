// Original field-policy and nullable container behavior controls.
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use timetuck::*;
#[derive(Debug)]
struct Leaf(i32);
struct Root {
    source: Arc<Leaf>,
    shared: Mutex<Option<Arc<Leaf>>>,
    deep: Mutex<Option<Arc<Leaf>>>,
}
#[test]
fn explicit_shared_reference_stays_original_even_when_another_field_copies_it() {
    use ObjectCloneFieldPolicy::{Required, Shared};
    let leaf = Arc::new(Leaf(7));
    let source = Arc::new(Root {
        source: leaf.clone(),
        shared: Mutex::new(None),
        deep: Mutex::new(None),
    });
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<Leaf>(|s| Ok(Leaf(s.0)), |_, _| Ok(()), |_, _, _| Ok(()))
        .unwrap();
    r.register::<Root>(
        |s| {
            Ok(Root {
                source: s.source.clone(),
                shared: Mutex::new(None),
                deep: Mutex::new(None),
            })
        },
        |s, c| {
            c.field(&s.source, Shared)?;
            c.field(&s.source, Required)
        },
        |s, t, c| {
            *t.shared.lock().unwrap() = Some(c.field(&s.source, Shared)?);
            *t.deep.lock().unwrap() = Some(c.field(&s.source, Required)?);
            Ok(())
        },
    )
    .unwrap();
    let copy = r.copy(&source).unwrap();
    assert!(Arc::ptr_eq(
        copy.root.shared.lock().unwrap().as_ref().unwrap(),
        &leaf
    ));
    assert!(!Arc::ptr_eq(
        copy.root.deep.lock().unwrap().as_ref().unwrap(),
        &leaf
    ));
    assert_eq!(copy.root.deep.lock().unwrap().as_ref().unwrap().0, 7);
}
#[test]
fn registered_policy_requires_discovery_and_shares_only_unregistered_types() {
    use ObjectCloneFieldPolicy::Registered;
    let source = Arc::new(Root {
        source: Arc::new(Leaf(9)),
        shared: Mutex::new(None),
        deep: Mutex::new(None),
    });
    let make = |registered: bool, discover: bool| {
        let mut r = ObjectGraphCloneRegistry::new();
        if registered {
            r.register::<Leaf>(|s| Ok(Leaf(s.0)), |_, _| Ok(()), |_, _, _| Ok(()))
                .unwrap();
        }
        r.register::<Root>(
            |s| {
                Ok(Root {
                    source: s.source.clone(),
                    shared: Mutex::new(None),
                    deep: Mutex::new(None),
                })
            },
            move |s, c| {
                if discover {
                    c.field(&s.source, Registered)?;
                }
                Ok(())
            },
            |s, t, c| {
                *t.deep.lock().unwrap() = Some(c.field(&s.source, Registered)?);
                Ok(())
            },
        )
        .unwrap();
        r
    };
    assert!(matches!(
        make(true, false).copy(&source),
        Err(ObjectGraphCloneError::UndiscoveredReference)
    ));
    let copy = make(false, true).copy(&source).unwrap();
    assert!(Arc::ptr_eq(
        copy.root.deep.lock().unwrap().as_ref().unwrap(),
        &source.source
    ));
    let copy = make(true, true).copy(&source).unwrap();
    assert!(!Arc::ptr_eq(
        copy.root.deep.lock().unwrap().as_ref().unwrap(),
        &source.source
    ));
}
#[test]
fn nullable_maps_and_sets_reject_rebased_key_collisions_and_lists_preserve_nulls() {
    use ObjectCloneFieldPolicy::{Required, Shared};
    let a = Arc::new("a".to_string());
    let b = Arc::new("b".to_string());
    let source = Arc::new(Mutex::new(BTreeMap::from([
        (None, None),
        (Some(a.clone()), None),
        (Some(b.clone()), Some(a.clone())),
    ])));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<String>(|_| Ok("collision".into()), |_, _| Ok(()), |_, _, _| Ok(()))
        .unwrap();
    r.register_nullable_map::<String, String>(Required, Shared)
        .unwrap();
    assert!(matches!(
        r.copy(&source),
        Err(ObjectGraphCloneError::Callback(_))
    ));
    r.register_set::<String>(Required).unwrap();
    let source = Arc::new(Mutex::new(BTreeSet::from([a.clone(), b])));
    assert!(matches!(
        r.copy(&source),
        Err(ObjectGraphCloneError::Callback(_))
    ));
    let source = Arc::new(Mutex::new(vec![Some(a.clone()), None, Some(a.clone())]));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_nullable_list::<String>(Shared).unwrap();
    let copy = r.copy(&source).unwrap();
    let out = copy.root.lock().unwrap();
    assert_eq!(out.len(), 3);
    assert!(out[1].is_none());
    assert!(Arc::ptr_eq(out[0].as_ref().unwrap(), &a));
    assert!(Arc::ptr_eq(
        out[0].as_ref().unwrap(),
        out[2].as_ref().unwrap()
    ));
    drop(out);
    source.lock().unwrap().clear();
    assert_eq!(copy.root.lock().unwrap().len(), 3);
}
