// Adapted pinned Community SelectorBasedListRuinRecreateMoveTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::rc::Rc;
use timetuck::*;
struct Solution;
struct Entity;
#[derive(Debug, Eq, PartialEq)]
struct Object {
    code: &'static str,
    members: Vec<Rc<Object>>,
}
fn value(code: &'static str) -> Rc<Object> {
    Rc::new(Object {
        code,
        members: vec![],
    })
}
fn entity(code: &'static str, members: Vec<Rc<Object>>) -> Rc<Object> {
    Rc::new(Object { code, members })
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::list::<Rc<Object>>("valueList", false),
    ])])
    .unwrap()
}
#[test]
fn rebase() {
    let s = schema();
    let descriptor = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    let v1 = value("v1");
    let v2 = value("v2");
    let e1 = entity("e1", vec![v1.clone()]);
    let e2 = entity("e2", vec![]);
    let e3 = entity("e3", vec![v1.clone()]);
    let destination_v1 = value("v1");
    let destination_v2 = value("v2");
    let destination_e1 = entity("e1", vec![destination_v1.clone()]);
    let destination_e2 = entity("e2", vec![]);
    let destination_e3 = entity("e3", vec![destination_v1.clone()]);
    let mappings = [
        (&v1, &destination_v1),
        (&v2, &destination_v2),
        (&e1, &destination_e1),
        (&e2, &destination_e2),
        (&e3, &destination_e3),
    ];
    // Preserve the helper's pointer-based lookup, rather than matching codes.
    let lookup = |source: &Rc<Object>| {
        mappings
            .iter()
            .find(|(key, _)| Rc::ptr_eq(source, key))
            .map(|(_, target)| (*target).clone())
            .ok_or("unmapped object")
    };
    let movement = ListRuinSelection::new(
        descriptor,
        vec![v1.clone(), v2.clone()],
        vec![e1.clone(), e2.clone(), e3.clone()],
    )
    .unwrap();
    let rebased = movement.rebase(lookup, lookup).unwrap();
    assert_eq!(rebased.variable(), descriptor);
    assert_eq!(rebased.planning_entities().len(), 3);
    for expected in [&destination_e1, &destination_e2, &destination_e3] {
        assert!(
            rebased
                .planning_entities()
                .iter()
                .any(|actual| Rc::ptr_eq(actual, expected))
        );
    }
    assert_eq!(rebased.planning_values().len(), 2);
    assert!(Rc::ptr_eq(&rebased.planning_values()[0], &destination_v1));
    assert!(Rc::ptr_eq(&rebased.planning_values()[1], &destination_v2));
}
#[test]
fn equality() {
    let s = schema();
    let descriptor = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    let v1 = value("v1");
    let v2 = value("v2");
    let e1 = entity("e1", vec![v1.clone()]);
    let e2 = entity("e2", vec![]);
    // Preserve the source's deliberate entity-in-value-list/value-in-entity-set
    // fixture. Metadata equality is available independently of executable binding.
    let movement = ListRuinSelection::new(descriptor, vec![e1.clone()], vec![v1.clone()]).unwrap();
    let same = ListRuinSelection::new(descriptor, vec![e1.clone()], vec![v1.clone()]).unwrap();
    assert_eq!(movement, same);
    let different = ListRuinSelection::new(descriptor, vec![e1.clone()], vec![v2]).unwrap();
    assert_ne!(movement, different);
    let another = ListRuinSelection::new(descriptor, vec![e2], vec![v1.clone()]).unwrap();
    assert_ne!(movement, another);
    let other_s = schema();
    let other_descriptor = other_s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    let yet_another = ListRuinSelection::new(other_descriptor, vec![e1], vec![v1]).unwrap();
    assert_ne!(movement, yet_another);
}
