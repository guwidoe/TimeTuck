// Adapted from Community ShadowVariableUpdateTest.emptyEntities, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use timetuck::*;
#[test]
fn empty_entities() {
    struct Solution;
    let schema = PlanningSchema::new::<Solution>(vec![]).unwrap();
    let mut session = ShadowSessionBuilder::<usize, ()>::new(&schema).build();
    let error = session
        .update_entities(&(), &[])
        .err()
        .expect("empty entity array must fail");
    assert_eq!(error, ShadowSessionError::EmptyEntityArray);
    assert!(
        error
            .to_string()
            .contains("The entity array cannot be empty.")
    );
}
