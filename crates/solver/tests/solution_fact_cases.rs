// Complete adaptations of selected Community SolutionDescriptorTest methods.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/solution_fact_fixture.rs"]
mod fixture;
use fixture::*;
use std::{collections::BTreeSet, sync::Arc};
use timetuck::*;
fn names(layout: &ComposedPlanningSolution, kind: SolutionMemberCategory, expected: &[&str]) {
    let actual = layout
        .members_in(kind)
        .map(CompiledSolutionMember::name)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.iter().copied().collect());
}
fn base<S: 'static>(
    members: Vec<SolutionMemberDefinition>,
) -> Result<ComposedPlanningSolution, SolutionCompositionError> {
    compile::<S>(members, &[t::<Entity>()])
}
#[test]
fn problem_fact_property() {
    let l = properties();
    names(&l, SolutionMemberCategory::ProblemFact, &["extraObject"]);
    names(
        &l,
        SolutionMemberCategory::ProblemFactCollection,
        &["valueList", "otherProblemFactList"],
    );
}
#[test]
fn read_method_problem_fact_collection_property() {
    let mut m = standard();
    m.push(SolutionMemberDefinition::problem_fact_collection::<String>(
        "createProblemFacts",
    ));
    let l = base::<ReadMethod>(m).unwrap();
    names(&l, SolutionMemberCategory::ProblemFact, &[]);
    names(
        &l,
        SolutionMemberCategory::ProblemFactCollection,
        &["valueList", "createProblemFacts"],
    );
}
#[test]
fn problem_fact_collection_property_with_argument() {
    let mut m = standard();
    m.push(
        SolutionMemberDefinition::problem_facts("problemFacts", vec![], false)
            .with_external_parameter::<i32>(),
    );
    assert!(matches!(
        base::<Argument>(m),
        Err(SolutionCompositionError::MemberParameters { .. })
    ));
}
#[test]
fn duplicate_problem_fact_collection_property() {
    let mut m = standard();
    m.push(SolutionMemberDefinition::problem_fact_collection::<Value>(
        "valueList",
    ));
    assert!(matches!(
        base::<DuplicateFacts>(m),
        Err(SolutionCompositionError::DuplicateMember { .. })
    ));
}
#[test]
fn duplicate_planning_entity_collection_property() {
    let mut m = standard();
    m.push(SolutionMemberDefinition::entity_collection::<Entity>(
        "entityList",
    ));
    assert!(matches!(
        base::<DuplicateEntities>(m),
        Err(SolutionCompositionError::DuplicateMember { .. })
    ));
}
#[test]
fn duplicate_planning_score_property_property() {
    let mut m = standard();
    m.push(SolutionMemberDefinition::score::<SimpleScore>("score"));
    assert!(matches!(
        base::<DuplicateScore>(m),
        Err(SolutionCompositionError::DuplicateMember { .. })
    ));
}
#[test]
fn missing_planning_score_property_property() {
    let mut m = standard();
    m.pop();
    assert!(matches!(
        base::<MissingScore>(m),
        Err(SolutionCompositionError::MissingScore)
    ));
}
#[test]
fn problem_fact_is_planning_entity_collection_property() {
    let mut m = standard();
    m[1] = SolutionMemberDefinition::entity_collection::<Entity>("entityList")
        .as_problem_fact_collection();
    assert!(matches!(
        base::<ConflictingRole>(m),
        Err(SolutionCompositionError::MemberRoleConflict(_))
    ));
}
#[test]
fn planning_entity_is_problem_fact_property() {
    let m = vec![
        SolutionMemberDefinition::entity::<Entity>("entity"),
        SolutionMemberDefinition::problem_fact::<Entity>("entityAsFact"),
        SolutionMemberDefinition::value_range::<Value>("values", "valueRange"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ];
    assert!(matches!(
        base::<EntityAsFact>(m),
        Err(SolutionCompositionError::FactIsEntity { .. })
    ));
}
#[test]
fn planning_entity_is_problem_fact_collection_property() {
    let m = vec![
        SolutionMemberDefinition::problem_fact_collection::<Entity>("entitiesAsFacts"),
        SolutionMemberDefinition::entity_collection::<Entity>("entities"),
        SolutionMemberDefinition::value_range::<Value>("values", "valueRange"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ];
    assert!(matches!(
        base::<EntitiesAsFacts>(m),
        Err(SolutionCompositionError::FactIsEntity { .. })
    ));
}
#[test]
fn planning_entity_is_problem_fact_array_property() {
    let m = vec![
        SolutionMemberDefinition::problem_fact_collection::<Entity>("entitiesAsFacts")
            .with_member_shape(SolutionMemberShape::Array),
        SolutionMemberDefinition::entity_collection::<Entity>("entities")
            .with_member_shape(SolutionMemberShape::Array),
        SolutionMemberDefinition::value_range::<Value>("values", "valueRange")
            .with_member_shape(SolutionMemberShape::Array),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ];
    assert!(matches!(
        base::<EntityArrayAsFacts>(m),
        Err(SolutionCompositionError::FactIsEntity { .. })
    ));
}
#[test]
fn wildcard_problem_fact_and_entity_properties() {
    let m = vec![
        range("extendsValueList"),
        SolutionMemberDefinition::problem_facts("supersValueList", vec![], false),
        SolutionMemberDefinition::entity_collection::<Entity>("extendsEntityList"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ];
    let l = base::<Wildcard>(m).unwrap();
    names(&l, SolutionMemberCategory::ProblemFact, &[]);
    names(
        &l,
        SolutionMemberCategory::ProblemFactCollection,
        &["extendsValueList", "supersValueList"],
    );
    names(&l, SolutionMemberCategory::Entity, &[]);
    names(
        &l,
        SolutionMemberCategory::EntityCollection,
        &["extendsEntityList"],
    );
}
#[test]
fn wildcard_supers_entity_list_property() {
    let l = base::<UnsupportedWildcard>(vec![
        range("valueList"),
        SolutionMemberDefinition::entity_collection::<Entity>("supersEntityList"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ])
    .unwrap();
    let v1 = Value { code: "v1".into() };
    let v2 = Arc::new(Value { code: "v2".into() });
    let e1 = Arc::new(Entity {
        code: "e1".into(),
        value: None,
    });
    let result = l.bind_basic(
        vec![SolutionCollection {
            member: "supersEntityList".into(),
            entities: vec![
                SolutionEntityRecord::new(e1, vec![("value".into(), None)]),
                SolutionEntityRecord::new(v2, vec![]),
            ],
        }],
        vec![SolutionRange {
            member: "valueList".into(),
            values: vec![v1],
        }],
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["valueRange".into()],
        )],
    );
    assert!(matches!(
        result,
        Err(SolutionBindingError::WrongEntityType { .. })
    ));
}
#[test]
fn no_problem_fact_property_with_easy_score_calculation() {
    let l = base::<NoFacts>(vec![
        SolutionMemberDefinition::value_range::<Value>("valueList", "valueRange"),
        SolutionMemberDefinition::entity_collection::<Entity>("entityList"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ])
    .unwrap();
    names(&l, SolutionMemberCategory::ProblemFactCollection, &[]);
    let bound = l
        .bind_basic(
            vec![SolutionCollection {
                member: "entityList".into(),
                entities: vec![],
            }],
            vec![SolutionRange {
                member: "valueList".into(),
                values: vec![],
            }],
            vec![SolutionFieldRange::new::<Entity>(
                "value",
                vec!["valueRange".into()],
            )],
        )
        .unwrap();
    let network = ConstraintNetwork::<Value, SimpleScore>::new(0);
    assert_eq!(
        solve(bound.problem(), &network, SolverPlan::default())
            .unwrap()
            .score,
        Some(SimpleScore::ZERO)
    );
}
#[test]
fn extended() {
    let l = fixture::extended();
    names(&l, SolutionMemberCategory::ProblemFact, &[]);
    names(
        &l,
        SolutionMemberCategory::ProblemFactCollection,
        &["valueList", "subValueList"],
    );
    names(&l, SolutionMemberCategory::Entity, &["entity", "subEntity"]);
    names(
        &l,
        SolutionMemberCategory::EntityCollection,
        &[
            "entityList",
            "subEntityList",
            "rawEntityList",
            "entityList",
            "objectEntityList",
        ],
    );
}
#[test]
fn set_properties() {
    let l = compile::<SetSolution>(
        vec![
            range("valueSet"),
            SolutionMemberDefinition::entity_collection::<SetEntity>("entitySet"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ],
        &[t::<SetEntity>()],
    )
    .unwrap();
    names(&l, SolutionMemberCategory::ProblemFact, &[]);
    names(
        &l,
        SolutionMemberCategory::ProblemFactCollection,
        &["valueSet"],
    );
    names(&l, SolutionMemberCategory::Entity, &[]);
    names(&l, SolutionMemberCategory::EntityCollection, &["entitySet"]);
}
#[test]
fn array_properties() {
    let l = compile::<ArraySolution>(
        vec![
            range("values").with_member_shape(SolutionMemberShape::Array),
            SolutionMemberDefinition::entity_collection::<ArrayEntity>("entities")
                .with_member_shape(SolutionMemberShape::Array),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ],
        &[t::<ArrayEntity>()],
    )
    .unwrap();
    names(&l, SolutionMemberCategory::ProblemFact, &[]);
    names(
        &l,
        SolutionMemberCategory::ProblemFactCollection,
        &["values"],
    );
    names(&l, SolutionMemberCategory::Entity, &[]);
    names(&l, SolutionMemberCategory::EntityCollection, &["entities"]);
}
#[test]
fn generic() {
    let l = compile::<GenericSolution>(
        vec![
            SolutionMemberDefinition::value_range::<GenericValue<()>>("valueList", "valueRange")
                .as_problem_fact_collection(),
            SolutionMemberDefinition::value_range::<ComplexValue>(
                "complexGenericValueList",
                "complexGenericValueRange",
            )
            .as_problem_fact_collection(),
            SolutionMemberDefinition::value_range::<GenericValue<()>>(
                "subTypeValueList",
                "subTypeValueRange",
            )
            .as_problem_fact_collection(),
            SolutionMemberDefinition::entity_collection::<GenericEntity>("entityList"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ],
        &[t::<GenericEntity>()],
    )
    .unwrap();
    names(
        &l,
        SolutionMemberCategory::ProblemFactCollection,
        &["valueList", "complexGenericValueList", "subTypeValueList"],
    );
    names(
        &l,
        SolutionMemberCategory::EntityCollection,
        &["entityList"],
    );
    assert_eq!(
        l.schema()
            .entity::<GenericEntity>()
            .unwrap()
            .variables()
            .map(|v| v.name())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["value", "subTypeValue", "complexGenericValue"])
    );
}
#[test]
fn test_no_solution() {
    let error =
        SolutionComposition::new(vec![PlanningSolutionComponent::plain::<NoSolution>(vec![
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<NoSolution>(
            &entities(),
            &[],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("is not registered as a planning solution but defines planning members")
    );
}
#[test]
fn test_bad_fact_collection() {
    let error = compile::<BadCollection>(
        vec![
            range("valueList").with_member_shape(SolutionMemberShape::Scalar),
            SolutionMemberDefinition::entity_collection::<Entity>("entityList"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ],
        &[],
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("does not return a Collection or an array.")
    );
}
