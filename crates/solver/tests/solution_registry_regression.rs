//! Original operational controls; these add no upstream method credit.
#[path = "fixtures/solution_registry_fixture.rs"]
mod fixture;
use fixture::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
#[test]
fn canonical_ranges_feed_real_list_and_two_basic_fields() {
    let layout = mixed();
    let mut fields = mixed_ranges();
    fields.reverse();
    let descriptors = layout.compile_variable_ranges(fields).unwrap();
    let first = descriptors.get::<FirstEntity>("valueList").unwrap();
    let second = descriptors.get::<SecondEntity>("basicValue").unwrap();
    let third = descriptors.get::<SecondEntity>("secondBasicValue").unwrap();
    assert_eq!(
        [first.ordinal(), second.ordinal(), third.ordinal()],
        [0, 1, 2]
    );
    assert_ne!(second.variable(), third.variable());
    assert_eq!(second.definition().providers, third.definition().providers);
    let first_values = vec![
        FirstValue { code: "a".into() },
        FirstValue { code: "b".into() },
    ];
    let list = first
        .bind_shared(
            &[SolutionRange {
                member: "valueList".into(),
                values: first_values.clone(),
            }],
            2,
        )
        .unwrap()
        .list_problem(vec![ListEntity::new(vec![]), ListEntity::new(vec![])])
        .unwrap();
    assert_eq!(list.values(), first_values);
    let reads = [SolutionRange {
        member: "otherValueList".into(),
        values: vec![
            SecondValue {
                code: "v1".into(),
                strength: 20,
            },
            SecondValue {
                code: "v2".into(),
                strength: 19,
            },
        ],
    }];
    let a = second.bind_shared(&reads, 2).unwrap();
    let b = third.bind_shared(&reads, 2).unwrap();
    let problem = FiniteProblem::new(vec![
        a.variable(0).unwrap(),
        b.variable(0).unwrap(),
        a.variable(1).unwrap(),
        b.variable(1).unwrap(),
    ])
    .unwrap();
    assert_eq!(problem.variables().len(), 4);
    for variable in problem.variables() {
        assert_eq!(variable.domain(), reads[0].values);
    }
    assert_eq!(
        descriptors
            .get_type(PlanningType::of::<SecondEntity>(), "basicValue")
            .unwrap()
            .ordinal(),
        1
    );
}
#[test]
fn ranges_reject_wrong_missing_duplicate_and_shadow_links() {
    let layout = mixed();
    let mut duplicate = mixed_ranges();
    duplicate.push(duplicate[0].clone());
    assert!(matches!(
        layout.compile_variable_ranges(duplicate),
        Err(SolutionBindingError::InvalidFieldRange { .. })
    ));
    let mut missing = mixed_ranges();
    missing.pop();
    assert!(matches!(
        layout.compile_variable_ranges(missing),
        Err(SolutionBindingError::MissingFieldRange { .. })
    ));
    let mut wrong = mixed_ranges();
    wrong[1].providers = vec!["valueRange".into()];
    assert!(matches!(
        layout.compile_variable_ranges(wrong),
        Err(SolutionBindingError::InvalidFieldRange { .. })
    ));
    let mut unknown = mixed_ranges();
    unknown[0].providers = vec!["typo".into()];
    assert!(matches!(
        layout.compile_variable_ranges(unknown),
        Err(SolutionBindingError::UnknownProvider(_))
    ));
    let shadow = missing_layout();
    assert!(matches!(
        shadow.compile_variable_ranges(vec![SolutionFieldRange::new::<MissingValue>(
            "endTime",
            vec!["valueRange".into()]
        )]),
        Err(SolutionBindingError::InvalidFieldRange { .. })
    ));
    let descriptors = layout.compile_variable_ranges(mixed_ranges()).unwrap();
    let first = descriptors.get::<FirstEntity>("valueList").unwrap();
    assert!(matches!(
        first.bind_shared::<FirstValue>(&[], 1),
        Err(SolutionBindingError::UnknownProvider(_))
    ));
    let reads = [SolutionRange {
        member: "valueList".into(),
        values: vec![2i32],
    }];
    assert!(matches!(
        first.bind_shared(&reads, 1),
        Err(SolutionBindingError::WrongValueType { .. })
    ));
    let reads = [
        SolutionRange {
            member: "valueList".into(),
            values: Vec::<FirstValue>::new(),
        },
        SolutionRange {
            member: "valueList".into(),
            values: vec![],
        },
    ];
    assert!(matches!(
        first.bind_shared(&reads, 1),
        Err(SolutionBindingError::DuplicateBinding(_))
    ));
}
struct Task {
    value: Option<i64>,
}
struct Weighted {
    weights: ConstraintWeightOverrides<SimpleScore>,
}
fn weighted_layout() -> Arc<ComposedPlanningSolution> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Task>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
    ])])
    .unwrap();
    Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Weighted>(
            vec![
                SolutionMemberDefinition::entity::<Task>("task"),
                SolutionMemberDefinition::value_range::<i64>("values", "range"),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
                SolutionMemberDefinition::constraint_weights::<SimpleScore>("weights"),
            ],
        )])
        .unwrap()
        .compile::<Weighted>(
            &entities,
            &[PlanningType::of::<Task>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
#[test]
fn bound_range_ordinals_and_solution_weights_drive_actual_optimization() {
    let layout = weighted_layout();
    let task = Arc::new(Task { value: None });
    let bound = layout
        .bind_basic(
            vec![SolutionCollection {
                member: "task".into(),
                entities: vec![SolutionEntityRecord::new(
                    task.clone(),
                    vec![("value".into(), task.value)],
                )],
            }],
            vec![SolutionRange {
                member: "values".into(),
                values: vec![1, 2],
            }],
            vec![SolutionFieldRange::new::<Task>(
                "value",
                vec!["range".into()],
            )],
        )
        .unwrap();
    assert_eq!(bound.range_ordinal::<Task>("value"), Some(0));
    assert_eq!(bound.value_range_descriptor_count(), 1);
    assert_eq!(bound.entities()[0].slot("value"), Some(0));
    let reads =
        SolutionWeightAccessor::new(layout.clone(), |s: &Weighted| s.weights.clone()).unwrap();
    let mut s = Weighted {
        weights: [
            ("one".into(), SimpleScore::new(5)),
            ("two".into(), SimpleScore::ZERO),
        ]
        .into_iter()
        .collect(),
    };
    let old = reads.read(&s);
    s.weights.insert("one", SimpleScore::ZERO);
    s.weights.insert("two", SimpleScore::new(7));
    let new = reads.read(&s);
    drop(reads);
    drop(s);
    assert!(Arc::ptr_eq(old.descriptor(), &layout));
    for (snapshot, value, score) in [(&old, 1, 5), (&new, 2, 7)] {
        let result = solve_stream(
            bound.problem(),
            || {
                let mut b = StreamGraphBuilder::new();
                let source = b.source::<usize, (usize, Option<i64>)>();
                b.score_weighted("one", source.port(), SimpleScore::ZERO, |(_, v)| {
                    i64::from(*v == Some(1))
                })?;
                b.score_weighted("two", source.port(), SimpleScore::ZERO, |(_, v)| {
                    i64::from(*v == Some(2))
                })?;
                Ok((b.build_with_weights(snapshot.weights())?, source))
            },
            SolverPlan::default(),
        )
        .unwrap();
        assert_eq!(result.assignment.values(), &[Some(value)]);
        assert_eq!(result.score, Some(SimpleScore::new(score)));
    }
    assert_eq!(old.weights().get("one"), Some(&SimpleScore::new(5)));
}
#[test]
fn weight_accessors_validate_types_before_callback_and_inherited_duplicates_fail() {
    let layout = weighted_layout();
    assert!(matches!(
        SolutionWeightAccessor::new(
            layout.clone(),
            |_: &Mixed| -> ConstraintWeightOverrides<SimpleScore> { panic!() }
        ),
        Err(SolutionWeightError::SolutionType { .. })
    ));
    assert!(matches!(
        SolutionWeightAccessor::new(
            layout,
            |_: &Weighted| -> ConstraintWeightOverrides<HardSoftScore> { panic!() }
        ),
        Err(SolutionWeightError::ScoreType { .. })
    ));
    assert!(matches!(
        SolutionWeightAccessor::new(
            Arc::new(mixed()),
            |_: &Mixed| -> ConstraintWeightOverrides<SimpleScore> { panic!() }
        ),
        Err(SolutionWeightError::MissingDeclaration)
    ));
    struct Parent;
    struct Child;
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Task>(vec![
        PlanningVariableDefinition::basic::<i64>("value", false),
    ])])
    .unwrap();
    let declarations = SolutionComposition::new(vec![
        PlanningSolutionComponent::registered::<Parent>(vec![
            SolutionMemberDefinition::constraint_weights::<SimpleScore>("parentWeights"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ]),
        PlanningSolutionComponent::registered::<Child>(vec![
            SolutionMemberDefinition::constraint_weights::<SimpleScore>("childWeights"),
        ])
        .with_parent::<Parent>(),
    ])
    .unwrap();
    assert!(matches!(
        declarations.compile::<Child>(
            &entities,
            &[PlanningType::of::<Task>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default()
        ),
        Err(SolutionCompositionError::MultipleConstraintWeights { .. })
    ));
    let wrong = SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Weighted>(
        vec![
            SolutionMemberDefinition::constraint_weights::<HardSoftScore>("weights"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ],
    )])
    .unwrap();
    assert!(matches!(
        wrong.compile::<Weighted>(
            &entities,
            &[PlanningType::of::<Task>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default()
        ),
        Err(SolutionCompositionError::ConstraintWeightScoreType { .. })
    ));
}
struct World {
    factor: i64,
    entity: Target,
}
struct Target {
    base: i64,
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<World>(vec![PlanningEntityDefinition::of::<Target>(vec![
        PlanningVariableDefinition::basic::<i64>("factor", false),
        PlanningVariableDefinition::shadow::<i64>("result"),
    ])])
    .unwrap()
}
#[test]
fn resolved_named_callback_survives_registry_and_runs_in_real_shadow_session() {
    let schema = schema();
    let calls = Arc::new(AtomicUsize::new(0));
    let c = calls.clone();
    let mut registry = NamedShadowSuppliers::<World>::new(&schema).unwrap();
    registry
        .register("calculate", move |e: &Target, w: &World| {
            c.fetch_add(1, Ordering::SeqCst);
            Some(e.base + w.factor)
        })
        .unwrap();
    registry
        .validate(&[(PlanningType::of::<Target>(), "result", "calculate")])
        .unwrap();
    let supplier = registry
        .resolve::<Target, i64>("result", "calculate")
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    drop(registry);
    let mut b = ShadowSessionBuilder::<usize, World>::new(&schema);
    let input = b
        .input(
            schema
                .entity::<Target>()
                .unwrap()
                .variable("factor")
                .unwrap(),
            0,
            |w| Some(w.factor),
        )
        .unwrap();
    let output = b
        .derive(supplier.target(), 0, move |w, _| {
            supplier.evaluate(&w.entity, w)
        })
        .unwrap();
    b.set_dependencies(&output, &[input.erase()]).unwrap();
    let mut session = b.build();
    for (factor, expected) in [(2, 9), (20, 27)] {
        session
            .update(&World {
                factor,
                entity: Target { base: 7 },
            })
            .unwrap();
        assert_eq!(session.read(&output).unwrap().value, Some(&expected));
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
#[test]
fn named_supplier_registration_is_transactional_and_checks_actual_signatures() {
    let schema = schema();
    assert!(matches!(
        NamedShadowSuppliers::<Mixed>::new(&schema),
        Err(NamedSupplierError::SolutionType { .. })
    ));
    let mut registry = NamedShadowSuppliers::<World>::new(&schema).unwrap();
    registry
        .register("calculate", |e: &Target, w: &World| Some(e.base + w.factor))
        .unwrap();
    assert!(matches!(
        registry.register("calculate", |_: &Target, _| -> Option<i64> { panic!() }),
        Err(NamedSupplierError::Duplicate { .. })
    ));
    assert!(matches!(
        registry.register("", |_: &Target, _| Some(1i64)),
        Err(NamedSupplierError::EmptyName)
    ));
    assert!(matches!(
        registry.register("calculate", |_: &MissingValue, _| Some(1i64)),
        Err(NamedSupplierError::InvalidOwner(_))
    ));
    assert!(matches!(
        registry.resolve::<Target, i64>("factor", "calculate"),
        Err(NamedSupplierError::InvalidTarget { .. })
    ));
    assert!(matches!(
        registry.resolve::<Target, String>("result", "calculate"),
        Err(NamedSupplierError::ReturnType { .. })
    ));
    registry
        .register("wrongResult", |_: &Target, _| Some("wrong".to_owned()))
        .unwrap();
    assert!(matches!(
        registry.resolve::<Target, String>("result", "wrongResult"),
        Err(NamedSupplierError::ReturnType { .. })
    ));
    let supplier = registry
        .resolve::<Target, i64>("result", "calculate")
        .unwrap();
    assert_eq!(
        supplier.evaluate(
            &Target { base: 4 },
            &World {
                factor: 3,
                entity: Target { base: 0 }
            }
        ),
        Some(7)
    );
    let missing = missing_layout();
    let mut reg = NamedShadowSuppliers::<MissingSolution>::new(missing.schema()).unwrap();
    reg.register("calculateStartTime", |v: &MissingValue, _| {
        Some(v.calculate_start_time())
    })
    .unwrap();
    let start = reg
        .resolve::<MissingValue, i64>("startTime", "calculateStartTime")
        .unwrap();
    let world = MissingSolution {
        entities: vec![],
        values: vec![],
        score: None,
    };
    let mut value = MissingValue {
        id: "v1".into(),
        start_time: None,
        end_time: None,
        invalid: false,
        entity_list: vec![],
        duration_seconds: 86_400,
    };
    assert_eq!(start.evaluate(&value, &world), Some(10));
    value.entity_list.push(Arc::new(MissingEntity {
        id: "e1".into(),
        value: None,
        duration_in_days: 0,
    }));
    assert_eq!(start.evaluate(&value, &world), Some(1));
}
