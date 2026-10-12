// Original ordered-set collision, comparator, insertion order and leaf controls.
use std::sync::{Arc, Mutex};
use timetuck::*;
#[test]
fn comparator_equality_duplicate_policy_and_order_survive_copy() {
    let cmp: ObjectComparator<String> = Arc::new(|a, b| b.to_lowercase().cmp(&a.to_lowercase()));
    let a = Arc::new(String::from("A"));
    let b = Arc::new(String::from("b"));
    let mut source = ObjectSortedSet::new(cmp.clone());
    assert!(source.insert(a.clone()));
    assert!(!source.insert(Arc::new("a".into())));
    assert!(source.insert(b.clone()));
    assert!(source.contains(&Arc::new("a".into())));
    let source = Arc::new(Mutex::new(source));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_cloned_leaf::<String>().unwrap();
    r.register_sorted_set::<String>(ObjectCloneFieldPolicy::Required)
        .unwrap();
    let copy = r.copy(&source).unwrap();
    let out = copy.root.lock().unwrap();
    assert!(Arc::ptr_eq(out.comparator(), &cmp));
    assert_eq!(
        out.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        vec!["b", "A"]
    );
    for v in [&a, &b] {
        assert!(!Arc::ptr_eq(v, &copy.objects.required(v).unwrap()));
    }
    drop(out);
    assert!(source.lock().unwrap().remove(&a));
    assert_eq!(copy.root.lock().unwrap().len(), 2);
}
#[test]
fn copied_comparator_key_collisions_fail_instead_of_dropping_members() {
    let cmp: ObjectComparator<String> = Arc::new(|a, b| a.cmp(b));
    let mut set = ObjectSortedSet::new(cmp);
    set.insert(Arc::new("a".into()));
    set.insert(Arc::new("b".into()));
    let source = Arc::new(Mutex::new(set));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<String>(|_| Ok("same".into()), |_, _| Ok(()), |_, _, _| Ok(()))
        .unwrap();
    r.register_sorted_set::<String>(ObjectCloneFieldPolicy::Required)
        .unwrap();
    assert!(matches!(
        r.copy(&source),
        Err(ObjectGraphCloneError::Callback(_))
    ));
    assert_eq!(source.lock().unwrap().len(), 2);
}
#[test]
fn linked_identity_duplicates_removal_reinsertion_and_clones_preserve_order() {
    let a = Arc::new(String::from("same"));
    let b = Arc::new(String::from("same"));
    let mut set = ObjectLinkedIdentitySet::new();
    assert!(set.is_empty());
    assert!(set.insert(a.clone()));
    assert!(set.insert(b.clone()));
    assert!(!set.insert(a.clone()));
    assert!(set.contains(&a));
    assert!(set.remove(&a));
    assert!(!set.remove(&a));
    assert!(set.insert(a.clone()));
    let source = Arc::new(Mutex::new(set));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_cloned_leaf::<String>().unwrap();
    r.register_linked_identity_set::<String>(ObjectCloneFieldPolicy::Required)
        .unwrap();
    let copy = r.copy(&source).unwrap();
    let out = copy.root.lock().unwrap();
    let entries: Vec<_> = out.iter().cloned().collect();
    assert_eq!(entries.len(), 2);
    assert!(Arc::ptr_eq(
        &entries[0],
        &copy.objects.required(&b).unwrap()
    ));
    assert!(Arc::ptr_eq(
        &entries[1],
        &copy.objects.required(&a).unwrap()
    ));
    assert!(!out.contains(&a));
}
#[test]
fn cloned_leaf_keeps_inner_arc_sharing_and_empty_sets_remain_empty() {
    #[derive(Clone)]
    struct Leaf(Arc<String>);
    let inner = Arc::new(String::from("fact"));
    let source = Arc::new(Leaf(inner.clone()));
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_cloned_leaf::<Leaf>().unwrap();
    let copied = r.copy(&source).unwrap();
    assert!(!Arc::ptr_eq(&source, &copied.root));
    assert!(Arc::ptr_eq(&inner, &copied.root.0));
    r.register_sorted_set::<Leaf>(ObjectCloneFieldPolicy::Shared)
        .unwrap();
    let source = Arc::new(Mutex::new(ObjectSortedSet::new(Arc::new(
        |_: &Leaf, _: &Leaf| std::cmp::Ordering::Equal,
    ))));
    assert!(r.copy(&source).unwrap().root.lock().unwrap().is_empty());
    r.register_linked_identity_set::<Leaf>(ObjectCloneFieldPolicy::Shared)
        .unwrap();
    let source = Arc::new(Mutex::new(ObjectLinkedIdentitySet::<Leaf>::new()));
    assert!(r.copy(&source).unwrap().root.lock().unwrap().is_empty());
}
