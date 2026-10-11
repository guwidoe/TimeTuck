// Adapted from pinned Community EnvironmentModeUtilTest. Apache-2.0.
use timetuck::{EnvironmentMode as M, EnvironmentProfile as P};
// PlannerTestUtils' default construction/local-search configuration has two phases.
fn config(global: Option<M>, phases: [Option<M>; 2]) -> P {
    P::new(global, phases.to_vec())
}
#[test]
fn resolve_without_phases_is_the_declared_mode() {
    let p = P::new(Some(M::FullAssert), vec![]);
    assert_eq!(p.global_mode(), M::FullAssert);
    assert!(p.phase_modes().next().is_none());
    p.validate().unwrap();
}
#[test]
fn every_phase_overriding_leaves_the_global_mode_alone() {
    let p = config(None, [Some(M::FullAssert); 2]);
    assert_eq!(p.global_mode(), M::PhaseAssert);
    assert_eq!(p.phase_modes().collect::<Vec<_>>(), [M::FullAssert; 2]);
    p.validate().unwrap();
}
#[test]
fn phase_which_does_not_override_uses_the_global_mode() {
    let p = config(None, [None, Some(M::FullAssert)]);
    assert_eq!(p.global_mode(), M::PhaseAssert);
    assert_eq!(
        p.phase_modes().collect::<Vec<_>>(),
        [M::PhaseAssert, M::FullAssert]
    );
}
#[test]
fn differing_phase_modes_leave_the_global_mode_alone() {
    let p = config(
        Some(M::StepAssert),
        [Some(M::NonIntrusiveFullAssert), Some(M::TrackedFullAssert)],
    );
    assert_eq!(p.global_mode(), M::StepAssert);
    assert_eq!(
        p.phase_modes().collect::<Vec<_>>(),
        [M::NonIntrusiveFullAssert, M::TrackedFullAssert]
    );
}
#[test]
fn phase_less_strict_than_global_is_rejected() {
    let p = config(Some(M::StepAssert), [None, Some(M::NoAssert)]);
    assert!(p.validate().unwrap_err().to_string().contains(
        "must have an assertion level higher than or equal to the global environment level"
    ));
    assert_eq!(p.global_mode(), M::StepAssert);
    assert_eq!(
        p.phase_modes().collect::<Vec<_>>(),
        [M::StepAssert, M::NoAssert]
    );
}
#[test]
fn resolve_phases_with_the_global_environment_mode() {
    let p = config(None, [None; 2]);
    assert!(p.overrides().next().is_none());
    let p = config(None, [None, Some(M::FullAssert)]);
    assert_eq!(
        p.overrides().map(|(_, mode)| mode).collect::<Vec<_>>(),
        [M::FullAssert]
    );
    assert_eq!(
        p.phase_modes().collect::<Vec<_>>(),
        [M::PhaseAssert, M::FullAssert]
    );
}
#[test]
fn resolve_phases_with_empty_phase_list() {
    let p = P::default();
    assert!(p.overrides().next().is_none());
    assert!(p.phase_modes().next().is_none());
}
#[test]
fn non_reproducible_global_admits_no_phase_override() {
    let p = config(Some(M::NonReproducible), [None, Some(M::FullAssert)]);
    assert!(
        p.validate()
            .unwrap_err()
            .to_string()
            .contains("is only possible when global environmentMode is reproducible")
    );
}
#[test]
fn non_reproducible_global_valid_without_overrides() {
    let p = config(Some(M::NonReproducible), [Some(M::NonReproducible), None]);
    p.validate().unwrap();
    assert_eq!(p.global_mode(), M::NonReproducible);
}
