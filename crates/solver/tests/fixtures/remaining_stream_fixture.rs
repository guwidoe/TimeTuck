// Adapted TEST-only lavish decimal and unnamed-package solution fixtures.
// Apache-2.0; explicit native registration replaces Java discovery/class loading.
#![allow(dead_code)]
#[path = "solution_fact_fixture.rs"]
pub mod basic;
#[path = "solution_stream_lavish_fixture.rs"]
pub mod lavish;
use std::sync::Arc;
use timetuck::*;
impl PartialEq for lavish::Value {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
    }
}
impl Eq for lavish::Value {}
pub struct DecimalEntity {
    pub base: lavish::Entity,
    pub decimal: Arc<Decimal>,
}
pub struct DecimalSolution {
    pub code: String,
    pub value_groups: Vec<Arc<lavish::ValueGroup>>,
    pub values: Vec<Arc<lavish::Value>>,
    pub entity_groups: Vec<Arc<lavish::EntityGroup>>,
    pub entities: Vec<Arc<DecimalEntity>>,
    pub extra: Vec<Arc<lavish::Extra>>,
    pub score: Option<SimpleScore>,
}
pub fn decimal_solution() -> DecimalSolution {
    let vg = Arc::new(lavish::ValueGroup {
        code: "Generated ValueGroup 0".into(),
    });
    let eg = Arc::new(lavish::EntityGroup {
        code: "Generated EntityGroup 0".into(),
    });
    let values = (0..2)
        .map(|i| {
            Arc::new(lavish::Value {
                code: format!("Generated Value {i}"),
                group: Some(vg.clone()),
            })
        })
        .collect::<Vec<_>>();
    let first = Arc::new("0.01".parse::<Decimal>().unwrap());
    let second = Arc::new("0.01".parse::<Decimal>().unwrap());
    let make = |code: String, value: Arc<lavish::Value>, decimal: Arc<Decimal>| {
        Arc::new(DecimalEntity {
            base: lavish::Entity {
                code,
                group: Some(eg.clone()),
                value: Some(value),
            },
            decimal,
        })
    };
    let entities = vec![
        make(
            "Generated Entity 0".into(),
            values[0].clone(),
            first.clone(),
        ),
        make("Generated Entity 1".into(), values[1].clone(), second),
        make("My Entity 0".into(), values[0].clone(), first),
    ];
    DecimalSolution {
        code: "Generated Solution 0".into(),
        value_groups: vec![vg],
        values,
        entity_groups: vec![eg],
        entities,
        extra: vec![],
        score: None,
    }
}
pub fn decimal_layout() -> Arc<ComposedPlanningSolution> {
    let entities =
        PlanningComposition::new(vec![PlanningComponent::registered::<DecimalEntity>(vec![
            PlanningVariableDefinition::basic::<Arc<lavish::Value>>("value", false),
        ])])
        .unwrap();
    Arc::new(
        SolutionComposition::new(vec![
            PlanningSolutionComponent::registered::<DecimalSolution>(vec![
                SolutionMemberDefinition::entity_collection::<DecimalEntity>("entityList"),
                SolutionMemberDefinition::value_range::<Arc<lavish::Value>>(
                    "valueList",
                    "valueRange",
                )
                .as_problem_fact_collection(),
                SolutionMemberDefinition::problem_fact_collection::<lavish::ValueGroup>(
                    "valueGroupList",
                ),
                SolutionMemberDefinition::problem_fact_collection::<lavish::EntityGroup>(
                    "entityGroupList",
                ),
                SolutionMemberDefinition::problem_fact_collection::<lavish::Extra>("extraList"),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ]),
        ])
        .unwrap()
        .compile::<DecimalSolution>(
            &entities,
            &[PlanningType::of::<DecimalEntity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
pub fn decimal_objects(
    layout: Arc<ComposedPlanningSolution>,
    s: &DecimalSolution,
) -> BoundBasicSolution<Arc<lavish::Value>> {
    let mut facts = SolutionFactAccessors::new(layout.clone()).unwrap();
    facts
        .collection("valueList", |s: &DecimalSolution| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    facts
        .collection("valueGroupList", |s: &DecimalSolution| {
            s.value_groups.clone()
        })
        .unwrap();
    facts
        .collection("entityGroupList", |s| s.entity_groups.clone())
        .unwrap();
    facts.collection("extraList", |s| s.extra.clone()).unwrap();
    layout
        .bind_basic_with_facts(
            vec![SolutionCollection {
                member: "entityList".into(),
                entities: s
                    .entities
                    .iter()
                    .map(|e| {
                        SolutionEntityRecord::new(
                            e.clone(),
                            vec![("value".into(), e.base.value.clone())],
                        )
                    })
                    .collect(),
            }],
            vec![SolutionRange {
                member: "valueList".into(),
                values: s.values.clone(),
            }],
            vec![SolutionFieldRange::new::<DecimalEntity>(
                "value",
                vec!["valueRange".into()],
            )],
            facts.read(s).unwrap(),
        )
        .unwrap()
}
pub struct UnnamedSolution {
    pub code: String,
    pub value_list: Vec<Arc<basic::Value>>,
    pub entity_list: Vec<Arc<basic::Entity>>,
    pub score: Option<SimpleScore>,
}
impl UnnamedSolution {
    pub fn generate() -> Self {
        let values = (0..5)
            .map(|i| {
                Arc::new(basic::Value {
                    code: format!("Generated Value {i}"),
                })
            })
            .collect::<Vec<_>>();
        let entities = (0..7)
            .map(|i| {
                Arc::new(basic::Entity {
                    code: format!("Generated Entity {i}"),
                    value: Some(values[i % 5].as_ref().clone()),
                })
            })
            .collect();
        Self {
            code: "Generated Solution 0".into(),
            value_list: values,
            entity_list: entities,
            score: None,
        }
    }
    pub fn layout() -> Arc<ComposedPlanningSolution> {
        Arc::new(basic::compile::<Self>(basic::standard(), &[basic::t::<basic::Entity>()]).unwrap())
    }
    pub fn bind(&self, layout: Arc<ComposedPlanningSolution>) -> BoundBasicSolution<basic::Value> {
        let mut facts = SolutionFactAccessors::new(layout.clone()).unwrap();
        facts
            .collection("valueList", |s: &Self| s.value_list.clone())
            .unwrap();
        layout
            .bind_basic_with_facts(
                vec![SolutionCollection {
                    member: "entityList".into(),
                    entities: self
                        .entity_list
                        .iter()
                        .map(|e| {
                            SolutionEntityRecord::new(
                                e.clone(),
                                vec![("value".into(), e.value.clone())],
                            )
                        })
                        .collect(),
                }],
                vec![SolutionRange {
                    member: "valueList".into(),
                    values: self.value_list.iter().map(|v| v.as_ref().clone()).collect(),
                }],
                vec![SolutionFieldRange::new::<basic::Entity>(
                    "value",
                    vec!["valueRange".into()],
                )],
                facts.read(self).unwrap(),
            )
            .unwrap()
    }
}
/// Actual provider implementation in a separate native module; source identity
/// never depends on the namespace of the registered planning solution.
pub mod named_provider {
    use super::*;
    pub fn define(
        b: &mut StreamGraphBuilder<'_, SimpleScore>,
        source: &CheckedSolutionStream<basic::Entity>,
    ) -> StreamConstraint<usize, SolutionStreamObject<basic::Entity>> {
        b.score("Always penalize", source.port(), |_| SimpleScore::new(-1))
            .unwrap()
    }
}
