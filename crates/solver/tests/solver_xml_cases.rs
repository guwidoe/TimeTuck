// Complete adapted pinned Community SolverFactoryTest/SolverConfigTest XML cases.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/xml_factory_fixture.rs"]
mod fixture;
use fixture::*;
use std::error::Error;
use timetuck::*;
#[test]
fn create_from_xml_resource() {
    actual_solver(registry().from_resource(RESOURCE, resource).unwrap());
}
#[test]
fn create_from_xml_resource_no_generics_for_backwards_compatibility() {
    let r = registry();
    let factory = r.from_resource(RESOURCE, resource).unwrap();
    actual_solver(factory);
}
#[test]
fn create_from_non_existing_xml_resource_fails_showing_resource() {
    let name = "ai/timefold/solver/core/api/solver/nonExistingSolverConfig.xml";
    let e = registry().from_resource(name, resource).err().unwrap();
    assert!(e.to_string().contains(name));
}
#[test]
fn create_from_non_existing_xml_file_fails_showing_path() {
    let t = Temp::new();
    let p = t.0.join("nonExistingSolverConfig.xml");
    let e = registry().from_file(&p).err().unwrap();
    assert!(e.to_string().contains(&p.display().to_string()));
}
#[test]
fn create_from_xml_resource_class_loader() {
    actual_solver(registry().from_resource("divertThroughClassLoader/ai/timefold/solver/core/api/solver/classloaderTestdataSolverConfig.xml",resource).unwrap());
}
#[test]
fn create_from_xml_file() {
    let t = Temp::new();
    let p = t.0.join("testdataSolverConfig.xml");
    std::fs::write(&p, SIMPLE).unwrap();
    actual_solver(registry().from_file(p).unwrap());
}
#[test]
fn create_from_xml_file_class_loader() {
    let t = Temp::new();
    let p = t.0.join("classloaderTestdataSolverConfig.xml");
    std::fs::write(&p,resource("divertThroughClassLoader/ai/timefold/solver/core/api/solver/classloaderTestdataSolverConfig.xml").unwrap()).unwrap();
    actual_solver(registry().from_file(p).unwrap());
}
fn reason(e: &dyn Error) -> String {
    let mut out = e.to_string();
    let mut current = e.source();
    while let Some(c) = current {
        out.push_str(&c.to_string());
        current = c.source();
    }
    out
}
#[test]
fn create_from_invalid_xml_resource_fails_showing_both_resource_and_reason() {
    let name = "ai/timefold/solver/core/api/solver/invalidSolverConfig.xml";
    let e = registry().from_resource(name, resource).err().unwrap();
    assert!(e.to_string().contains(name));
    assert!(reason(&e).contains("invalidElementThatShouldNotBeHere"));
}
#[test]
fn create_from_invalid_xml_file_fails_showing_both_path_and_reason() {
    let t = Temp::new();
    let p = t.0.join("invalidSolverConfig.xml");
    std::fs::write(&p, INVALID).unwrap();
    let e = registry().from_file(&p).err().unwrap();
    assert!(e.to_string().contains(&p.display().to_string()));
    assert!(reason(&e).contains("invalidElementThatShouldNotBeHere"));
}
#[test]
fn xml_config_remains_same_after_read_write() {
    for input in [COMPLETE, NAMESPACE] {
        let c = SolverXmlConfiguration::parse(input).unwrap();
        let original = input.replace("solver xmlns=\"https://timefold.ai/xsd/solver\"", "solver");
        let remove_white = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        assert_eq!(remove_white(&c.to_xml()), remove_white(&original));
    }
}
#[test]
fn read_xml_config_with_namespace() {
    let c = SolverXmlConfiguration::parse(NAMESPACE).unwrap();
    assert_eq!(
        c.phase_names(),
        vec!["constructionHeuristic", "localSearch"]
    );
    assert_eq!(
        c.environment_mode().unwrap(),
        Some(EnvironmentMode::FullAssert)
    );
    assert_eq!(
        c.solution_key(),
        Some("ai.timefold.solver.core.testdomain.TestdataSolution")
    );
    assert_eq!(
        c.root()
            .child("scoreDirectorFactory")
            .unwrap()
            .value("constraintProviderClass"),
        Some("ai.timefold.solver.core.config.solver.SolverConfigTest$DummyConstraintProvider")
    );
}
#[test]
fn white_chars_in_class_name() {
    let name = "ai.timefold.solver.core.testdomain.TestdataSolution";
    let xml = format!(
        "<solver xmlns=\"https://timefold.ai/xsd/solver\"><solutionClass>  {name} \n</solutionClass></solver>"
    );
    assert_eq!(
        SolverXmlConfiguration::parse(&xml).unwrap().solution_key(),
        Some(name)
    );
}
#[test]
fn read_and_validate_invalid_solver_config_fails_indicating_the_issue() {
    let input = "<solver xmlns=\"https://timefold.ai/xsd/solver\"><constructionHeuristic><changeMoveSelector><valueSelector><variableName>subValue</variableName></valueSelector></changeMoveSelector></constructionHeuristic></solver>";
    let e = SolverXmlConfiguration::parse(input).unwrap_err();
    assert!(e.to_string().contains("Node: variableName"));
    assert!(e.source().unwrap().is::<SolverXmlSchemaError>());
}
#[test]
fn with_enable_preview_feature_set() {
    let c = SolverXmlConfiguration::default();
    assert!(c.preview_features().is_none());
    let c = c.with_preview_feature("DIVERSIFIED_LATE_ACCEPTANCE");
    assert_eq!(
        c.preview_features().unwrap(),
        vec!["DIVERSIFIED_LATE_ACCEPTANCE"]
    );
}
#[test]
fn inherit() {
    let p = SolverXmlConfiguration::parse(COMPLETE).unwrap();
    assert_eq!(SolverXmlConfiguration::default().inherit(&p), p);
}
#[test]
fn inherit_enum_set() {
    let p = SolverXmlConfiguration::parse(ENUMS).unwrap();
    assert_eq!(SolverXmlConfiguration::default().inherit(&p), p);
}
#[test]
fn visit_referenced_classes() {
    let c = SolverXmlConfiguration::parse(COMPLETE).unwrap();
    let refs = c.referenced_components();
    for class in [
        "ai.timefold.solver.core.testdomain.inheritance.entity.single.baseannotated.classes.childtoo.TestdataBothAnnotatedSolution",
        "ai.timefold.solver.core.testdomain.TestdataEntity",
        "ai.timefold.solver.core.testdomain.inheritance.entity.single.baseannotated.classes.childtoo.TestdataBothAnnotatedChildEntity",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyEasyScoreCalculator",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyConstraintProvider",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyIncrementalScoreCalculator",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyNearbyDistanceClass",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyEntityFilter",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyValueFilter",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyChangeMoveFilter",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyMoveIteratorFactory",
        "ai.timefold.solver.core.config.solver.SolverConfigTest$DummyMoveListFactory",
        "ai.timefold.solver.core.api.solver.phase.PhaseCommand",
    ] {
        assert!(refs.contains(&class), "missing {class}");
    }
}
