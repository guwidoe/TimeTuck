// Complete pinned Community TEST adaptations; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::RefCell, sync::Arc};
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<Id>("value", true),
    ])])
    .unwrap()
}
// Equal labels in different snapshots are distinct source/destination identities.
#[derive(Clone, Debug)]
struct Id(Arc<str>);
impl PartialEq for Id {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Id {}
fn id(s: &'static str) -> Id {
    Id(Arc::from(s))
}
#[test]
fn rebase() {
    let schema = schema();
    let variable = schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let v1 = id("v1");
    let v2 = id("v2");
    let d1 = id("v1");
    let d2 = id("v2");
    let source = Assignment::new(vec![Some(v1.clone()), None, Some(v1.clone())]);
    let destination = Assignment::new(vec![Some(d1.clone()), None, Some(d1.clone())]);
    let movement =
        BasicRuinSelection::new(variable, vec![0, 1, 2], vec![v1.clone(), v2.clone()]).unwrap();
    let calls = RefCell::new(vec![]);
    let rebased = movement
        .rebase(
            |i| {
                calls.borrow_mut().push(format!("e{i}"));
                Ok::<_, ()>(i + 3)
            },
            |v| {
                calls.borrow_mut().push((*v.0).to_owned());
                if v == &v1 {
                    Ok(d1.clone())
                } else {
                    assert_eq!(v, &v2);
                    Ok(d2.clone())
                }
            },
        )
        .unwrap();
    assert_eq!(rebased.planning_entities(), &[3, 4, 5]);
    assert_eq!(rebased.planning_values().len(), 2);
    assert!(rebased.planning_values().contains(&d1));
    assert!(rebased.planning_values().contains(&d2));
    assert_eq!(*calls.borrow(), vec!["e0", "e1", "e2", "v1", "v2"]);
    assert_ne!(v1, d1);
    assert_ne!(v2, d2);
    assert!(source.values()[1].is_none());
    assert!(destination.values()[1].is_none());
    assert_eq!(rebased.variable(), variable);
}
#[test]
fn equality() {
    let other_schema = schema();
    let schema = schema();
    let variable = schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let other = other_schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let v1 = id("v1");
    let v2 = id("v2");
    let make =
        |field, entity, value| BasicRuinSelection::new(field, vec![entity], vec![value]).unwrap();
    let movement = make(variable, 0, v1.clone());
    assert_eq!(movement, make(variable, 0, v1.clone()));
    assert_ne!(movement, make(variable, 0, v2));
    assert_ne!(movement, make(variable, 1, v1.clone()));
    assert_ne!(movement, make(other, 0, v1));
}
