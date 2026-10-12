// Adapted TEST-only lavish source roles/relationships; Apache-2.0.
// Unused arithmetic properties are omitted; no arithmetic-test credit claimed.
#![allow(dead_code)]
use std::sync::Arc;
use timetuck::*;
pub struct ValueGroup {
    pub code: String,
}
pub struct EntityGroup {
    pub code: String,
}
pub struct Value {
    pub code: String,
    pub group: Option<Arc<ValueGroup>>,
}
pub struct Extra {
    pub code: String,
}
pub struct Entity {
    pub code: String,
    pub group: Option<Arc<EntityGroup>>,
    pub value: Option<Arc<Value>>,
}
pub struct Lavish {
    pub code: String,
    pub value_group_list: Vec<Arc<ValueGroup>>,
    pub value_list: Vec<Arc<Value>>,
    pub extra_list: Vec<Arc<Extra>>,
    pub entity_group_list: Vec<Arc<EntityGroup>>,
    pub entity_list: Vec<Arc<Entity>>,
    pub score: Option<SimpleScore>,
}
pub fn registry() -> SolutionStreamRegistry {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<Value>("value", false),
    ])])
    .unwrap();
    let layout =
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Lavish>(vec![
            SolutionMemberDefinition::problem_fact_collection::<ValueGroup>("valueGroupList"),
            SolutionMemberDefinition::value_range::<Value>("valueList", "valueRange")
                .as_problem_fact_collection(),
            SolutionMemberDefinition::problem_fact_collection::<Extra>("extraList"),
            SolutionMemberDefinition::problem_fact_collection::<EntityGroup>("entityGroupList"),
            SolutionMemberDefinition::entity_collection::<Entity>("entityList"),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<Lavish>(
            &entities,
            &[PlanningType::of::<Entity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap();
    SolutionStreamRegistry::new(Arc::new(layout))
}
pub fn arc_id<T>(value: &Arc<T>) -> usize {
    Arc::as_ptr(value) as usize
}
pub fn object_id<T: 'static>(value: &SolutionStreamObject<T>) -> usize {
    Arc::as_ptr(value.allocation()) as *const () as usize
}
