//! Original prepared configuration and diagnostic boundary checks.
use timetuck::{EnvironmentMode as M, EnvironmentProfile as P, EnvironmentProfileError as E};
#[test]
fn sparse_overrides_keep_phase_indices_and_preparation_preserves_declarations() {
    let p = P::new(
        None,
        vec![
            None,
            Some(M::PhaseAssert),
            Some(M::FullAssert),
            None,
            Some(M::TrackedFullAssert),
        ],
    );
    let original = p.clone();
    assert_eq!(
        p.overrides().collect::<Vec<_>>(),
        [(2, M::FullAssert), (4, M::TrackedFullAssert)]
    );
    let prepared = p.prepare().unwrap();
    assert_eq!(p, original);
    assert_eq!(prepared.global_mode(), M::PhaseAssert);
    assert_eq!(prepared.phase_mode(1), Some(M::PhaseAssert));
    assert_eq!(prepared.phase_mode(4), Some(M::TrackedFullAssert));
    assert_eq!(prepared.phase_mode(5), None);
    assert_eq!(prepared.phase_mode(usize::MAX), None);
    assert_eq!(prepared.phase_modes().len(), p.declared_phases().len());
    assert_eq!(p.declared_global(), None);
}
#[test]
fn preparation_rejects_first_invalid_phase_without_promoting_global_reproducibility() {
    let p = P::new(
        Some(M::NoAssert),
        vec![Some(M::FullAssert), Some(M::NonReproducible), None],
    );
    let original = p.clone();
    assert_eq!(
        p.prepare(),
        Err(E::WeakerPhase {
            phase: 1,
            global: M::NoAssert,
            requested: M::NonReproducible
        })
    );
    assert_eq!(p, original);
    let p = P::new(
        Some(M::NonReproducible),
        vec![Some(M::NonReproducible), Some(M::NoAssert)],
    );
    assert_eq!(
        p.prepare(),
        Err(E::NonReproducibleOverride {
            phase: 1,
            requested: M::NoAssert
        })
    );
}
