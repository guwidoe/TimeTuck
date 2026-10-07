//! Original constructor integration and failure-priority regressions.
use std::ptr;
use timetuck::*;

#[test]
fn returned_models_outlive_input_and_run_all_three_solver_paths() {
    let (b, l, m) = {
        let variables = [Variable::new(vec![7, 9])];
        let values = ["x", "y"];
        let entities = [ListEntity::new(vec![]).with_value_range(values.to_vec())];
        (
            build_basic_with_diagnostics(&variables).unwrap(),
            build_list_with_diagnostics(&values, &entities, false).unwrap(),
            build_mixed_with_diagnostics(&variables, &values, &entities, false).unwrap(),
        )
    };
    let limits = SolverLimits {
        max_evaluations: 32,
        ..Default::default()
    };
    let config = SolverConfig::<SimpleScore> {
        limits,
        ..Default::default()
    };
    let rb = solve(&b, &ConstraintNetwork::new(1), config.clone()).unwrap();
    let rl = solve_list(&l, &ListConstraintNetwork::new(1, 2), config).unwrap();
    let rm = solve_mixed(
        &m,
        &MixedConstraintNetwork::new(1, 1, 2),
        MixedSolverConfig::<SimpleScore> {
            limits,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(rb.complete && rl.complete && rm.complete);
    b.validate(&rb.assignment).unwrap();
    l.validate(&rl.assignment).unwrap();
    m.validate(&rm.assignment).unwrap();
    assert_eq!(rb.score, Some(SimpleScore::new(0)));
    assert_eq!(rl.score, rb.score);
    assert_eq!(rm.score, rb.score);
}

#[test]
fn diagnostics_borrow_original_inputs_and_keep_metadata_error_priority() {
    let variables = [Variable::new(vec![10, 20]).with_initial(30)];
    let e = build_basic_with_diagnostics(&variables).unwrap_err();
    let v = e.violation.unwrap();
    assert!(ptr::eq(v.value, variables[0].initial().unwrap()));
    assert!(ptr::eq(v.allowed, variables[0].domain()));
    let values = [10, 20];
    let entities = [ListEntity::new(vec![20]).with_value_range(vec![10])];
    let e = build_list_with_diagnostics(&values, &entities, true).unwrap_err();
    assert_eq!(e.cause, ListError::ValueOutsideEntityRange { entity: 0 });
    let v = e.violation.unwrap();
    assert!(ptr::eq(v.value, &entities[0].initial()[0]));
    assert!(ptr::eq(v.allowed, entities[0].value_range().unwrap()));
    assert!(v.allows_unassigned);
    for (entities, cause) in [
        (
            vec![ListEntity::new(vec![99]).with_value_range(vec![99])],
            ListError::EntityRangeValueOutsideDomain { entity: 0 },
        ),
        (
            vec![ListEntity::new(vec![99]).with_pinned_prefix(2)],
            ListError::InvalidPin { entity: 0 },
        ),
    ] {
        let e = build_list_with_diagnostics(&values, &entities, false).unwrap_err();
        assert_eq!(e.cause, cause);
        assert!(e.violation.is_none());
        assert_eq!(entities[0].initial(), &[99]);
    }
    let variables = [Variable::new(vec![10, 10]).with_initial(99)];
    let e = build_basic_with_diagnostics(&variables).unwrap_err();
    assert_eq!(e.cause, ModelError::DuplicateValue { variable: 0 });
    assert!(e.violation.is_none());
}

#[test]
fn optionality_pins_and_empty_domains_survive_successful_construction() {
    let variables = [Variable::<i32>::new(vec![])
        .allowing_unassigned()
        .with_pinned(true)];
    let values = [1, 2];
    let entities = [
        ListEntity::new(vec![1])
            .with_pinned_prefix(1)
            .with_value_range(vec![1]),
        ListEntity::new(vec![])
            .with_pinned(true)
            .with_value_range(vec![]),
    ];
    let p = build_mixed_with_diagnostics(&variables, &values, &entities, true).unwrap();
    assert!(p.basic().variables()[0].is_pinned());
    assert!(p.basic().variables()[0].allows_unassigned());
    assert!(p.lists().allows_unassigned());
    assert_eq!(p.lists().entities()[0].pinned_prefix(), 1);
    assert!(p.lists().entities()[1].is_pinned());
    assert_eq!(p.lists().entities()[1].value_range(), Some([].as_slice()));
    p.validate(&p.initial_assignment()).unwrap();
    assert!(p.lists().is_complete(&p.lists().initial_assignment()));
}
