// Adapted TEST-only polymorphic and no-inverse list fixtures, Apache-2.0.
#![allow(dead_code)]
use std::{any::Any, sync::Arc};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub code: String,
}
pub struct BaseEntity {
    pub code: String,
    pub value: Option<Arc<Value>>,
}
pub struct ChildEntity {
    pub base: BaseEntity,
    pub extra_object: Option<Arc<dyn Any + Send + Sync>>,
}
pub enum EntityObject {
    Base(Arc<BaseEntity>),
    Child(Arc<ChildEntity>),
}
pub struct BaseSolution {
    pub code: String,
    pub value_list: Vec<Arc<Value>>,
    pub entity_list: Vec<EntityObject>,
    pub score: Option<SimpleScore>,
    pub weights: ConstraintWeightOverrides<SimpleScore>,
}
pub struct ExtendedSolution {
    pub base: BaseSolution,
    pub extra_object: Option<Arc<dyn Any + Send + Sync>>,
}
pub fn polymorphic_layout() -> Arc<ComposedPlanningSolution> {
    let entities = PlanningComposition::new(vec![
        PlanningComponent::registered::<BaseEntity>(vec![PlanningVariableDefinition::basic::<
            Arc<Value>,
        >("value", false)]),
        PlanningComponent::plain::<ChildEntity>(vec![])
            .with_parent::<BaseEntity>(PlanningParentKind::Owned),
    ])
    .unwrap();
    Arc::new(
        SolutionComposition::new(vec![
            PlanningSolutionComponent::registered::<BaseSolution>(vec![
                SolutionMemberDefinition::value_range::<Arc<Value>>("valueList", "valueRange")
                    .as_problem_fact_collection(),
                SolutionMemberDefinition::entities(
                    "entityList",
                    vec![
                        PlanningType::of::<BaseEntity>(),
                        PlanningType::of::<ChildEntity>(),
                    ],
                    false,
                ),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
                SolutionMemberDefinition::constraint_weights::<SimpleScore>(
                    "constraintWeightOverrides",
                ),
            ]),
            PlanningSolutionComponent::plain::<ExtendedSolution>(vec![])
                .with_parent::<BaseSolution>(),
        ])
        .unwrap()
        .compile::<ExtendedSolution>(
            &entities,
            &[
                PlanningType::of::<BaseEntity>(),
                PlanningType::of::<ChildEntity>(),
            ],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
pub fn polymorphic_solution() -> ExtendedSolution {
    let v1 = Arc::new(Value { code: "v1".into() });
    let v2 = Arc::new(Value { code: "v2".into() });
    let child = |code: &str| {
        Arc::new(ChildEntity {
            base: BaseEntity {
                code: code.into(),
                value: Some(v1.clone()),
            },
            extra_object: None,
        })
    };
    ExtendedSolution {
        base: BaseSolution {
            code: "s1".into(),
            value_list: vec![v1.clone(), v2],
            entity_list: vec![
                EntityObject::Child(child("Cat")),
                EntityObject::Base(Arc::new(BaseEntity {
                    code: "Animal".into(),
                    value: Some(v1.clone()),
                })),
                EntityObject::Child(child("Dog")),
            ],
            score: None,
            weights: ConstraintWeightOverrides::default(),
        },
        extra_object: None,
    }
}
pub fn bind_polymorphic(
    layout: Arc<ComposedPlanningSolution>,
    s: &ExtendedSolution,
) -> BoundBasicSolution<Arc<Value>> {
    let mut reads = SolutionFactAccessors::new(layout.clone()).unwrap();
    reads
        .collection("valueList", |s: &ExtendedSolution| {
            s.base
                .value_list
                .iter()
                .cloned()
                .map(Arc::new)
                .collect::<Vec<_>>()
        })
        .unwrap();
    let objects = s
        .base
        .entity_list
        .iter()
        .map(|e| match e {
            EntityObject::Base(e) => {
                SolutionEntityRecord::new(e.clone(), vec![("value".into(), e.value.clone())])
            }
            EntityObject::Child(e) => {
                SolutionEntityRecord::new(e.clone(), vec![("value".into(), e.base.value.clone())])
            }
        })
        .collect();
    layout
        .bind_basic_with_facts(
            vec![SolutionCollection {
                member: "entityList".into(),
                entities: objects,
            }],
            vec![SolutionRange {
                member: "valueList".into(),
                values: s.base.value_list.clone(),
            }],
            vec![
                SolutionFieldRange::new::<BaseEntity>("value", vec!["valueRange".into()]),
                SolutionFieldRange::new::<ChildEntity>("value", vec!["valueRange".into()]),
            ],
            reads.read(s).unwrap(),
        )
        .unwrap()
}
#[derive(Debug)]
pub struct NoInverseValue {
    pub code: String,
    pub index: Option<usize>,
}
impl PartialEq for NoInverseValue {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
    }
}
impl Eq for NoInverseValue {}
impl std::hash::Hash for NoInverseValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.code, state);
    }
}
pub struct NoInverseOwner {
    pub code: String,
    pub value_list: Vec<Arc<NoInverseValue>>,
    pub pinned: bool,
}
pub struct NoInverseSolution {
    pub value_list: Vec<Arc<NoInverseValue>>,
    pub entity_list: Vec<Arc<NoInverseOwner>>,
    pub score: Option<SimpleScore>,
}
pub fn no_inverse_layout() -> Arc<ComposedPlanningSolution> {
    let entities = PlanningComposition::new(vec![
        PlanningComponent::registered::<NoInverseOwner>(vec![PlanningVariableDefinition::list::<
            Arc<NoInverseValue>,
        >("valueList", false)]),
        PlanningComponent::registered::<NoInverseValue>(vec![
            PlanningVariableDefinition::shadow::<usize>("index"),
        ]),
    ])
    .unwrap();
    Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<
            NoInverseSolution,
        >(vec![
            SolutionMemberDefinition::entity_collection::<NoInverseOwner>("entityList"),
            SolutionMemberDefinition::entity_collection::<NoInverseValue>("valueList"),
            SolutionMemberDefinition::value_range::<Arc<NoInverseValue>>(
                "valueProvider",
                "valueRange",
            ),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<NoInverseSolution>(
            &entities,
            &[
                PlanningType::of::<NoInverseOwner>(),
                PlanningType::of::<NoInverseValue>(),
            ],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
pub fn no_inverse_solution() -> NoInverseSolution {
    let v1 = Arc::new(NoInverseValue {
        code: "v1".into(),
        index: Some(0),
    });
    let v2 = Arc::new(NoInverseValue {
        code: "v2".into(),
        index: None,
    });
    NoInverseSolution {
        value_list: vec![v1.clone(), v2],
        entity_list: vec![
            Arc::new(NoInverseOwner {
                code: "e1".into(),
                value_list: vec![v1],
                pinned: false,
            }),
            Arc::new(NoInverseOwner {
                code: "e2".into(),
                value_list: vec![],
                pinned: false,
            }),
        ],
        score: None,
    }
}
pub type NoInverseInputs = (
    Vec<SolutionListCollection<Arc<NoInverseValue>>>,
    Vec<SolutionRange<Arc<NoInverseValue>>>,
    Vec<SolutionFieldRange>,
);
pub fn no_inverse_inputs(s: &NoInverseSolution) -> NoInverseInputs {
    (
        vec![
            SolutionListCollection {
                member: "entityList".into(),
                entities: s
                    .entity_list
                    .iter()
                    .map(|e| {
                        SolutionListEntityRecord::new(
                            e.clone(),
                            vec![("valueList".into(), e.value_list.clone())],
                        )
                        .with_pin_policy(ListPinPolicy {
                            fully_pinned: e.pinned,
                            prefix: 0,
                        })
                    })
                    .collect(),
            },
            SolutionListCollection {
                member: "valueList".into(),
                entities: s
                    .value_list
                    .iter()
                    .map(|v| SolutionListEntityRecord::new(v.clone(), vec![]))
                    .collect(),
            },
        ],
        vec![SolutionRange {
            member: "valueProvider".into(),
            values: s.value_list.clone(),
        }],
        vec![SolutionFieldRange::new::<NoInverseOwner>(
            "valueList",
            vec!["valueRange".into()],
        )],
    )
}
pub fn bind_no_inverse(
    layout: Arc<ComposedPlanningSolution>,
    s: &NoInverseSolution,
) -> BoundListSolution<Arc<NoInverseValue>> {
    let facts = SolutionFactAccessors::new(layout.clone())
        .unwrap()
        .read(s)
        .unwrap();
    let (c, r, f) = no_inverse_inputs(s);
    layout.bind_list_with_facts(c, r, f, facts).unwrap()
}
