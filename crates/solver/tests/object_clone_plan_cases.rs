// Complete adapted pinned Community TEST cases, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/clone_class_plan_fixture.rs"]
mod classes;
#[path = "fixtures/clone_root_contract_fixture.rs"]
mod roots;
use std::{cell::Cell, sync::Arc};
use timetuck::*;
#[test]
fn get_deep_cloned_classes() {
    use classes::*;
    let p = builder()
        .compile::<TestdataFieldAnnotatedDeepCloningSolution>(
            vec![PlanningType::of::<TestdataDeepCloningEntity>()],
            vec![],
        )
        .unwrap();
    let mut expected = vec![
        PlanningType::of::<TestdataDeepCloningEntity>(),
        PlanningType::of::<ExtraDeepClonedObject>(),
        PlanningType::of::<TestdataFieldAnnotatedDeepCloningEntity>(),
        PlanningType::of::<AnnotatedTestdataVariousTypes>(),
        PlanningType::of::<TestdataFieldAnnotatedDeepCloningSolution>(),
        PlanningType::of::<TestdataVariousTypes>(),
    ];
    expected.sort_by_key(|t| t.name());
    assert_eq!(p.deep_cloned_classes(), expected);
}
#[test]
fn clone_extended_solution() {
    use roots::*;
    let calls = Cell::new(0);
    let plan = builder(&calls).compile::<Base>(vec![], vec![]).unwrap();
    let original = extended::source::<extended::AnnotatedBase>();
    let entities = original.base.entities.lock().unwrap().clone();
    assert_eq!(entities.len(), 4);
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
    let e = match plan.copy(&original) {
        Err(e) => e,
        Ok(_) => panic!("unregistered extended root copied"),
    };
    assert!(matches!(e, ObjectClonePlanError::UnsupportedRoot { .. }));
    let message = e.to_string();
    assert!(message.contains(&format!(
        "Failed to create clone: encountered ({})",
        PlanningType::of::<Extended>().name()
    )));
    assert!(message.contains(&format!(
        "which is not a known subclass of the solution class ({}).",
        PlanningType::of::<Base>().name()
    )));
    assert!(message.contains(&format!(
        "The known subclasses are: [{}]",
        PlanningType::of::<Base>().name()
    )));
    assert!(message.contains("Maybe register this root type explicitly?"));
    assert_eq!(calls.get(), 0);
    clear_cycle(&original);
}
