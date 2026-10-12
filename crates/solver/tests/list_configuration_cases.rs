// Adapted Community SolverConfigTest and its TEST-only two-list declarations.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;
#[derive(Eq, PartialEq)]
struct DummyEntityForListVariable;
struct DummyEntityWithTwoListVariables;
struct DummySolutionWithTwoListVariablesEntity;

#[test]
fn entity_with_two_planning_list_variables() {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<
        DummyEntityWithTwoListVariables,
    >(vec![
        PlanningVariableDefinition::list::<Arc<DummyEntityForListVariable>>(
            "firstListVariable",
            false,
        ),
        PlanningVariableDefinition::list::<Arc<DummyEntityForListVariable>>(
            "secondListVariable",
            false,
        ),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<
            DummySolutionWithTwoListVariablesEntity,
        >(vec![
            SolutionMemberDefinition::entity_collection::<DummyEntityWithTwoListVariables>(
                "entities",
            ),
            SolutionMemberDefinition::value_range::<Arc<DummyEntityForListVariable>>(
                "firstListValueRange",
                "firstListValueRange",
            ),
            SolutionMemberDefinition::value_range::<Arc<DummyEntityForListVariable>>(
                "secondListValueRange",
                "secondListValueRange",
            ),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<DummySolutionWithTwoListVariablesEntity>(
            &entities,
            &[PlanningType::of::<DummyEntityWithTwoListVariables>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let facts = SolutionFactAccessors::new(layout.clone())
        .unwrap()
        .read(&DummySolutionWithTwoListVariablesEntity)
        .unwrap();
    let error = layout
        .bind_list_with_facts::<Arc<DummyEntityForListVariable>>(
            vec![],
            vec![],
            vec![
                SolutionFieldRange::new::<DummyEntityWithTwoListVariables>(
                    "firstListVariable",
                    vec!["firstListValueRange".into()],
                ),
                SolutionFieldRange::new::<DummyEntityWithTwoListVariables>(
                    "secondListVariable",
                    vec!["secondListValueRange".into()],
                ),
            ],
            facts,
        )
        .err()
        .unwrap();
    assert!(matches!(
        error,
        ListSolutionBindingError::MultipleListFields(_)
    ));
    let message = error.to_string();
    assert!(message.contains("DummyEntityWithTwoListVariables"));
    assert!(message.contains("firstListVariable"));
    assert!(message.contains("secondListVariable"));
    let ListSolutionBindingError::MultipleListFields(fields) = error else {
        unreachable!()
    };
    assert_eq!(
        fields,
        vec![
            (
                PlanningType::of::<DummyEntityWithTwoListVariables>(),
                "firstListVariable".into()
            ),
            (
                PlanningType::of::<DummyEntityWithTwoListVariables>(),
                "secondListVariable".into()
            ),
        ]
    );
}
