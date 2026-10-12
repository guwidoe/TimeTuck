// Adapted TEST-only SolutionDescriptorTest fixtures, Apache-2.0.
// Native explicit declarations/callables replace annotations and reflection.
#![allow(dead_code)]
#[path = "solution_fact_fixture.rs"]
pub mod basic;
use std::sync::Arc;
use timetuck::*;
pub struct InvalidWeights {
    pub code: String,
    pub value_list: Vec<Arc<basic::Value>>,
    pub entity_list: Vec<Arc<basic::Entity>>,
    pub score: Option<SimpleScore>,
    pub constraint_weight_overrides: ConstraintWeightOverrides<SimpleScore>,
    pub second_constraint_weight_overrides: ConstraintWeightOverrides<SimpleScore>,
}
pub fn invalid_weights() -> Result<ComposedPlanningSolution, SolutionCompositionError> {
    let mut members = basic::standard();
    members.extend([
        SolutionMemberDefinition::constraint_weights::<SimpleScore>("constraintWeightOverrides"),
        SolutionMemberDefinition::constraint_weights::<SimpleScore>(
            "secondConstraintWeightOverrides",
        ),
    ]);
    basic::compile::<InvalidWeights>(members, &[basic::t::<basic::Entity>()])
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FirstValue {
    pub code: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SecondValue {
    pub code: String,
    pub strength: i32,
}
pub struct FirstEntity {
    pub code: String,
    pub value_list: Vec<FirstValue>,
    pub difficulty: i32,
}
pub struct SecondEntity {
    pub code: String,
    pub basic_value: Option<SecondValue>,
    pub second_basic_value: Option<SecondValue>,
}
pub struct Mixed {
    pub value_list: Vec<FirstValue>,
    pub other_value_list: Vec<SecondValue>,
    pub entity_list: Vec<FirstEntity>,
    pub other_entity_list: Vec<SecondEntity>,
    pub score: Option<SimpleScore>,
}
pub fn mixed() -> ComposedPlanningSolution {
    let entities = PlanningComposition::new(vec![
        PlanningComponent::registered::<FirstEntity>(vec![PlanningVariableDefinition::list::<
            FirstValue,
        >("valueList", false)]),
        PlanningComponent::registered::<SecondEntity>(vec![
            PlanningVariableDefinition::basic::<SecondValue>("basicValue", false),
            PlanningVariableDefinition::basic::<SecondValue>("secondBasicValue", false),
        ]),
    ])
    .unwrap();
    SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Mixed>(vec![
        SolutionMemberDefinition::value_range::<FirstValue>("valueList", "valueRange")
            .as_problem_fact_collection(),
        SolutionMemberDefinition::value_range::<SecondValue>("otherValueList", "otherValueRange")
            .as_problem_fact_collection(),
        SolutionMemberDefinition::entity_collection::<FirstEntity>("entityList"),
        SolutionMemberDefinition::entity_collection::<SecondEntity>("otherEntityList"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ])])
    .unwrap()
    .compile::<Mixed>(
        &entities,
        &[basic::t::<FirstEntity>(), basic::t::<SecondEntity>()],
        SolutionCompositionPolicy::default(),
        PlanningCompositionPolicy::default(),
    )
    .unwrap()
}
pub fn mixed_ranges() -> Vec<SolutionFieldRange> {
    vec![
        SolutionFieldRange::new::<FirstEntity>("valueList", vec!["valueRange".into()]),
        SolutionFieldRange::new::<SecondEntity>("basicValue", vec!["otherValueRange".into()]),
        SolutionFieldRange::new::<SecondEntity>("secondBasicValue", vec!["otherValueRange".into()]),
    ]
}
pub struct MissingEntity {
    pub id: String,
    pub value: Option<Arc<MissingValue>>,
    pub duration_in_days: i64,
}
impl MissingEntity {
    pub fn update_duration_in_days(&self) -> i64 {
        self.value
            .as_ref()
            .map_or(0, |v| v.duration_seconds / 86_400)
    }
}
/// Dates are integral days relative to 2025-04-29 18:40; duration retains seconds.
pub struct MissingValue {
    pub id: String,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub invalid: bool,
    pub entity_list: Vec<Arc<MissingEntity>>,
    pub duration_seconds: i64,
}
impl MissingValue {
    pub fn calculate_start_time(&self) -> i64 {
        if self.entity_list.is_empty() {
            10
        } else {
            self.entity_list.len() as i64
        }
    }
}
pub struct MissingSolution {
    pub entities: Vec<Arc<MissingEntity>>,
    pub values: Vec<Arc<MissingValue>>,
    pub score: Option<HardSoftScore>,
}
pub fn missing_layout() -> ComposedPlanningSolution {
    let entities = PlanningComposition::new(vec![
        PlanningComponent::registered::<MissingEntity>(vec![
            PlanningVariableDefinition::basic::<MissingValue>("value", false),
            PlanningVariableDefinition::shadow::<i64>("durationInDays"),
        ]),
        PlanningComponent::registered::<MissingValue>(vec![
            PlanningVariableDefinition::shadow::<i64>("startTime"),
            PlanningVariableDefinition::shadow::<i64>("endTime"),
            PlanningVariableDefinition::shadow::<bool>("invalid"),
            PlanningVariableDefinition::shadow::<Vec<Arc<MissingEntity>>>("entityList"),
        ]),
    ])
    .unwrap();
    SolutionComposition::new(vec![
        PlanningSolutionComponent::registered::<MissingSolution>(vec![
            SolutionMemberDefinition::entity_collection::<MissingEntity>("entities"),
            SolutionMemberDefinition::entity_collection::<MissingValue>("values"),
            SolutionMemberDefinition::value_range::<MissingValue>("valueProvider", "valueRange"),
            SolutionMemberDefinition::score::<HardSoftScore>("score"),
        ]),
    ])
    .unwrap()
    .compile::<MissingSolution>(
        &entities,
        &[basic::t::<MissingEntity>(), basic::t::<MissingValue>()],
        SolutionCompositionPolicy::default(),
        PlanningCompositionPolicy::default(),
    )
    .unwrap()
}
pub fn missing_descriptor() -> Result<ComposedPlanningSolution, NamedSupplierError> {
    let layout = missing_layout();
    let mut registry = NamedShadowSuppliers::<MissingSolution>::new(layout.schema())?;
    registry.register("updateDurationInDays", |e: &MissingEntity, _| {
        Some(e.update_duration_in_days())
    })?;
    registry.register("calculateStartTime", |v: &MissingValue, _| {
        Some(v.calculate_start_time())
    })?;
    registry.validate(&[
        (
            basic::t::<MissingEntity>(),
            "durationInDays",
            "updateDurationInDays",
        ),
        (
            basic::t::<MissingValue>(),
            "startTime",
            "calculateStartTime",
        ),
        (basic::t::<MissingValue>(), "endTime", "calculateEndTime"),
    ])?;
    Ok(layout)
}
