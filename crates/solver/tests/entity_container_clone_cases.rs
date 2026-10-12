// Complete adapted AbstractSolutionClonerTest collection/array cases, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// These keys hash owning allocation identity, independent of mutable payloads.
#![allow(clippy::mutable_key_type)]
#[path = "fixtures/entity_container_clone_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::ObjectIdentity;
fn entities(old: &Root, new: &Root) -> (Vec<Arc<Entity>>, Vec<Arc<Entity>>) {
    assert_eq!(new.code, "solution");
    assert!(Arc::ptr_eq(&old.values, &new.values));
    assert_eq!(new.score, old.score);
    let a = old.objects();
    let b = new.objects();
    assert_eq!(b.len(), 3);
    for (i, (code, value)) in [("a", "1"), ("b", "1"), ("c", "3")].into_iter().enumerate() {
        assert!(!Arc::ptr_eq(&a[i], &b[i]));
        assert_eq!(a[i].code, code);
        assert_eq!(b[i].code, code);
        assert_eq!(b[i].value.lock().unwrap().0, value);
    }
    (a, b)
}
fn check_refs(actual: &[Arc<Entity>], expected: &[&Arc<Entity>]) {
    assert_eq!(actual.len(), expected.len());
    for (a, b) in actual.iter().zip(expected) {
        assert!(Arc::ptr_eq(a, b));
    }
}
#[test]
fn clone_entity_collection_property_solution() {
    let original = source(true);
    let copy = registry()
        .copy_basic(&accessors(), &original, |s| s.score)
        .unwrap();
    let clone = &copy.graph.root;
    assert!(!Arc::ptr_eq(&original, clone));
    assert!(!Arc::ptr_eq(
        original.entities.lock().unwrap().as_ref().unwrap(),
        clone.entities.lock().unwrap().as_ref().unwrap()
    ));
    let (old, new) = entities(&original, clone);
    for (i, refs) in [vec![&new[1], &new[2]], vec![], vec![&new[0], &new[2]]]
        .into_iter()
        .enumerate()
    {
        let a = old[i].list.lock().unwrap().clone().unwrap();
        let b = new[i].list.lock().unwrap().clone().unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
        check_refs(&b.lock().unwrap(), &refs);
        let a = old[i].set.lock().unwrap().clone().unwrap();
        let b = new[i].set.lock().unwrap().clone().unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
        let set = b.lock().unwrap();
        assert_eq!(set.len(), refs.len());
        for e in &refs {
            assert!(set.contains(&ObjectIdentity::new((*e).clone())));
        }
        drop(set);
        let a = old[i].text_map.lock().unwrap().clone().unwrap();
        let b = new[i].text_map.lock().unwrap().clone().unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
        let map = b.lock().unwrap();
        assert_eq!(map.len(), refs.len());
        for e in refs {
            assert!(Arc::ptr_eq(
                map[&Some(Arc::new(e.code.into()))].as_ref().unwrap(),
                e
            ));
        }
    }
    let a = old[0].entity_map.lock().unwrap().clone().unwrap();
    let b = new[0].entity_map.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    let map = b.lock().unwrap();
    assert_eq!(map.len(), 2);
    for i in [1, 2] {
        assert_eq!(
            map[&ObjectIdentity::new(new[i].clone())].as_str(),
            new[i].code
        );
    }
    drop(map);
    let a = old[0].list_map.lock().unwrap().clone().unwrap();
    let b = new[0].list_map.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    let map = b.lock().unwrap();
    assert_eq!(map.len(), 1);
    check_refs(
        &map[&Some(Arc::new("bc".into()))]
            .as_ref()
            .unwrap()
            .lock()
            .unwrap(),
        &[&new[1], &new[2]],
    );
    drop(map);
    for i in [1, 2] {
        assert!(new[i].entity_map.lock().unwrap().is_none());
        assert!(new[i].list_map.lock().unwrap().is_none());
    }
    original.detach();
    clone.detach();
}
#[test]
fn clone_entity_array_property_solution() {
    let original = source(false);
    let copy = registry()
        .copy_basic(&accessors(), &original, |s| s.score)
        .unwrap();
    let clone = &copy.graph.root;
    assert!(!Arc::ptr_eq(&original, clone));
    assert!(!Arc::ptr_eq(
        original.arrays.lock().unwrap().as_ref().unwrap(),
        clone.arrays.lock().unwrap().as_ref().unwrap()
    ));
    let (old, new) = entities(&original, clone);
    for (i, refs) in [vec![&new[1], &new[2]], vec![], vec![&new[0], &new[2]]]
        .into_iter()
        .enumerate()
    {
        let a = old[i].array.lock().unwrap().clone().unwrap();
        let b = new[i].array.lock().unwrap().clone().unwrap();
        assert!(!Arc::ptr_eq(&a, &b));
        check_refs(&b.lock().unwrap(), &refs);
    }
    original.detach();
    clone.detach();
}
