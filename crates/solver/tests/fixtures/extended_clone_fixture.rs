// Adapted Community TEST-only extension/access clone fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::sync::{Arc, Mutex, OnceLock};
use timetuck::*;
#[derive(Eq, PartialEq, Debug)]
pub struct Value(pub &'static str);
// Two distinct base representations; Rust composition replaces Java inheritance.
pub trait BaseData: Send + Sync + 'static {
    fn new(code: &'static str, value: Arc<Value>) -> Self;
    fn code(&self) -> &'static str;
    fn value(&self) -> Arc<Value>;
    fn set_value(&self, value: Arc<Value>);
}
pub struct AnnotatedBase {
    code: &'static str,
    value: Mutex<Arc<Value>>,
}
pub struct ThirdPartyPojo {
    code: &'static str,
    value: Mutex<Arc<Value>>,
}
macro_rules! base {
    ($t:ty) => {
        impl BaseData for $t {
            fn new(code: &'static str, value: Arc<Value>) -> Self {
                Self {
                    code,
                    value: Mutex::new(value),
                }
            }
            fn code(&self) -> &'static str {
                self.code
            }
            fn value(&self) -> Arc<Value> {
                self.value.lock().unwrap().clone()
            }
            fn set_value(&self, value: Arc<Value>) {
                *self.value.lock().unwrap() = value;
            }
        }
    };
}
base!(AnnotatedBase);
base!(ThirdPartyPojo);
pub struct Child<B> {
    pub base: B,
    pub extra: Mutex<Option<ObjectGraphObject>>,
}
pub struct BaseRoot<B> {
    pub code: &'static str,
    pub values: Arc<Vec<Arc<Value>>>,
    pub entities: Mutex<Arc<Vec<Arc<Child<B>>>>>,
    pub score: Option<SimpleScore>,
}
pub struct ExtendedRoot<B> {
    pub base: BaseRoot<B>,
    pub extra: Mutex<Option<ObjectGraphObject>>,
}
pub fn source<B: BaseData>() -> Arc<ExtendedRoot<B>> {
    let values = Arc::new(vec![
        Arc::new(Value("1")),
        Arc::new(Value("2")),
        Arc::new(Value("3")),
    ]);
    let nodes: Vec<_> = [("a", 0), ("b", 0), ("c", 2), ("d", 2)]
        .into_iter()
        .map(|(code, i)| {
            Arc::new(Child {
                base: B::new(code, values[i].clone()),
                extra: Mutex::new(None),
            })
        })
        .collect();
    *nodes[1].extra.lock().unwrap() = Some(Arc::new(String::from("extraObjectOnEntity")));
    *nodes[2].extra.lock().unwrap() = Some(nodes[3].clone());
    *nodes[3].extra.lock().unwrap() = Some(nodes[2].clone());
    Arc::new(ExtendedRoot {
        base: BaseRoot {
            code: "solution",
            values,
            entities: Mutex::new(Arc::new(nodes)),
            score: None,
        },
        extra: Mutex::new(Some(Arc::new(String::from("extraObjectOnSolution")))),
    })
}
pub fn registry<B: BaseData>() -> ObjectGraphCloneRegistry<'static> {
    use ObjectCloneFieldPolicy::Registered;
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<ExtendedRoot<B>>(
        |s| {
            Ok(ExtendedRoot {
                base: BaseRoot {
                    code: s.base.code,
                    values: s.base.values.clone(),
                    entities: Mutex::new(Arc::new(vec![])),
                    score: s.base.score,
                },
                extra: Mutex::new(None),
            })
        },
        |s, c| {
            for e in s.base.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            c.optional_object(&s.extra.lock().unwrap(), Registered)
        },
        |s, t, c| {
            *t.base.entities.lock().unwrap() =
                Arc::new(c.collection(&s.base.entities.lock().unwrap())?);
            *t.extra.lock().unwrap() = c.optional_object(&s.extra.lock().unwrap(), Registered)?;
            Ok(())
        },
    )
    .unwrap();
    r.register::<Child<B>>(
        |s| {
            Ok(Child {
                base: B::new(s.base.code(), s.base.value()),
                extra: Mutex::new(None),
            })
        },
        |s, c| c.optional_object(&s.extra.lock().unwrap(), Registered),
        |s, t, c| {
            *t.extra.lock().unwrap() = c.optional_object(&s.extra.lock().unwrap(), Registered)?;
            Ok(())
        },
    )
    .unwrap();
    r
}
pub fn accessors<B: BaseData>() -> SolutionBasicAccessors<'static, Arc<ExtendedRoot<B>>, Arc<Value>>
{
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Child<B>>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<
            Arc<ExtendedRoot<B>>,
        >(vec![
            SolutionMemberDefinition::entity_collection::<Child<B>>("entities"),
            SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
                .as_problem_fact_collection(),
            SolutionMemberDefinition::score::<SimpleScore>("score"),
        ])])
        .unwrap()
        .compile::<Arc<ExtendedRoot<B>>>(
            &entities,
            &[PlanningType::of::<Child<B>>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut a = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<Child<B>>(
            "value",
            vec!["range".into()],
        )],
    )
    .unwrap();
    a.entities("entities", |s: &Arc<ExtendedRoot<B>>| {
        s.base
            .entities
            .lock()
            .unwrap()
            .iter()
            .map(|e| (e.clone(), vec![("value".into(), Some(e.base.value()))]))
            .collect()
    })
    .unwrap();
    a.range("values", |s| s.base.values.as_ref().clone())
        .unwrap();
    a.facts_mut()
        .collection("values", |s| {
            s.base
                .values
                .iter()
                .cloned()
                .map(Arc::new)
                .collect::<Vec<_>>()
        })
        .unwrap();
    a
}
pub const STATIC_FINAL: &str = "staticFinalFieldValue";
static STATIC_FIELD: OnceLock<Mutex<Option<Arc<()>>>> = OnceLock::new();
pub fn set_static(value: Arc<()>) {
    *STATIC_FIELD
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap() = Some(value);
}
pub fn get_static() -> Arc<()> {
    STATIC_FIELD
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .clone()
}
pub struct AccessRoot {
    pub base: BaseRoot<AnnotatedBase>,
    final_field: String,
    read_write_field: String,
    private_field: Option<String>,
    pub public_field: Option<String>,
    private_property_field: Option<String>,
    friendly_property_field: Option<String>,
    protected_property_field: Option<String>,
    public_property_field: Option<String>,
}
impl AccessRoot {
    pub fn new() -> Self {
        let source = source::<AnnotatedBase>();
        let base = BaseRoot {
            code: source.base.code,
            values: source.base.values.clone(),
            entities: Mutex::new(source.base.entities.lock().unwrap().clone()),
            score: source.base.score,
        };
        // Access-modifier input has ordinary entities, no extension payloads.
        for e in base.entities.lock().unwrap().iter() {
            *e.extra.lock().unwrap() = None;
        }
        Self {
            base,
            final_field: "Constructor with argument code (solution)".into(),
            read_write_field: String::new(),
            private_field: None,
            public_field: None,
            private_property_field: None,
            friendly_property_field: None,
            protected_property_field: None,
            public_property_field: None,
        }
    }
    pub fn final_field(&self) -> &str {
        &self.final_field
    }
    pub fn read_only(&self) -> String {
        format!("read{}", self.read_write_field)
    }
    pub fn set_write_only(&mut self, value: &str) {
        self.read_write_field = value
            .strip_prefix("write")
            .expect("write prefix required")
            .to_string();
    }
}
pub fn access_registry() -> ObjectGraphCloneRegistry<'static> {
    let mut r = registry::<AnnotatedBase>();
    r.register::<AccessRoot>(
        |s| {
            Ok(AccessRoot {
                base: BaseRoot {
                    code: s.base.code,
                    values: s.base.values.clone(),
                    entities: Mutex::new(Arc::new(vec![])),
                    score: s.base.score,
                },
                final_field: s.final_field.clone(),
                read_write_field: s.read_write_field.clone(),
                private_field: s.private_field.clone(),
                public_field: s.public_field.clone(),
                private_property_field: s.private_property_field.clone(),
                friendly_property_field: s.friendly_property_field.clone(),
                protected_property_field: s.protected_property_field.clone(),
                public_property_field: s.public_property_field.clone(),
            })
        },
        |s, c| {
            for e in s.base.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            Ok(())
        },
        |s, t, c| {
            *t.base.entities.lock().unwrap() =
                Arc::new(c.collection(&s.base.entities.lock().unwrap())?);
            Ok(())
        },
    )
    .unwrap();
    r
}
pub fn access_accessors() -> SolutionBasicAccessors<'static, Arc<AccessRoot>, Arc<Value>> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<
        Child<AnnotatedBase>,
    >(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![
            PlanningSolutionComponent::registered::<Arc<AccessRoot>>(vec![
                SolutionMemberDefinition::entity_collection::<Child<AnnotatedBase>>("entities"),
                SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
                    .as_problem_fact_collection(),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ]),
        ])
        .unwrap()
        .compile::<Arc<AccessRoot>>(
            &entities,
            &[PlanningType::of::<Child<AnnotatedBase>>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut a = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<Child<AnnotatedBase>>(
            "value",
            vec!["range".into()],
        )],
    )
    .unwrap();
    a.entities("entities", |s: &Arc<AccessRoot>| {
        s.base
            .entities
            .lock()
            .unwrap()
            .iter()
            .map(|e| (e.clone(), vec![("value".into(), Some(e.base.value()))]))
            .collect()
    })
    .unwrap();
    a.range("values", |s| s.base.values.as_ref().clone())
        .unwrap();
    a.facts_mut()
        .collection("values", |s| {
            s.base
                .values
                .iter()
                .cloned()
                .map(Arc::new)
                .collect::<Vec<_>>()
        })
        .unwrap();
    a
}

pub struct InvalidEntityProvidingEntity {
    pub code: &'static str,
    pub range: Arc<Vec<Arc<Value>>>,
    pub value: Option<Arc<Value>>,
}
pub struct InvalidEntityProvidingSolution {
    pub code: &'static str,
    pub entities: Vec<Arc<InvalidEntityProvidingEntity>>,
    pub score: Option<SimpleScore>,
}
pub fn invalid_source() -> Arc<InvalidEntityProvidingSolution> {
    let v1 = Arc::new(Value("1"));
    let v2 = Arc::new(Value("2"));
    Arc::new(InvalidEntityProvidingSolution {
        code: "s1",
        score: None,
        entities: vec![
            Arc::new(InvalidEntityProvidingEntity {
                code: "1",
                range: Arc::new(vec![v1.clone(), v2.clone()]),
                value: Some(v1.clone()),
            }),
            Arc::new(InvalidEntityProvidingEntity {
                code: "2",
                range: Arc::new(vec![v1, v2.clone()]),
                value: Some(v2),
            }),
        ],
    })
}
pub fn invalid_registry() -> ObjectGraphCloneRegistry<'static> {
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<InvalidEntityProvidingSolution>(
        |s| {
            Ok(InvalidEntityProvidingSolution {
                code: s.code,
                entities: vec![],
                score: s.score,
            })
        },
        |s, c| {
            for e in &s.entities {
                c.required(e)?;
            }
            Ok(())
        },
        |_, _, _| Ok(()),
    )
    .unwrap();
    r.register::<InvalidEntityProvidingEntity>(
        |s| {
            Ok(InvalidEntityProvidingEntity {
                code: s.code,
                range: s.range.clone(),
                value: s.value.clone(),
            })
        },
        |s, c| c.required_optional_field::<InvalidEntityProvidingEntity, Value>("value", &s.value),
        |_, _, _| Ok(()),
    )
    .unwrap();
    r
}
