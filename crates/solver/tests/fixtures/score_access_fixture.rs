// TEST-derived ScoreDescriptorTest solution fixture, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::sync::Arc;
use timetuck::*;
pub struct Entity;
#[derive(Default)]
pub struct Solution {
    pub score: Option<Arc<SimpleScore>>,
}
pub fn layout() -> Arc<ComposedPlanningSolution> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<i32>("value", false),
    ])])
    .unwrap();
    let solution =
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Solution>(
            vec![
                SolutionMemberDefinition::entity_collection::<Entity>("entities"),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
        )])
        .unwrap();
    Arc::new(
        solution
            .compile::<Solution>(
                &entities,
                &[PlanningType::of::<Entity>()],
                SolutionCompositionPolicy::default(),
                PlanningCompositionPolicy::default(),
            )
            .unwrap(),
    )
}
pub fn accessor() -> SolutionScoreAccessor<'static, Solution, SimpleScore> {
    SolutionScoreAccessor::new(
        layout(),
        "score",
        |s: &Solution| s.score.as_ref(),
        |s, score| s.score = score,
    )
    .unwrap()
}
