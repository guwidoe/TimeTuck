// Adapted TEST-only backlinked/dependency fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::sync::{Arc, Mutex, Weak};
use timetuck::*;
#[derive(Debug, Eq, PartialEq)]
pub struct Value(pub String);
pub struct Entity {
    pub code: String,
    pub value: Arc<Value>,
    pub root: Mutex<Weak<Root>>,
}
pub struct Root {
    pub code: String,
    pub values: Arc<Vec<Arc<Value>>>,
    pub entities: Mutex<Vec<Arc<Entity>>>,
    pub score: Option<SimpleScore>,
}
pub fn source() -> Arc<Root> {
    let values = Arc::new(
        (0..2)
            .map(|i| Arc::new(Value(format!("Generated Value {i}"))))
            .collect::<Vec<_>>(),
    );
    let root = Arc::new(Root {
        code: "Generated Solution 0".into(),
        values: values.clone(),
        entities: Mutex::new(vec![]),
        score: None,
    });
    *root.entities.lock().unwrap() = (0..2)
        .map(|i| {
            Arc::new(Entity {
                code: format!("Generated Entity {i}"),
                value: values[i].clone(),
                root: Mutex::new(Arc::downgrade(&root)),
            })
        })
        .collect();
    root
}
pub fn registry() -> ObjectGraphCloneRegistry<'static> {
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<Root>(
        |s| {
            Ok(Root {
                code: s.code.clone(),
                values: s.values.clone(),
                entities: Mutex::new(vec![]),
                score: s.score,
            })
        },
        |s, c| {
            for e in s.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            Ok(())
        },
        |s, t, c| {
            *t.entities.lock().unwrap() = c.collection(&s.entities.lock().unwrap())?;
            Ok(())
        },
    )
    .unwrap();
    r.register::<Entity>(
        |s| {
            Ok(Entity {
                code: s.code.clone(),
                value: s.value.clone(),
                root: Mutex::new(Weak::new()),
            })
        },
        |s, c| c.weak(&s.root.lock().unwrap()),
        |s, t, c| {
            *t.root.lock().unwrap() = c.weak(&s.root.lock().unwrap())?;
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
            .map(|e| (e.clone(), vec![("value".into(), Some(e.value.clone()))]))
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
pub struct DependencyValue {
    pub id: String,
    pub duration_seconds: u64,
    pub dependencies: Mutex<Option<Vec<Arc<DependencyValue>>>>,
}
pub struct DependencyEntity {
    pub start: [i32; 6],
    pub values: Mutex<Vec<Arc<DependencyValue>>>,
}
pub struct DependencyRoot {
    pub entities: Mutex<Vec<Arc<DependencyEntity>>>,
    pub values: Mutex<Vec<Arc<DependencyValue>>>,
    pub score: Option<HardSoftScore>,
}
pub fn dependency_registry() -> ObjectGraphCloneRegistry<'static> {
    let mut r = ObjectGraphCloneRegistry::new();
    r.register::<DependencyRoot>(
        |s| {
            Ok(DependencyRoot {
                entities: Mutex::new(vec![]),
                values: Mutex::new(vec![]),
                score: s.score,
            })
        },
        |s, c| {
            for e in s.entities.lock().unwrap().iter() {
                c.required(e)?;
            }
            for v in s.values.lock().unwrap().iter() {
                c.required(v)?;
            }
            Ok(())
        },
        |s, t, c| {
            *t.entities.lock().unwrap() = c.collection(&s.entities.lock().unwrap())?;
            *t.values.lock().unwrap() = c.collection(&s.values.lock().unwrap())?;
            Ok(())
        },
    )
    .unwrap();
    r.register::<DependencyEntity>(
        |s| {
            Ok(DependencyEntity {
                start: s.start,
                values: Mutex::new(vec![]),
            })
        },
        |s, c| {
            for v in s.values.lock().unwrap().iter() {
                c.required(v)?;
            }
            Ok(())
        },
        |s, t, c| {
            *t.values.lock().unwrap() = c.collection(&s.values.lock().unwrap())?;
            Ok(())
        },
    )
    .unwrap();
    r.register::<DependencyValue>(
        |s| {
            Ok(DependencyValue {
                id: s.id.clone(),
                duration_seconds: s.duration_seconds,
                dependencies: Mutex::new(None),
            })
        },
        |s, c| {
            if let Some(deps) = s.dependencies.lock().unwrap().as_ref() {
                for v in deps {
                    c.required(v)?;
                }
            }
            Ok(())
        },
        |s, t, c| {
            *t.dependencies.lock().unwrap() = s
                .dependencies
                .lock()
                .unwrap()
                .as_ref()
                .map(|v| c.collection(v))
                .transpose()?;
            Ok(())
        },
    )
    .unwrap();
    r
}
