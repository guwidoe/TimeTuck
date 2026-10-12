// Complete adapted AbstractSolutionClonerTest cases, Apache-2.0; see provenance/notices.
#[path = "fixtures/extended_clone_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
type EntityList<B> = Arc<Vec<Arc<Child<B>>>>;
fn check_base<B: BaseData>(old: &BaseRoot<B>, new: &BaseRoot<B>) -> (EntityList<B>, EntityList<B>) {
    assert_eq!(new.code, "solution");
    assert!(Arc::ptr_eq(&old.values, &new.values));
    assert_eq!(new.score, old.score);
    let a = old.entities.lock().unwrap().clone();
    let b = new.entities.lock().unwrap().clone();
    assert_eq!(b.len(), 4);
    assert!(!Arc::ptr_eq(&a, &b));
    for (i, (code, value)) in [("a", "1"), ("b", "1"), ("c", "3"), ("d", "3")]
        .into_iter()
        .enumerate()
    {
        assert!(!Arc::ptr_eq(&a[i], &b[i]));
        assert_eq!(a[i].base.code(), code);
        assert_eq!(b[i].base.code(), code);
        assert_eq!(b[i].base.value().0, value);
    }
    a[1].base.set_value(old.values[1].clone());
    assert_eq!(a[1].base.value().0, "2");
    assert_eq!(b[1].base.value().0, "1");
    (a, b)
}
fn extended<B: BaseData>() {
    let original = source::<B>();
    let copy = registry::<B>()
        .copy_basic(&accessors::<B>(), &original, |s| s.base.score)
        .unwrap();
    let clone = &copy.graph.root;
    assert!(!Arc::ptr_eq(&original, clone));
    assert_eq!(
        clone
            .extra
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .downcast_ref::<String>()
            .unwrap(),
        "extraObjectOnSolution"
    );
    let (old, new) = check_base(&original.base, &clone.base);
    assert!(new[0].extra.lock().unwrap().is_none());
    assert_eq!(
        new[1]
            .extra
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .downcast_ref::<String>()
            .unwrap(),
        "extraObjectOnEntity"
    );
    for (i, j) in [(2, 3), (3, 2)] {
        let extra = new[i]
            .extra
            .lock()
            .unwrap()
            .clone()
            .unwrap()
            .downcast::<Child<B>>()
            .unwrap();
        assert!(Arc::ptr_eq(&extra, &new[j]));
    }
    // Native strong cycles are detached only after all original assertions.
    for node in old.iter().chain(new.iter()) {
        node.extra.lock().unwrap().take();
    }
}
#[test]
fn clone_extended_solution() {
    extended::<AnnotatedBase>();
}
#[test]
fn clone_extended_third_party_solution() {
    extended::<ThirdPartyPojo>();
}
#[test]
fn clone_access_modifier_solution() {
    let static_object = Arc::new(());
    set_static(static_object.clone());
    let mut original = AccessRoot::new();
    original.set_write_only("writeHello");
    let original = Arc::new(original);
    let copied = access_registry()
        .copy_basic(&access_accessors(), &original, |s| s.base.score)
        .unwrap();
    let clone = &copied.graph.root;
    assert!(std::ptr::eq(
        STATIC_FINAL.as_ptr(),
        "staticFinalFieldValue".as_ptr()
    ));
    assert!(Arc::ptr_eq(&get_static(), &static_object));
    assert!(!Arc::ptr_eq(&original, clone));
    assert_eq!(clone.final_field(), original.final_field());
    assert_eq!(clone.read_only(), "readHello");
    check_base(&original.base, &clone.base);
}

#[test]
fn fail_deep_clone_required_type_annotation() {
    let source = invalid_source();
    let err = match invalid_registry().copy(&source) {
        Ok(_) => panic!("must reject deep field with shared-only type"),
        Err(e) => e,
    };
    let message = err.to_string();
    let owner = std::any::type_name::<InvalidEntityProvidingEntity>();
    let target = std::any::type_name::<Value>();
    assert!(message.contains(&format!(
        "The field (value) of type ({owner}) is configured to be deep-cloned"
    )));
    assert!(message.contains(&format!("but its type ({target}) is not deep-cloned")));
    assert!(message.contains("Maybe use Shared for the field?"));
    assert!(message.contains(&format!(
        "Maybe register a clone factory for the type ({target})?"
    )));
}
