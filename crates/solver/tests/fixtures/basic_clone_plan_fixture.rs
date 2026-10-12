// Adapted Community TEST-only basic/field clone fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::sync::{Arc, Mutex};
use timetuck::*;
#[derive(Debug, Eq, PartialEq)]
pub struct Value(pub &'static str);
pub trait EntityData: Send + Sync + 'static {
    fn new(code: &'static str, value: Option<Arc<Value>>) -> Self;
    fn code(&self) -> &'static str;
    fn value(&self) -> Option<Arc<Value>>;
    fn set_value(&self, value: Arc<Value>);
}
pub struct PropertyEntity {
    code: &'static str,
    value: Mutex<Option<Arc<Value>>>,
}
pub struct FieldEntity {
    code: &'static str,
    value: Mutex<Option<Arc<Value>>>,
}
macro_rules! entity {
    ($t:ty) => {
        impl EntityData for $t {
            fn new(code: &'static str, value: Option<Arc<Value>>) -> Self {
                Self {
                    code,
                    value: Mutex::new(value),
                }
            }
            fn code(&self) -> &'static str {
                self.code
            }
            fn value(&self) -> Option<Arc<Value>> {
                self.value.lock().unwrap().clone()
            }
            fn set_value(&self, value: Arc<Value>) {
                *self.value.lock().unwrap() = Some(value);
            }
        }
    };
}
entity!(PropertyEntity);
entity!(FieldEntity);
pub struct Root<E> {
    pub code: &'static str,
    pub values: Arc<Vec<Arc<Value>>>,
    pub entities: Arc<Vec<Arc<E>>>,
    pub other: Arc<Vec<Arc<E>>>,
    pub score: Option<SimpleScore>,
}
pub fn source<E: EntityData>() -> Root<E> {
    let values = Arc::new(vec![
        Arc::new(Value("1")),
        Arc::new(Value("2")),
        Arc::new(Value("3")),
    ]);
    Root {
        code: "solution",
        entities: Arc::new(
            [("a", 0), ("b", 0), ("c", 2), ("d", 2)]
                .into_iter()
                .map(|(code, i)| Arc::new(E::new(code, Some(values[i].clone()))))
                .collect(),
        ),
        values,
        other: Arc::new(vec![]),
        score: None,
    }
}
pub fn accessors<E: EntityData>(
    other: bool,
) -> SolutionBasicAccessors<'static, Root<E>, Arc<Value>> {
    let entities = PlanningComposition::new(vec![PlanningComponent::registered::<E>(vec![
        PlanningVariableDefinition::basic::<Arc<Value>>("value", false),
    ])])
    .unwrap();
    let mut members = vec![
        SolutionMemberDefinition::entity_collection::<E>("entities"),
        SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
            .as_problem_fact_collection(),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ];
    if other {
        members.push(SolutionMemberDefinition::entity_collection::<E>("other"));
    }
    let layout = Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Root<E>>(
            members,
        )])
        .unwrap()
        .compile::<Root<E>>(
            &entities,
            &[PlanningType::of::<E>()],
            SolutionCompositionPolicy::default(),
            PlanningCompositionPolicy::default(),
        )
        .unwrap(),
    );
    let mut a = SolutionBasicAccessors::new(
        layout,
        vec![SolutionFieldRange::new::<E>("value", vec!["range".into()])],
    )
    .unwrap();
    a.entities("entities", |s: &Root<E>| {
        s.entities
            .iter()
            .map(|e| (e.clone(), vec![("value".into(), e.value())]))
            .collect()
    })
    .unwrap();
    if other {
        a.entities("other", |s: &Root<E>| {
            s.other
                .iter()
                .map(|e| (e.clone(), vec![("value".into(), e.value())]))
                .collect()
        })
        .unwrap();
    }
    a.range("values", |s| s.values.as_ref().clone()).unwrap();
    a.facts_mut()
        .collection("values", |s| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    a
}
pub fn plan<E: EntityData>() -> BasicSolutionClonePlan<'static, Root<E>, Arc<Value>, SimpleScore> {
    let mut p = BasicSolutionClonePlan::new(
        |s: &Root<E>, m: &BasicEntityClones| {
            Ok(Root {
                code: s.code,
                values: s.values.clone(),
                entities: Arc::new(m.collection(&s.entities)?),
                other: Arc::new(m.collection(&s.other)?),
                score: s.score,
            })
        },
        |s| s.score,
    );
    p.entity::<E>(|old, fields| {
        Ok(E::new(
            old.code(),
            fields
                .iter()
                .find(|(name, _)| name == "value")
                .unwrap()
                .1
                .clone(),
        ))
    })
    .unwrap();
    p
}
