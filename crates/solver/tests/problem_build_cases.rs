// Adapted from pinned Community DefaultSolverTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;

fn assert_detail(
    v: &ValueRangeViolation<'_, i32>,
    entity: &str,
    variable: &str,
    value: i32,
    allowed: &[i32],
    optional: bool,
) {
    assert_eq!(*v.value, value);
    assert_eq!(v.allowed, allowed);
    assert_eq!(v.allows_unassigned, optional);
    let text = v.describe(entity, variable, i32::to_string);
    assert!(text.contains(&format!(
        "entity {entity}, variable {variable}: value {value}"
    )));
    assert!(text.contains(&format!("allowed range {allowed:?}")));
    assert_eq!(text.contains("unassigned allowed"), optional);
}
#[test]
fn fail_basic_variable_invalid_value_range() {
    let input = [
        Variable::new(vec![1, 2]).with_initial(3),
        Variable::new(vec![1, 2]).with_initial(1),
    ];
    let e = build_basic_with_diagnostics(&input).unwrap_err();
    assert_eq!(e.cause, ModelError::ValueOutsideDomain { variable: 0 });
    let v = e.violation.unwrap();
    assert_eq!(v.location, RangeViolationLocation::Basic { variable: 0 });
    assert_detail(&v, "e1", "value", 3, &[1, 2], false);
}
// Adapted fixture: the three annotated optional fields and constructor in
// TestdataAllowsUnassignedMultiVarEntityProvidingEntity; flatten entity fields.
fn multi_var_entity(
    first: Vec<i32>,
    value: i32,
    second: Vec<i32>,
    second_value: i32,
    solution_value: i32,
) -> [Variable<i32>; 3] {
    [
        Variable::new(first)
            .allowing_unassigned()
            .with_initial(value),
        Variable::new(second)
            .allowing_unassigned()
            .with_initial(second_value),
        Variable::new(vec![1])
            .allowing_unassigned()
            .with_initial(solution_value),
    ]
}
#[test]
fn fail_multiple_basic_variable_invalid_value_range() {
    let input = [
        multi_var_entity(vec![1, 2, 3], 3, vec![1, 2], 3, 3),
        multi_var_entity(vec![1, 2, 3], 1, vec![1, 2, 3], 1, 1),
    ]
    .concat();
    let e = build_basic_with_diagnostics(&input).unwrap_err();
    assert_eq!(e.cause, ModelError::ValueOutsideDomain { variable: 1 });
    let v = e.violation.unwrap();
    assert_eq!(v.location, RangeViolationLocation::Basic { variable: 1 });
    assert_detail(&v, "e1", "secondValue", 3, &[1, 2], true);
}
#[test]
fn fail_list_variable_invalid_value_range() {
    let input = [
        ListEntity::new(vec![3]).with_value_range(vec![1, 2]),
        ListEntity::new(vec![1]).with_value_range(vec![1, 2]),
    ];
    let e = build_list_with_diagnostics(&[1, 2], &input, false).unwrap_err();
    assert_eq!(e.cause, ListError::ValueOutsideDomain);
    let v = e.violation.unwrap();
    assert_eq!(
        v.location,
        RangeViolationLocation::List {
            position: ListPosition {
                entity: 0,
                index: 0
            }
        }
    );
    assert_detail(&v, "e1", "valueList", 3, &[1, 2], false);
}
#[test]
fn fail_mixed_model_invalid_value_range() {
    let input = [
        Variable::new(vec![2, 3]).with_initial(1),
        Variable::new(vec![2, 3]),
    ];
    let lists = [ListEntity::new(vec![])];
    let e = build_mixed_with_diagnostics(&input, &[1, 2, 3], &lists, false).unwrap_err();
    let MixedRangeValidationError::Basic(e) = e else {
        panic!("invalid basic variable")
    };
    assert_eq!(e.cause, ModelError::ValueOutsideDomain { variable: 0 });
    assert_detail(&e.violation.unwrap(), "e1", "basicValue", 1, &[2, 3], false);
    let input = [Variable::new(vec![2, 3]), Variable::new(vec![2, 3])];
    let lists = [ListEntity::new(vec![1])];
    let e = build_mixed_with_diagnostics(&input, &[2, 3], &lists, false).unwrap_err();
    let MixedRangeValidationError::Lists(e) = e else {
        panic!("invalid list variable")
    };
    assert_eq!(e.cause, ListError::ValueOutsideDomain);
    assert_detail(&e.violation.unwrap(), "e1", "valueList", 1, &[2, 3], false);
}
