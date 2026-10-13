// Adapted pinned Community SubListChange/SwapMoveSelectorFactoryTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::num::NonZeroUsize;
use timetuck::*;
struct ListSolution;
struct ListEntityType;
struct ListValue;
struct MultiSolution;
struct FirstEntity;
struct FirstValue;
struct SecondEntity;
struct SecondValue;
fn single() -> PlanningSchema {
    PlanningSchema::new::<ListSolution>(vec![
        PlanningEntityDefinition::of::<ListEntityType>(vec![PlanningVariableDefinition::list::<
            ListValue,
        >("valueList", false)]),
        PlanningEntityDefinition::of::<ListValue>(vec![
            PlanningVariableDefinition::shadow::<ListEntityType>("entity"),
            PlanningVariableDefinition::shadow::<i32>("index"),
        ]),
    ])
    .unwrap()
}
fn multi() -> PlanningSchema {
    PlanningSchema::new::<MultiSolution>(vec![
        PlanningEntityDefinition::of::<FirstEntity>(vec![PlanningVariableDefinition::list::<
            FirstValue,
        >("valueList", false)]),
        PlanningEntityDefinition::of::<SecondEntity>(vec![
            PlanningVariableDefinition::basic::<SecondValue>("basicValue", false),
            PlanningVariableDefinition::basic::<SecondValue>("secondBasicValue", false),
        ]),
    ])
    .unwrap()
}
fn build(s: &PlanningSchema, factory: SchemaSubListFactory) -> SchemaSubListSource {
    let field = s
        .genuine_entities()
        .flat_map(|e| e.genuine_variables())
        .find(|v| matches!(v.kind(), PlanningVariableKind::List { .. }))
        .unwrap();
    // Source factory methods instantiate a schema, not working entities/values.
    // Empty explicit bindings preserve that state while creating a real source.
    factory
        .bind(
            s,
            &[SchemaVariableSlots {
                variable: field,
                slots: vec![],
            }],
            0,
            NonZeroUsize::new(256).unwrap(),
            NonZeroUsize::new(4096).unwrap(),
        )
        .unwrap()
}
#[test]
fn change_build_move_selector() {
    let s = single();
    let source = build(&s, SchemaSubListFactory::change());
    assert!(source.is_never_ending());
    assert!(source.selects_reversing());
}
#[test]
fn change_build_move_selector_multi_entity() {
    let s = multi();
    let source = build(&s, SchemaSubListFactory::change());
    assert!(source.is_never_ending());
    assert!(source.selects_reversing());
}
#[test]
fn swap_build_base_move_selector() {
    let s = single();
    let source = build(&s, SchemaSubListFactory::swap());
    assert!(source.is_never_ending());
    assert!(source.selects_reversing());
}
#[test]
fn swap_build_move_selector_multi_entity() {
    let s = multi();
    let source = build(&s, SchemaSubListFactory::swap());
    assert!(source.is_never_ending());
    assert!(source.selects_reversing());
}
