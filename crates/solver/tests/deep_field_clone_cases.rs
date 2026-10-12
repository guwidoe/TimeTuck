// Adapted full AbstractSolutionClonerTest.fieldAnnotatedDeepPlanningClone.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/deep_field_clone_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
fn same_optional<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) {
    match (a, b) {
        (None, None) => {}
        (Some(a), Some(b)) => assert!(Arc::ptr_eq(a, b)),
        _ => panic!("null/reference mismatch"),
    }
}
#[test]
fn field_annotated_deep_planning_clone() {
    let original = source();
    let copy = registry()
        .copy_basic(&accessors(), &original, |s| s.score)
        .unwrap();
    let clone = &copy.graph.root;
    assert!(!Arc::ptr_eq(&original, clone));
    assert_eq!(clone.code, "solution");
    assert!(Arc::ptr_eq(&clone.values, &original.values));
    assert_eq!(clone.score, original.score);
    let old = original.entities.lock().unwrap().clone();
    let new = clone.entities.lock().unwrap().clone();
    assert_eq!(new.len(), 4);
    assert!(!Arc::ptr_eq(&old, &new));
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
                    same_optional(a, b);
                }
            }
            _ => panic!("null list changed"),
        }
        match (&*a.map.lock().unwrap(), &*b.map.lock().unwrap()) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert!(!Arc::ptr_eq(a, b));
                let a = a.lock().unwrap();
                let b = b.lock().unwrap();
                assert_eq!(a.len(), b.len());
                for (k, v) in a.iter() {
                    same_optional(v, b.get(k).unwrap());
                }
            }
            _ => panic!("null map changed"),
        }
        match (
            &*a.list_to_text.lock().unwrap(),
            &*b.list_to_text.lock().unwrap(),
        ) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert!(!Arc::ptr_eq(a, b));
                let a = a.lock().unwrap();
                let b = b.lock().unwrap();
                assert_eq!(a.len(), b.len());
                for (k, v) in a.iter() {
                    same_optional(v, b.get(k).unwrap());
                }
            }
            _ => panic!("null nested map changed"),
        }
        match (
            &*a.text_to_list.lock().unwrap(),
            &*b.text_to_list.lock().unwrap(),
        ) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert!(!Arc::ptr_eq(a, b));
                let a = a.lock().unwrap();
                let b = b.lock().unwrap();
                assert_eq!(a.len(), b.len());
                for (k, v) in a.iter() {
                    assert_eq!(v, b.get(k).unwrap());
                }
            }
            _ => panic!("null nested map changed"),
        }
    }
    let a = original.general.lock().unwrap().clone().unwrap();
    let b = clone.general.lock().unwrap().clone().unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    let a = a.lock().unwrap();
    let b = b.lock().unwrap();
    assert_eq!(b.len(), 2);
    same_optional(&a[0], &b[0]);
    assert_eq!(a[1], b[1]);
    drop(a);
    drop(b);
    *old[1].value.lock().unwrap() = original.values[1].clone();
    assert_eq!(old[1].value.lock().unwrap().0, "2");
    assert_eq!(new[1].value.lock().unwrap().0, "1");
    old[1]
        .map
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .insert(
            Some(Arc::new("shadow key b1".into())),
            Some(Arc::new("other shadow value b1".into())),
        );
    let key = Some(Arc::new("shadow key b1".into()));
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
}
