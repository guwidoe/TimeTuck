// Complete native adaptations of pinned Community TEST assertions; Apache-2.0.
#[path = "fixtures/constraint_services_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::Arc;
use timetuck::*;
#[test]
fn constraint_meta_model() {
    let f = factory(Backend::Dense);
    assert_eq!(f.meta_model().constraints()[0].name, NAME);
}
#[test]
fn constraint_verifier() {
    check_verifier(&factory(Backend::Dense));
}
#[test]
fn constraint_verifier_bavet() {
    let xml = SolverXmlConfiguration::parse(include_str!(
        "fixtures/constraint_services/bavetSolverConfig.xml"
    ))
    .unwrap();
    assert_eq!(
        xml.root()
            .child("scoreDirectorFactory")
            .unwrap()
            .value("constraintProviderClass"),
        Some(
            "ai.timefold.solver.spring.boot.autoconfigure.basic.constraints.TestdataSpringConstraintProvider"
        )
    );
    let provider = xml
        .root()
        .child("scoreDirectorFactory")
        .unwrap()
        .value("constraintProviderClass")
        .unwrap();
    let f = match provider {
        "ai.timefold.solver.spring.boot.autoconfigure.basic.constraints.TestdataSpringConstraintProvider" => {
            factory(Backend::Stream)
        }
        _ => panic!("unregistered native provider"),
    };
    check_verifier(&f);
}
#[test]
fn read_only_concrete_provider_class() {
    let concrete: Arc<dyn ReadOnlyConstraintProvider<String, SimpleScore>> = Arc::new(PairProvider);
    let recipe = Arc::new(ScoreFactoryResource::new(concrete.clone()).unwrap());
    let f = factory_with(recipe, Backend::Stream);
    assert!(Arc::ptr_eq(f.score_factory().provider(), &concrete));
}
#[test]
fn singleton_solver_factory() {
    let f = factory(Backend::Dense);
    let solution_manager = f.solution_manager();
    assert!(Arc::ptr_eq(
        f.score_factory(),
        solution_manager.score_factory()
    ));
    let mut manager = f.build_manager(1).unwrap();
    assert!(Arc::ptr_eq(manager.factory(), &f));
    manager.shutdown().unwrap();
}
#[test]
fn quarkus_singleton_solver_factory() {
    let f = factory(Backend::Stream);
    assert!(!f.meta_model().constraints().is_empty());
}
