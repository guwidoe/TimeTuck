// Complete pinned Community RuinRecreateConstructionHeuristicPhaseBuilderTest.
// Apache-2.0; see compat/provenance.json.
#[path = "fixtures/reconstruction_builder_fixture.rs"]
mod fixture;
use fixture::*;
use std::rc::Rc;
use timetuck::*;
#[test]
fn build_single_threaded() {
    let s = schema();
    let context = context(EnvironmentMode::PhaseAssert);
    let builder = ReconstructionPhaseBuilder::new(&context, placer(&s), 1).unwrap();
    let phase = builder.build_for(&context).unwrap();
    assert!(Rc::ptr_eq(phase.placer(), builder.placer()));
}
#[test]
fn nested_phase_runs_in_the_enclosing_phase_environment_mode() {
    let s = schema();
    let context = context(EnvironmentMode::FullAssert);
    let builder = ReconstructionPhaseBuilder::new(&context, placer(&s), 1).unwrap();
    assert_eq!(
        builder.build_for(&context).unwrap().environment_mode(),
        EnvironmentMode::FullAssert
    );
}
#[test]
fn build_multi_threaded() {
    let s = schema();
    let context = context(EnvironmentMode::PhaseAssert);
    let worker = context.derive_worker(0);
    assert!(worker.is_derived());
    let builder = ReconstructionPhaseBuilder::new(&context, placer(&s), 2).unwrap();
    let phase = builder.build_for(&worker).unwrap();
    assert!(!Rc::ptr_eq(phase.placer(), builder.placer()));
}
