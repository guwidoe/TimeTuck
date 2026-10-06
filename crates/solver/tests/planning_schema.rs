//! Original typed model construction, metadata identity and binding regressions.
use timetuck::*;
struct Schedule;
struct Job;
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Job>(vec![
        PlanningVariableDefinition::basic::<i32>("worker", false),
        PlanningVariableDefinition::list::<u32>("visits", false),
        PlanningVariableDefinition::shadow::<i64>("arrival"),
    ])])
    .unwrap()
}
#[test]
fn typed_declarations_build_a_solvable_mixed_model_and_named_statistics() {
    let s = schema();
    let e = s.genuine_entity::<Job>().unwrap();
    let worker = e.genuine_variable("worker").unwrap();
    let visits = e.genuine_variable("visits").unwrap();
    let basics = worker
        .entity_ranges(vec![vec![vec![1, 2]], vec![vec![2, 3]]])
        .unwrap();
    let lists = visits.shared_ranges(vec![vec![10u32, 11, 12]], 2).unwrap();
    let p = MixedProblem::new(
        FiniteProblem::new(vec![
            basics.variable(0).unwrap(),
            basics.variable(1).unwrap(),
        ])
        .unwrap(),
        lists
            .list_problem(vec![ListEntity::new(vec![]); 2])
            .unwrap(),
    );
    let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(2, 2, 3);
    let r = solve_mixed(
        &p,
        &n,
        MixedSolverConfig {
            local_search: vec![],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.complete);
    p.validate(&r.assignment).unwrap();
    let binding = e
        .statistics_binding(
            2,
            vec![
                visits.statistics_binding(vec![0, 1], None).unwrap(),
                worker.statistics_binding(vec![0, 1], None).unwrap(),
            ],
        )
        .unwrap();
    assert_eq!(
        binding
            .variables
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        vec!["worker", "visits"]
    );
    let stats = NamedProblemSize::from_mixed(&p, &[binding]).unwrap();
    assert_eq!(
        (
            stats.entity_count,
            stats.variable_count,
            stats.approximate_value_count
        ),
        (2, 4, 6)
    );
    assert_eq!(stats.assignment_space.decimal(), "96");
    assert_eq!(
        stats.values_by_type_variable[e.entity_type().name()]["visits"],
        3
    );
    assert_eq!(e.shadow_variables().count(), 1);
}
#[test]
fn schemas_reject_duplicate_metadata_wrong_types_and_incomplete_bindings() {
    for fields in [
        vec![PlanningVariableDefinition::basic::<i32>("", false)],
        vec![
            PlanningVariableDefinition::basic::<i32>("x", false),
            PlanningVariableDefinition::shadow::<i32>("x"),
        ],
    ] {
        assert!(
            PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Job>(fields)])
                .is_err()
        );
    }
    assert!(matches!(
        PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<Job>(vec![]); 2]),
        Err(PlanningSchemaError::DuplicateEntity(_))
    ));
    let s = schema();
    let e = s.entity::<Job>().unwrap();
    let w = e.variable("worker").unwrap();
    assert!(matches!(
        w.shared_ranges(vec![vec![1u32]], 1),
        Err(PlanningSchemaError::WrongValueType { .. })
    ));
    let arrival = e.variable("arrival").unwrap();
    assert!(matches!(
        arrival.shared_ranges(vec![vec![1i64]], 1),
        Err(PlanningSchemaError::WrongVariableKind { .. })
    ));
    assert!(arrival.statistics_binding(vec![0], None).is_err());
    assert!(s.shadow_entity::<Job>().is_err());
    assert!(e.shadow_variable("worker").is_err());
    assert!(e.genuine_variable("arrival").is_err());
    let w = w.statistics_binding(vec![0], None).unwrap();
    assert!(matches!(
        e.statistics_binding(1, vec![w.clone()]),
        Err(PlanningSchemaError::MissingBinding { .. })
    ));
    assert!(matches!(
        e.statistics_binding(2, vec![w.clone()]),
        Err(PlanningSchemaError::BindingLength { .. })
    ));
    assert!(matches!(
        e.statistics_binding(1, vec![w.clone(), w.clone()]),
        Err(PlanningSchemaError::DuplicateVariable { .. })
    ));
    let mut bad = w.clone();
    bad.kind = StatisticsVariableKind::List;
    assert!(matches!(
        e.statistics_binding(1, vec![bad]),
        Err(PlanningSchemaError::WrongVariableKind { .. })
    ));
    let mut bad = w;
    bad.name = "absent".into();
    assert!(matches!(
        e.statistics_binding(1, vec![bad]),
        Err(PlanningSchemaError::UnknownVariable { .. })
    ));
}
#[test]
fn type_and_canonical_identity_are_scoped_to_the_actual_schema() {
    mod first {
        pub struct Same;
    }
    mod second {
        pub struct Same;
    }
    let s = PlanningSchema::new::<Schedule>(vec![
        PlanningEntityDefinition::of::<first::Same>(vec![
            PlanningVariableDefinition::basic::<i32>("x", true),
        ]),
        PlanningEntityDefinition::of::<second::Same>(vec![
            PlanningVariableDefinition::shadow::<i32>("x"),
        ]),
    ])
    .unwrap();
    let a = s.entity::<first::Same>().unwrap();
    let b = s.entity::<second::Same>().unwrap();
    assert_ne!(a.entity_type(), b.entity_type());
    assert_eq!(s.genuine_entities().count(), 1);
    assert_eq!(s.shadow_entities().count(), 1);
    assert!(std::ptr::eq(
        s.entities().next().unwrap().definition(),
        a.definition()
    ));
    assert!(std::ptr::eq(
        a.variables().next().unwrap().definition(),
        a.variable("x").unwrap().definition()
    ));
    assert!(a.entity_type().is::<first::Same>());
    assert!(!a.entity_type().is::<second::Same>());
    let other =
        PlanningSchema::new::<Schedule>(vec![PlanningEntityDefinition::of::<first::Same>(vec![
            PlanningVariableDefinition::basic::<i32>("x", true),
        ])])
        .unwrap();
    assert_ne!(a, other.entity::<first::Same>().unwrap());
    assert_ne!(
        a.variable("x").unwrap(),
        other
            .entity::<first::Same>()
            .unwrap()
            .variable("x")
            .unwrap()
    );
    let ranges = a
        .variable("x")
        .unwrap()
        .shared_ranges::<i32>(vec![], 1)
        .unwrap();
    assert!(ranges.variable(0).unwrap().allows_unassigned());
    let empty = a
        .statistics_binding(
            0,
            vec![
                a.variable("x")
                    .unwrap()
                    .statistics_binding(vec![], Some(3))
                    .unwrap(),
            ],
        )
        .unwrap();
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    assert_eq!(
        NamedProblemSize::from_basic(&p, &[empty])
            .unwrap()
            .approximate_value_count,
        3
    );
}
