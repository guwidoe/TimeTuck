// Original runtime controls; no upstream-method credit.
use serde_json::{Value, json};
use timetuck::*;
#[test]
fn real_model_failures_escape_payloads_and_keep_owned_summary_snapshots() {
    let error = FiniteProblem::<i32>::new(vec![Variable::new(vec![1, 1])]).unwrap_err();
    let detail = error.to_string();
    let result = ServiceValidationBuilder::new()
        .add_issue(
            ServiceValidationIssue::new(
                "DuplicateValue",
                ValidationSeverity::Error,
                vec![
                    ("message".into(), json!(detail)),
                    (
                        "key\"\n".into(),
                        json!({"nested": ["quote\"slash\\newline\n", null, 3]}),
                    ),
                ],
            )
            .unwrap(),
        )
        .build();
    let parsed: Value = serde_json::from_str(&result.to_pretty_json()).unwrap();
    assert_eq!(parsed["issues"][0]["message"], detail);
    assert_eq!(
        parsed["issues"][0]["key\"\n"]["nested"][0],
        "quote\"slash\\newline\n"
    );
    assert_eq!(result.to_json(), parsed);
    let summary = ServiceValidationSummary::of(&result);
    assert_eq!(
        summary.error_counts_by_code.as_ref().unwrap()["DuplicateValue"],
        1
    );
    let mut edited = result.clone();
    edited.issues.clear();
    assert_eq!(summary.error_count, Some(1));
    assert_eq!(ServiceValidationSummary::of(&edited).error_count, Some(0));
    let legacy: Value = serde_json::from_str(&result.to_legacy_pretty_json()).unwrap();
    assert_eq!(
        legacy["errors"][0],
        "Validation error of code (DuplicateValue) occurred."
    );
}
#[test]
fn severity_precedence_and_payload_rejection_are_independent_of_issue_order() {
    let make = |code: &str, severity| ServiceValidationIssue::new(code, severity, vec![]).unwrap();
    assert_eq!(
        ServiceValidationBuilder::new().build().status,
        ServiceValidationStatus::Ok
    );
    assert_eq!(
        ServiceValidationBuilder::new()
            .add_issue(make("W", ValidationSeverity::Warning))
            .build()
            .status,
        ServiceValidationStatus::Warnings
    );
    for reverse in [false, true] {
        let mut issues = vec![
            make("same", ValidationSeverity::Error),
            make("same", ValidationSeverity::Warning),
            make("same", ValidationSeverity::Error),
        ];
        if reverse {
            issues.reverse();
        }
        let result = issues
            .into_iter()
            .fold(ServiceValidationBuilder::new(), |b, i| b.add_issue(i))
            .build();
        assert_eq!(result.status, ServiceValidationStatus::Errors);
        let summary = ServiceValidationSummary::of(&result);
        assert_eq!(summary.error_count, Some(2));
        assert_eq!(summary.warning_count, Some(1));
        assert_eq!(summary.error_counts_by_code.unwrap()["same"], 2);
        assert_eq!(summary.warning_counts_by_code.unwrap()["same"], 1);
    }
    for key in ["code", "severity", "metadata"] {
        assert!(
            ServiceValidationIssue::new(
                "bad",
                ValidationSeverity::Error,
                vec![(key.into(), json!(1))]
            )
            .is_err()
        );
    }
    assert!(
        ServiceValidationIssue::new(
            "bad",
            ValidationSeverity::Warning,
            vec![("x".into(), json!(1)), ("x".into(), json!(2))]
        )
        .is_err()
    );
}
#[test]
fn maturity_alias_is_canonical_and_future_labels_round_trip() {
    assert_eq!(
        ServiceModelMaturity::from_json("\"Example\"")
            .unwrap()
            .to_json(),
        json!("Template")
    );
    for label in ["custom", "quote\"\n", ""] {
        let source = serde_json::to_string(label).unwrap();
        assert_eq!(
            ServiceModelMaturity::from_json(&source).unwrap().to_json(),
            json!(label)
        );
    }
    for bad in ["null", "[]", "{}", "1", "true", "not-json"] {
        assert!(ServiceModelMaturity::from_json(bad).is_err());
    }
}
