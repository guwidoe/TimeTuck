// TEST-visible extended and invalid-parameter fixtures adapted from Community.
// Apache-2.0; see compat/provenance.json. No source production implementation.
#![allow(dead_code)]
use timetuck::*;
pub struct BaseValue {
    pub code: String,
    pub previous: Option<Box<BaseValue>>,
}
pub struct SubValue {
    pub base: BaseValue,
    pub code_sum: String,
}
impl SubValue {
    pub fn sample() -> Self {
        Self {
            base: BaseValue {
                code: "value".into(),
                previous: None,
            },
            code_sum: "value".into(),
        }
    }
}
pub fn hierarchy() -> SourceTypeHierarchy {
    let mut hierarchy = SourceTypeHierarchy::default();
    hierarchy
        .register::<SubValue, BaseValue>(|value| &value.base)
        .unwrap();
    hierarchy
}
pub struct TestdataInvalidDeclarativeParameterSolution {
    pub entities: Vec<TestdataInvalidDeclarativeParameterEntity>,
    pub values: Vec<TestdataInvalidDeclarativeParameterValue>,
    pub score: Option<SimpleScore>,
}
pub struct TestdataInvalidDeclarativeParameterEntity {
    pub values: Vec<TestdataInvalidDeclarativeParameterValue>,
}
pub struct TestdataInvalidDeclarativeParameterValue {
    pub fact: Option<Box<Self>>,
    pub group: Vec<Self>,
    pub previous: Option<Box<Self>>,
    pub invalid_parameter: Option<Box<Self>>,
    pub inconsistent: bool,
}
pub fn invalid_schema() -> PlanningSchema {
    PlanningSchema::new::<TestdataInvalidDeclarativeParameterSolution>(vec![
        PlanningEntityDefinition::of::<TestdataInvalidDeclarativeParameterEntity>(vec![
            PlanningVariableDefinition::list::<TestdataInvalidDeclarativeParameterValue>(
                "values", false,
            ),
        ]),
        PlanningEntityDefinition::of::<TestdataInvalidDeclarativeParameterValue>(vec![
            PlanningVariableDefinition::shadow::<TestdataInvalidDeclarativeParameterValue>(
                "previous",
            ),
            PlanningVariableDefinition::shadow::<TestdataInvalidDeclarativeParameterValue>(
                "invalidParameter",
            ),
        ]),
    ])
    .unwrap()
}
