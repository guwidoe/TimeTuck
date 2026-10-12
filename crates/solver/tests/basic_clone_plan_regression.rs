// Original allocation-map, registration and failure-isolation checks.
#[path = "fixtures/basic_clone_plan_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, sync::Arc};
use timetuck::*;
#[test]
fn aliases_allocate_once_across_collections_and_maps_retain_origin_identity() {
    let mut source = source::<PropertyEntity>();
    let a = source.entities[0].clone();
    let b = source.entities[1].clone();
    source.entities = Arc::new(vec![a.clone(), b.clone(), a.clone()]);
    source.other = Arc::new(vec![b.clone(), a.clone()]);
    source.score = Some(SimpleScore::new(7));
    let calls = Cell::new(0);
    let mut p = BasicSolutionClonePlan::new(
        |s: &Root<PropertyEntity>, m: &BasicEntityClones| {
            Ok(Root {
                code: s.code,
                values: s.values.clone(),
                entities: Arc::new(m.collection(&s.entities)?),
                other: Arc::new(m.collection(&s.other)?),
                score: s.score,
            })
        },
        |s| s.score,
    );
    p.entity::<PropertyEntity>(|e, f| {
        calls.set(calls.get() + 1);
        Ok(PropertyEntity::new(e.code(), f[0].1.clone()))
    })
    .unwrap();
    let copied = p.copy(&accessors::<PropertyEntity>(true), &source).unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(copied.entities.len(), 2);
    assert_eq!(copied.solution.score, Some(SimpleScore::new(7)));
    assert!(Arc::ptr_eq(
        &copied.solution.entities[0],
        &copied.solution.entities[2]
    ));
    assert!(Arc::ptr_eq(
        &copied.solution.entities[1],
        &copied.solution.other[0]
    ));
    let foreign = Arc::new(PropertyEntity::new("a", a.value()));
    assert!(matches!(
        copied.entities.entity(&foreign),
        Err(BasicSolutionCloneError::UnknownCloneEntity)
    ));
    let weak = Arc::downgrade(&a);
    drop(a);
    drop(b);
    drop(source);
    assert!(weak.upgrade().is_some());
    drop(copied.entities);
    assert!(weak.upgrade().is_none());
}
#[test]
fn missing_and_duplicate_factories_are_atomic_and_root_bypass_is_rejected() {
    let source = source::<PropertyEntity>();
    let a = accessors::<PropertyEntity>(false);
    let roots = Cell::new(0);
    let mut p = BasicSolutionClonePlan::new(
        |s: &Root<PropertyEntity>, m: &BasicEntityClones| {
            roots.set(roots.get() + 1);
            Ok(Root {
                code: s.code,
                values: s.values.clone(),
                entities: Arc::new(m.collection(&s.entities)?),
                other: Arc::new(vec![]),
                score: s.score,
            })
        },
        |s| s.score,
    );
    assert!(matches!(
        p.copy(&a, &source),
        Err(BasicSolutionCloneError::MissingCloneFactory(_))
    ));
    assert_eq!(roots.get(), 0);
    p.entity::<PropertyEntity>(|e, f| Ok(PropertyEntity::new(e.code(), f[0].1.clone())))
        .unwrap();
    assert!(matches!(
        p.entity::<PropertyEntity>(|_, _| panic!("duplicate must not replace")),
        Err(BasicSolutionCloneError::DuplicateCloneFactory(_))
    ));
    assert!(p.copy(&a, &source).is_ok());
    let mut bypass = BasicSolutionClonePlan::new(
        |s: &Root<PropertyEntity>, _: &BasicEntityClones| {
            Ok(Root {
                code: s.code,
                values: s.values.clone(),
                entities: Arc::new(
                    s.entities
                        .iter()
                        .map(|e| Arc::new(PropertyEntity::new(e.code(), e.value())))
                        .collect(),
                ),
                other: Arc::new(vec![]),
                score: s.score,
            })
        },
        |s| s.score,
    );
    bypass
        .entity::<PropertyEntity>(|e, f| Ok(PropertyEntity::new(e.code(), f[0].1.clone())))
        .unwrap();
    assert!(matches!(
        bypass.copy(&a, &source),
        Err(BasicSolutionCloneError::UnknownCloneEntity)
    ));
}
#[test]
fn factory_panics_publish_nothing_and_allow_retry_without_mutating_input() {
    let source = source::<PropertyEntity>();
    let calls = Cell::new(0);
    let mut p = BasicSolutionClonePlan::new(
        |s: &Root<PropertyEntity>, m: &BasicEntityClones| {
            Ok(Root {
                code: s.code,
                values: s.values.clone(),
                entities: Arc::new(m.collection(&s.entities)?),
                other: Arc::new(vec![]),
                score: s.score,
            })
        },
        |s| s.score,
    );
    p.entity::<PropertyEntity>(|e, f| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            panic!("factory failure");
        }
        Ok(PropertyEntity::new(e.code(), f[0].1.clone()))
    })
    .unwrap();
    let a = accessors::<PropertyEntity>(false);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| p.copy(&a, &source))).is_err()
    );
    let copied = p.copy(&a, &source).unwrap();
    assert_eq!(calls.get(), 5);
    assert_eq!(copied.solution.entities.len(), 4);
    assert_eq!(source.entities[1].value().unwrap().0, "1");
    assert_eq!(source.score, None);
}
