// Adapted Community TEST helper setup. Apache-2.0, see sibling NOTICE.txt.
use std::sync::Arc;
use timetuck::*;
#[derive(Default, Debug)]
pub struct TestModel {
    pub score: Option<SimpleScore>,
    pub constraint_weight_overrides: Option<()>,
}
pub fn setup() -> (TrivialModelConvertor<TestModel>, Arc<TestModel>, Arc<()>) {
    (
        TrivialModelConvertor::new(),
        Arc::new(TestModel::default()),
        Arc::new(()),
    )
}
