// Adapted field-annotated deep-clone TEST fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use timetuck::*;
pub type TextList = Mutex<Vec<Option<Arc<String>>>>;
pub type TextMap = Mutex<BTreeMap<Option<Arc<String>>, Option<Arc<String>>>>;
pub type NullableTextVec = Vec<Option<Arc<String>>>;
pub type ListToText = Mutex<BTreeMap<Option<Arc<NullableTextVec>>, Option<Arc<String>>>>;
pub type TextToList = Mutex<BTreeMap<Option<Arc<String>>, Option<Arc<NullableTextVec>>>>;
#[derive(Eq, PartialEq, Debug)]
pub struct Value(pub &'static str);
pub struct Entity {
    pub code: &'static str,
    pub value: Mutex<Arc<Value>>,
    pub list: Mutex<Option<Arc<TextList>>>,
    pub map: Mutex<Option<Arc<TextMap>>>,
    pub list_to_text: Mutex<Option<Arc<ListToText>>>,
    pub text_to_list: Mutex<Option<Arc<TextToList>>>,
}
pub struct Root {
    pub code: &'static str,
    pub values: Arc<Vec<Arc<Value>>>,
    pub entities: Mutex<Arc<Vec<Arc<Entity>>>>,
    pub general: Mutex<Option<Arc<TextList>>>,
    pub score: Option<SimpleScore>,
}
fn text(s: &str) -> Arc<String> {
    Arc::new(s.into())
}
fn list(s: &[&str]) -> Arc<TextList> {
    Arc::new(Mutex::new(s.iter().map(|v| Some(text(v))).collect()))
}
pub fn source() -> Arc<Root> {
    let values = Arc::new(vec![
        Arc::new(Value("1")),
        Arc::new(Value("2")),
        Arc::new(Value("3")),
    ]);
    let entities: Vec<_> = [("a", 0), ("b", 0), ("c", 2), ("d", 2)]
        .into_iter()
        .map(|(code, i)| {
            Arc::new(Entity {
                code,
                value: Mutex::new(values[i].clone()),
                list: Mutex::new(None),
                map: Mutex::new(None),
                list_to_text: Mutex::new(Some(Arc::new(Mutex::new(BTreeMap::new())))),
                text_to_list: Mutex::new(Some(Arc::new(Mutex::new(BTreeMap::new())))),
            })
        })
        .collect();
    *entities[0].list.lock().unwrap() = Some(list(&["shadow a1", "shadow a2"]));
    *entities[0].list_to_text.lock().unwrap() = Some(Arc::new(Mutex::new(BTreeMap::from([
        (None, Some(text("a"))),
        (Some(Arc::new(vec![Some(text("b"))])), None),
        (Some(Arc::new(vec![Some(text("c")), None])), None),
    ]))));
    *entities[1].map.lock().unwrap() = Some(Arc::new(Mutex::new(BTreeMap::from([
        (Some(text("shadow key b1")), Some(text("shadow value b1"))),
        (Some(text("shadow key b2")), Some(text("shadow value b2"))),
    ]))));
    *entities[1].text_to_list.lock().unwrap() = Some(Arc::new(Mutex::new(BTreeMap::from([
        (Some(text("a")), None),
        (None, Some(Arc::new(vec![Some(text("b"))]))),
        (Some(text("c")), Some(Arc::new(vec![Some(text("d")), None]))),
    ]))));
    *entities[2].list.lock().unwrap() = Some(list(&["shadow c1", "shadow c2"]));
    Arc::new(Root {
        code: "solution",
        values,
        entities: Mutex::new(Arc::new(entities)),
        general: Mutex::new(Some(list(&["shadow g1", "shadow g2"]))),
        score: None,
    })
}
pub fn registry() -> ObjectGraphCloneRegistry<'static> {
    use ObjectCloneFieldPolicy::{Required, Shared};
    let mut r = ObjectGraphCloneRegistry::new();
    r.register_nullable_list::<String>(Shared).unwrap();
    r.register_nullable_map::<String, String>(Shared, Shared)
        .unwrap();
    r.register_nullable_map::<NullableTextVec, String>(Shared, Shared)
        .unwrap();
    r.register_nullable_map::<String, NullableTextVec>(Shared, Shared)
        .unwrap();
    r.register::<Root>(
        |s| {
            Ok(Root {
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
            c.optional_field(&s.general.lock().unwrap(), Required)?;
            Ok(())
        },
        |s, t, c| {
            *t.entities.lock().unwrap() = Arc::new(c.collection(&s.entities.lock().unwrap())?);
            *t.general.lock().unwrap() = c.optional_field(&s.general.lock().unwrap(), Required)?;
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
                map: Mutex::new(None),
                list_to_text: Mutex::new(None),
                text_to_list: Mutex::new(None),
            })
        },
        |s, c| {
            c.optional_field(&s.list.lock().unwrap(), Required)?;
            c.optional_field(&s.map.lock().unwrap(), Required)?;
            c.optional_field(&s.list_to_text.lock().unwrap(), Required)?;
            c.optional_field(&s.text_to_list.lock().unwrap(), Required)?;
            Ok(())
        },
        |s, t, c| {
            *t.list.lock().unwrap() = c.optional_field(&s.list.lock().unwrap(), Required)?;
            *t.map.lock().unwrap() = c.optional_field(&s.map.lock().unwrap(), Required)?;
            *t.list_to_text.lock().unwrap() =
                c.optional_field(&s.list_to_text.lock().unwrap(), Required)?;
            *t.text_to_list.lock().unwrap() =
                c.optional_field(&s.text_to_list.lock().unwrap(), Required)?;
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
