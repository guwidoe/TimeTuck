// Complete selected Community TEST model behavior; Apache-2.0, see provenance.
use std::rc::Rc;
use timetuck::*;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value(pub String);
impl Value {
    pub fn new(s: &str) -> Rc<Self> {
        Rc::new(Self(s.into()))
    }
}
#[derive(Default)]
pub struct Entity {
    pub value: Option<Rc<Value>>,
}
impl Entity {
    pub fn get_value(&self) -> Option<Rc<Value>> {
        self.value.clone()
    }
    pub fn set_value(&mut self, v: Option<Rc<Value>>) {
        self.value = v;
    }
    pub fn update_value(&mut self) {
        let code = &self.value.as_ref().unwrap().0;
        self.value = Some(Rc::new(Value(format!("{code}/{code}"))));
    }
}
pub struct GizmoEntity {
    pub id: String,
    pub value: Option<Rc<Value>>,
    pub is_pinned: bool,
}
impl GizmoEntity {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            value: None,
            is_pinned: false,
        }
    }
    pub fn id(&self) -> String {
        self.id.clone()
    }
    pub fn pinned(&self) -> bool {
        self.is_pinned
    }
    pub fn set_pinned(&mut self, v: bool) {
        self.is_pinned = v;
    }
}
pub fn field_value(e: &GizmoEntity) -> Option<Rc<Value>> {
    e.value.clone()
}
pub fn field_set_value(e: &mut GizmoEntity, v: Option<Rc<Value>>) {
    e.value = v;
}
pub fn field_pin(e: &GizmoEntity) -> bool {
    e.is_pinned
}
pub fn variable() -> Vec<CompiledMemberAnnotation> {
    vec![CompiledMemberAnnotation::PlanningVariable { range_ids: vec![] }]
}
pub fn value_registration() -> CompiledMemberRegistration<Entity, Option<Rc<Value>>> {
    CompiledMemberRegistration::new(
        "getValue",
        CompiledMemberKind::Method,
        vec![CompiledMemberAnnotation::PlanningVariable {
            range_ids: vec!["valueRange".into()],
        }],
        CompiledMemberBody::Read(Entity::get_value),
    )
    .with_setter(Entity::set_value)
}
#[derive(Default)]
pub struct RangeEntity {
    pub values: Vec<Rc<Value>>,
}
#[derive(Default)]
pub struct RangeSolution;
impl RangeEntity {
    pub fn values(&self, _: &RangeSolution) -> Vec<Rc<Value>> {
        self.values.clone()
    }
    pub fn invalid_values(&self, _: &RangeSolution, _: &RangeSolution) -> Vec<Rc<Value>> {
        self.values.clone()
    }
}
pub fn range_registration() -> CompiledMemberRegistration<RangeEntity, Vec<Rc<Value>>, RangeSolution>
{
    CompiledMemberRegistration::new(
        "getValueRange",
        CompiledMemberKind::Method,
        vec![CompiledMemberAnnotation::ValueRangeProvider {
            id: "valueRange".into(),
        }],
        CompiledMemberBody::ReadOne(RangeEntity::values),
    )
}
#[derive(Default)]
pub struct BaseSolution {
    pub values: Vec<Rc<Value>>,
}
#[derive(Default)]
pub struct ExtendedSolution {
    pub base: BaseSolution,
}
#[derive(Default)]
pub struct BaseEntity;
impl BaseEntity {
    pub fn values(&self, solution: &ExtendedSolution) -> Vec<Rc<Value>> {
        solution.base.values.clone()
    }
    pub fn other_values(&self, solution: &BaseSolution) -> Vec<Rc<Value>> {
        solution.values.clone()
    }
}
#[derive(Default)]
pub struct ChildEntity {
    pub base: BaseEntity,
}
pub fn inherited_values(e: &ChildEntity, s: &ExtendedSolution) -> Vec<Rc<Value>> {
    e.base.values(s)
}
pub fn inherited_other_values(e: &ChildEntity, s: &BaseSolution) -> Vec<Rc<Value>> {
    e.base.other_values(s)
}
