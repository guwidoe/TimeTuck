#[path = "fixtures/reconstruction_builder_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[test]
fn workers_own_independent_reconstruction_assignments_and_fresh_live_placer_sessions() {
    let s = schema();
    let mut root = context(EnvironmentMode::FullAssert);
    let mut left = root.derive_worker(0);
    let mut right = root.derive_worker(1);
    let builder = ReconstructionPhaseBuilder::new(&root, placer(&s), 2).unwrap();
    let mut l = builder.build_for(&left).unwrap();
    let mut r = builder.build_for(&right).unwrap();
    assert!(!Rc::ptr_eq(l.placer(), r.placer()));
    let network = network();
    assert!(matches!(
        l.reconstruct(&mut right, &network, ConstructionConfig::default(), fresh),
        Err(ReconstructionRunError::ForeignContext)
    ));

    let out = l
        .reconstruct(&mut left, &network, ConstructionConfig::default(), fresh)
        .unwrap();
    assert!(out.complete);
    assert_eq!(left.assignment().values(), &[Some(2), Some(2)]);
    assert_eq!(out.work.construction_steps, 2);
    assert_eq!(right.assignment().values(), &[None, None]);
    assert_eq!(root.assignment().values(), &[None, None]);
    assert_eq!(
        l.placer().borrow().session().queue().state(),
        SelectionCacheState::Idle
    );
    assert_eq!(
        r.placer().borrow().session().queue().state(),
        SelectionCacheState::Idle
    );
    right
        .replace_assignment(Assignment::new(vec![Some(1), None]))
        .unwrap();
    r.reconstruct(&mut right, &network, ConstructionConfig::default(), fresh)
        .unwrap();
    assert_eq!(right.assignment().values(), &[Some(1), Some(2)]);
    assert_eq!(left.assignment().values(), &[Some(2), Some(2)]);
    let mut enclosing = builder.build_for(&root).unwrap();
    assert!(Rc::ptr_eq(enclosing.placer(), builder.placer()));
    enclosing
        .reconstruct(&mut root, &network, ConstructionConfig::default(), fresh)
        .unwrap();
    assert_eq!(root.assignment().values(), &[Some(2), Some(2)]);
    // A later reconstruction attempt starts from an actual ruined child snapshot.
    let mut repair = root.derive_worker(0);
    let mut ruined = repair.assignment().clone();
    ruined
        .apply(&Move::Change {
            variable: 0,
            value: None,
        })
        .unwrap()
        .commit();
    repair.replace_assignment(ruined).unwrap();
    let mut nested = builder.build_for(&repair).unwrap();
    let repaired = nested
        .reconstruct(&mut repair, &network, ConstructionConfig::default(), fresh)
        .unwrap();
    assert_eq!(repaired.work.construction_steps, 1);
    assert_eq!(repair.assignment().values(), &[Some(2), Some(2)]);
    assert_eq!(root.assignment().values(), &[Some(2), Some(2)]);
}
#[test]
fn inherited_full_assertions_detect_actual_corruption_and_preserve_failed_context() {
    let s = schema();
    let mut context = context(EnvironmentMode::FullAssert);
    let builder = ReconstructionPhaseBuilder::new(&context, placer(&s), 1).unwrap();
    let mut phase = builder.build_for(&context).unwrap();
    let before = context.assignment().clone();
    let mut bad = ConstraintNetwork::new(2);
    bad.add_constraint("corrupted sum", vec![0, 1], |_| SimpleScore::new(123))
        .unwrap();
    let checks = Cell::new(0);
    let result = phase.reconstruct(&mut context, &bad, ConstructionConfig::default(), |a| {
        checks.set(checks.get() + 1);
        fresh(a)
    });
    assert!(result.is_err());
    assert!(checks.get() > 0);
    assert_eq!(context.assignment(), &before);
    let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2]); 2]).unwrap();
    let initial = problem.initial_assignment();
    let mut unchecked = ReconstructionContext::new(
        problem,
        initial,
        EnvironmentMode::NoAssert,
        EnvironmentMode::NoAssert,
    )
    .unwrap();
    let builder = ReconstructionPhaseBuilder::new(&unchecked, placer(&s), 1).unwrap();
    let mut phase = builder.build_for(&unchecked).unwrap();
    let out = phase
        .reconstruct(&mut unchecked, &bad, ConstructionConfig::default(), |_| {
            panic!("NoAssert must not recompute")
        })
        .unwrap();
    assert!(out.complete);
    assert_eq!(out.score, Some(SimpleScore::new(123)));
}
#[test]
fn live_context_lineage_worker_bounds_and_noncopyable_sessions_reject_before_mutation() {
    let s = schema();
    let root = context(EnvironmentMode::PhaseAssert);
    let foreign = context(EnvironmentMode::PhaseAssert);
    let builder = ReconstructionPhaseBuilder::new(&root, placer(&s), 2).unwrap();
    assert!(matches!(
        builder.build_for(&foreign),
        Err(ReconstructionBuildError::ForeignContext)
    ));
    assert!(matches!(
        builder.build_for(&root.derive_worker(2)),
        Err(ReconstructionBuildError::WorkerIndex { .. })
    ));
    let factory = placer(&s);
    let provider = factory.configuration().unwrap().build_provider().unwrap();
    let manual: RebuildablePlacer<Provider, Recipe<'_>, ConstructionFactoryError> =
        RebuildablePlacer::from_session(provider);
    let no_copy = ReconstructionPhaseBuilder::new(&root, manual, 2).unwrap();
    assert!(matches!(
        no_copy.build_for(&root.derive_worker(0)),
        Err(ReconstructionBuildError::Copy(PlacerCopyError::Unavailable))
    ));
}
