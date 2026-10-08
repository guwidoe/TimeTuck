// Adapted from Timefold Community SolverFactoryTest.validateInitializationPhases.
// Copyright Timefold BV and contributors. Apache-2.0; see compat/provenance.json.
use timetuck::*;
fn construction() -> SolverPhase<'static> {
    SolverPhase::Construction(Default::default())
}
fn local() -> SolverPhase<'static> {
    SolverPhase::LocalSearch(Default::default())
}
fn custom() -> SolverPhase<'static> {
    SolverPhase::Custom(Default::default())
}
fn check(sequence: SolverSequence<'static>, expected: &[(SolverPhaseKind, bool)]) {
    let factory = SolverFactory::new(sequence, |_: (), _, _| Ok::<_, ()>(())).unwrap();
    let phases = factory.configuration().phase_metadata().collect::<Vec<_>>();
    assert_eq!(phases.len(), expected.len());
    for (index, (phase, &(kind, last))) in phases.iter().zip(expected).enumerate() {
        assert_eq!(phase.index, index);
        assert_eq!(phase.kind, kind);
        assert_eq!(phase.is_last_initializing, last);
        assert_eq!(
            phase.kind.can_initialize(),
            kind != SolverPhaseKind::LocalSearch
        );
    }
    assert_eq!(
        factory.configuration().initialization_phase(),
        phases.iter().copied().find(|p| p.is_last_initializing)
    );
}
#[test]
fn validate_initialization_phases() {
    use SolverPhaseKind::{Construction as C, Custom as P, LocalSearch as L};
    check(SolverSequence::default(), &[(C, true), (L, false)]);
    for (phases, expected) in [
        (vec![construction()], vec![(C, false)]),
        (vec![local()], vec![(L, false)]),
        (
            vec![construction(), construction(), local()],
            vec![(C, false), (C, true), (L, false)],
        ),
        (
            vec![custom(), construction(), local()],
            vec![(P, false), (C, true), (L, false)],
        ),
        (
            vec![construction(), custom(), local()],
            vec![(C, false), (P, true), (L, false)],
        ),
        (vec![custom(), custom()], vec![(P, false), (P, false)]),
    ] {
        check(
            SolverSequence {
                phases,
                ..Default::default()
            },
            &expected,
        );
    }
}
