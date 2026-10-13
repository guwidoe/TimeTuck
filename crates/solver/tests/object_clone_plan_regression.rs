//! Original executable dependency closure, ownership, aliases and validation.
#[path = "fixtures/clone_class_plan_fixture.rs"]
mod classes;
#[path = "fixtures/clone_root_contract_fixture.rs"]
mod roots;
use std::{
    cell::Cell,
    sync::{Arc, Mutex},
};
use timetuck::*;
#[test]
fn all_six_factories_execute_with_owned_aliases_and_shared_fields() {
    use classes::*;
    for count in [0, 1, 5] {
        let extra = Arc::new(ExtraDeepClonedObject {
            id: format!("extra-{count}"),
        });
        let various = Arc::new(TestdataVariousTypes {
            extra: Mutex::new(Some(extra.clone())),
        });
        let annotated = Arc::new(AnnotatedTestdataVariousTypes {
            base: Mutex::new(Some(various.clone())),
        });
        let deep = Arc::new(TestdataDeepCloningEntity {
            owned: Mutex::new(Some(various.clone())),
            shared: Mutex::new(Some(various.clone())),
            annotated: Mutex::new(Some(annotated.clone())),
        });
        let field = Arc::new(TestdataFieldAnnotatedDeepCloningEntity {
            shadow: Mutex::new((0..count).map(|i| format!("shadow-{i}")).collect()),
        });
        let root = Arc::new(TestdataFieldAnnotatedDeepCloningSolution {
            entities: Mutex::new(vec![field.clone(), field.clone()]),
        });
        let p = builder()
            .compile::<TestdataFieldAnnotatedDeepCloningSolution>(
                vec![PlanningType::of::<TestdataDeepCloningEntity>()],
                vec![
                    PlanningType::of::<TestdataDeepCloningEntity>(),
                    PlanningType::of::<TestdataVariousTypes>(),
                    PlanningType::of::<AnnotatedTestdataVariousTypes>(),
                    PlanningType::of::<ExtraDeepClonedObject>(),
                    PlanningType::of::<TestdataFieldAnnotatedDeepCloningEntity>(),
                ],
            )
            .unwrap();
        let c = p.copy(&deep).unwrap();
        assert_eq!(c.objects.len(), 4);
        let owned = c.root.owned.lock().unwrap().clone().unwrap();
        let a = c.root.annotated.lock().unwrap().clone().unwrap();
        assert!(!Arc::ptr_eq(&c.root, &deep));
        assert!(!Arc::ptr_eq(&owned, &various));
        assert!(Arc::ptr_eq(
            &owned,
            a.base.lock().unwrap().as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(
            c.root.shared.lock().unwrap().as_ref().unwrap(),
            &various
        ));
        let copied_extra = owned.extra.lock().unwrap().clone().unwrap();
        assert!(!Arc::ptr_eq(&copied_extra, &extra));
        assert_eq!(copied_extra.id, extra.id);
        let c = p.copy(&root).unwrap();
        let entities = c.root.entities.lock().unwrap();
        assert!(Arc::ptr_eq(&entities[0], &entities[1]));
        assert!(!Arc::ptr_eq(&entities[0], &field));
        entities[0].shadow.lock().unwrap().push("changed".into());
        assert_eq!(field.shadow.lock().unwrap().len(), count);
        assert!(!Arc::ptr_eq(&p.copy(&various).unwrap().root, &various));
        assert!(!Arc::ptr_eq(&p.copy(&annotated).unwrap().root, &annotated));
        assert!(!Arc::ptr_eq(&p.copy(&extra).unwrap().root, &extra));
        assert!(!Arc::ptr_eq(&p.copy(&field).unwrap().root, &field));
    }
}
#[test]
fn allowed_root_copies_cycle_fact_sharing_and_independent_allocations() {
    use roots::*;
    let calls = Cell::new(0);
    let p = builder(&calls)
        .compile::<Base>(
            vec![],
            vec![
                PlanningType::of::<Extended>(),
                PlanningType::of::<Extended>(),
            ],
        )
        .unwrap();
    assert_eq!(
        p.known_roots(),
        [PlanningType::of::<Base>(), PlanningType::of::<Extended>()]
    );
    let source = extended::source::<extended::AnnotatedBase>();
    let c = p.copy(&source).unwrap();
    assert!(Arc::ptr_eq(&source.base.values, &c.root.base.values));
    assert!(!Arc::ptr_eq(&source, &c.root));
    assert_eq!(calls.get(), 5);
    let entities = c.root.base.entities.lock().unwrap().clone();
    let originals = source.base.entities.lock().unwrap().clone();
    for i in 0..4 {
        assert!(!Arc::ptr_eq(&entities[i], &originals[i]));
    }
    for (i, j) in [(2, 3), (3, 2)] {
        let next = entities[i]
            .extra
            .lock()
            .unwrap()
            .clone()
            .unwrap()
            .downcast::<ChildEntity>()
            .unwrap();
        assert!(Arc::ptr_eq(&next, &entities[j]));
    }
    assert!(Arc::ptr_eq(
        entities[1].extra.lock().unwrap().as_ref().unwrap(),
        originals[1].extra.lock().unwrap().as_ref().unwrap()
    ));
    // Base-only runtime path uses the same executable child factories.
    let base = Arc::new(Base {
        code: source.base.code,
        values: source.base.values.clone(),
        entities: Mutex::new(originals),
        score: None,
    });
    let copied_base = p.copy(&base).unwrap();
    assert_eq!(copied_base.objects.len(), 5);
    let base_entities = copied_base.root.entities.lock().unwrap();
    *base_entities[2].extra.lock().unwrap() = None;
    *base_entities[3].extra.lock().unwrap() = None;
    clear_cycle(&source);
    clear_cycle(&c.root);
}
#[test]
fn missing_dependency_and_unknown_registered_root_never_allocate() {
    struct Root;
    struct Missing;
    let calls = Cell::new(0);
    let mut b = ObjectClonePlanBuilder::new();
    b.factory::<Root>(
        vec![PlanningType::of::<Missing>()],
        |_| {
            calls.set(calls.get() + 1);
            Ok(Root)
        },
        |_, _| Ok(()),
        |_, _, _| Ok(()),
    )
    .unwrap();
    assert!(
        matches!(b.compile::<Root>(vec![],vec![]),Err(ObjectClonePlanError::MissingDependency(t)) if t==PlanningType::of::<Missing>())
    );
    assert_eq!(calls.get(), 0);
    let p = roots::builder(&calls)
        .compile::<roots::Base>(vec![], vec![])
        .unwrap();
    let source = roots::extended::source::<roots::extended::AnnotatedBase>();
    assert!(matches!(
        p.copy(&source),
        Err(ObjectClonePlanError::UnsupportedRoot { .. })
    ));
    assert_eq!(calls.get(), 0);
    roots::clear_cycle(&source);
}
