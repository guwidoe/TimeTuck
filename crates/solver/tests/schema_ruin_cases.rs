// Adapted from pinned Community DefaultSolverTest; Apache-2.0.
// All asserted diagnostics and full 2/2/2 fixtures retained. See provenance.
#[path = "fixtures/schema_ruin_fixture.rs"]
mod fixture;
use timetuck::*;
fn failed(multi: bool, variable: Option<&str>) -> FactoryAssemblyError<std::io::Error> {
    let d = fixture::Domain::new(multi, false);
    assert_eq!(d.model.basic().variables().len(), 4);
    assert_eq!(d.model.lists().entities().len(), 2);
    assert_eq!(d.model.lists().values().len(), 2);
    let error = d
        .declaration()
        .assemble_solver_factory(d.configuration(variable, 0), fixture::shape())
        .err()
        .unwrap();
    assert_eq!(d.preparations.get(), 0);
    error
}
#[test]
fn fail_ruin_recreate_with_multi_var() {
    let error = failed(false, None);
    assert!(matches!(
        error,
        FactoryAssemblyError::SchemaRuin {
            phase: 0,
            error: SchemaRuinError::AmbiguousVariables { count: 3, .. }
        }
    ));
    let message = error.to_string();
    for fragment in [
        "The entity class",
        "TestdataMixedEntity",
        "contains several variables",
        "it cannot be deduced automatically.",
        "Maybe set the property variableName",
    ] {
        assert!(message.contains(fragment), "{message}");
    }
}
#[test]
fn fail_ruin_recreate_with_bad_var() {
    let error = failed(false, Some("badVariable"));
    assert!(matches!(
        error,
        FactoryAssemblyError::SchemaRuin {
            phase: 0,
            error: SchemaRuinError::UnknownVariable { .. }
        }
    ));
    let message = error.to_string();
    for fragment in [
        "The entity class",
        "TestdataMixedEntity",
        "has no variable named badVariable",
    ] {
        assert!(message.contains(fragment), "{message}");
    }
}
#[test]
fn fail_ruin_recreate_with_multi_entity_multi_var() {
    let error = failed(true, None);
    assert!(matches!(
        error,
        FactoryAssemblyError::SchemaRuin {
            phase: 0,
            error: SchemaRuinError::AmbiguousEntities { count: 2 }
        }
    ));
    let message = error.to_string();
    for fragment in [
        "has no entityClass configured and because there are multiple in the entityClassSet",
        "it cannot be deduced automatically",
    ] {
        assert!(message.contains(fragment), "{message}");
    }
}
