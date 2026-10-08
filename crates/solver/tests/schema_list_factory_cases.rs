// Adapted from pinned Timefold Community list selector factory tests.
// SPDX-License-Identifier: Apache-2.0
// Typed metadata and enum branches replace reflection and selector class checks.
use timetuck::*;
struct Solution;
struct Basic;
struct Lists;
struct Value;
fn schema(list: bool) -> PlanningSchema {
    let entity = if list {
        PlanningEntityDefinition::of::<Lists>(vec![PlanningVariableDefinition::list::<Value>(
            "valueList",
            false,
        )])
    } else {
        PlanningEntityDefinition::of::<Basic>(vec![PlanningVariableDefinition::basic::<Value>(
            "value", false,
        )])
    };
    PlanningSchema::new::<Solution>(vec![entity]).unwrap()
}

#[test]
fn list_change_no_unfolding() {
    let s = schema(true);
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::ListChange);
    c.variables = vec!["valueList".into()];
    c.entity = Some(PlanningType::of::<Lists>());
    let p = c.resolve(&s).unwrap();
    assert!(!p.is_union());
    assert_eq!(p.branches()[0].kind, SchemaMoveBranchKind::ListChange);
}

#[test]
fn list_change_unfolded_single_list_variable() {
    let s = schema(true);
    let p = SchemaMoveConfig::new(SchemaMoveKind::ListChange)
        .resolve(&s)
        .unwrap();
    assert!(!p.is_union());
    assert_eq!(p.branches()[0].kind, SchemaMoveBranchKind::ListChange);
}

#[test]
fn list_change_unfolding_fails_if_there_is_no_list_variable() {
    let s = schema(false);
    let error = SchemaMoveConfig::new(SchemaMoveKind::ListChange)
        .resolve(&s)
        .unwrap_err();
    assert_eq!(error, SchemaMoveError::CannotDeduceListVariable);
    assert!(error.to_string().contains("cannot unfold"));
}

#[test]
fn list_change_explicit_config_must_use_list_variable() {
    let s = schema(false);
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::ListChange);
    c.variables = vec!["value".into()];
    c.entity = Some(PlanningType::of::<Basic>());
    let error = c.resolve(&s).unwrap_err();
    assert_eq!(error, SchemaMoveError::ListVariableRequired("value".into()));
    assert!(error.to_string().contains("not a planning list variable"));
}

#[test]
fn list_swap_no_unfolding() {
    let s = schema(true);
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::ListSwap);
    c.variables = vec!["valueList".into()];
    let p = c.resolve(&s).unwrap();
    assert!(!p.is_union());
    assert_eq!(p.branches()[0].kind, SchemaMoveBranchKind::ListSwap);
}

#[test]
fn list_swap_unfolded_single_list_variable() {
    let s = schema(true);
    let p = SchemaMoveConfig::new(SchemaMoveKind::ListSwap)
        .resolve(&s)
        .unwrap();
    assert!(!p.is_union());
    assert_eq!(p.branches()[0].kind, SchemaMoveBranchKind::ListSwap);
}

#[test]
fn list_swap_unfolding_fails_if_there_is_no_list_variable() {
    let s = schema(false);
    let error = SchemaMoveConfig::new(SchemaMoveKind::ListSwap)
        .resolve(&s)
        .unwrap_err();
    assert_eq!(error, SchemaMoveError::CannotDeduceListVariable);
    assert!(
        error
            .to_string()
            .contains("cannot be deduced automatically")
    );
}

#[test]
fn list_swap_explicit_config_must_use_list_variable() {
    let s = schema(false);
    let mut c = SchemaMoveConfig::new(SchemaMoveKind::ListSwap);
    c.variables = vec!["value".into()];
    let error = c.resolve(&s).unwrap_err();
    assert_eq!(error, SchemaMoveError::ListVariableRequired("value".into()));
    assert!(
        error
            .to_string()
            .contains("cannot be deduced automatically")
    );
}

#[test]
fn sub_list_change_unfolding_fails_if_there_is_no_list_variable() {
    let s = schema(false);
    let error =
        SchemaMoveConfig::new(SchemaMoveKind::SubListChange(SubListChangeConfig::default()))
            .resolve(&s)
            .unwrap_err();
    assert_eq!(error, SchemaMoveError::CannotDeduceListVariable);
    assert!(
        error
            .to_string()
            .contains("cannot be deduced automatically")
    );
}

#[test]
fn sub_list_change_explicit_config_must_use_list_variable() {
    let s = schema(false);
    let mut c =
        SchemaMoveConfig::new(SchemaMoveKind::SubListChange(SubListChangeConfig::default()));
    c.variables = vec!["value".into()];
    c.entity = Some(PlanningType::of::<Basic>());
    let error = c.resolve(&s).unwrap_err();
    assert_eq!(error, SchemaMoveError::ListVariableRequired("value".into()));
    assert!(error.to_string().contains("not a planning list variable"));
}

#[test]
fn sub_list_change_disable_select_reversing_move_too() {
    let s = schema(true);
    let config = SubListChangeConfig::default().with_reversing(false);
    let p = SchemaMoveConfig::new(SchemaMoveKind::SubListChange(config))
        .resolve(&s)
        .unwrap();
    let SchemaMoveBranchKind::SubListChange(resolved) = p.branches()[0].kind else {
        panic!("wrong selector family")
    };
    assert!(!resolved.reversing());
}

#[test]
fn sub_list_swap_unfolding_fails_if_there_is_no_list_variable() {
    let s = schema(false);
    let error = SchemaMoveConfig::new(SchemaMoveKind::SubListSwap(SubListSwapConfig::default()))
        .resolve(&s)
        .unwrap_err();
    assert_eq!(error, SchemaMoveError::CannotDeduceListVariable);
    assert!(
        error
            .to_string()
            .contains("cannot be deduced automatically")
    );
}

#[test]
fn sub_list_swap_disable_select_reversing_move_too() {
    let s = schema(true);
    let config = SubListSwapConfig::default().with_reversing(false);
    let p = SchemaMoveConfig::new(SchemaMoveKind::SubListSwap(config))
        .resolve(&s)
        .unwrap();
    let SchemaMoveBranchKind::SubListSwap(resolved) = p.branches()[0].kind else {
        panic!("wrong selector family")
    };
    assert!(!resolved.reversing());
}
