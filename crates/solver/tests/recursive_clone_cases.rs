// Adapted AbstractSolutionClonerTest cases, Apache-2.0; see provenance/notices.
#[path = "fixtures/recursive_clone_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::{Arc, Mutex};
#[test]
fn supports_entity_to_solution_backlinking() {
    let source = source();
    let copied = registry()
        .copy_basic(&accessors(), &source, |s| s.score)
        .unwrap();
    let cloned = &copied.graph.root;
    assert!(!Arc::ptr_eq(&source, cloned));
    for i in 0..2 {
        let old = source.entities.lock().unwrap()[i].clone();
        let new = cloned.entities.lock().unwrap()[i].clone();
        assert!(!Arc::ptr_eq(&old, &new));
        assert!(Arc::ptr_eq(
            &new.root.lock().unwrap().upgrade().unwrap(),
            cloned
        ));
    }
}
#[test]
fn clone_deeply_nested_solution() {
    // Native stress depth replaces JVM-measured MAX_STACK_FRAMES+128. Iterative
    // discovery/population must handle a depth beyond ordinary recursive traversal.
    const NESTED_COUNT: usize = 100_000;
    let entity = Arc::new(DependencyEntity {
        start: [2000, 1, 1, 0, 0, 0],
        values: Mutex::new(vec![]),
    });
    let root_value = Arc::new(DependencyValue {
        id: "root".into(),
        duration_seconds: 3600,
        dependencies: Mutex::new(None),
    });
    let mut nodes = vec![root_value.clone()];
    for i in 0..NESTED_COUNT {
        let next = Arc::new(DependencyValue {
            id: i.to_string(),
            duration_seconds: 3600,
            dependencies: Mutex::new(None),
        });
        *nodes.last().unwrap().dependencies.lock().unwrap() = Some(vec![next.clone()]);
        nodes.push(next);
    }
    let source = Arc::new(DependencyRoot {
        entities: Mutex::new(vec![entity.clone()]),
        values: Mutex::new(vec![root_value.clone()]),
        score: None,
    });
    let cloned = dependency_registry().copy(&source).unwrap();
    assert!(!Arc::ptr_eq(&source, &cloned.root));
    {
        let a = source.entities.lock().unwrap();
        let b = cloned.root.entities.lock().unwrap();
        assert!(!std::ptr::eq(a.as_ptr(), b.as_ptr()));
        assert_eq!(b.len(), 1);
        assert!(!Arc::ptr_eq(&b[0], &entity));
        assert_eq!(b[0].start, [2000, 1, 1, 0, 0, 0]);
    }
    {
        let a = source.values.lock().unwrap();
        let b = cloned.root.values.lock().unwrap();
        assert!(!std::ptr::eq(a.as_ptr(), b.as_ptr()));
        assert_eq!(b.len(), 1);
        assert!(!Arc::ptr_eq(&b[0], &root_value));
        assert_eq!(b[0].id, root_value.id);
    }
    let mut old = root_value.clone();
    let mut new = cloned.root.values.lock().unwrap()[0].clone();
    for _ in 0..NESTED_COUNT {
        assert!(!Arc::ptr_eq(&new, &root_value));
        assert_eq!(new.id, old.id);
        let a = old.dependencies.lock().unwrap().as_ref().unwrap()[0].clone();
        let b = new.dependencies.lock().unwrap().as_ref().unwrap()[0].clone();
        old = a;
        new = b;
    }
    assert!(!Arc::ptr_eq(&new, &root_value));
    assert_eq!(new.id, old.id);
    // Java uses GC; detach native strong chains iteratively after assertions so
    // ordinary Arc destruction does not introduce a separate recursive drop test.
    for node in nodes {
        cloned
            .objects
            .required(&node)
            .unwrap()
            .dependencies
            .lock()
            .unwrap()
            .take();
        node.dependencies.lock().unwrap().take();
    }
}
