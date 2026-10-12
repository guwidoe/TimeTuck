// Complete adapted AbstractSolutionClonerTest final cases, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Hash membership uses immutable owning allocation addresses.
#![allow(clippy::mutable_key_type)]
#[path = "fixtures/complete_clone_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::ObjectIdentity;
fn assert_various(a: &Various, b: &Various) {
    assert_eq!(a.primitives, b.primitives);
    macro_rules! refs {($($name:ident),*)=> {$(assert!(Arc::ptr_eq(&a.refs.$name,&b.refs.$name));)*};}
    refs!(
        boolean,
        byte,
        character,
        short,
        int,
        long,
        float,
        double,
        big_integer,
        big_decimal,
        uuid,
        text,
        record
    );
    let ad = a.deep.lock().unwrap().clone().unwrap();
    let bd = b.deep.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&ad, &bd));
    assert!(Arc::ptr_eq(
        ad.lock().unwrap()[0].as_ref().unwrap(),
        bd.lock().unwrap()[0].as_ref().unwrap()
    ));
    let ae = a.extra.lock().unwrap().clone().unwrap();
    let be = b.extra.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&ae, &be));
    assert!(Arc::ptr_eq(&ae.id, &be.id));
    assert!(Arc::ptr_eq(&a.shallow, &b.shallow));
    let first = a.shallow.lock().unwrap()[0].clone().unwrap();
    let second = b.shallow.lock().unwrap()[0].clone().unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}
#[test]
fn deep_planning_clone() {
    let original = deep_source();
    let copied = registry()
        .copy_basic(&deep_accessors(), &original, |s| s.score)
        .unwrap();
    let clone = &copied.graph.root;
    assert!(!Arc::ptr_eq(&original, clone));
    assert_eq!(clone.code, "solution");
    assert!(Arc::ptr_eq(&original.values, &clone.values));
    assert_eq!(clone.score, original.score);
    let old = original.entities.lock().unwrap().clone();
    let new = clone.entities.lock().unwrap().clone();
    assert!(!Arc::ptr_eq(&old, &new));
    assert_eq!(new.len(), 4);
    for (i, code) in ["a", "b", "c", "d"].into_iter().enumerate() {
        let a = &old[i];
        let b = &new[i];
        assert!(!Arc::ptr_eq(a, b));
        assert_eq!(a.code, code);
        assert_eq!(b.code, code);
        assert!(Arc::ptr_eq(
            &a.value.lock().unwrap(),
            &b.value.lock().unwrap()
        ));
        match (&*a.list.lock().unwrap(), &*b.list.lock().unwrap()) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert!(!Arc::ptr_eq(a, b));
                let a = a.lock().unwrap();
                let b = b.lock().unwrap();
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b.iter()) {
                    assert!(Arc::ptr_eq(a.as_ref().unwrap(), b.as_ref().unwrap()));
                }
            }
            _ => panic!("nullable shadow list changed"),
        }
        match (&*a.map.lock().unwrap(), &*b.map.lock().unwrap()) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert!(!Arc::ptr_eq(a, b));
                let a = a.lock().unwrap();
                let b = b.lock().unwrap();
                assert_eq!(a.len(), b.len());
                for (k, v) in a.iter() {
                    assert!(Arc::ptr_eq(v.as_ref().unwrap(), b[k].as_ref().unwrap()));
                }
            }
            _ => panic!("nullable shadow map changed"),
        }
    }
    let a = original.general.lock().unwrap().clone().unwrap();
    let b = clone.general.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    let a = a.lock().unwrap();
    let b = b.lock().unwrap();
    assert_eq!(b.len(), 2);
    assert!(Arc::ptr_eq(a[0].as_ref().unwrap(), b[0].as_ref().unwrap()));
    assert_eq!(a[1], b[1]);
    drop(a);
    drop(b);
    *old[1].value.lock().unwrap() = original.values[1].clone();
    assert_eq!(old[1].value.lock().unwrap().0, "2");
    assert_eq!(new[1].value.lock().unwrap().0, "1");
    let key = Some(Arc::new(String::from("shadow key b1")));
    old[1]
        .map
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .insert(key.clone(), Some(Arc::new("other shadow value b1".into())));
    assert_eq!(
        old[1].map.lock().unwrap().as_ref().unwrap().lock().unwrap()[&key]
            .as_ref()
            .unwrap()
            .as_str(),
        "other shadow value b1"
    );
    assert_eq!(
        new[1].map.lock().unwrap().as_ref().unwrap().lock().unwrap()[&key]
            .as_ref()
            .unwrap()
            .as_str(),
        "shadow value b1"
    );
    let a = &old[0];
    let b = &new[0];
    assert!(Arc::ptr_eq(
        a.copied.lock().unwrap().as_ref().unwrap(),
        b.copied.lock().unwrap().as_ref().unwrap()
    ));
    assert!(Arc::ptr_eq(
        a.alias.lock().unwrap().as_ref().unwrap(),
        b.alias.lock().unwrap().as_ref().unwrap()
    ));
    let av = a.cloned.lock().unwrap().clone().unwrap();
    let bv = b.cloned.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&av, &bv));
    assert_various(&av, &bv);
    let av = a.annotated.lock().unwrap().clone().unwrap();
    let bv = b.annotated.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&av, &bv));
    assert_various(&av.base, &bv.base);
    let av = a.annotated_erased.lock().unwrap().clone().unwrap();
    let bv = b.annotated_erased.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&av, &bv));
    assert_various(
        &av.downcast::<AnnotatedVarious>().unwrap().base,
        &bv.downcast::<AnnotatedVarious>().unwrap().base,
    );
}
#[test]
fn clone_set_based_solution() {
    let original = set_source();
    let copied = registry()
        .copy_basic(&set_accessors(), &original, |s| s.score)
        .unwrap();
    let clone = &copied.graph.root;
    assert!(!Arc::ptr_eq(&original, clone));
    assert!(Arc::ptr_eq(&original.values, &clone.values));
    assert_eq!(clone.score, original.score);
    assert_eq!(clone.code, "solution");
    let old_set = original.entities.lock().unwrap().clone().unwrap();
    let new_set = clone.entities.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&old_set, &new_set));
    let old_set = old_set.lock().unwrap();
    let new_set = new_set.lock().unwrap();
    assert!(Arc::ptr_eq(old_set.comparator(), new_set.comparator()));
    assert_eq!(new_set.len(), 4);
    let old: Vec<_> = old_set.iter().cloned().collect();
    let new: Vec<_> = new_set.iter().cloned().collect();
    for (i, (code, value)) in [("d", "3"), ("c", "3"), ("b", "1"), ("a", "1")]
        .into_iter()
        .enumerate()
    {
        assert!(!Arc::ptr_eq(&old[i], &new[i]));
        assert_eq!(old[i].code, code);
        assert_eq!(new[i].code, code);
        assert_eq!(new[i].value.lock().unwrap().0, value);
    }
    let old = original.hash.lock().unwrap().clone().unwrap();
    let new_hash = clone.hash.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&old, &new_hash));
    let hash = new_hash.lock().unwrap();
    assert_eq!(hash.len(), 4);
    for e in &new {
        assert!(hash.contains(&ObjectIdentity::new(e.clone())));
    }
    drop(hash);
    let old = original.linked.lock().unwrap().clone().unwrap();
    let new_link = clone.linked.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&old, &new_link));
    let linked = new_link.lock().unwrap();
    assert_eq!(linked.len(), 4);
    for (a, b) in linked.iter().zip(new.iter()) {
        assert!(Arc::ptr_eq(a, b));
    }
    drop(linked);
    let old = original.tree.lock().unwrap().clone().unwrap();
    let new_tree = clone.tree.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&old, &new_tree));
    let tree = new_tree.lock().unwrap();
    assert_eq!(tree.len(), 4);
    for (a, b) in tree.iter().zip(new.iter()) {
        assert!(Arc::ptr_eq(a, b));
    }
    drop(tree);
    let val2 = original.values.iter().find(|v| v.0 == "2").unwrap().clone();
    let b = old_set.iter().find(|e| e.code == "b").unwrap();
    *b.value.lock().unwrap() = val2;
    assert_eq!(b.value.lock().unwrap().0, "2");
    assert_eq!(new[2].value.lock().unwrap().0, "1");
}
