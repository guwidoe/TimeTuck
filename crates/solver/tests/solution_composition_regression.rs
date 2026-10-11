// Original tests. The reused test-domain setup is attributed in its module.
mod solution_composition_support;
use solution_composition_support::*;
use std::{num::NonZeroUsize, sync::Arc};
use timetuck::*;

#[test]
fn deep_default_solution_composition_runs_real_models_in_both_backends() {
    let layout = compile::<10>(Shape::Multiple, SolutionCompositionPolicy::default()).unwrap();
    assert_eq!(
        layout.member("valueList").unwrap().declaring_type(),
        t::<Grand<10>>()
    );
    assert_eq!(
        layout.member("entityList").unwrap().declaring_type(),
        t::<Parent<10>>()
    );
    assert_eq!(
        layout
            .entity_composition()
            .field_origin(t::<ChildEntity<10>>(), "value"),
        Some(t::<ChildEntity<10>>())
    );
    let fixture = generate::<10>(Shape::Multiple);
    assert_eq!(fixture.actual_shape, Shape::Multiple);
    let bound = bind(&layout, fixture);
    for result in solve_fixture::<10>(&bound, 1, 50) {
        assert_eq!(result.score, Some(SimpleScore::new(2)));
        assert!(result.assignment.values().iter().all(Option::is_some));
    }
    let error = compile::<10>(
        Shape::BaseOnly,
        SolutionCompositionPolicy {
            maximum_depth: NonZeroUsize::new(1),
        },
    )
    .unwrap_err();
    assert_eq!(
        error,
        SolutionCompositionError::DepthLimit {
            solution: t::<Root<10>>(),
            actual: 2,
            maximum: 1
        }
    );
    assert!(error.to_string().contains("depth (2) exceeds maximum (1)"));
}
fn record<const N: usize>(
    object: &Arc<ChildEntity<N>>,
    initial: Option<String>,
) -> SolutionEntityRecord<String> {
    // Reversed input fields must still bind in canonical inherited/local order.
    SolutionEntityRecord::new(
        object.clone(),
        vec![("subValue".into(), None), ("value".into(), initial)],
    )
}
#[test]
fn aliases_keep_canonical_identity_slots_and_complete_entity_eligibility() {
    let layout = compile::<11>(Shape::Both, SolutionCompositionPolicy::default()).unwrap();
    let (a, b) = (
        Arc::new(ChildEntity::<11> {
            code: "equal".into(),
        }),
        Arc::new(ChildEntity::<11> {
            code: "equal".into(),
        }),
    );
    let base = Arc::new(BaseEntity::<11> {
        code: "singleton".into(),
    });
    let mut f = generate::<11>(Shape::Both);
    for c in &mut f.collections {
        c.entities = match c.member.as_str() {
            "entityList" => vec![record(&a, None), record(&b, None), record(&a, None)],
            "subEntityList" => vec![record(&a, None)],
            "subEntity" => vec![record(&b, None)],
            "entity" => vec![SolutionEntityRecord::new(
                base.clone(),
                vec![("value".into(), None)],
            )],
            _ => vec![],
        };
    }
    let bound = bind(&layout, f);
    assert_eq!(bound.entities().len(), 3);
    assert_eq!(bound.problem().variables().len(), 5);
    assert_eq!(bound.collection("entityList"), Some([0, 1, 0].as_slice()));
    assert_eq!(bound.collection("subEntityList"), Some([0].as_slice()));
    assert_eq!(bound.collection("subEntity"), Some([1].as_slice()));
    assert_eq!(bound.collection("entity"), Some([2].as_slice()));
    assert_eq!(bound.collection("rawEntityList"), Some([].as_slice()));
    assert!(bound.collection("missing").is_none());
    assert_eq!(
        bound.entities()[0].slots(),
        &[("value".into(), 0), ("subValue".into(), 1)]
    );
    assert_eq!(
        bound.entities()[2]
            .object_ref::<BaseEntity<11>>()
            .unwrap()
            .code,
        "singleton"
    );
    assert_eq!(bound.entities()[2].entity_type(), t::<BaseEntity<11>>());
    assert_eq!(
        bound.entities()[0]
            .object_ref::<ChildEntity<11>>()
            .unwrap()
            .code,
        "equal"
    );
    assert!(!Arc::ptr_eq(
        bound.entities()[0].object(),
        bound.entities()[1].object()
    ));
    assert_eq!(
        layout.member("valueList").unwrap().declaring_type(),
        t::<Parent<11>>()
    );
    assert_eq!(
        layout.member("rawEntityList").unwrap().declaring_type(),
        t::<Root<11>>()
    );
    assert_eq!(
        layout
            .entity_composition()
            .field_origin(t::<ChildEntity<11>>(), "value"),
        Some(t::<BaseEntity<11>>())
    );
    let network = dense::<11>(&bound, 2);
    let mut dense = ScoreDirector::new(
        bound.problem(),
        &network,
        bound.problem().initial_assignment(),
    )
    .unwrap();
    let (graph, source) = stream::<11>(&bound, 2).unwrap();
    let mut streamed = StreamScoreDirector::new(
        bound.problem(),
        graph,
        source,
        bound.problem().initial_assignment(),
    )
    .unwrap();
    for (row, field, value, expected) in [
        (0, "value", Some("Generated Value 0"), 0),
        (0, "subValue", Some("Generated Subvalue 0"), 2),
        (2, "value", Some("Generated Value 1"), 3),
        (1, "value", Some("Generated Value 2"), 3),
        (1, "subValue", Some("Generated Subvalue 2"), 5),
        (0, "subValue", None, 3),
    ] {
        let movement = Move::Change {
            variable: bound.entities()[row].slot(field).unwrap(),
            value: value.map(String::from),
        };
        dense.apply(&movement).unwrap().commit();
        streamed.apply(&movement).unwrap().commit();
        assert_eq!(dense.score().with_structural(0), SimpleScore::new(expected));
        assert_eq!(dense.score(), streamed.score());
        assert_eq!(
            dense.score(),
            network
                .calculate(bound.problem(), dense.assignment())
                .unwrap()
        );
    }
    for result in solve_fixture::<11>(&bound, 2, 50) {
        assert_eq!(result.score, Some(SimpleScore::new(5)));
    }
    drop(a);
    drop(b);
    drop(base);
    assert_eq!(
        bound.entities()[2]
            .object_ref::<BaseEntity<11>>()
            .unwrap()
            .code,
        "singleton"
    );
}
#[test]
fn aliases_with_conflicting_initial_fields_are_rejected() {
    let layout = compile::<12>(Shape::Both, SolutionCompositionPolicy::default()).unwrap();
    let object = Arc::new(ChildEntity::<12> {
        code: "same".into(),
    });
    let mut f = generate::<12>(Shape::Both);
    f.collections[0].entities = vec![
        record(&object, None),
        record(&object, Some("Generated Value 0".into())),
    ];
    assert_eq!(
        layout
            .bind_basic(f.collections, f.ranges, f.fields)
            .err()
            .unwrap(),
        SolutionBindingError::ConflictingEntityRecord(t::<ChildEntity<12>>())
    );
}
#[test]
fn bindings_validate_names_types_cardinality_fields_and_provider_links() {
    let layout = compile::<13>(Shape::Both, SolutionCompositionPolicy::default()).unwrap();
    for scenario in 0..9 {
        let mut f = generate::<13>(Shape::Both);
        let expected = match scenario {
            0 => {
                f.collections[0].member = "unknown".into();
                SolutionBindingError::UnknownMember("unknown".into())
            }
            1 => {
                f.collections.pop();
                SolutionBindingError::MissingBinding("subEntity".into())
            }
            2 => {
                f.fields[0].providers = vec!["missing".into()];
                SolutionBindingError::UnknownProvider("missing".into())
            }
            3 => {
                f.fields.remove(1);
                SolutionBindingError::MissingFieldRange {
                    entity: t::<ChildEntity<13>>(),
                    variable: "subValue".into(),
                }
            }
            4 => {
                f.collections[0].entities = vec![SolutionEntityRecord::new(
                    Arc::new(BaseEntity::<13> {
                        code: "base".into(),
                    }),
                    vec![("value".into(), None)],
                )];
                SolutionBindingError::WrongEntityType {
                    member: "entityList".into(),
                    entity: t::<BaseEntity<13>>(),
                }
            }
            5 => {
                f.collections[0].entities = vec![SolutionEntityRecord::new(
                    Arc::new(ChildEntity::<13> { code: "bad".into() }),
                    vec![("value".into(), None), ("value".into(), None)],
                )];
                SolutionBindingError::InvalidFields(t::<ChildEntity<13>>())
            }
            6 => {
                f.collections.push(SolutionCollection {
                    member: "entityList".into(),
                    entities: vec![],
                });
                SolutionBindingError::DuplicateBinding("entityList".into())
            }
            7 => {
                let object = Arc::new(ChildEntity::<13> {
                    code: "single".into(),
                });
                f.collections.last_mut().unwrap().entities =
                    vec![record(&object, None), record(&object, None)];
                SolutionBindingError::SingletonCardinality("subEntity".into())
            }
            _ => {
                f.fields.push(f.fields[0].clone());
                SolutionBindingError::InvalidFieldRange {
                    entity: t::<ChildEntity<13>>(),
                    variable: "value".into(),
                }
            }
        };
        assert_eq!(
            layout
                .bind_basic(f.collections, f.ranges, f.fields)
                .err()
                .unwrap(),
            expected,
            "scenario {scenario}"
        );
    }
    let f = generate::<13>(Shape::Both);
    let error = layout
        .bind_basic::<i32>(
            vec![],
            vec![SolutionRange {
                member: "valueList".into(),
                values: vec![1],
            }],
            f.fields,
        )
        .err()
        .unwrap();
    assert_eq!(
        error,
        SolutionBindingError::WrongValueType {
            member: "valueList".into(),
            expected: t::<String>(),
            actual: t::<i32>()
        }
    );
}
struct Empty;
struct A;
struct B;
struct Unused;
fn entity_graph() -> PlanningComposition {
    declarations::<14>(Shape::BaseOnly).1
}
fn compile_custom(
    components: Vec<PlanningSolutionComponent>,
) -> Result<ComposedPlanningSolution, SolutionCompositionError> {
    SolutionComposition::new(components)?.compile::<Empty>(
        &entity_graph(),
        &[t::<ChildEntity<14>>()],
        SolutionCompositionPolicy::default(),
        PlanningCompositionPolicy::default(),
    )
}
#[test]
fn solution_graph_errors_and_scoped_members_are_real_metadata_failures() {
    assert!(
        matches!(SolutionComposition::new(vec![PlanningSolutionComponent::plain::<A>(vec![]),PlanningSolutionComponent::plain::<A>(vec![])]).err(),Some(SolutionCompositionError::DuplicateComponent(t)) if t==self::t::<A>())
    );
    assert_eq!(
        compile_custom(vec![]).unwrap_err(),
        SolutionCompositionError::UnknownComponent(t::<Empty>())
    );
    assert_eq!(
        compile_custom(vec![
            PlanningSolutionComponent::plain::<Empty>(vec![]).with_parent::<A>()
        ])
        .unwrap_err(),
        SolutionCompositionError::UnknownComponent(t::<A>())
    );
    assert_eq!(
        compile_custom(vec![
            PlanningSolutionComponent::plain::<Empty>(vec![]).with_parent::<A>(),
            PlanningSolutionComponent::plain::<A>(vec![]).with_parent::<Empty>()
        ])
        .unwrap_err(),
        SolutionCompositionError::Cycle(t::<Empty>())
    );
    let layout = compile_custom(vec![
        PlanningSolutionComponent::registered::<A>(vec![SolutionMemberDefinition::score::<
            SimpleScore,
        >("score")]),
        PlanningSolutionComponent::plain::<Empty>(vec![]).with_parent::<A>(),
        PlanningSolutionComponent::plain::<Unused>(vec![SolutionMemberDefinition::score::<i64>(
            "invalid but unreachable",
        )]),
    ])
    .unwrap();
    assert_eq!(layout.members().len(), 1);
    assert_eq!(
        layout.member("score").unwrap().kind(),
        &SolutionMemberKind::Score(t::<SimpleScore>())
    );
    assert_eq!(
        compile_custom(vec![PlanningSolutionComponent::registered::<Empty>(vec![])]).unwrap_err(),
        SolutionCompositionError::MissingScore
    );
    assert_eq!(
        compile_custom(vec![PlanningSolutionComponent::registered::<Empty>(vec![
            SolutionMemberDefinition::score::<SimpleScore>("one"),
            SolutionMemberDefinition::score::<SimpleScore>("two")
        ])])
        .unwrap_err(),
        SolutionCompositionError::MultipleScores {
            first: "one".into(),
            second: "two".into()
        }
    );
    for (members, expected) in [
        (
            vec![SolutionMemberDefinition::score::<SimpleScore>("")],
            SolutionCompositionError::EmptyMemberName(t::<Empty>()),
        ),
        (
            vec![
                SolutionMemberDefinition::value_range::<String>("values", ""),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
            SolutionCompositionError::EmptyProvider("values".into()),
        ),
        (
            vec![
                SolutionMemberDefinition::value_range::<String>("one", "p"),
                SolutionMemberDefinition::value_range::<String>("two", "p"),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
            SolutionCompositionError::DuplicateProvider("p".into()),
        ),
        (
            vec![
                SolutionMemberDefinition::entities("entities", vec![], false),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
            SolutionCompositionError::InvalidEntityTypes("entities".into()),
        ),
    ] {
        assert_eq!(
            compile_custom(vec![PlanningSolutionComponent::registered::<Empty>(
                members
            )])
            .unwrap_err(),
            expected
        );
    }
}
#[test]
fn invalid_initial_values_preserve_native_model_classification() {
    let layout = compile::<15>(Shape::BaseOnly, SolutionCompositionPolicy::default()).unwrap();
    let mut f = generate::<15>(Shape::BaseOnly);
    f.collections[0].entities = vec![SolutionEntityRecord::new(
        Arc::new(ChildEntity::<15> { code: "bad".into() }),
        vec![("value".into(), Some("outside".into()))],
    )];
    assert_eq!(
        layout
            .bind_basic(f.collections, f.ranges, f.fields)
            .err()
            .unwrap(),
        SolutionBindingError::Model(ModelError::ValueOutsideDomain { variable: 0 })
    );
}
#[test]
fn empty_collections_bind_and_optional_basic_fields_keep_native_domains() {
    let graph = PlanningComposition::new(vec![PlanningComponent::registered::<B>(vec![
        PlanningVariableDefinition::basic::<i32>("value", true),
        PlanningVariableDefinition::shadow::<usize>("derived"),
    ])])
    .unwrap();
    let solution =
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Empty>(vec![
            SolutionMemberDefinition::entity_collection::<B>("objects"),
            SolutionMemberDefinition::value_range::<i32>("values", "range"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap();
    let layout = solution
        .compile::<Empty>(
            &graph,
            &[t::<B>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap();
    for present in [false, true] {
        let bound = layout
            .bind_basic(
                vec![SolutionCollection {
                    member: "objects".into(),
                    entities: if present {
                        vec![SolutionEntityRecord::new(
                            Arc::new(B),
                            vec![("value".into(), None)],
                        )]
                    } else {
                        vec![]
                    },
                }],
                vec![SolutionRange {
                    member: "values".into(),
                    values: vec![2, 2, 1],
                }],
                vec![SolutionFieldRange::new::<B>("value", vec!["range".into()])],
            )
            .unwrap();
        assert_eq!(bound.problem().variables().len(), usize::from(present));
        if present {
            assert!(bound.problem().variables()[0].allows_unassigned());
            assert_eq!(bound.problem().variables()[0].domain(), &[2, 1]);
        } else {
            assert_eq!(bound.collection("objects"), Some([].as_slice()));
            assert!(bound.into_problem().variables().is_empty());
        }
    }
}
