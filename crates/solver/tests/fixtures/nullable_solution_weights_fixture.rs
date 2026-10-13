// Pinned Community TEST fixture adaptations; Apache-2.0, see compat/provenance.json.
#![allow(dead_code)]
use std::sync::Arc;
use timetuck::*;
#[derive(Clone)]
pub struct Entity {
    pub value: Option<i32>,
}
#[derive(Default)]
pub struct Solution {
    pub weights: Option<ConstraintWeightOverrides<SimpleScore>>,
    pub entities: Vec<Entity>,
    pub values: Vec<i32>,
    pub score: Option<SimpleScore>,
}
pub struct ExtendedSolution;
pub fn layout(extended: bool) -> Result<Arc<ComposedPlanningSolution>, SolutionCompositionError> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap();
    let base = PlanningSolutionComponent::registered::<Solution>(vec![
        SolutionMemberDefinition::constraint_weights::<SimpleScore>("constraintWeightOverrides"),
        SolutionMemberDefinition::entity_collection::<Entity>("entityList"),
        SolutionMemberDefinition::value_range::<i32>("valueList", "valueRange")
            .as_problem_fact_collection(),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ]);
    let declarations = if extended {
        vec![
            base,
            PlanningSolutionComponent::registered::<ExtendedSolution>(vec![
                SolutionMemberDefinition::constraint_weights::<SimpleScore>(
                    "constraintWeightOverrides",
                ),
            ])
            .with_parent::<Solution>(),
        ]
    } else {
        vec![base]
    };
    let composition = SolutionComposition::new(declarations).unwrap();
    let roots = [PlanningType::of::<Entity>()];
    let policy = SolutionCompositionPolicy::default();
    let epolicy = PlanningCompositionPolicy::default();
    if extended {
        composition.compile::<ExtendedSolution>(&entities, &roots, policy, epolicy)
    } else {
        composition.compile::<Solution>(&entities, &roots, policy, epolicy)
    }
    .map(Arc::new)
}
pub fn first_constraint_assertion(
    solution: &Solution,
    weights: &BoundSolutionWeights<SimpleScore>,
) -> ConstraintAssertion<SimpleScore, usize> {
    let mut b = StreamGraphBuilder::<SimpleScore>::new();
    let source = b.source::<usize, Entity>();
    let initialized = b
        .filter(source.port(), |entity| entity.value.is_some())
        .unwrap();
    let constraint = track_constraint(
        &mut b,
        "First weight",
        &initialized,
        SimpleScore::new(1),
        |_| 1i64,
    )
    .unwrap();
    let mut graph = b.build_with_weights(weights.weights()).unwrap();
    let mut tx = graph.transaction();
    for (id, entity) in solution.entities.iter().enumerate() {
        tx.apply(&source, TupleChange::put(id, entity.clone()))
            .unwrap();
    }
    tx.commit().unwrap();
    constraint_assertion(&graph, &constraint, |_, _| 0usize).unwrap()
}
