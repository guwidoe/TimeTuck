// Adapted complete configured-unfolding cases from pinned Community factory TESTS.
// SPDX-License-Identifier: Apache-2.0
// Explicit metadata keys replace Java configuration classes/reflection.
use timetuck::*;
struct TestdataListSolution;
struct TestdataListEntity;
struct TestdataListValue;
struct DummyEntityComparator;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<TestdataListSolution>(vec![PlanningEntityDefinition::of::<
        TestdataListEntity,
    >(vec![
        PlanningVariableDefinition::list::<TestdataListValue>("valueList", false),
    ])])
    .unwrap()
}
#[test]
fn change_unfold_configured_into_list_change_move_selector_config() {
    let schema = schema();
    let mut config = SchemaMoveConfig::new(SchemaMoveKind::Change);
    config.entity = Some(PlanningType::of::<TestdataListEntity>());
    config.entity_comparator = Some(PlanningType::of::<DummyEntityComparator>());
    config.variables = vec!["valueList".into()];
    config.cache_scope = SelectionCacheScope::Phase;
    config.order = UnionOrder::Sequential;
    config.selected_count_limit = Some(200);
    let plan = config.resolve(&schema).unwrap();
    let selector = plan.list_selectors().next().unwrap();
    assert_eq!(selector.kind, SchemaMoveBranchKind::ListChange);
    assert_eq!(selector.value.name(), "valueList");
    assert_eq!(selector.cache_scope, SelectionCacheScope::Phase);
    assert_eq!(selector.order, UnionOrder::Sequential);
    assert_eq!(selector.selected_count_limit, Some(200));
    let destination = selector.destination.unwrap();
    assert_eq!(destination.entity, PlanningType::of::<TestdataListEntity>());
    assert_eq!(
        destination.entity_comparator,
        Some(PlanningType::of::<DummyEntityComparator>())
    );
    assert_eq!(destination.value.name(), "valueList");
}
#[test]
fn swap_unfold_configured_into_list_swap_move_selector_config() {
    let schema = schema();
    let mut config = SchemaMoveConfig::new(SchemaMoveKind::Swap);
    config.entity = Some(PlanningType::of::<TestdataListEntity>());
    config.cache_scope = SelectionCacheScope::Phase;
    config.selected_count_limit = Some(200);
    let plan = config.resolve(&schema).unwrap();
    let selector = plan.list_selectors().next().unwrap();
    assert_eq!(selector.kind, SchemaMoveBranchKind::ListSwap);
    assert_eq!(selector.value.name(), "valueList");
    assert_eq!(selector.cache_scope, SelectionCacheScope::Phase);
    assert_eq!(selector.selected_count_limit, Some(200));
}
#[test]
fn list_change_unfolded_config_inherits_from_folded_config() {
    let schema = schema();
    let mut config = SchemaMoveConfig::new(SchemaMoveKind::ListChange);
    config.cache_scope = SelectionCacheScope::Step;
    config.order = UnionOrder::Sequential;
    config.selected_count_limit = Some(200);
    let plan = config.resolve(&schema).unwrap();
    let selector = plan.list_selectors().next().unwrap();
    assert_eq!(selector.kind, SchemaMoveBranchKind::ListChange);
    assert_eq!(selector.value.name(), "valueList");
    assert_eq!(selector.cache_scope, SelectionCacheScope::Step);
    assert_eq!(selector.order, UnionOrder::Sequential);
    assert_eq!(selector.selected_count_limit, Some(200));
    let destination = selector.destination.unwrap();
    assert_eq!(destination.entity, PlanningType::of::<TestdataListEntity>());
    assert_eq!(destination.value.name(), "valueList");
}
#[test]
fn union_enable_nearby_mixed_model() {
    assert!(
        !NearbyMoveConfig::union(vec![])
            .kind
            .supports_nearby_in_mixed()
    );
}
