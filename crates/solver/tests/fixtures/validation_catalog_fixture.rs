// Adapted Community TEST helper. Apache-2.0; see sibling NOTICE.txt.
use serde_json::json;
use timetuck::*;
pub struct TestIssue;
pub struct DuplicateTestIssue;
pub fn description() -> ValidationTypeDescription {
    ValidationTypeDescription {
        code: "MalformedEntityIssue".into(),
        severity: ValidationSeverity::Error,
        metadata: vec![json!({"message":"description","type":"Message"})],
    }
}
pub fn catalog() -> ValidationTypeCatalog {
    let mut catalog = ValidationTypeCatalog::new();
    catalog.register::<TestIssue>(description()).unwrap();
    catalog
}
pub fn validator(_: &(), _: &()) -> Result<ServiceValidationResult, ServiceValidationError> {
    Ok(ServiceValidationBuilder::new()
        .add_issue(ServiceValidationIssue::new(
            "MalformedEntityIssue",
            ValidationSeverity::Error,
            vec![("id".into(), json!("test-id"))],
        )?)
        .build())
}
