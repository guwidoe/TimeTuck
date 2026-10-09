// Adapted from pinned Timefold Community move-provider tests; see provenance.json.
// SPDX-License-Identifier: Apache-2.0
// Native schema resolution replaces Java metamodel/provider construction.
use timetuck::*;
struct Solution;
struct Entity;
fn schema(list: bool, optional: bool) -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        if list {
            PlanningVariableDefinition::list::<&'static str>("valueList", optional)
        } else {
            PlanningVariableDefinition::basic::<&'static str>("value", optional)
        },
    ])])
    .unwrap()
}
#[test]
fn change_rejects_explicit_crossing_on_required_variable() {
    let s = schema(false, false);
    assert!(matches!(
        SchemaMoveConfig::new(SchemaMoveKind::Change)
            .with_crossing_unassigned(true)
            .resolve(&s),
        Err(SchemaMoveError::CrossingUnassigned(_))
    ));
}
#[test]
fn list_change_accepts_crossing_for_optional_variable() {
    let s = schema(true, true);
    let variable = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    assert_eq!(
        variable.kind(),
        PlanningVariableKind::List {
            allows_unassigned: true
        }
    );
    assert!(
        SchemaMoveConfig::new(SchemaMoveKind::ListChange)
            .with_crossing_unassigned(true)
            .resolve(&s)
            .is_ok()
    );
}
#[test]
fn list_change_crossing_requires_optional_variable() {
    let s = schema(true, false);
    let variable = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("valueList")
        .unwrap();
    assert_eq!(
        variable.kind(),
        PlanningVariableKind::List {
            allows_unassigned: false
        }
    );
    let error = SchemaMoveConfig::new(SchemaMoveKind::ListChange)
        .with_crossing_unassigned(true)
        .resolve(&s)
        .unwrap_err();
    assert!(matches!(error, SchemaMoveError::CrossingUnassigned(_)));
    assert!(error.to_string().contains("crossing_unassigned"));
}
#[test]
fn list_assign_rejects_required_variable() {
    let s = schema(true, false);
    assert!(matches!(
        SchemaMoveConfig::new(SchemaMoveKind::Assign).resolve(&s),
        Err(SchemaMoveError::TransitionRequiresOptional(_))
    ));
}
#[test]
fn list_unassign_rejects_required_variable() {
    let s = schema(true, false);
    assert!(matches!(
        SchemaMoveConfig::new(SchemaMoveKind::Unassign).resolve(&s),
        Err(SchemaMoveError::TransitionRequiresOptional(_))
    ));
}
#[test]
fn optional_basic_change_without_crossing_skips_unassigned_sources() {
    let s = schema(false, true);
    let variable = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let q = SchemaMoveConfig::new(SchemaMoveKind::Change)
        .with_crossing_unassigned(false)
        .resolve(&s)
        .unwrap()
        .bind(&[SchemaVariableSlots {
            variable,
            slots: vec![0, 1],
        }])
        .unwrap();
    let first = "Generated Value 0";
    let second = "Generated Value 1";
    let p = FiniteProblem::new(vec![
        Variable::new(vec![first, second]).allowing_unassigned(),
        Variable::new(vec![first, second])
            .allowing_unassigned()
            .with_initial(second),
    ])
    .unwrap();
    let a = p.initial_assignment();
    let moves: Vec<_> = q.try_basic_moves(&p, &a, 0).unwrap().collect();
    assert!(moves.contains(&Move::Change {
        variable: 1,
        value: Some(first)
    }));
    assert!(!moves.contains(&Move::Change {
        variable: 1,
        value: Some(second)
    }));
    assert_eq!(
        moves,
        vec![Move::Change {
            variable: 1,
            value: Some(first)
        }]
    );
}
