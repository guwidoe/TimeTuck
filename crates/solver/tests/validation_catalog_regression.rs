//! Original controls for live input/configuration, real solving and rejected execution.
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use timetuck::*;
struct RangeIssue;
struct LargeRangeWarning;
fn catalog() -> ValidationTypeCatalog {
    let mut catalog = ValidationTypeCatalog::new();
    for (code, severity, warning) in [
        ("RangeIssue", ValidationSeverity::Error, false),
        ("LargeRange", ValidationSeverity::Warning, true),
    ] {
        let description = ValidationTypeDescription {
            code: code.into(),
            severity,
            metadata: vec![json!({"message":"domain policy","type":"Message"})],
        };
        if warning {
            catalog.register::<LargeRangeWarning>(description).unwrap();
        } else {
            catalog.register::<RangeIssue>(description).unwrap();
        }
    }
    catalog
}
#[test]
fn live_validator_gates_real_dense_and_stream_solves_and_preserves_warnings() {
    let calls = AtomicUsize::new(0);
    let runner = ValidatedModelRunner::new(catalog(), |values: &Vec<i32>, maximum: &i32| {
        let mut builder = ServiceValidationBuilder::new();
        if values.is_empty() || values.iter().any(|value| value > maximum) {
            builder = builder.add_issue(ServiceValidationIssue::new(
                "RangeIssue",
                ValidationSeverity::Error,
                vec![("maximum".into(), json!(maximum))],
            )?);
        } else if values.len() > 3 {
            builder = builder.add_issue(ServiceValidationIssue::new(
                "LargeRange",
                ValidationSeverity::Warning,
                vec![("size".into(), json!(values.len()))],
            )?);
        }
        Ok(builder.build())
    });
    for stream in [false, true] {
        for maximum in [3, 7] {
            let values = (0..=maximum).collect();
            let outcome = runner
                .run(&values, &maximum, |values, limit| -> Result<_, String> {
                    calls.fetch_add(1, Ordering::SeqCst);
                    assert!(values.iter().all(|v| v <= limit));
                    let problem = FiniteProblem::new(vec![
                        Variable::new(values.clone()),
                        Variable::new(values.clone()),
                    ])
                    .unwrap();
                    let config = SolverSequence {
                        phases: vec![SolverPhase::Exact(ExactPhaseConfig::default())],
                        ..Default::default()
                    };
                    let result = if stream {
                        solve_stream(
                            &problem,
                            || {
                                let mut builder = StreamGraphBuilder::new();
                                let source = builder.source::<usize, (usize, Option<i32>)>();
                                builder.score("value", source.port(), |(_, v)| {
                                    SimpleScore::new(i64::from(v.unwrap_or(0)))
                                })?;
                                Ok((builder.build(), source))
                            },
                            config,
                        )
                        .map_err(|e| e.to_string())?
                    } else {
                        let mut network = ConstraintNetwork::new(2);
                        for index in 0..2 {
                            network
                                .add_constraint(format!("value{index}"), vec![index], |values| {
                                    SimpleScore::new(i64::from(*values[0].unwrap_or(&0)))
                                })
                                .unwrap();
                        }
                        solve(&problem, &network, config).map_err(|e| e.to_string())?
                    };
                    problem.validate(&result.assignment).unwrap();
                    let sum: i64 = result
                        .assignment
                        .values()
                        .iter()
                        .map(|v| i64::from(v.unwrap()))
                        .sum();
                    assert_eq!(result.score, Some(SimpleScore::new(sum)));
                    assert_eq!(sum, 2 * i64::from(*limit));
                    Ok(result.assignment)
                })
                .unwrap();
            assert_eq!(outcome.validation.status, ServiceValidationStatus::Warnings);
            assert_eq!(outcome.validation.issues[0].code(), "LargeRange");
            assert_eq!(outcome.output.values(), &[Some(maximum); 2]);
            assert_eq!(values, (0..=maximum).collect::<Vec<_>>());
        }
        let rejected = runner.run(&vec![0, 9], &3, |_, _| -> Result<(), String> {
            panic!("invalid model reached native solve")
        });
        assert!(
            matches!(rejected, Err(ValidatedModelRunError::Rejected(ref report)) if report.issues.len() == 1 && report.issues[0].code() == "RangeIssue")
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 4);
}
#[test]
fn unknown_misclassified_or_failed_validation_never_runs_execution() {
    for (code, severity, status) in [
        (
            "Unknown",
            ValidationSeverity::Error,
            ServiceValidationStatus::Errors,
        ),
        (
            "RangeIssue",
            ValidationSeverity::Warning,
            ServiceValidationStatus::Warnings,
        ),
        (
            "RangeIssue",
            ValidationSeverity::Error,
            ServiceValidationStatus::Ok,
        ),
    ] {
        let runner = ValidatedModelRunner::new(catalog(), move |_: &(), _: &()| {
            Ok(ServiceValidationResult {
                status,
                issues: vec![ServiceValidationIssue::new(code, severity, vec![])?],
            })
        });
        assert!(matches!(
            runner.run(&(), &(), |_, _| -> Result<(), String> {
                panic!("invalid validation contract reached execution")
            }),
            Err(ValidatedModelRunError::Validation(_))
        ));
    }
    let runner = ValidatedModelRunner::new(catalog(), |_: &(), _: &()| {
        Err(ServiceValidationError("validator failed".into()))
    });
    assert!(
        matches!(runner.run(&(), &(), |_, _| -> Result<(), String> { panic!("failed validator reached execution") }), Err(ValidatedModelRunError::Validation(ServiceValidationError(message))) if message == "validator failed")
    );
}
#[test]
fn empty_valid_catalog_and_execution_error_keep_actual_result_boundaries() {
    let runner = ValidatedModelRunner::new(
        ValidationTypeCatalog::new(),
        |model: &usize, config: &usize| {
            assert_eq!(*model + *config, 9);
            Ok(ServiceValidationBuilder::new().build())
        },
    );
    let result = runner
        .run(&4, &5, |model, config| Ok::<_, String>(*model * *config))
        .unwrap();
    assert_eq!(result.output, 20);
    assert_eq!(result.validation.status, ServiceValidationStatus::Ok);
    assert!(matches!(
        runner.run(&4, &5, |_, _| Err::<(), _>("actual execution error")),
        Err(ValidatedModelRunError::Execution("actual execution error"))
    ));
}
