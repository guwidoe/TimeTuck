// Adapted Community TEST-only interface/record fixtures and calculator/provider.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::sync::Arc;
use timetuck::*;
pub trait ValueApi: Send + Sync {}
pub struct InterfaceValue(pub Arc<dyn ValueApi>);
impl PartialEq for InterfaceValue {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for InterfaceValue {}
pub trait EntityApi: Send + Sync {
    fn id(&self) -> &str;
    fn value(&self) -> Option<Arc<InterfaceValue>>;
}
pub struct InterfaceEntity(pub Arc<dyn EntityApi>);
pub trait SolutionApi: Send + Sync {
    fn entities(&self) -> Vec<Arc<InterfaceEntity>>;
    fn values(&self) -> Vec<Arc<InterfaceValue>>;
    fn score(&self) -> Option<SimpleScore>;
}
pub struct InterfaceSolution(pub Arc<dyn SolutionApi>);
struct EmptyValue;
impl ValueApi for EmptyValue {}
struct AnonymousEntity;
impl EntityApi for AnonymousEntity {
    fn id(&self) -> &str {
        "entity"
    }
    fn value(&self) -> Option<Arc<InterfaceValue>> {
        None
    }
}
struct AnonymousSolution {
    entities: Vec<Arc<InterfaceEntity>>,
    values: Vec<Arc<InterfaceValue>>,
}
impl SolutionApi for AnonymousSolution {
    fn entities(&self) -> Vec<Arc<InterfaceEntity>> {
        self.entities.clone()
    }
    fn values(&self) -> Vec<Arc<InterfaceValue>> {
        self.values.clone()
    }
    fn score(&self) -> Option<SimpleScore> {
        None
    }
}
pub fn interface_solution() -> InterfaceSolution {
    InterfaceSolution(Arc::new(AnonymousSolution {
        entities: vec![Arc::new(InterfaceEntity(Arc::new(AnonymousEntity)))],
        values: vec![Arc::new(InterfaceValue(Arc::new(EmptyValue)))],
    }))
}
pub fn basic_layout<S: 'static, E: 'static, V: 'static>(
    facts: bool,
) -> Arc<ComposedPlanningSolution> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<E>(vec![
        PlanningVariableDefinition::basic::<V>("value", false),
    ])])
    .unwrap();
    let range = SolutionMemberDefinition::value_range::<V>("valueList", "valueRange");
    Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<S>(vec![
            SolutionMemberDefinition::entity_collection::<E>("entityList"),
            if facts {
                range.as_problem_fact_collection()
            } else {
                range
            },
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<S>(
            &entities,
            &[PlanningType::of::<E>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    )
}
pub fn interface_getters<'a>(
    layout: Arc<ComposedPlanningSolution>,
) -> SolutionBasicAccessors<'a, InterfaceSolution, Arc<InterfaceValue>> {
    let mut getters = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<InterfaceEntity>(
            "value",
            vec!["valueRange".into()],
        )],
    )
    .unwrap();
    getters
        .entities("entityList", |s: &InterfaceSolution| {
            s.0.entities()
                .into_iter()
                .map(|e| {
                    let v = e.0.value();
                    (e, vec![("value".into(), v)])
                })
                .collect()
        })
        .unwrap();
    getters.range("valueList", |s| s.0.values()).unwrap();
    getters
}
pub type InterfaceConstraint = StreamConstraint<
    ConcatId<usize, usize>,
    WorkingBasicObject<InterfaceEntity, Arc<InterfaceValue>>,
>;
pub fn interface_provider<'a>(
    builder: &mut StreamGraphBuilder<'a, SimpleScore>,
    model: &WorkingBasicModelStream<InterfaceEntity, Arc<InterfaceValue>>,
) -> Result<InterfaceConstraint, GraphError> {
    builder.score("Always penalize", model.port(), |row| {
        assert_eq!(row.object().0.id(), "entity");
        SimpleScore::new(-1)
    })
}
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct RecordValue {
    pub code: String,
}
pub struct RecordEntity {
    pub code: String,
    pub value: Arc<RecordValue>,
}
pub struct RecordSolution {
    pub code: String,
    pub values: Vec<Arc<RecordValue>>,
    pub entities: Vec<Arc<RecordEntity>>,
    pub score: Option<SimpleScore>,
}
pub fn record_solution() -> RecordSolution {
    let values: Vec<_> = (0..5)
        .map(|i| {
            Arc::new(RecordValue {
                code: format!("Generated Value {i}"),
            })
        })
        .collect();
    let entities = (0..7)
        .map(|i| {
            Arc::new(RecordEntity {
                code: format!("Generated Entity {i}"),
                value: values[i % 5].clone(),
            })
        })
        .collect();
    RecordSolution {
        code: "Generated Solution 0".into(),
        values,
        entities,
        score: None,
    }
}
pub fn record_getters<'a>(
    layout: Arc<ComposedPlanningSolution>,
) -> SolutionBasicAccessors<'a, RecordSolution, Arc<RecordValue>> {
    let mut getters = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<RecordEntity>(
            "value",
            vec!["valueRange".into()],
        )],
    )
    .unwrap();
    getters
        .entities("entityList", |s: &RecordSolution| {
            s.entities
                .iter()
                .map(|e| (e.clone(), vec![("value".into(), Some(e.value.clone()))]))
                .collect()
        })
        .unwrap();
    getters.range("valueList", |s| s.values.clone()).unwrap();
    getters
        .facts_mut()
        .collection("valueList", |s: &RecordSolution| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    getters
}
pub fn record_calculator(s: &RecordSolution) -> SimpleScore {
    SimpleScore::new(s.entities.len() as i64)
}
