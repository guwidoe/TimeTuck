// Original iterative/cyclic clone registration, alias and failure controls.
#[path = "fixtures/recursive_clone_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    cell::Cell,
    sync::{Arc, Weak},
};
use timetuck::*;
#[test]
fn repeated_references_and_weak_self_cycles_allocate_once_and_share_only_facts() {
    let source = source();
    let first = source.entities.lock().unwrap()[0].clone();
    source.entities.lock().unwrap().push(first.clone());
    let copy = registry()
        .copy_basic(&accessors(), &source, |s| s.score)
        .unwrap();
    assert_eq!(copy.graph.objects.len(), 3);
    let entities = copy.graph.root.entities.lock().unwrap();
    assert!(Arc::ptr_eq(&entities[0], &entities[2]));
    assert!(Arc::ptr_eq(&entities[0].value, &source.values[0]));
    assert!(Arc::ptr_eq(
        &entities[0].root.lock().unwrap().upgrade().unwrap(),
        &copy.graph.root
    ));
    let foreign = Arc::new(Entity {
        code: first.code.clone(),
        value: first.value.clone(),
        root: std::sync::Mutex::new(Weak::new()),
    });
    assert!(matches!(
        copy.graph.objects.required(&foreign),
        Err(ObjectGraphCloneError::UndiscoveredReference)
    ));
    assert!(Arc::ptr_eq(
        &first.root.lock().unwrap().upgrade().unwrap(),
        &source
    ));
}
#[test]
fn missing_and_duplicate_factories_fail_without_publishing_or_replacing_rules() {
    let source = source();
    let mut missing = ObjectGraphCloneRegistry::new();
    assert!(matches!(
        missing.copy(&source),
        Err(ObjectGraphCloneError::MissingFactory(_))
    ));
    missing
        .register::<Root>(
            |s| {
                Ok(Root {
                    code: s.code.clone(),
                    values: s.values.clone(),
                    entities: std::sync::Mutex::new(vec![]),
                    score: s.score,
                })
            },
            |s, c| {
                for e in s.entities.lock().unwrap().iter() {
                    c.required(e)?;
                }
                Ok(())
            },
            |_, _, _| Ok(()),
        )
        .unwrap();
    assert!(matches!(
        missing.copy(&source),
        Err(ObjectGraphCloneError::MissingFactory(_))
    ));
    assert_eq!(source.entities.lock().unwrap().len(), 2);
    let mut full = registry();
    assert!(matches!(
        full.register::<Root>(
            |_| panic!("must not replace"),
            |_, _| Ok(()),
            |_, _, _| Ok(())
        ),
        Err(ObjectGraphCloneError::DuplicateFactory(_))
    ));
    assert!(full.copy(&source).is_ok());
}
#[test]
fn undiscovered_populate_edges_and_callback_failure_publish_no_graph_and_can_retry() {
    let source = source();
    let fail = Cell::new(true);
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<Root>(
        |s| {
            Ok(Root {
                code: s.code.clone(),
                values: s.values.clone(),
                entities: std::sync::Mutex::new(vec![]),
                score: s.score,
            })
        },
        |s, c| {
            for e in s.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            Ok(())
        },
        |s, t, c| {
            if fail.replace(false) {
                return Err(ObjectGraphCloneError::Callback("fill failure".into()));
            }
            *t.entities.lock().unwrap() = c.collection(&s.entities.lock().unwrap())?;
            Ok(())
        },
    )
    .unwrap();
    r.register::<Entity>(
        |s| {
            Ok(Entity {
                code: s.code.clone(),
                value: s.value.clone(),
                root: std::sync::Mutex::new(Weak::new()),
            })
        },
        |_, _| Ok(()),
        |s, t, c| {
            *t.root.lock().unwrap() = c.weak(&s.root.lock().unwrap())?;
            Ok(())
        },
    )
    .unwrap();
    assert!(matches!(
        r.copy(&source),
        Err(ObjectGraphCloneError::Callback(_))
    ));
    let copied = r.copy(&source).unwrap();
    assert_eq!(copied.root.entities.lock().unwrap().len(), 2);
    let mut bad = ObjectGraphCloneRegistry::new();
    bad.register::<Root>(
        |s| {
            Ok(Root {
                code: s.code.clone(),
                values: s.values.clone(),
                entities: std::sync::Mutex::new(vec![]),
                score: s.score,
            })
        },
        |_, _| Ok(()),
        |s, t, c| {
            *t.entities.lock().unwrap() = c.collection(&s.entities.lock().unwrap())?;
            Ok(())
        },
    )
    .unwrap();
    assert!(matches!(
        bad.copy(&source),
        Err(ObjectGraphCloneError::UndiscoveredReference)
    ));
    assert_eq!(source.entities.lock().unwrap().len(), 2);
}
