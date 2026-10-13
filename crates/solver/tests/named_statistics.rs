//! Original metadata validation and assignment-space regressions.
use num_bigint::BigUint;
use timetuck::*;
#[test]
fn fixed_point_statistic_preserves_rounding_and_saturation_boundaries() {
    for (log, expected) in [
        (0.0000004, 0),
        (0.0000005, 1),
        (0.0000006, 1),
        (-0.0000005, 0),
        (f64::NAN, 0),
        (f64::NEG_INFINITY, i64::MIN),
        (f64::INFINITY, i64::MAX),
    ] {
        assert_eq!(ProblemScale::from_log10(log).fixed_point_log(), expected);
    }
    for scaled in [4_503_599_627_370_497_f64, -4_503_599_627_370_497_f64] {
        let log = scaled / ProblemScale::FIXED_POINT_PRECISION as f64;
        let actual_scaled = log * ProblemScale::FIXED_POINT_PRECISION as f64;
        assert_eq!(
            ProblemScale::from_log10(log).fixed_point_log(),
            actual_scaled as i64
        );
    }
    assert_eq!(ProblemScale::from_log10(-0.5).scaled_log(1), -1);
}
#[test]
fn mixed_entities_are_counted_once_and_each_native_slot_is_accounted_for() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            Variable::new(vec![0, 1]),
            Variable::new(vec![10, 11, 12]),
            Variable::new(vec![0, 1]),
            Variable::new(vec![10, 11, 12]),
        ])
        .unwrap(),
        ListProblem::new(vec![100, 101, 102, 103], vec![ListEntity::new(vec![]); 2]).unwrap(),
    );
    let schema = vec![StatisticsEntityType {
        name: "Job".into(),
        entity_count: 2,
        variables: vec![
            StatisticsVariable::basic("machine", vec![0, 2]),
            StatisticsVariable::basic("shift", vec![1, 3]),
            StatisticsVariable::list("visits", vec![0, 1]),
        ],
    }];
    let s = NamedProblemSize::from_mixed(&p, &schema).unwrap();
    assert_eq!(
        (s.entity_count, s.variable_count, s.approximate_value_count),
        (2, 6, 9)
    );
    assert_eq!(s.assignment_space.decimal(), "4320");
    assert!(s.assignment_space.exact);
    for (name, n) in [("machine", 2), ("shift", 3), ("visits", 4)] {
        assert_eq!(s.values_by_type_variable["Job"][name], n);
    }
    let mut bad = schema.clone();
    bad[0].variables[0].slots[1] = 0;
    assert!(matches!(
        NamedProblemSize::from_mixed(&p, &bad),
        Err(StatisticsSchemaError::DuplicateSlot { slot: 0, .. })
    ));
    let mut bad = schema.clone();
    bad[0].variables.pop();
    assert!(matches!(
        NamedProblemSize::from_mixed(&p, &bad),
        Err(StatisticsSchemaError::UnboundSlots {
            kind: StatisticsVariableKind::List,
            ..
        })
    ));
    let mut bad = schema.clone();
    bad[0].variables[0].slots[1] = 10;
    assert!(matches!(
        NamedProblemSize::from_mixed(&p, &bad),
        Err(StatisticsSchemaError::InvalidSlot { slot: 10, .. })
    ));
    let mut bad = schema.clone();
    bad[0].variables[0].slots.pop();
    assert!(matches!(
        NamedProblemSize::from_mixed(&p, &bad),
        Err(StatisticsSchemaError::SlotCount { .. })
    ));
    let mut bad = schema;
    bad[0].variables[1].name = "machine".into();
    assert!(matches!(
        NamedProblemSize::from_mixed(&p, &bad),
        Err(StatisticsSchemaError::DuplicateVariable { .. })
    ));
}
#[test]
fn empty_basic_ranges_require_metadata_but_empty_list_owners_keep_the_registry() {
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    let mut schema = vec![StatisticsEntityType {
        name: "Empty".into(),
        entity_count: 0,
        variables: vec![StatisticsVariable::basic("v", vec![])],
    }];
    assert!(matches!(
        NamedProblemSize::from_basic(&p, &schema),
        Err(StatisticsSchemaError::MissingEmptyRange { .. })
    ));
    schema[0].variables[0].empty_value_count = Some(17);
    let s = NamedProblemSize::from_basic(&p, &schema).unwrap();
    assert_eq!(s.approximate_value_count, 17);
    assert_eq!(s.assignment_space.decimal(), "1");
    let list = ListProblem::new(vec![1, 2], vec![]).unwrap();
    schema[0].variables = vec![StatisticsVariable::list("values", vec![])];
    let s = NamedProblemSize::from_list(&list, &schema).unwrap();
    assert_eq!(s.approximate_value_count, 2);
    assert_eq!(s.assignment_space.decimal(), "0");
}
#[test]
fn pinned_uninitialized_required_slots_are_impossible_and_dependencies_are_upper_bounds() {
    for optional in [false, true] {
        for initialized in [false, true] {
            let mut v = Variable::new(vec![0, 1]).with_pinned(true);
            if optional {
                v = v.allowing_unassigned();
            }
            if initialized {
                v = v.with_initial(0);
            }
            let p = FiniteProblem::new(vec![v, Variable::new(vec![0, 1, 2])]).unwrap();
            let expected = if initialized || optional { 3 } else { 0 };
            assert_eq!(
                p.problem_size_statistics().assignment_space.count,
                BigUint::from(expected as u32)
            );
            assert_eq!(p.search_space_size(), Some(expected));
        }
    }
    let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![]); 2]).unwrap();
    assert_eq!(p.problem_size_statistics().assignment_space.decimal(), "6");
    assert!(p.problem_size_statistics().assignment_space.exact);
    let p = p.with_dependencies(vec![vec![], vec![0]]).unwrap();
    let assignments = [
        vec![vec![0, 1], vec![]],
        vec![vec![1, 0], vec![]],
        vec![vec![], vec![0, 1]],
        vec![vec![], vec![1, 0]],
        vec![vec![0], vec![1]],
        vec![vec![1], vec![0]],
    ];
    assert_eq!(
        assignments
            .into_iter()
            .filter(|a| p.validate(&ListAssignment::new(a.clone())).is_ok())
            .count(),
        4
    );
    let space = p.problem_size_statistics().assignment_space;
    assert_eq!(space.decimal(), "6");
    assert!(!space.exact);
}
#[test]
fn logarithms_remain_finite_for_spaces_beyond_float_range_and_round_sensibly() {
    let count = BigUint::from(10u32).pow(2000);
    let scale = ProblemScale::from_count(&count);
    assert!((scale.log10() - 2000.0).abs() < 1e-10);
    assert_eq!(scale.formatted(), "1.000000 × 10^2000");
    assert_eq!(scale.scaled_log(1000), 2_000_000);
    assert_eq!(
        ProblemScale::from_count(&BigUint::from(0u32)).formatted(),
        "0"
    );
    assert_eq!(
        ProblemScale::from_count(&BigUint::from(250u32)).formatted(),
        "250"
    );
    assert_eq!(
        ProblemScale::from_log10(11.999999999).formatted(),
        "1.000000 × 10^12"
    );
}
