// Adapted Community TEST-only collection/array clone fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// These keys hash owning allocation identity, independent of mutable payloads.
#![allow(clippy::mutable_key_type)]
#![allow(dead_code)]
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};
use timetuck::*;
pub type List = Mutex<Vec<Arc<Entity>>>;
pub type Array = Mutex<Box<[Arc<Entity>]>>;
pub type Set = Mutex<HashSet<ObjectIdentity<Entity>>>;
pub type TextMap = Mutex<BTreeMap<Option<Arc<String>>, Option<Arc<Entity>>>>;
pub type EntityMap = Mutex<HashMap<ObjectIdentity<Entity>, Arc<String>>>;
pub type ListMap = Mutex<BTreeMap<Option<Arc<String>>, Option<Arc<List>>>>;
#[derive(Eq, PartialEq, Debug)]
pub struct Value(pub &'static str);
pub struct Entity {
    pub code: &'static str,
    pub value: Mutex<Arc<Value>>,
    pub list: Mutex<Option<Arc<List>>>,
    pub array: Mutex<Option<Arc<Array>>>,
    pub set: Mutex<Option<Arc<Set>>>,
    pub text_map: Mutex<Option<Arc<TextMap>>>,
    pub entity_map: Mutex<Option<Arc<EntityMap>>>,
    pub list_map: Mutex<Option<Arc<ListMap>>>,
}
pub struct Root {
    pub code: &'static str,
    pub values: Arc<Vec<Arc<Value>>>,
    pub entities: Mutex<Option<Arc<List>>>,
    pub arrays: Mutex<Option<Arc<Array>>>,
    pub score: Option<SimpleScore>,
}
impl Root {
    pub fn objects(&self) -> Vec<Arc<Entity>> {
        if let Some(list) = self.entities.lock().unwrap().as_ref() {
            return list.lock().unwrap().clone();
        }
        self.arrays
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .to_vec()
    }
    pub fn detach(&self) {
        // Strong cycles use explicit native ownership cleanup, after assertions.
        for e in self.objects() {
            e.list.lock().unwrap().take();
            e.array.lock().unwrap().take();
            e.set.lock().unwrap().take();
            e.text_map.lock().unwrap().take();
            e.entity_map.lock().unwrap().take();
            e.list_map.lock().unwrap().take();
        }
    }
}
pub fn source(collections: bool) -> Arc<Root> {
    let values = Arc::new(vec![
        Arc::new(Value("1")),
        Arc::new(Value("2")),
        Arc::new(Value("3")),
    ]);
    let nodes: Vec<_> = [("a", 0), ("b", 0), ("c", 2)]
        .into_iter()
        .map(|(code, i)| {
            Arc::new(Entity {
                code,
                value: Mutex::new(values[i].clone()),
                list: Mutex::new(None),
                array: Mutex::new(None),
                set: Mutex::new(None),
                text_map: Mutex::new(None),
                entity_map: Mutex::new(None),
                list_map: Mutex::new(None),
            })
        })
        .collect();
    for (i, refs) in [
        vec![nodes[1].clone(), nodes[2].clone()],
        vec![],
        vec![nodes[0].clone(), nodes[2].clone()],
    ]
    .into_iter()
    .enumerate()
    {
        if collections {
            *nodes[i].list.lock().unwrap() = Some(Arc::new(Mutex::new(refs.clone())));
            *nodes[i].set.lock().unwrap() = Some(Arc::new(Mutex::new(
                refs.iter().cloned().map(ObjectIdentity::new).collect(),
            )));
            *nodes[i].text_map.lock().unwrap() = Some(Arc::new(Mutex::new(
                refs.iter()
                    .map(|e| (Some(Arc::new(e.code.to_string())), Some(e.clone())))
                    .collect(),
            )));
        } else {
            *nodes[i].array.lock().unwrap() = Some(Arc::new(Mutex::new(refs.into_boxed_slice())));
        }
    }
    if collections {
        *nodes[0].entity_map.lock().unwrap() = Some(Arc::new(Mutex::new(HashMap::from([
            (ObjectIdentity::new(nodes[1].clone()), Arc::new("b".into())),
            (ObjectIdentity::new(nodes[2].clone()), Arc::new("c".into())),
        ]))));
        *nodes[0].list_map.lock().unwrap() = Some(Arc::new(Mutex::new(BTreeMap::from([(
            Some(Arc::new("bc".into())),
            Some(Arc::new(Mutex::new(vec![
                nodes[1].clone(),
                nodes[2].clone(),
            ]))),
        )]))));
    }
    Arc::new(Root {
        code: "solution",
        values,
        entities: Mutex::new(collections.then(|| Arc::new(Mutex::new(nodes.clone())))),
        arrays: Mutex::new((!collections).then(|| Arc::new(Mutex::new(nodes.into_boxed_slice())))),
        score: None,
    })
}
pub fn registry() -> ObjectGraphCloneRegistry<'static> {
    use ObjectCloneFieldPolicy::{Required, Shared};
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_list::<Entity>(Required).unwrap();
    r.register_slice::<Entity>(Required).unwrap();
    r.register_identity_set::<Entity>(Required).unwrap();
    r.register_nullable_map::<String, Entity>(Shared, Required)
        .unwrap();
    r.register_identity_map::<Entity, String>(Required, Shared)
        .unwrap();
    r.register_nullable_map::<String, List>(Shared, Required)
        .unwrap();
    r.register::<Root>(
        |s| {
            Ok(Root {
                code: s.code,
                values: s.values.clone(),
                entities: Mutex::new(None),
                arrays: Mutex::new(None),
                score: s.score,
            })
        },
        |s, c| {
            c.optional_field(&s.entities.lock().unwrap(), Required)?;
            c.optional_field(&s.arrays.lock().unwrap(), Required)
        },
        |s, t, c| {
            *t.entities.lock().unwrap() =
                c.optional_field(&s.entities.lock().unwrap(), Required)?;
            *t.arrays.lock().unwrap() = c.optional_field(&s.arrays.lock().unwrap(), Required)?;
            Ok(())
        },
    )
    .unwrap();
    r.register::<Entity>(
        |s| {
            Ok(Entity {
                code: s.code,
                value: Mutex::new(s.value.lock().unwrap().clone()),
                list: Mutex::new(None),
                array: Mutex::new(None),
                set: Mutex::new(None),
                text_map: Mutex::new(None),
                entity_map: Mutex::new(None),
                list_map: Mutex::new(None),
            })
        },
        |s, c| {
            c.optional_field(&s.list.lock().unwrap(), Required)?;
            c.optional_field(&s.array.lock().unwrap(), Required)?;
            c.optional_field(&s.set.lock().unwrap(), Required)?;
            c.optional_field(&s.text_map.lock().unwrap(), Required)?;
            c.optional_field(&s.entity_map.lock().unwrap(), Required)?;
            c.optional_field(&s.list_map.lock().unwrap(), Required)
        },
        |s, t, c| {
            *t.list.lock().unwrap() = c.optional_field(&s.list.lock().unwrap(), Required)?;
            *t.array.lock().unwrap() = c.optional_field(&s.array.lock().unwrap(), Required)?;
            *t.set.lock().unwrap() = c.optional_field(&s.set.lock().unwrap(), Required)?;
            *t.text_map.lock().unwrap() =
                c.optional_field(&s.text_map.lock().unwrap(), Required)?;
            *t.entity_map.lock().unwrap() =
                c.optional_field(&s.entity_map.lock().unwrap(), Required)?;
            *t.list_map.lock().unwrap() =
                c.optional_field(&s.list_map.lock().unwrap(), Required)?;
            Ok(())
        },
    )
    .unwrap();
    r
}

pub fn accessors() -> SolutionBasicAccessors<'static, Arc<Root>, Arc<Value>> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<Entity>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Arc<Root>>(
            vec![
                SolutionMemberDefinition::entity_collection::<Entity>("entities"),
                SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
                    .as_problem_fact_collection(),
                SolutionMemberDefinition::score::<SimpleScore>("score"),
            ],
        )])
        .unwrap()
        .compile::<Arc<Root>>(
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
    a.entities("entities", |s: &Arc<Root>| {
        s.objects()
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
