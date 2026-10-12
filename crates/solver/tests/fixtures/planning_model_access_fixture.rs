// Adapted Community TEST-only extended solution/entity and array-range helpers.
// Apache-2.0; provenance and modifications in compat/provenance.json.
use std::any::Any;
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListValue(pub i32);
pub struct ListEntityType;
pub struct ArraySolution {
    pub values: [ListValue; 3],
}
pub fn list_schema() -> PlanningSchema {
    PlanningSchema::new::<ArraySolution>(vec![PlanningEntityDefinition::of::<ListEntityType>(
        vec![PlanningVariableDefinition::list::<ListValue>(
            "valueList",
            false,
        )],
    )])
    .unwrap()
}
#[derive(Debug, Eq, PartialEq)]
pub struct BaseEntity {
    pub code: String,
}
impl BaseEntity {
    pub fn new(code: &str) -> Self {
        Self { code: code.into() }
    }
}
#[derive(Debug)]
pub struct ChildEntity {
    pub base: BaseEntity,
}
impl ChildEntity {
    pub fn new(code: &str) -> Self {
        Self {
            base: BaseEntity::new(code),
        }
    }
}
pub struct ExtendedSolution {
    pub entity: Option<BaseEntity>,
    pub sub_entity: Option<ChildEntity>,
    pub entities: Vec<BaseEntity>,
    pub sub_entities: Vec<ChildEntity>,
    pub raw: Vec<Box<dyn Any>>,
    pub objects: Vec<Box<dyn Any>>,
}
pub fn extraction_schema() -> PlanningSchema {
    PlanningSchema::new::<ExtendedSolution>(vec![
        PlanningEntityDefinition::of::<BaseEntity>(vec![PlanningVariableDefinition::basic::<i32>(
            "value", false,
        )]),
        PlanningEntityDefinition::of::<ChildEntity>(vec![
            PlanningVariableDefinition::basic::<i32>("value", false),
        ]),
    ])
    .unwrap()
}
pub fn extractor() -> BorrowedEntityExtractor<ExtendedSolution, BaseEntity> {
    let mut hierarchy = SourceTypeHierarchy::default();
    hierarchy
        .register::<ChildEntity, BaseEntity>(|e| &e.base)
        .unwrap();
    let mut access = BorrowedEntityExtractor::new(&extraction_schema()).unwrap();
    access.project::<ChildEntity>(&hierarchy).unwrap();
    access
        .member("entity", |s: &ExtendedSolution| {
            s.entity.iter().map(|e| e as &dyn Any).collect()
        })
        .unwrap();
    access
        .member("subEntity", |s: &ExtendedSolution| {
            s.sub_entity.iter().map(|e| e as &dyn Any).collect()
        })
        .unwrap();
    access
        .member("entityList", |s: &ExtendedSolution| {
            s.entities.iter().map(|e| e as &dyn Any).collect()
        })
        .unwrap();
    access
        .member("subEntityList", |s: &ExtendedSolution| {
            s.sub_entities.iter().map(|e| e as &dyn Any).collect()
        })
        .unwrap();
    access
        .member("rawEntityList", |s: &ExtendedSolution| {
            s.raw.iter().map(|e| e.as_ref()).collect()
        })
        .unwrap();
    access
        .member("objectEntityList", |s: &ExtendedSolution| {
            s.objects.iter().map(|e| e.as_ref()).collect()
        })
        .unwrap();
    access
}
pub fn extended_solution() -> ExtendedSolution {
    ExtendedSolution {
        entity: Some(BaseEntity::new("entity-singleton")),
        sub_entity: Some(ChildEntity::new("subEntity-singleton")),
        entities: vec![BaseEntity::new("entity1"), BaseEntity::new("entity2")],
        sub_entities: vec![
            ChildEntity::new("subEntity1"),
            ChildEntity::new("subEntity2"),
            ChildEntity::new("subEntity3"),
        ],
        raw: vec![
            Box::new(ChildEntity::new("subEntity1-R")),
            Box::new(ChildEntity::new("subEntity2-R")),
        ],
        objects: vec![
            Box::new(BaseEntity::new("entity3")),
            Box::new(BaseEntity::new("entity4")),
            Box::new("randomData".to_owned()),
        ],
    }
}
