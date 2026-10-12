// Complete GenuineVariableDescriptorTest.valueRangeDescriptorWithSolution adaptation.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/solution_parameter_range_fixture.rs"]
mod fixture;
#[test]
fn value_range_descriptor_with_solution() {
    // Both native descriptor/accessor registrations must accept the actual
    // solution parameter, including explicit base projection on the child root.
    fixture::plain().validate().unwrap();
    fixture::extended().validate().unwrap();
}
