// Adapted from pinned Community SolverEntityInheritanceTest, Apache-2.0.
// Test-only native declarations replace Java reflection. See provenance/notices.
mod planning_composition_support;
use planning_composition_support::*;
use timetuck::*;
fn positive<const N: usize>(shape: Shape, limit: u64, expected: i64, fields: &[&str]) {
    let fixture = generate::<N>(shape);
    assert!(fixture.rows.iter().flatten().all(Option::is_none));
    let composed = compile::<N>(&fixture, PlanningCompositionPolicy::single_level()).unwrap();
    let names = composed
        .schema()
        .genuine_entity::<Child<N>>()
        .unwrap()
        .genuine_variables()
        .map(|v| v.name())
        .collect::<Vec<_>>();
    assert_eq!(names, fields);
    assert_eq!(
        composed.field_origin(PlanningType::of::<Child<N>>(), "value"),
        Some(PlanningType::of::<Base<N>>())
    );
    for result in solve_fixture::<N>(&fixture, &composed, limit) {
        assert_eq!(result.score, Some(SimpleScore::new(expected)));
        assert!(result.assignment.values().iter().all(Option::is_some));
        assert!(result.work.move_evaluations <= limit);
    }
}
#[test]
fn test_both_classes_annotated() {
    positive::<0>(Shape::BothOwned, 20, 2, &["value", "subValue"]);
}
#[test]
fn test_both_classes_annotated_and_base_is_interface() {
    positive::<1>(Shape::BothProjected, 10, 2, &["value"]);
}
#[test]
fn test_only_base_class_annotated() {
    positive::<2>(Shape::BaseOwned, 10, 2, &["value"]);
}
#[test]
fn test_only_base_class_annotated_base_is_interface() {
    positive::<3>(Shape::BaseProjected, 10, 2, &["value"]);
}
#[test]
fn test_both_classes_annotated_add_new_variable() {
    positive::<6>(Shape::AddOwned, 100, 4, &["value", "value2"]);
}
#[test]
fn test_both_classes_annotated_add_new_variable_base_is_interface() {
    positive::<7>(Shape::AddProjected, 100, 4, &["value", "value2"]);
}
fn unregistered<const N: usize>(shape: Shape, class_fixture: bool) {
    let fixture = generate::<N>(shape);
    assert_eq!(fixture.actual_class_fixture, class_fixture);
    assert!(fixture.rows.iter().flatten().all(Option::is_some));
    let error = compile::<N>(&fixture, PlanningCompositionPolicy::single_level()).unwrap_err();
    assert_eq!(
        error,
        PlanningCompositionError::UnregisteredVariables(PlanningType::of::<Base<N>>())
    );
    assert!(error.to_string().contains(
        "is not registered as a planning component but defines genuine or shadow variables."
    ));
}
#[test]
fn test_only_child_class_annotated() {
    unregistered::<4>(Shape::OnlyChildOwned, false);
}
#[test]
fn test_only_child_class_annotated_base_is_interface() {
    unregistered::<5>(Shape::OnlyChildProjected, true);
}
fn replacement<const N: usize>(shape: Shape) {
    let fixture = generate::<N>(shape);
    let error = compile::<N>(&fixture, PlanningCompositionPolicy::single_level()).unwrap_err();
    assert_eq!(
        error,
        PlanningCompositionError::RedefinedVariable {
            entity: PlanningType::of::<Child<N>>(),
            variable: "value".into(),
            original: PlanningType::of::<Base<N>>(),
            redefining: PlanningType::of::<Child<N>>()
        }
    );
    assert!(
        error
            .to_string()
            .contains("redefines the planning variables ([value]), which is not permitted.")
    );
}
#[test]
fn test_both_classes_annotated_replace_variable() {
    replacement::<8>(Shape::ReplaceOwned);
}
#[test]
fn test_both_classes_annotated_replace_variable_base_is_interface() {
    replacement::<9>(Shape::ReplaceProjected);
}
fn depth<const N: usize>(shape: Shape, initialized: bool) {
    let fixture = generate::<N>(shape);
    assert!(
        fixture
            .rows
            .iter()
            .flatten()
            .all(|v| v.is_some() == initialized)
    );
    let error = compile::<N>(&fixture, PlanningCompositionPolicy::single_level()).unwrap_err();
    assert_eq!(
        error,
        PlanningCompositionError::DepthLimit {
            entity: PlanningType::of::<Child<N>>(),
            actual: 3,
            maximum: 2
        }
    );
    assert!(
        error
            .to_string()
            .contains("single-level inheritance structure")
    );
}
#[test]
fn test_multiple_both_classes_annotated() {
    depth::<10>(Shape::DeepBothOwned, false);
}
#[test]
fn test_multiple_both_classes_annotated_base_is_interface() {
    depth::<11>(Shape::DeepBothProjected, false);
}
#[test]
fn test_multiple_only_base_class_annotated() {
    depth::<12>(Shape::DeepBaseOwned, true);
}
#[test]
fn test_multiple_only_base_class_annotated_base_is_interface() {
    depth::<13>(Shape::DeepBaseProjected, true);
}
#[test]
fn test_multiple_both_classes_annotated_mixed_pattern() {
    let fixture = generate::<14>(Shape::Mixed);
    let error = compile::<14>(&fixture, PlanningCompositionPolicy::single_level()).unwrap_err();
    assert_eq!(
        error,
        PlanningCompositionError::MixedParents(PlanningType::of::<Child<14>>())
    );
    assert!(
        error
            .to_string()
            .contains("Mixed inheritance is not permitted")
    );
}
