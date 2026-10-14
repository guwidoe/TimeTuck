// Adapted Community TESTs. Apache-2.0; see fixtures/validation_catalog/NOTICE.txt.
#[path = "fixtures/validation_catalog_fixture.rs"]
mod fixture;
use fixture::*;
use serde_json::{Value, json};
use timetuck::*;
#[test]
fn validate() {
    let runner = ValidatedModelRunner::new(catalog(), validator);
    let result = runner.validate(&(), &()).unwrap();
    assert_eq!(result.status, ServiceValidationStatus::Errors);
    assert_eq!(result.issues.len(), 1);
    let issue = &result.issues[0];
    assert_eq!(issue.code(), "MalformedEntityIssue");
    assert_eq!(issue.severity(), ValidationSeverity::Error);
    assert_eq!(result.to_json()["issues"][0]["id"], "test-id");
}
#[test]
fn validation_issue_type_catalog_registered() {
    let catalog = catalog();
    let descriptions: Vec<_> = catalog.descriptions().collect();
    assert_eq!(descriptions.len(), 1);
    assert_eq!(descriptions[0], &description());
    assert_eq!(catalog.description_for::<TestIssue>(), Some(&description()));
}
#[test]
fn validation_issue_types_written_in_descriptor() {
    let directory = std::env::temp_dir().join(format!(
        "timetuck-validation-catalog-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let file = directory.join("validation-issue-types.json");
    catalog().write_descriptor(&file).unwrap();
    let issue_types: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(issue_types.as_array().unwrap().len(), 1);
    assert_eq!(issue_types[0]["code"], "MalformedEntityIssue");
    assert_eq!(issue_types[0]["severity"], "ERROR");
    assert_eq!(
        issue_types[0]["metadata"],
        json!([{"message":"description","type":"Message"}])
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn validation_issue_types_duplicate_detected() {
    let mut catalog = ValidationTypeCatalog::new();
    let mut issue_type = description();
    issue_type.code = "DuplicateIssueCode".into();
    issue_type.metadata.clear();
    catalog.register::<TestIssue>(issue_type.clone()).unwrap();
    let error = catalog
        .register::<DuplicateTestIssue>(issue_type)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("IssueCode (DuplicateIssueCode) was found in 2 types")
    );
    assert_eq!(catalog.descriptions().len(), 1);
}
