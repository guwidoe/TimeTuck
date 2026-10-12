// Adapted TEST-only SolutionDescriptorTest domain fixtures, Apache-2.0.
// Explicit declarations replace annotations/reflection; see compat/provenance.json.
#![allow(dead_code)]
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    marker::PhantomData,
    sync::Arc,
};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Value {
    pub code: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Entity {
    pub code: String,
    pub value: Option<Value>,
}
pub struct ChildEntity {
    pub base: Entity,
    pub sub_value: Option<Value>,
}
pub struct ArrayEntity {
    pub code: String,
    pub value: Option<Value>,
    pub entities: Box<[Arc<ArrayEntity>]>,
}
#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct SetEntity {
    pub code: String,
    pub value: Option<Value>,
}
macro_rules! standard {
    ($($name:ident),+) => {$ (
        pub struct $name {pub code:String,pub value_list:Vec<Arc<Value>>,pub entity_list:Vec<Arc<Entity>>,pub score:Option<SimpleScore>}
    )+};
}
standard!(
    ReadMethod,
    Argument,
    DuplicateFacts,
    DuplicateEntities,
    DuplicateScore,
    ConflictingRole,
    NoFacts
);
impl ReadMethod {
    pub fn create_problem_facts(&self) -> Vec<Arc<String>> {
        ["a", "b", "c"]
            .into_iter()
            .map(|s| Arc::new(s.into()))
            .collect()
    }
}
impl Argument {
    pub fn problem_facts(&self, _invalid_argument: i32) -> Vec<SolutionFactRecord> {
        vec![]
    }
}
pub struct Properties {
    pub code: String,
    pub value_list: Vec<Arc<Value>>,
    pub other_problem_fact_list: Vec<SolutionFactRecord>,
    pub extra_object: Option<SolutionFactRecord>,
    pub entity_list: Vec<Arc<Entity>>,
    pub score: Option<SimpleScore>,
}
pub struct MissingScore {
    pub value_list: Vec<Arc<Value>>,
    pub entity_list: Vec<Arc<Entity>>,
}
pub struct EntityAsFact {
    pub entity: Option<Arc<Entity>>,
    pub values: Vec<Arc<Value>>,
    pub score: Option<SimpleScore>,
}
pub struct EntitiesAsFacts {
    pub entities: Vec<Arc<Entity>>,
    pub values: Vec<Arc<Value>>,
    pub score: Option<SimpleScore>,
}
pub struct EntityArrayAsFacts {
    pub entities: Box<[Arc<Entity>]>,
    pub values: Box<[Arc<Value>]>,
    pub score: Option<SimpleScore>,
}
pub struct Wildcard {
    pub code: String,
    pub extends_value_list: Vec<Arc<Value>>,
    pub supers_value_list: Vec<SolutionFactRecord>,
    pub extends_entity_list: Vec<Arc<Entity>>,
    pub score: Option<SimpleScore>,
}
pub struct UnsupportedWildcard {
    pub code: String,
    pub value_list: Vec<Arc<Value>>,
    pub supers_entity_list: Vec<SolutionEntityRecord<Value>>,
    pub score: Option<SimpleScore>,
}
pub struct Parent {
    pub constraint_weight_overrides: Option<ConstraintWeightOverrides<SimpleScore>>,
    pub code: String,
    pub value_list: Vec<Arc<Value>>,
    pub sub_value_list: Vec<Arc<Value>>,
    pub entity_list: Vec<Arc<Entity>>,
    pub score: Option<SimpleScore>,
}
pub struct Extended {
    pub parent: Parent,
    pub entity: Option<Arc<Entity>>,
    pub sub_entity: Option<Arc<ChildEntity>>,
    pub sub_entity_list: Vec<Arc<ChildEntity>>,
    pub object_entity_list: Vec<SolutionEntityRecord<Value>>,
    pub raw_entity_list: Vec<SolutionEntityRecord<Value>>,
}
pub struct SetSolution {
    pub code: String,
    pub value_set: HashSet<Arc<Value>>,
    pub entity_set: HashSet<Arc<SetEntity>>,
    pub entity_hash_set: HashSet<Arc<SetEntity>>,
    pub entity_linked_hash_set: Vec<Arc<SetEntity>>,
    pub entity_tree_set: BTreeSet<Arc<SetEntity>>,
    pub score: Option<SimpleScore>,
}
pub struct ArraySolution {
    pub code: String,
    pub values: Box<[Arc<Value>]>,
    pub entities: Box<[Arc<ArrayEntity>]>,
    pub score: Option<SimpleScore>,
}
pub struct GenericValue<T> {
    pub code: String,
    pub marker: PhantomData<T>,
}
pub type ComplexValue = GenericValue<HashMap<(), GenericValue<()>>>;
pub struct GenericEntity {
    pub code: String,
    pub value: Option<Arc<GenericValue<()>>>,
    pub sub_type_value: Option<Arc<GenericValue<()>>>,
    pub complex_generic_value: Option<Arc<ComplexValue>>,
}
pub struct GenericSolution {
    pub code: String,
    pub value_list: Vec<Arc<GenericValue<()>>>,
    pub sub_type_value_list: Vec<Arc<GenericValue<()>>>,
    pub complex_generic_value_list: Vec<Arc<ComplexValue>>,
    pub entity_list: Vec<Arc<GenericEntity>>,
    pub score: Option<SimpleScore>,
}
pub struct NoSolution {
    pub code: String,
    pub score: Option<SimpleScore>,
}
pub struct BadCollection {
    pub value_list: Option<Arc<Value>>,
    pub entity_list: Vec<Arc<Entity>>,
    pub score: Option<SimpleScore>,
}

pub fn t<T: 'static>() -> PlanningType {
    PlanningType::of::<T>()
}
pub fn entities() -> PlanningComposition {
    PlanningComposition::new(vec![
        PlanningComponent::registered::<Entity>(vec![PlanningVariableDefinition::basic::<Value>(
            "value", false,
        )]),
        PlanningComponent::registered::<ChildEntity>(vec![PlanningVariableDefinition::basic::<
            Value,
        >("subValue", false)])
        .with_parent::<Entity>(PlanningParentKind::Owned),
        PlanningComponent::registered::<SetEntity>(vec![
            PlanningVariableDefinition::basic::<Value>("value", false),
        ]),
        PlanningComponent::registered::<ArrayEntity>(vec![PlanningVariableDefinition::basic::<
            Value,
        >("value", false)]),
        PlanningComponent::registered::<GenericEntity>(vec![
            PlanningVariableDefinition::basic::<GenericValue<()>>("value", false),
            PlanningVariableDefinition::basic::<GenericValue<()>>("subTypeValue", false),
            PlanningVariableDefinition::basic::<ComplexValue>("complexGenericValue", false),
        ]),
    ])
    .unwrap()
}
pub fn range(name: &str) -> SolutionMemberDefinition {
    SolutionMemberDefinition::value_range::<Value>(name, "valueRange").as_problem_fact_collection()
}
pub fn standard() -> Vec<SolutionMemberDefinition> {
    vec![
        range("valueList"),
        SolutionMemberDefinition::entity_collection::<Entity>("entityList"),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ]
}
pub fn compile<S: 'static>(
    members: Vec<SolutionMemberDefinition>,
    roots: &[PlanningType],
) -> Result<ComposedPlanningSolution, SolutionCompositionError> {
    SolutionComposition::new(vec![PlanningSolutionComponent::registered::<S>(members)])
        .unwrap()
        .compile::<S>(
            &entities(),
            roots,
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
}
pub fn properties() -> ComposedPlanningSolution {
    let mut m = standard();
    m.push(SolutionMemberDefinition::problem_facts(
        "otherProblemFactList",
        vec![],
        false,
    ));
    m.push(SolutionMemberDefinition::problem_facts(
        "extraObject",
        vec![],
        true,
    ));
    compile::<Properties>(m, &[t::<Entity>()]).unwrap()
}
pub fn extended() -> ComposedPlanningSolution {
    let mut parent = standard();
    parent.push(
        SolutionMemberDefinition::value_range::<Value>("subValueList", "subValueRange")
            .as_problem_fact_collection(),
    );
    let child = vec![
        SolutionMemberDefinition::entity::<Entity>("entity"),
        SolutionMemberDefinition::entity::<ChildEntity>("subEntity"),
        SolutionMemberDefinition::entity_collection::<ChildEntity>("subEntityList"),
        SolutionMemberDefinition::entities(
            "rawEntityList",
            vec![t::<Entity>(), t::<ChildEntity>()],
            false,
        ),
        SolutionMemberDefinition::entities(
            "objectEntityList",
            vec![t::<Entity>(), t::<ChildEntity>()],
            false,
        ),
    ];
    SolutionComposition::new(vec![
        PlanningSolutionComponent::registered::<Parent>(parent),
        PlanningSolutionComponent::registered::<Extended>(child).with_parent::<Parent>(),
    ])
    .unwrap()
    .compile::<Extended>(
        &entities(),
        &[t::<Entity>(), t::<ChildEntity>()],
        SolutionCompositionPolicy::default(),
        PlanningCompositionPolicy::default(),
    )
    .unwrap()
}
