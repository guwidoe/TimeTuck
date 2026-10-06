// Original development-protocol regressions; no upstream coverage credit.
#[path = "../examples/solver_support/mod.rs"]
mod solver_support;
use solver_support::Case;
use timetuck::{HardSoftScore, exhaustive_search};

#[test]
fn extended_domains_optional_values_and_pins_preserve_the_assignment_space() {
    let case = Case::parse(
        "mixed\t2\t-,-,-\t001\t0:2\t5,9,0,3,0,0\t;1;0:1\t101\t4,0,-2",
        false,
    )
    .unwrap();
    assert_eq!(case.id, "mixed");
    let vars = case.problem.variables();
    assert!(vars[0].allows_unassigned());
    assert!(vars[0].domain().is_empty());
    assert_eq!(vars[1].domain(), &[1]);
    assert!(!vars[1].allows_unassigned());
    assert!(vars[2].is_pinned());
    assert!(vars[2].initial().is_none());
    assert_eq!(case.problem.search_space_size(), Some(1));
    let result = exhaustive_search(&case.problem, |state| case.score(state), 1);
    assert!(result.is_optimal());
    let best = result.best.unwrap();
    assert_eq!(best.assignment.values(), &[None, Some(1), None]);
    // Null/null edges do not conflict. Null preferences still contribute.
    assert_eq!(best.score, HardSoftScore::new(0, 5));
}

#[test]
fn supplied_initial_values_are_validated_then_reset_or_retained_explicitly() {
    let line = "initial\t3\t2,0\t01\t\t0,0,9,5,0,0\t2:1;0\t10\t-4,0";
    let reset = Case::parse(line, false).unwrap();
    let retained = Case::parse(line, true).unwrap();
    assert_eq!(
        reset.problem.initial_assignment().values(),
        &[None, Some(0)]
    );
    assert_eq!(
        retained.problem.initial_assignment().values(),
        &[Some(2), Some(0)]
    );
    assert_eq!(reset.problem.search_space_size(), Some(3));
    assert_eq!(
        reset.score(&reset.problem.initial_assignment()),
        HardSoftScore::new(0, 1)
    );
    assert_eq!(
        retained.score(&retained.problem.initial_assignment()),
        HardSoftScore::new(0, 14)
    );
}

#[test]
fn malformed_extended_metadata_cannot_silently_change_the_problem() {
    let valid = "case\t2\t-\t0\t\t1,2\t0:1\t1\t3";
    for (index, value) in [
        (6, "0;1"),
        (6, "0:0"),
        (6, "0:2"),
        (7, ""),
        (7, "2"),
        (8, "0,1"),
        (8, "NaN"),
        (3, "2"),
    ] {
        let mut fields: Vec<_> = valid.split('\t').collect();
        fields[index] = value;
        assert!(
            Case::parse(&fields.join("\t"), false).is_err(),
            "field {index}: {value}"
        );
    }
    assert!(Case::parse("bad\t2\t1\t0\t\t1,2\t0\t1\t0", false).is_err());
}

#[test]
fn legacy_six_field_cases_keep_required_homogeneous_ranges() {
    let case = Case::parse("legacy\t2\t1,0\t10\t0:1\t0,3,4,0", false).unwrap();
    assert_eq!(case.problem.initial_assignment().values(), &[Some(1), None]);
    assert!(
        case.problem
            .variables()
            .iter()
            .all(|v| !v.allows_unassigned() && v.domain() == [0, 1])
    );
    let result = exhaustive_search(&case.problem, |state| case.score(state), 2);
    assert!(result.is_optimal());
    assert_eq!(result.best.unwrap().score, HardSoftScore::new(0, 7));
}

#[test]
fn required_unassigned_pin_is_valid_partial_input_with_no_complete_result() {
    let case = Case::parse("partial\t2\t-\t1\t\t1,2\t0\t0\t0", false).unwrap();
    assert!(case.problem.variables()[0].is_pinned());
    assert!(!case.problem.variables()[0].allows_unassigned());
    assert!(!case.problem.is_complete(&case.problem.initial_assignment()));
    assert_eq!(case.problem.search_space_size(), Some(0));
    let result = exhaustive_search(
        &case.problem,
        |_| -> HardSoftScore { panic!("no complete assignment may reach scoring") },
        10,
    );
    assert!(result.best.is_none());
    assert_eq!(result.evaluations, 0);
    assert!(!result.is_optimal());
}
