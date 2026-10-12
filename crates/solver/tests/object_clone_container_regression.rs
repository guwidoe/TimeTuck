// Original allocation identity and container isolation controls.
// Identity keys hash only the owning Arc address, independently of payload mutation.
#![allow(clippy::mutable_key_type)]
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, Weak},
};
use timetuck::*;
#[test]
fn identity_keys_retain_ownership_and_survive_payload_mutation() {
    let a = Arc::new(Mutex::new(7));
    let b = Arc::new(Mutex::new(7));
    let weak: Weak<Mutex<i32>> = Arc::downgrade(&a);
    let key = ObjectIdentity::new(a.clone());
    let mut map = HashMap::from([(key.clone(), "a"), (ObjectIdentity::new(b), "b")]);
    assert_eq!(map.len(), 2);
    *a.lock().unwrap() = 99;
    assert_eq!(map[&key], "a");
    drop(a);
    assert!(weak.upgrade().is_some());
    map.remove(&key);
    drop(key);
    assert!(weak.upgrade().is_none());
}
#[test]
fn identity_sets_and_maps_rebase_equal_payloads_without_collapsing_keys() {
    use ObjectCloneFieldPolicy::{Required, Shared};
    let a = Arc::new(Mutex::new(7));
    let b = Arc::new(Mutex::new(7));
    let text = Arc::new(String::from("same value"));
    let set = Arc::new(Mutex::new(HashSet::from([
        ObjectIdentity::new(a.clone()),
        ObjectIdentity::new(b.clone()),
    ])));
    let map = Arc::new(Mutex::new(HashMap::from([
        (ObjectIdentity::new(a.clone()), text.clone()),
        (ObjectIdentity::new(b.clone()), text.clone()),
    ])));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<Mutex<i32>>(
        |s| Ok(Mutex::new(*s.lock().unwrap())),
        |_, _| Ok(()),
        |_, _, _| Ok(()),
    )
    .unwrap();
    r.register_identity_set::<Mutex<i32>>(Required).unwrap();
    r.register_identity_map::<Mutex<i32>, String>(Required, Shared)
        .unwrap();
    let copied = r.copy(&set).unwrap();
    let out = copied.root.lock().unwrap();
    assert_eq!(out.len(), 2);
    assert!(!out.contains(&ObjectIdentity::new(a.clone())));
    assert!(!out.contains(&ObjectIdentity::new(b.clone())));
    for source in [&a, &b] {
        assert!(out.contains(&ObjectIdentity::new(
            copied.objects.required(source).unwrap()
        )));
    }
    drop(out);
    let copied = r.copy(&map).unwrap();
    let out = copied.root.lock().unwrap();
    assert_eq!(out.len(), 2);
    for source in [&a, &b] {
        let target = copied.objects.required(source).unwrap();
        assert!(!Arc::ptr_eq(source, &target));
        assert_eq!(*target.lock().unwrap(), 7);
        assert!(Arc::ptr_eq(&out[&ObjectIdentity::new(target)], &text));
    }
    drop(out);
    *a.lock().unwrap() = 8;
    map.lock().unwrap().clear();
    assert_eq!(copied.root.lock().unwrap().len(), 2);
    assert_eq!(*copied.objects.required(&a).unwrap().lock().unwrap(), 7);
}
#[test]
fn slices_and_lists_preserve_aliases_order_and_field_policy() {
    use ObjectCloneFieldPolicy::{Required, Shared};
    let a = Arc::new(Mutex::new(1));
    let b = Arc::new(Mutex::new(2));
    let source = Arc::new(Mutex::new(
        vec![b.clone(), a.clone(), b.clone()].into_boxed_slice(),
    ));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<Mutex<i32>>(
        |s| Ok(Mutex::new(*s.lock().unwrap())),
        |_, _| Ok(()),
        |_, _, _| Ok(()),
    )
    .unwrap();
    r.register_slice::<Mutex<i32>>(Required).unwrap();
    r.register_list::<Mutex<i32>>(Shared).unwrap();
    let copied = r.copy(&source).unwrap();
    let out = copied.root.lock().unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(*out[0].lock().unwrap(), 2);
    assert_eq!(*out[1].lock().unwrap(), 1);
    assert!(Arc::ptr_eq(&out[0], &out[2]));
    assert!(!Arc::ptr_eq(&out[0], &b));
    drop(out);
    *source.lock().unwrap() = Vec::new().into_boxed_slice();
    assert_eq!(copied.root.lock().unwrap().len(), 3);
    assert!(r.copy(&source).unwrap().root.lock().unwrap().is_empty());
    let source = Arc::new(Mutex::new(vec![a.clone(), a.clone()]));
    let copied = r.copy(&source).unwrap();
    assert_eq!(copied.objects.len(), 1);
    let out = copied.root.lock().unwrap();
    assert!(Arc::ptr_eq(&out[0], &a));
    assert!(Arc::ptr_eq(&out[0], &out[1]));
    drop(out);
    source.lock().unwrap().clear();
    assert_eq!(copied.root.lock().unwrap().len(), 2);
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_slice::<Mutex<i32>>(Required).unwrap();
    let source = Arc::new(Mutex::new(vec![a].into_boxed_slice()));
    assert!(matches!(
        r.copy(&source),
        Err(ObjectGraphCloneError::MissingFactory(_))
    ));
}
