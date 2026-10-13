// Full pinned Community NullOverridesTest/ConstraintWeightOverridesTest adaptations.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/nullable_solution_weights_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;
#[test]
fn does_not_throw_npe_on_no_overrides() {
    let solution = Solution::default();
    let accessor = SolutionWeightAccessor::new_nullable(layout(false).unwrap(), |s: &Solution| {
        s.weights.clone()
    })
    .unwrap();
    let weights = accessor.read(&solution);
    let assertion = first_constraint_assertion(&solution, &weights);
    assertion
        .check(
            ConstraintExpectation::RewardImpact(ConstraintComparison::Exactly(Decimal::ZERO)),
            None,
        )
        .unwrap();
}
#[test]
fn fails_fast_on_extended_solution() {
    let error = layout(true).unwrap_err();
    assert!(matches!(
        error,
        SolutionCompositionError::DuplicateMember {
            kind: SolutionMemberCategory::ConstraintWeights,
            ..
        }
    ));
    assert!(error.to_string().contains("already found"));
}
