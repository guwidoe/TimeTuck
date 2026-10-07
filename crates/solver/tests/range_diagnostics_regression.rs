//! Original diagnostics regressions; no upstream method credit.
use std::{error::Error, ptr};
use timetuck::*;

#[test]
fn basic_diagnostics_borrow_nonclone_payloads_and_preserve_partial_and_pin_semantics() {
    #[derive(Debug, Eq, PartialEq)]
    struct Value(u8);
    let problem = FiniteProblem::new(vec![
        Variable::new(vec![Value(2), Value(8)]).allowing_unassigned(),
        Variable::new(vec![Value(4)])
            .with_initial(Value(4))
            .with_pinned(true),
    ])
    .unwrap();
    let a = Assignment::new(vec![Some(Value(5)), Some(Value(4))]);
    let error = validate_basic_with_diagnostics(&problem, &a).unwrap_err();
    assert_eq!(error.cause, ModelError::ValueOutsideDomain { variable: 0 });
    assert!(
        error
            .source()
            .unwrap()
            .downcast_ref::<ModelError>()
            .is_some()
    );
    let detail = error.violation.unwrap();
    assert!(ptr::eq(detail.value, a.values()[0].as_ref().unwrap()));
    assert!(ptr::eq(detail.allowed, problem.variables()[0].domain()));
    assert_eq!(
        detail.location,
        RangeViolationLocation::Basic { variable: 0 }
    );
    assert_eq!(
        detail.describe("worker-A", "task", |v| v.0.to_string()),
        "entity worker-A, variable task: value 5 outside allowed range [2, 8]; unassigned allowed"
    );
    assert!(
        validate_basic_with_diagnostics(&problem, &Assignment::new(vec![None, Some(Value(4))]))
            .is_ok()
    );
    let a = Assignment::new(vec![None, None]);
    let e = validate_basic_with_diagnostics(&problem, &a).unwrap_err();
    assert_eq!(e.cause, ModelError::PinnedValueChanged { variable: 1 });
    assert!(e.violation.is_none());
    let a = Assignment::new(vec![Some(Value(99))]);
    let e = validate_basic_with_diagnostics(&problem, &a).unwrap_err();
    assert_eq!(
        e.cause,
        ModelError::AssignmentLength {
            expected: 2,
            actual: 1
        }
    );
    assert!(e.violation.is_none());
}

#[test]
fn list_diagnostics_distinguish_registry_owner_ranges_and_pins() {
    let p = ListProblem::new(
        vec![10, 20, 30],
        vec![
            ListEntity::new(vec![10])
                .with_pinned_prefix(1)
                .with_value_range(vec![10, 20]),
            ListEntity::new(vec![]).with_value_range(vec![]),
        ],
    )
    .unwrap()
    .allowing_unassigned();
    for (a, cause, position, allowed) in [
        (
            vec![vec![10, 30], vec![]],
            ListError::ValueOutsideEntityRange { entity: 0 },
            ListPosition {
                entity: 0,
                index: 1,
            },
            vec![10, 20],
        ),
        (
            vec![vec![10], vec![20]],
            ListError::ValueOutsideEntityRange { entity: 1 },
            ListPosition {
                entity: 1,
                index: 0,
            },
            vec![],
        ),
        (
            vec![vec![10], vec![99]],
            ListError::ValueOutsideDomain,
            ListPosition {
                entity: 1,
                index: 0,
            },
            vec![10, 20, 30],
        ),
    ] {
        let a = ListAssignment::new(a);
        let e = validate_list_with_diagnostics(&p, &a).unwrap_err();
        assert_eq!(e.cause, cause);
        let v = e.violation.unwrap();
        assert_eq!(v.location, RangeViolationLocation::List { position });
        assert_eq!(v.allowed, allowed);
        assert!(v.allows_unassigned);
        assert!(ptr::eq(
            v.value,
            &a.lists()[position.entity][position.index]
        ));
    }
    for (a, cause) in [
        (
            vec![vec![30], vec![99]],
            ListError::PinChanged { entity: 0 },
        ),
        (
            vec![vec![10, 10], vec![99]],
            ListError::DuplicateAssignedValue,
        ),
        (
            vec![vec![99]],
            ListError::ListCount {
                expected: 2,
                actual: 1,
            },
        ),
    ] {
        let a = ListAssignment::new(a);
        let e = validate_list_with_diagnostics(&p, &a).unwrap_err();
        assert_eq!(e.cause, cause);
        assert!(e.violation.is_none());
    }
}

#[test]
fn mixed_diagnostics_keep_distinct_types_and_component_error_precedence() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap(),
        ListProblem::new(vec!["a", "b"], vec![ListEntity::new(vec![])]).unwrap(),
    );
    let bad = MixedAssignment::new(
        Assignment::new(vec![Some(3)]),
        ListAssignment::new(vec![vec!["c"]]),
    );
    let e = validate_mixed_with_diagnostics(&p, &bad).unwrap_err();
    assert!(e.source().unwrap().downcast_ref::<ModelError>().is_some());
    let MixedRangeValidationError::Basic(e) = e else {
        panic!("basic first")
    };
    assert_eq!(*e.violation.unwrap().value, 3);
    let fixed_basic = MixedAssignment::new(Assignment::new(vec![None]), bad.lists().clone());
    let e = validate_mixed_with_diagnostics(&p, &fixed_basic).unwrap_err();
    assert!(e.source().unwrap().downcast_ref::<ListError>().is_some());
    let MixedRangeValidationError::Lists(e) = e else {
        panic!("list second")
    };
    assert_eq!(*e.violation.unwrap().value, "c");
    assert_eq!(bad.basic().values(), &[Some(3)]);
    assert_eq!(bad.lists().lists(), &[vec!["c"]]);
}

#[test]
fn every_small_list_candidate_retains_the_ordinary_validation_result() {
    let p = ListProblem::new(
        vec![0, 1, 2],
        vec![
            ListEntity::new(vec![]).with_value_range(vec![0, 2]),
            ListEntity::new(vec![]).with_value_range(vec![1, 2]),
        ],
    )
    .unwrap();
    let mut rows = vec![vec![]];
    for a in 0..4 {
        rows.push(vec![a]);
        for b in 0..4 {
            rows.push(vec![a, b]);
        }
    }
    let mut checked = 0;
    for left in &rows {
        for right in &rows {
            let a = ListAssignment::new(vec![left.clone(), right.clone()]);
            let regular = p.validate(&a);
            let diagnostic = validate_list_with_diagnostics(&p, &a);
            assert_eq!(
                regular,
                diagnostic.as_ref().map(|_| ()).map_err(|e| e.cause.clone())
            );
            if let Err(e) = diagnostic {
                let range_error = matches!(
                    e.cause,
                    ListError::ValueOutsideDomain | ListError::ValueOutsideEntityRange { .. }
                );
                assert_eq!(e.violation.is_some(), range_error);
                if let Some(v) = e.violation {
                    assert!(!v.allowed.contains(v.value));
                    let RangeViolationLocation::List { position } = v.location else {
                        panic!("list position")
                    };
                    assert!(ptr::eq(
                        v.value,
                        &a.lists()[position.entity][position.index]
                    ));
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 441);
}
