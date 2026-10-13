// Adapted pinned Community accessor TEST helpers; Apache-2.0.
#[derive(Debug, Eq, PartialEq)]
struct Value(&'static str);
struct Entity {
    value: Arc<Value>,
}
impl Entity {
    fn get_value(&self) -> Arc<Value> {
        self.value.clone()
    }
    fn set_value(&mut self, value: Arc<Value>) {
        self.value = value;
    }
}
struct FieldEntity {
    value: Arc<Value>,
}
impl FieldEntity {
    fn get_value(&self) -> Arc<Value> {
        self.value.clone()
    }
}
struct ParameterSolution;
struct ParameterEntity {
    values: Option<Vec<Arc<Value>>>,
    _value: Option<Arc<Value>>,
}
impl ParameterEntity {
    fn get_value_range(&self, _: &ParameterSolution) -> Option<Vec<Arc<Value>>> {
        self.values.clone()
    }
}
struct BaseSolution {
    values: Vec<Arc<Value>>,
}
struct ExtendedSolution {
    base: BaseSolution,
}
struct BaseEntity {
    _value: Arc<Value>,
}
impl BaseEntity {
    fn get_value_list(&self, solution: &ExtendedSolution) -> Vec<Arc<Value>> {
        solution.base.values.clone()
    }
    fn get_other_value_list(&self, solution: &BaseSolution) -> Vec<Arc<Value>> {
        solution.values.clone()
    }
}
struct ChildEntity {
    base: BaseEntity,
}
fn planning() -> Vec<MemberAnnotation> {
    vec![MemberAnnotation::PlanningVariable {
        range_ids: vec!["valueRange".into()],
    }]
}
fn range(id: &str) -> Vec<MemberAnnotation> {
    vec![MemberAnnotation::ValueRangeProvider { id: id.into() }]
}
