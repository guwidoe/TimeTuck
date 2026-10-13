// Original graph notification controls; no upstream method credit.
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<usize>("input", false),
        PlanningVariableDefinition::shadow::<usize>("derived"),
    ])])
    .unwrap()
}
#[test]
fn detach_validates_whole_selection_and_refresh_failure_retains_pending_graph() {
    let schema = schema();
    let entity = schema.entity::<Entity>().unwrap();
    let calls = Cell::new(0);
    let fail = Cell::new(false);
    let mut b = ShadowSessionBuilder::<usize, usize>::new(&schema);
    let a = b
        .input(entity.variable("input").unwrap(), 0, |_: &usize| {
            calls.set(calls.get() + 1);
            Some(10usize)
        })
        .unwrap();
    let c = b
        .input(entity.variable("input").unwrap(), 1, |_: &usize| {
            Some(20usize)
        })
        .unwrap();
    let target = b
        .derive(
            entity.variable("derived").unwrap(),
            2,
            |_: &usize, inputs| {
                assert!(!fail.get(), "supplier failed");
                inputs
                    .for_field::<usize>(entity.variable("input").unwrap())
                    .unwrap()[0]
                    .copied()
            },
        )
        .unwrap();
    let sources = [a.erase(), c.erase()];
    b.bind_dependencies(&target, move |world| Ok(vec![sources[*world].clone()]))
        .unwrap();
    let mut s = b.build();
    s.update(&0).unwrap();
    let saved = s.checkpoint().unwrap();
    let original = s.dependency_edges();
    let mut foreign = ShadowSessionBuilder::<usize, usize>::new(&schema);
    let foreign = foreign
        .derive(entity.variable("derived").unwrap(), 2, |_, _| Some(0usize))
        .unwrap();
    assert_eq!(
        s.before_dependencies_changed(&[target.erase(), foreign.erase()]),
        Err(ShadowSessionError::ForeignNode)
    );
    assert_eq!(s.dependency_edges(), original);
    s.assert_current().unwrap();
    assert!(
        s.before_dependencies_changed(&[target.erase(), a.erase()])
            .is_err()
    );
    assert_eq!(s.dependency_edges(), original);
    let removed = s
        .before_dependencies_changed(&[target.erase(), target.erase()])
        .unwrap();
    assert_eq!(removed, original);
    assert_eq!(calls.get(), 1);
    assert!(s.dependency_edges().is_empty());
    assert_eq!(s.read(&target).err(), Some(ShadowSessionError::NotCurrent));
    assert!(s.checkpoint().is_err());
    fail.set(true);
    assert!(
        catch_unwind(AssertUnwindSafe(
            || s.update_notified_with_dependency_events(&1)
        ))
        .is_err()
    );
    assert!(s.dependency_edges().is_empty());
    assert_eq!(s.assert_current(), Err(ShadowSessionError::NotCurrent));
    fail.set(false);
    let update = s.update_notified_with_dependency_events(&1).unwrap();
    assert!(update.removed.is_empty());
    assert_eq!(update.added.len(), 1);
    assert_eq!(update.added[0].source, c.erase());
    assert_eq!(s.read(&target).unwrap().value, Some(&20));
    assert_eq!(calls.get(), 1);
    let idle = s.update_notified_with_dependency_events(&1).unwrap();
    assert!(idle.added.is_empty() && idle.removed.is_empty() && idle.changed.is_empty());
    s.restore(&saved, &0).unwrap();
    assert_eq!(s.dependency_edges(), original);
    assert_eq!(s.read(&target).unwrap().value, Some(&10));
}
#[test]
fn detaching_a_cycle_invalidates_all_cycle_members_until_recovery() {
    let schema = schema();
    let field = schema
        .entity::<Entity>()
        .unwrap()
        .variable("derived")
        .unwrap();
    let mut b = ShadowSessionBuilder::<usize, bool>::new(&schema);
    let a = b
        .derive(field, 0, |_: &bool, inputs| {
            Some(
                inputs
                    .for_field::<usize>(field)
                    .unwrap()
                    .first()
                    .and_then(|v| *v)
                    .copied()
                    .unwrap_or(0)
                    + 1,
            )
        })
        .unwrap();
    let c = b
        .derive(field, 1, |_: &bool, inputs| {
            Some(
                inputs.for_field::<usize>(field).unwrap()[0]
                    .copied()
                    .unwrap()
                    + 1,
            )
        })
        .unwrap();
    let c_ref = c.erase();
    b.bind_dependencies(&a, move |cyclic| {
        Ok(if *cyclic { vec![c_ref.clone()] } else { vec![] })
    })
    .unwrap();
    b.set_dependencies(&c, &[a.erase()]).unwrap();
    let mut s = b.build();
    s.update(&true).unwrap();
    assert!(s.read(&a).unwrap().inconsistent && s.read(&c).unwrap().inconsistent);
    assert_eq!(
        s.before_dependencies_changed(&[a.erase()]).unwrap().len(),
        1
    );
    assert_eq!(s.read(&c).err(), Some(ShadowSessionError::NotCurrent));
    let result = s.update_notified_with_dependency_events(&false).unwrap();
    assert!(result.added.is_empty() && result.removed.is_empty());
    assert_eq!(s.read(&a).unwrap().value, Some(&1));
    assert_eq!(s.read(&c).unwrap().value, Some(&2));
    assert!(!s.read(&c).unwrap().inconsistent);
}
