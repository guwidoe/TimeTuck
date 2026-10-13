// Adapted pinned Community PillarSwapMoveTest and MassChangeMoveTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use timetuck::*;
struct Solution;
struct Entity;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<&str>("value", false),
    ])])
    .unwrap()
}
fn hash(value: &impl Hash) -> u64 {
    let mut h = DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}
#[test]
fn equals_and_hash_code_ignore_member_draw_order_within_each_pillar() {
    let schema = schema();
    let variable = schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let make = |l: Vec<usize>, r: Vec<usize>| {
        PillarSwapPlan::new(
            vec![variable],
            Pillar::new(l).unwrap(),
            Pillar::new(r).unwrap(),
        )
        .unwrap()
    };
    let a = make(vec![0, 1], vec![2, 3]);
    let b = make(vec![1, 0], vec![3, 2]);
    let c = make(vec![0, 1], vec![2]);
    assert_eq!(a, b);
    assert_eq!(hash(&a), hash(&b));
    assert_ne!(a, c);
    assert_ne!(Some(&a), None);
    let unrelated: &dyn std::any::Any = &"not a move";
    // Rust statically disallows unlike-type equality; this preserves the source's
    // unrelated-object assertion through the actual runtime type identity.
    assert_ne!(
        unrelated.type_id(),
        std::any::TypeId::of::<PillarSwapPlan<'static>>()
    );
}
#[test]
fn pillar_describe_includes_entity_and_variable_name() {
    let schema = schema();
    let variable = schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let movement = PillarSwapPlan::new(
        vec![variable],
        Pillar::new([0]).unwrap(),
        Pillar::new([1]).unwrap(),
    )
    .unwrap();
    assert_eq!(
        movement.describe(Some("TestdataEntity")),
        "PillarSwapMove(TestdataEntity.value)"
    );
}
#[test]
fn mass_describe_includes_entity_and_variable_name() {
    let schema = schema();
    let variable = schema
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let movement =
        NamedMassChange::new(variable, mass_change_from_members([0], Some("v"))).unwrap();
    assert_eq!(
        movement.describe(Some("TestdataEntity")),
        "MassChangeMove(TestdataEntity.value)"
    );
}
