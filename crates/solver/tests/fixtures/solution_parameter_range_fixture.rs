// Adapted TEST-only solution-parameter provider declarations, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::sync::Arc;
use timetuck::*;
#[derive(Eq, PartialEq)]
pub struct Value(pub &'static str);
pub struct Entity {
    pub range: Vec<Arc<Value>>,
    pub value: Option<Arc<Value>>,
}
pub struct Solution {
    pub entities: Vec<Arc<Entity>>,
}
impl Entity {
    pub fn range(&self, _solution: &Solution) -> Vec<Arc<Value>> {
        self.range.clone()
    }
}
pub struct BaseEntity {
    pub value: Option<Arc<Value>>,
}
pub struct ChildEntity {
    pub base: BaseEntity,
}
pub struct BaseSolution {
    pub values: Vec<Arc<Value>>,
    pub entities: Vec<Arc<ChildEntity>>,
}
pub struct ExtendedSolution {
    pub base: BaseSolution,
}
impl BaseEntity {
    pub fn range(&self, solution: &ExtendedSolution) -> Vec<Arc<Value>> {
        solution.base.values.clone()
    }
    pub fn other_range(&self, solution: &BaseSolution) -> Vec<Arc<Value>> {
        solution.values.clone()
    }
}
pub fn plain() -> SolutionEntityRangeAccessors<'static, Solution, Arc<Value>> {
    let schema = Arc::new(
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
        ])])
        .unwrap(),
    );
    let mut a = SolutionEntityRangeAccessors::new(
        schema,
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["valueRange".into()],
        )],
    )
    .unwrap();
    a.entities(|s: &Solution| {
        s.entities
            .iter()
            .map(|e| (e.clone(), vec![("value".into(), e.value.clone())]))
            .collect()
    })
    .unwrap();
    a.provider("valueRange", |s, e: &Entity| e.range(s))
        .unwrap();
    a
}
pub fn extended() -> SolutionEntityRangeAccessors<'static, ExtendedSolution, Arc<Value>> {
    let components = PlanningComposition::new(vec![
        PlanningComponent::registered::<BaseEntity>(vec![PlanningVariableDefinition::basic::<
            Arc<Value>,
        >("value", false)]),
        PlanningComponent::plain::<ChildEntity>(vec![])
            .with_parent::<BaseEntity>(PlanningParentKind::Owned),
    ])
    .unwrap();
    let schema = Arc::new(
        components
            .compile::<ExtendedSolution>(
                &[
                    PlanningType::of::<BaseEntity>(),
                    PlanningType::of::<ChildEntity>(),
                ],
                PlanningCompositionPolicy::default(),
            )
            .unwrap()
            .into_schema(),
    );
    let mut a = SolutionEntityRangeAccessors::new(
        schema,
        vec![
            SolutionFieldRange::new::<BaseEntity>(
                "value",
                vec!["valueRange".into(), "otherValueRange".into()],
            ),
            SolutionFieldRange::new::<ChildEntity>(
                "value",
                vec!["valueRange".into(), "otherValueRange".into()],
            ),
        ],
    )
    .unwrap();
    a.entities::<BaseEntity>(|_| vec![]).unwrap();
    a.entities(|s: &ExtendedSolution| {
        s.base
            .entities
            .iter()
            .map(|e| (e.clone(), vec![("value".into(), e.base.value.clone())]))
            .collect()
    })
    .unwrap();
    a.provider("valueRange", |s, e: &BaseEntity| e.range(s))
        .unwrap();
    a.provider("otherValueRange", |s, e: &BaseEntity| {
        e.other_range(&s.base)
    })
    .unwrap();
    a.provider("valueRange", |s, e: &ChildEntity| e.base.range(s))
        .unwrap();
    a.provider("otherValueRange", |s, e: &ChildEntity| {
        e.base.other_range(&s.base)
    })
    .unwrap();
    a
}
