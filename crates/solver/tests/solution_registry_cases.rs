// Complete Community SolutionDescriptorTest adaptations; Apache-2.0.
// See compat/provenance.json for native diagnostic and registration changes.
#[path = "fixtures/solution_registry_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn test_multiple_constraint_weights() {
    let error = invalid_weights().unwrap_err().to_string();
    assert!(error.contains("has more than one field"));
    assert!(error.contains("of type ConstraintWeightOverrides"));
}
#[test]
fn missing_declarative_supplier_name() {
    let error = missing_descriptor().unwrap_err().to_string();
    for expected in [
        "shadow variable (endTime)".into(),
        "supplierName (calculateEndTime) that does not exist".into(),
        format!(
            "inside its declaring class ({}).",
            PlanningType::of::<MissingValue>().name()
        ),
        format!(
            "Maybe you included a parameter which is not a planning solution ({})",
            PlanningType::of::<MissingSolution>().name()
        ),
        "Maybe you misspelled the supplierName name?".into(),
    ] {
        assert!(error.contains(&expected), "{expected}: {error}");
    }
}
#[test]
fn test_ordinal_id() {
    let layout = mixed();
    assert_eq!(
        layout
            .schema()
            .entities()
            .map(|e| e.ordinal())
            .collect::<Vec<_>>(),
        [0, 1]
    );
    let descriptors = layout.compile_variable_ranges(mixed_ranges()).unwrap();
    assert_eq!(descriptors.iter().len(), 3);
    let mut ordinals = descriptors
        .iter()
        .filter(|d| matches!(d.variable().kind(), PlanningVariableKind::Basic { .. }))
        .map(SolutionVariableRange::ordinal)
        .collect::<Vec<_>>();
    ordinals.push(
        descriptors
            .get::<FirstEntity>("valueList")
            .unwrap()
            .ordinal(),
    );
    ordinals.sort_unstable();
    assert_eq!(ordinals, [0, 1, 2]);
}
