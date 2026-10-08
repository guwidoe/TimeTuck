// Adapted from Timefold Community factory tests at the revision in provenance.json.
// SPDX-License-Identifier: Apache-2.0
// Native typed metadata replaces Java reflection and concrete selector classes.
use timetuck::*;
struct Solution;
struct Multi;
struct Value;
struct Other;
struct Lead;
struct Herd;
struct Unknown;
struct ListEntityType;
fn multivar() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Multi>(vec![
        PlanningVariableDefinition::basic::<Value>("primaryValue", false),
        PlanningVariableDefinition::basic::<Value>("secondaryValue", false),
        PlanningVariableDefinition::basic::<Other>("tertiaryValueAllowedUnassigned", true),
    ])])
    .unwrap()
}
fn multientity() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Lead>(vec![PlanningVariableDefinition::basic::<Value>(
            "value", false,
        )]),
        PlanningEntityDefinition::of::<Herd>(vec![PlanningVariableDefinition::basic::<Lead>(
            "leadEntity",
            false,
        )]),
    ])
    .unwrap()
}
fn list() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<ListEntityType>(vec![
        PlanningVariableDefinition::list::<Value>("valueList", false),
    ])])
    .unwrap()
}
fn config(kind: SchemaMoveKind) -> SchemaMoveConfig {
    SchemaMoveConfig {
        order: UnionOrder::Random { seed: 0 },
        ..SchemaMoveConfig::new(kind)
    }
}
fn assert_single(plan: &SchemaMovePlan<'_>, kind: SchemaMoveBranchKind) {
    assert!(!plan.is_union());
    assert_eq!(plan.branches().len(), 1);
    assert_eq!(plan.branches()[0].kind, kind);
}

#[test]
fn change_deducible_multi_var() {
    let schema = multivar();
    let mut c = config(SchemaMoveKind::Change);
    c.variables = vec!["secondaryValue".into()];
    let plan = c.resolve(&schema).unwrap();
    assert_single(&plan, SchemaMoveBranchKind::BasicChange);
    assert_eq!(plan.branches()[0].variables[0].name(), "secondaryValue");
}

#[test]
fn change_undeducible_multi_var() {
    let schema = multivar();
    let mut c = config(SchemaMoveKind::Change);
    c.variables = vec!["nonExistingValue".into()];
    assert!(c.resolve(&schema).is_err());
}

#[test]
fn change_unfolded_multi_var() {
    let schema = multivar();
    let plan = config(SchemaMoveKind::Change).resolve(&schema).unwrap();
    assert!(plan.is_union());
    assert_eq!(plan.branches().len(), 3);
}

#[test]
fn change_deducible_multi_entity() {
    let schema = multientity();
    let mut c = config(SchemaMoveKind::Change);
    c.entity = Some(PlanningType::of::<Herd>());
    assert_single(
        &c.resolve(&schema).unwrap(),
        SchemaMoveBranchKind::BasicChange,
    );
}

#[test]
fn change_undeducible_multi_entity() {
    let schema = multientity();
    let mut c = config(SchemaMoveKind::Change);
    c.entity = Some(PlanningType::of::<Unknown>());
    assert!(c.resolve(&schema).is_err());
}

#[test]
fn change_unfolded_multi_entity() {
    let schema = multientity();
    let plan = config(SchemaMoveKind::Change).resolve(&schema).unwrap();
    assert!(plan.is_union());
    assert_eq!(plan.branches().len(), 2);
}

#[test]
fn change_unfold_empty_into_list_change_move_selector_config() {
    let schema = list();
    assert_single(
        &config(SchemaMoveKind::Change).resolve(&schema).unwrap(),
        SchemaMoveBranchKind::ListChange,
    );
}

#[test]
fn swap_deducible_multi_var() {
    let schema = multivar();
    let mut c = config(SchemaMoveKind::Swap);
    c.variables = vec!["secondaryValue".into()];
    let plan = c.resolve(&schema).unwrap();
    assert_single(&plan, SchemaMoveBranchKind::BasicSwap);
    assert_eq!(plan.branches()[0].variables[0].name(), "secondaryValue");
}

#[test]
fn swap_undeducible_multi_var() {
    let schema = multivar();
    let mut c = config(SchemaMoveKind::Swap);
    c.variables = vec!["nonExistingValue".into()];
    assert!(c.resolve(&schema).is_err());
}

#[test]
fn swap_unfolded_multi_var() {
    let schema = multivar();
    let plan = config(SchemaMoveKind::Swap).resolve(&schema).unwrap();
    assert_single(&plan, SchemaMoveBranchKind::BasicSwap);
    assert_eq!(plan.branches()[0].variables.len(), 3);
}

#[test]
fn swap_deducible_multi_entity() {
    let schema = multientity();
    let mut c = config(SchemaMoveKind::Swap);
    c.entity = Some(PlanningType::of::<Herd>());
    assert_single(
        &c.resolve(&schema).unwrap(),
        SchemaMoveBranchKind::BasicSwap,
    );
}

#[test]
fn swap_undeducible_multi_entity() {
    let schema = multientity();
    let mut c = config(SchemaMoveKind::Swap);
    c.entity = Some(PlanningType::of::<Unknown>());
    assert!(c.resolve(&schema).is_err());
}

#[test]
fn swap_unfolded_multi_entity() {
    let schema = multientity();
    let plan = config(SchemaMoveKind::Swap).resolve(&schema).unwrap();
    assert!(plan.is_union());
    assert_eq!(plan.branches().len(), 2);
}

#[test]
fn swap_unfold_empty_into_list_swap_move_selector_config() {
    let schema = list();
    assert_single(
        &config(SchemaMoveKind::Swap).resolve(&schema).unwrap(),
        SchemaMoveBranchKind::ListSwap,
    );
}

#[test]
fn swap_deducible_multi_entity_with_secondary_entity_selector() {
    let schema = multientity();
    let mut c = config(SchemaMoveKind::Swap);
    c.entity = Some(PlanningType::of::<Herd>());
    c.secondary_entity = c.entity;
    assert_single(
        &c.resolve(&schema).unwrap(),
        SchemaMoveBranchKind::BasicSwap,
    );
}

#[test]
fn swap_unswappable_multi_entity_with_secondary_entity_selector() {
    let schema = multientity();
    let mut c = config(SchemaMoveKind::Swap);
    c.entity = Some(PlanningType::of::<Lead>());
    c.secondary_entity = Some(PlanningType::of::<Herd>());
    assert!(c.resolve(&schema).is_err());
}

#[test]
fn swap_unfolded_multi_entity_with_secondary_entity_selector() {
    let schema = multientity();
    let plan = config(SchemaMoveKind::Swap).resolve(&schema).unwrap();
    assert!(plan.is_union());
    assert_eq!(plan.branches().len(), 2);
}
