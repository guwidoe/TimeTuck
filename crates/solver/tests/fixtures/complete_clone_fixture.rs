// Adapted final abstract-cloner TEST fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Identity keys do not hash mutable payload fields.
#![allow(dead_code, clippy::mutable_key_type)]
use num_bigint::BigInt;
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};
use timetuck::*;
#[derive(Eq, PartialEq, Debug)]
pub struct Value(pub &'static str);
pub type TextList = Mutex<Vec<Option<Arc<String>>>>;
pub type TextMap = Mutex<BTreeMap<Option<Arc<String>>, Option<Arc<String>>>>;
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Primitives {
    pub boolean: bool,
    pub byte: i8,
    pub character: u16,
    pub short: i16,
    pub int: i32,
    pub long: i64,
    pub float: f32,
    pub double: f64,
}
#[derive(Clone)]
pub struct ImmutableRefs {
    pub boolean: Arc<bool>,
    pub byte: Arc<i8>,
    pub character: Arc<u16>,
    pub short: Arc<i16>,
    pub int: Arc<i32>,
    pub long: Arc<i64>,
    pub float: Arc<f32>,
    pub double: Arc<f64>,
    pub big_integer: Arc<BigInt>,
    pub big_decimal: Arc<(BigInt, u32)>,
    pub uuid: Arc<[u8; 16]>,
    pub text: Arc<String>,
    pub record: Arc<NonClonedRecord>,
}
pub struct NonClonedRecord {
    pub a: i32,
    pub b: String,
}
#[derive(Clone)]
pub struct Extra {
    pub id: Arc<String>,
}
pub struct Various {
    pub primitives: Primitives,
    pub refs: ImmutableRefs,
    pub shallow: Arc<TextList>,
    pub deep: Mutex<Option<Arc<TextList>>>,
    pub extra: Mutex<Option<Arc<Extra>>>,
}
pub struct AnnotatedVarious {
    pub base: Various,
}
fn text_list(s: &[&str]) -> Arc<TextList> {
    Arc::new(Mutex::new(
        s.iter().map(|s| Some(Arc::new(s.to_string()))).collect(),
    ))
}
impl Various {
    pub fn new(id: u8) -> Self {
        // Deterministic distinct version4 UUIDs replace random UUID input; only
        // identity/string linkage is asserted by the upstream case.
        let mut uuid = [0_u8; 16];
        uuid[6] = 0x40;
        uuid[8] = 0x80;
        uuid[15] = id;
        let text = Arc::new(format!("00000000-0000-4000-8000-0000000000{id:02x}"));
        let list = || Arc::new(Mutex::new(vec![Some(text.clone())]));
        Self {
            primitives: Primitives {
                boolean: true,
                byte: 1,
                character: 2,
                short: 3,
                int: 4,
                long: 5,
                float: 6.0,
                double: 7.0,
            },
            refs: ImmutableRefs {
                boolean: Arc::new(true),
                byte: Arc::new(8),
                character: Arc::new(9),
                short: Arc::new(10),
                int: Arc::new(11),
                long: Arc::new(12),
                float: Arc::new(13.0),
                double: Arc::new(14.0),
                big_integer: Arc::new(BigInt::from(15)),
                big_decimal: Arc::new((BigInt::from(16), 0)),
                uuid: Arc::new(uuid),
                text: text.clone(),
                record: Arc::new(NonClonedRecord {
                    a: 19,
                    b: "19".into(),
                }),
            },
            shallow: list(),
            deep: Mutex::new(Some(list())),
            extra: Mutex::new(Some(Arc::new(Extra {
                id: Arc::new("extra".into()),
            }))),
        }
    }
    fn shell(&self) -> Self {
        Self {
            primitives: self.primitives,
            refs: self.refs.clone(),
            shallow: self.shallow.clone(),
            deep: Mutex::new(None),
            extra: Mutex::new(None),
        }
    }
    fn discover(&self, c: &mut ObjectGraphDiscovery<'_, '_>) -> Result<(), ObjectGraphCloneError> {
        c.optional_field(&self.deep.lock().unwrap(), ObjectCloneFieldPolicy::Required)?;
        c.optional_field(
            &self.extra.lock().unwrap(),
            ObjectCloneFieldPolicy::Required,
        )
    }
    fn fill(&self, t: &Self, c: &ObjectGraphCloneView) -> Result<(), ObjectGraphCloneError> {
        *t.deep.lock().unwrap() =
            c.optional_field(&self.deep.lock().unwrap(), ObjectCloneFieldPolicy::Required)?;
        *t.extra.lock().unwrap() = c.optional_field(
            &self.extra.lock().unwrap(),
            ObjectCloneFieldPolicy::Required,
        )?;
        Ok(())
    }
}
pub struct Entity {
    pub code: &'static str,
    pub value: Mutex<Arc<Value>>,
    pub list: Mutex<Option<Arc<TextList>>>,
    pub map: Mutex<Option<Arc<TextMap>>>,
    pub copied: Mutex<Option<Arc<Various>>>,
    pub cloned: Mutex<Option<Arc<Various>>>,
    pub alias: Mutex<Option<Arc<Various>>>,
    pub annotated: Mutex<Option<Arc<AnnotatedVarious>>>,
    pub annotated_erased: Mutex<Option<ObjectGraphObject>>,
}
impl Entity {
    pub fn new(code: &'static str, value: Arc<Value>) -> Self {
        Self {
            code,
            value: Mutex::new(value),
            list: Mutex::new(None),
            map: Mutex::new(None),
            copied: Mutex::new(None),
            cloned: Mutex::new(None),
            alias: Mutex::new(None),
            annotated: Mutex::new(None),
            annotated_erased: Mutex::new(None),
        }
    }
}
pub struct DeepRoot {
    pub code: &'static str,
    pub values: Arc<Vec<Arc<Value>>>,
    pub entities: Mutex<Arc<Vec<Arc<Entity>>>>,
    pub general: Mutex<Option<Arc<TextList>>>,
    pub score: Option<SimpleScore>,
}
pub fn deep_source() -> Arc<DeepRoot> {
    let values = Arc::new(vec![
        Arc::new(Value("1")),
        Arc::new(Value("2")),
        Arc::new(Value("3")),
    ]);
    let nodes: Vec<_> = [("a", 0), ("b", 0), ("c", 2), ("d", 2)]
        .into_iter()
        .map(|(code, i)| Arc::new(Entity::new(code, values[i].clone())))
        .collect();
    *nodes[0].copied.lock().unwrap() = Some(Arc::new(Various::new(1)));
    let cloned = Arc::new(Various::new(2));
    *nodes[0].cloned.lock().unwrap() = Some(cloned.clone());
    *nodes[0].alias.lock().unwrap() = Some(cloned);
    *nodes[0].annotated.lock().unwrap() = Some(Arc::new(AnnotatedVarious {
        base: Various::new(3),
    }));
    *nodes[0].annotated_erased.lock().unwrap() = Some(Arc::new(AnnotatedVarious {
        base: Various::new(4),
    }));
    *nodes[0].list.lock().unwrap() = Some(text_list(&["shadow a1", "shadow a2"]));
    *nodes[2].list.lock().unwrap() = Some(text_list(&["shadow c1", "shadow c2"]));
    *nodes[1].map.lock().unwrap() = Some(Arc::new(Mutex::new(BTreeMap::from([
        (
            Some(Arc::new("shadow key b1".into())),
            Some(Arc::new("shadow value b1".into())),
        ),
        (
            Some(Arc::new("shadow key b2".into())),
            Some(Arc::new("shadow value b2".into())),
        ),
    ]))));
    Arc::new(DeepRoot {
        code: "solution",
        values,
        entities: Mutex::new(Arc::new(nodes)),
        general: Mutex::new(Some(text_list(&["shadow g1", "shadow g2"]))),
        score: None,
    })
}
pub type SortedEntities = Mutex<ObjectSortedSet<Entity>>;
pub type LinkedEntities = Mutex<ObjectLinkedIdentitySet<Entity>>;
pub type HashedEntities = Mutex<HashSet<ObjectIdentity<Entity>>>;
pub struct SetRoot {
    pub code: &'static str,
    pub values: Arc<ObjectSortedSet<Value>>,
    pub entities: Mutex<Option<Arc<SortedEntities>>>,
    pub hash: Mutex<Option<Arc<HashedEntities>>>,
    pub linked: Mutex<Option<Arc<LinkedEntities>>>,
    pub tree: Mutex<Option<Arc<SortedEntities>>>,
    pub score: Option<SimpleScore>,
}
pub fn set_source() -> Arc<SetRoot> {
    let v1 = Arc::new(Value("1"));
    let v2 = Arc::new(Value("2"));
    let v3 = Arc::new(Value("3"));
    let mut values = ObjectSortedSet::new(Arc::new(|a: &Value, b: &Value| b.0.cmp(a.0)));
    for v in [v1.clone(), v2, v3.clone()] {
        values.insert(v);
    }
    let nodes: Vec<_> = [("a", v1.clone()), ("b", v1), ("c", v3.clone()), ("d", v3)]
        .into_iter()
        .map(|(code, v)| Arc::new(Entity::new(code, v)))
        .collect();
    let comparator: ObjectComparator<Entity> = Arc::new(|a, b| b.code.cmp(a.code));
    let mut entities = ObjectSortedSet::new(comparator.clone());
    for e in &nodes {
        entities.insert(e.clone());
    }
    let mut hash = HashSet::new();
    let mut linked = ObjectLinkedIdentitySet::new();
    let mut tree = ObjectSortedSet::new(comparator);
    for e in entities.iter() {
        hash.insert(ObjectIdentity::new(e.clone()));
        linked.insert(e.clone());
        tree.insert(e.clone());
    }
    Arc::new(SetRoot {
        code: "solution",
        values: Arc::new(values),
        entities: Mutex::new(Some(Arc::new(Mutex::new(entities)))),
        hash: Mutex::new(Some(Arc::new(Mutex::new(hash)))),
        linked: Mutex::new(Some(Arc::new(Mutex::new(linked)))),
        tree: Mutex::new(Some(Arc::new(Mutex::new(tree)))),
        score: None,
    })
}
pub fn registry() -> ObjectGraphCloneRegistry<'static> {
    use ObjectCloneFieldPolicy::{Registered, Required, Shared};
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_nullable_list::<String>(Shared).unwrap();
    r.register_nullable_map::<String, String>(Shared, Shared)
        .unwrap();
    r.register_cloned_leaf::<Extra>().unwrap();
    r.register::<Various>(
        |s| Ok(s.shell()),
        |s, c| s.discover(c),
        |s, t, c| s.fill(t, c),
    )
    .unwrap();
    r.register::<AnnotatedVarious>(
        |s| {
            Ok(AnnotatedVarious {
                base: s.base.shell(),
            })
        },
        |s, c| s.base.discover(c),
        |s, t, c| s.base.fill(&t.base, c),
    )
    .unwrap();
    r.register::<Entity>(
        |s| Ok(Entity::new(s.code, s.value.lock().unwrap().clone())),
        |s, c| {
            c.optional_field(&s.list.lock().unwrap(), Required)?;
            c.optional_field(&s.map.lock().unwrap(), Required)?;
            c.optional_field(&s.cloned.lock().unwrap(), Required)?;
            c.optional_field(&s.annotated.lock().unwrap(), Required)?;
            c.optional_object(&s.annotated_erased.lock().unwrap(), Registered)
        },
        |s, t, c| {
            *t.list.lock().unwrap() = c.optional_field(&s.list.lock().unwrap(), Required)?;
            *t.map.lock().unwrap() = c.optional_field(&s.map.lock().unwrap(), Required)?;
            *t.cloned.lock().unwrap() = c.optional_field(&s.cloned.lock().unwrap(), Required)?;
            *t.annotated.lock().unwrap() =
                c.optional_field(&s.annotated.lock().unwrap(), Required)?;
            *t.annotated_erased.lock().unwrap() =
                c.optional_object(&s.annotated_erased.lock().unwrap(), Registered)?;
            *t.copied.lock().unwrap() = c.optional_field(&s.copied.lock().unwrap(), Shared)?;
            *t.alias.lock().unwrap() = c.optional_field(&s.alias.lock().unwrap(), Shared)?;
            Ok(())
        },
    )
    .unwrap();
    r.register::<DeepRoot>(
        |s| {
            Ok(DeepRoot {
                code: s.code,
                values: s.values.clone(),
                entities: Mutex::new(Arc::new(vec![])),
                general: Mutex::new(None),
                score: s.score,
            })
        },
        |s, c| {
            for e in s.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            c.optional_field(&s.general.lock().unwrap(), Required)
        },
        |s, t, c| {
            *t.entities.lock().unwrap() = Arc::new(c.collection(&s.entities.lock().unwrap())?);
            *t.general.lock().unwrap() = c.optional_field(&s.general.lock().unwrap(), Required)?;
            Ok(())
        },
    )
    .unwrap();
    r.register_sorted_set::<Entity>(Required).unwrap();
    r.register_linked_identity_set::<Entity>(Required).unwrap();
    r.register_identity_set::<Entity>(Required).unwrap();
    r.register::<SetRoot>(
        |s| {
            Ok(SetRoot {
                code: s.code,
                values: s.values.clone(),
                entities: Mutex::new(None),
                hash: Mutex::new(None),
                linked: Mutex::new(None),
                tree: Mutex::new(None),
                score: s.score,
            })
        },
        |s, c| {
            c.optional_field(&s.entities.lock().unwrap(), Required)?;
            c.optional_field(&s.hash.lock().unwrap(), Required)?;
            c.optional_field(&s.linked.lock().unwrap(), Required)?;
            c.optional_field(&s.tree.lock().unwrap(), Required)
        },
        |s, t, c| {
            *t.entities.lock().unwrap() =
                c.optional_field(&s.entities.lock().unwrap(), Required)?;
            *t.hash.lock().unwrap() = c.optional_field(&s.hash.lock().unwrap(), Required)?;
            *t.linked.lock().unwrap() = c.optional_field(&s.linked.lock().unwrap(), Required)?;
            *t.tree.lock().unwrap() = c.optional_field(&s.tree.lock().unwrap(), Required)?;
            Ok(())
        },
    )
    .unwrap();
    r
}

pub fn deep_accessors() -> SolutionBasicAccessors<'static, Arc<DeepRoot>, Arc<Value>> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![
            PlanningSolutionComponent::registered::<Arc<DeepRoot>>(vec![
                SolutionMemberDefinition::entity_collection::<Entity>("entities"),
                SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
                    .as_problem_fact_collection(),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ]),
        ])
        .unwrap()
        .compile::<Arc<DeepRoot>>(
            &entities,
            &[PlanningType::of::<Entity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut a = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["range".into()],
        )],
    )
    .unwrap();
    a.entities("entities", |s: &Arc<DeepRoot>| {
        s.entities
            .lock()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e.clone(),
                    vec![("value".into(), Some(e.value.lock().unwrap().clone()))],
                )
            })
            .collect()
    })
    .unwrap();
    a.range("values", |s| s.values.as_ref().clone()).unwrap();
    a.facts_mut()
        .collection("values", |s| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    a
}

pub fn set_accessors() -> SolutionBasicAccessors<'static, Arc<SetRoot>, Arc<Value>> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Arc<SetRoot>>(
            vec![
                SolutionMemberDefinition::entity_collection::<Entity>("entities"),
                SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
                    .as_problem_fact_collection(),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
        )])
        .unwrap()
        .compile::<Arc<SetRoot>>(
            &entities,
            &[PlanningType::of::<Entity>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut a = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["range".into()],
        )],
    )
    .unwrap();
    a.entities("entities", |s: &Arc<SetRoot>| {
        s.entities
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .iter()
            .map(|e| {
                (
                    e.clone(),
                    vec![("value".into(), Some(e.value.lock().unwrap().clone()))],
                )
            })
            .collect()
    })
    .unwrap();
    a.range("values", |s| s.values.iter().cloned().collect())
        .unwrap();
    a.facts_mut()
        .collection("values", |s| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    a
}
