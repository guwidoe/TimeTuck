// Adapted pinned Community TEST domain/keys/XML fixtures; Apache-2.0, NOTICE.
#![allow(dead_code)]
#[path = "named_validation_fixture.rs"]
pub mod validation;
use timetuck::*;
pub use validation::base;
pub const BASIC: &str =
    "ai.timefold.solver.spring.boot.autoconfigure.basic.domain.TestdataSpringSolution";
pub const ENTITY: &str =
    "ai.timefold.solver.spring.boot.autoconfigure.basic.domain.TestdataSpringEntity";
pub const PROVIDER: &str = "ai.timefold.solver.spring.boot.autoconfigure.basic.constraints.TestdataSpringConstraintProvider";
pub const DEFAULT: &str = "solverConfig.xml";
pub const CUSTOM: &str =
    "ai/timefold/solver/spring/boot/autoconfigure/customSpringBootSolverConfig.xml";
pub const NO_GLOBAL: &str =
    "ai/timefold/solver/spring/boot/autoconfigure/solverConfigWithoutGlobalTermination.xml";
pub const PHASES: &str = "ai/timefold/solver/spring/boot/autoconfigure/solverConfigWithPhases.xml";
pub fn source(default: bool) -> NativeSolverConfigurationSource {
    let mut context = NativeSolverConfigurationSource::new();
    context.register_entity::<base::Entity>(ENTITY).unwrap();
    for (key, xml) in [
        (
            DEFAULT,
            include_str!("configured_factories/solverConfig.xml"),
        ),
        (
            CUSTOM,
            include_str!("configured_factories/customSpringBootSolverConfig.xml"),
        ),
        (
            NO_GLOBAL,
            include_str!("configured_factories/solverConfigWithoutGlobalTermination.xml"),
        ),
        (
            PHASES,
            include_str!("configured_factories/solverConfigWithPhases.xml"),
        ),
    ] {
        context
            .register_resource(key, SolverXmlConfiguration::parse(xml).unwrap())
            .unwrap();
    }
    context.default_resource(default.then(|| DEFAULT.into()));
    context.global_resource_property(Some("timefold.solver-config-xml".into()));
    context
}
pub fn properties(values: &[(&str, &str)]) -> OwnedPropertySource {
    OwnedPropertySource::new(values.iter().map(|(k, v)| (k.to_string(), v.to_string())))
}
pub fn registry() -> NamedModelFactoryRegistry {
    let mut r = NamedModelFactoryRegistry::default();
    r.register(BASIC, base::schema(), vec![base::scoring(PROVIDER, true)])
        .unwrap();
    r
}
pub fn request(default: bool, properties: &[(&str, &str)]) -> ConfiguredSolverRequest {
    source(default)
        .resolve(
            base::configuration(None, false),
            &self::properties(properties),
            "timefold.solver",
            &mut |_| {},
        )
        .unwrap()
}
pub fn metadata(bank: &NamedSolverFactoryBank) {
    let (model, score, schema) = bank.selection("solver").unwrap();
    assert_eq!(model, BASIC);
    assert_eq!(score, PROVIDER);
    assert_eq!(
        schema.solution_type(),
        PlanningType::of::<base::BasicModel>()
    );
    assert_eq!(
        schema
            .entities()
            .map(|e| e.entity_type())
            .collect::<Vec<_>>(),
        vec![PlanningType::of::<base::Entity>()]
    );
}
pub fn execute(request: ConfiguredSolverRequest) -> base::ResultType {
    let bank = registry()
        .prepare_configured([("solver".into(), request.clone())])
        .unwrap();
    metadata(&bank);
    let factory = bank
        .unique_factory::<base::BasicModel, base::ResultType, String, SimpleScore>()
        .unwrap();
    let mut manager = request.build_manager(factory).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(base::problem(bank.selection("solver").unwrap().2))
        .run()
        .unwrap();
    let solution = job.wait();
    let solution = solution.as_ref().as_ref().unwrap().clone();
    base::positive(&solution);
    manager.shutdown().unwrap();
    solution
}
