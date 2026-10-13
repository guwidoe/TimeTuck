// Adapted pinned Timefold Community TESTS, Apache-2.0; compat/provenance.json.
use serde_json::json;
use std::collections::BTreeMap;
use timetuck::*;
fn issue(code: &str, severity: ValidationSeverity) -> ServiceValidationIssue {
    ServiceValidationIssue::new(code, severity, vec![]).unwrap()
}
fn malformed() -> ServiceValidationIssue {
    ServiceValidationIssue::new(
        "MalformedEntityIssue",
        ValidationSeverity::Error,
        vec![("entityId".into(), json!("entityId"))],
    )
    .unwrap()
}
#[test]
fn serialize_validation_result() {
    let result = ServiceValidationBuilder::new()
        .add_issue(malformed())
        .build();
    assert_eq!(
        result.to_pretty_json(),
        "{\n  \"status\" : \"ERRORS\",\n  \"issues\" : [ {\n    \"code\" : \"MalformedEntityIssue\",\n    \"severity\" : \"ERROR\",\n    \"entityId\" : \"entityId\"\n  } ]\n}"
    );
}
#[test]
fn serialize_legacy_validation_result_without_legacy_message() {
    let result = ServiceValidationBuilder::new()
        .add_issue(malformed())
        .build();
    assert_eq!(
        result.to_legacy_pretty_json(),
        "{\n  \"summary\" : \"ERRORS\",\n  \"errors\" : [ \"Validation error of code (MalformedEntityIssue) occurred.\" ]\n}"
    );
}
#[test]
fn serialize_legacy_validation_result() {
    let result = ServiceValidationBuilder::new()
        .add_issue(malformed().with_legacy_message("Entity (entityId) is malformed."))
        .build();
    assert_eq!(
        result.to_legacy_pretty_json(),
        "{\n  \"summary\" : \"ERRORS\",\n  \"errors\" : [ \"Entity (entityId) is malformed.\" ]\n}"
    );
}
#[test]
fn serialize_validation_issue_type() {
    let kind = ServiceValidationIssueType {
        code: "MalformedEntityIssue".into(),
        severity: ValidationSeverity::Error,
        message: "Entity (entityId) is malformed.".into(),
    };
    assert_eq!(
        kind.to_pretty_json(),
        "{\n  \"code\" : \"MalformedEntityIssue\",\n  \"severity\" : \"ERROR\",\n  \"metadata\" : [ {\n    \"message\" : \"Entity (entityId) is malformed.\",\n    \"type\" : \"Message\"\n  } ]\n}"
    );
}
#[test]
fn of_counts_issues_by_code_and_severity() {
    let result = ServiceValidationResult {
        status: ServiceValidationStatus::Errors,
        issues: vec![
            issue("MISSING_SKILL", ValidationSeverity::Error),
            issue("MISSING_SKILL", ValidationSeverity::Error),
            issue("UNUSED_VEHICLE", ValidationSeverity::Warning),
        ],
    };
    let summary = ServiceValidationSummary::of(&result);
    assert_eq!(summary.status, ServiceValidationStatus::Errors);
    assert_eq!(summary.error_count, Some(2));
    assert_eq!(summary.warning_count, Some(1));
    assert_eq!(
        summary.error_counts_by_code,
        Some(BTreeMap::from([("MISSING_SKILL".into(), 2)]))
    );
    assert_eq!(
        summary.warning_counts_by_code,
        Some(BTreeMap::from([("UNUSED_VEHICLE".into(), 1)]))
    );
}
#[test]
fn of_with_no_issues_reports_zero_counts() {
    let summary = ServiceValidationSummary::of(&ServiceValidationResult {
        status: ServiceValidationStatus::Ok,
        issues: vec![],
    });
    assert_eq!(summary.status, ServiceValidationStatus::Ok);
    assert_eq!(summary.error_count, Some(0));
    assert_eq!(summary.warning_count, Some(0));
    assert!(summary.error_counts_by_code.unwrap().is_empty());
    assert!(summary.warning_counts_by_code.unwrap().is_empty());
}
#[test]
fn of_when_validation_not_supported_carries_status_only() {
    let summary = ServiceValidationSummary::of(&ServiceValidationResult {
        status: ServiceValidationStatus::NotSupported,
        issues: vec![],
    });
    assert_eq!(summary.status, ServiceValidationStatus::NotSupported);
    assert_eq!(summary.error_count, None);
    assert_eq!(summary.warning_count, None);
    assert_eq!(summary.error_counts_by_code, None);
    assert_eq!(summary.warning_counts_by_code, None);
}
#[test]
fn deserialize_example_as_template() {
    assert_eq!(
        ServiceModelMaturity::from_json("\"Example\"").unwrap(),
        ServiceModelMaturity::Template
    );
}
#[test]
fn deserialize_template_as_template() {
    assert_eq!(
        ServiceModelMaturity::from_json("\"Template\"").unwrap(),
        ServiceModelMaturity::Template
    );
}
