// Adapted TEST-only AbstractSolutionClonerTest list fixtures, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
#![allow(dead_code)]
use std::sync::{Arc, Mutex, Weak};
use timetuck::*;
#[derive(Debug)]
pub struct Value {
    pub code: &'static str,
    pub owner: Mutex<Option<Weak<Owner>>>,
    pub index: Mutex<Option<usize>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
impl Eq for Value {}
impl Value {
    pub fn new(code: &'static str) -> Self {
        Self {
            code,
            owner: Mutex::new(None),
            index: Mutex::new(None),
        }
    }
}
pub struct Owner {
    pub code: &'static str,
    pub list: Mutex<Arc<Vec<Arc<Value>>>>,
    pub prefix: usize,
}
pub struct Root {
    pub values: Arc<Vec<Arc<Value>>>,
    pub owners: Arc<Vec<Arc<Owner>>>,
    pub score: Option<SimpleScore>,
}
pub fn update_shadows(root: &Root) {
    for v in root.values.iter() {
        *v.owner.lock().unwrap() = None;
        *v.index.lock().unwrap() = None;
    }
    for owner in root.owners.iter() {
        for (i, v) in owner.list.lock().unwrap().iter().enumerate() {
            *v.owner.lock().unwrap() = Some(Arc::downgrade(owner));
            *v.index.lock().unwrap() = Some(i);
        }
    }
}
pub fn source(shadows: bool) -> Root {
    let values = Arc::new(vec![
        Arc::new(Value::new("1")),
        Arc::new(Value::new("2")),
        Arc::new(Value::new("3")),
    ]);
    let owners = Arc::new(vec![
        Arc::new(Owner {
            code: "a",
            list: Mutex::new(Arc::new(vec![values[0].clone(), values[2].clone()])),
            prefix: 0,
        }),
        Arc::new(Owner {
            code: "b",
            list: Mutex::new(Arc::new(vec![values[1].clone()])),
            prefix: 0,
        }),
    ]);
    let root = Root {
        values,
        owners,
        score: Some(SimpleScore::new(1)),
    };
    if shadows {
        update_shadows(&root);
    }
    root
}
pub fn layout(shadows: bool, optional: bool) -> Arc<ComposedPlanningSolution> {
    let mut entities = vec![PlanningComponent::registered::<Owner>(vec![
        PlanningVariableDefinition::list::<Arc<Value>>("valueList", optional),
    ])];
    let mut types = vec![PlanningType::of::<Owner>()];
    let mut members = vec![
        SolutionMemberDefinition::entity_collection::<Owner>("owners"),
        SolutionMemberDefinition::value_range::<Arc<Value>>("values", "range")
            .as_problem_fact_collection(),
        SolutionMemberDefinition::score::<SimpleScore>("score"),
    ];
    if shadows {
        entities.push(PlanningComponent::registered::<Value>(vec![
            PlanningVariableDefinition::shadow::<usize>("index"),
        ]));
        types.push(PlanningType::of::<Value>());
        members.push(SolutionMemberDefinition::entity_collection::<Value>(
            "valueObjects",
        ));
    }
    let entities = PlanningComposition::new(entities).unwrap();
    Arc::new(
        SolutionComposition::new(vec![PlanningSolutionComponent::registered::<Root>(members)])
            .unwrap()
            .compile::<Root>(
                &entities,
                &types,
                SolutionCompositionPolicy::default(),
                PlanningCompositionPolicy::default(),
            )
            .unwrap(),
    )
}
pub fn bind(
    layout: Arc<ComposedPlanningSolution>,
    shadows: bool,
    s: &Root,
) -> Result<BoundListSolution<Arc<Value>>, ListSolutionBindingError> {
    let mut facts = SolutionFactAccessors::new(layout.clone()).unwrap();
    facts
        .collection("values", |s: &Root| {
            s.values.iter().cloned().map(Arc::new).collect::<Vec<_>>()
        })
        .unwrap();
    let mut collections = vec![SolutionListCollection {
        member: "owners".into(),
        entities: s
            .owners
            .iter()
            .map(|o| {
                SolutionListEntityRecord::new(
                    o.clone(),
                    vec![("valueList".into(), o.list.lock().unwrap().as_ref().clone())],
                )
                .with_pin_policy(ListPinPolicy {
                    fully_pinned: false,
                    prefix: o.prefix,
                })
            })
            .collect(),
    }];
    if shadows {
        collections.push(SolutionListCollection {
            member: "valueObjects".into(),
            entities: s
                .values
                .iter()
                .map(|v| SolutionListEntityRecord::new(v.clone(), vec![]))
                .collect(),
        });
    }
    layout.bind_list_with_facts(
        collections,
        vec![SolutionRange {
            member: "values".into(),
            values: s.values.as_ref().clone(),
        }],
        vec![SolutionFieldRange::new::<Owner>(
            "valueList",
            vec!["range".into()],
        )],
        facts.read(s).unwrap(),
    )
}
pub fn plan(
    shadows: bool,
    optional: bool,
) -> ListSolutionClonePlan<'static, Root, Arc<Value>, SimpleScore> {
    let layout = layout(shadows, optional);
    let mut p = ListSolutionClonePlan::new(
        move |s| bind(layout.clone(), shadows, s),
        move |s: &Root, m: &BasicEntityClones| {
            let root = Root {
                values: if shadows {
                    Arc::new(m.collection(&s.values)?)
                } else {
                    s.values.clone()
                },
                owners: Arc::new(m.collection(&s.owners)?),
                score: s.score,
            };
            if shadows {
                update_shadows(&root);
            }
            Ok(root)
        },
        move |v: &Arc<Value>, m: &BasicEntityClones| {
            if shadows {
                Ok(m.entity(v)?)
            } else {
                Ok(v.clone())
            }
        },
        |s| s.score,
    );
    if shadows {
        p.entity::<Value>(|v, _| Ok(Value::new(v.code))).unwrap();
    }
    p.entity::<Owner>(|o, fields| {
        Ok(Owner {
            code: o.code,
            list: Mutex::new(Arc::new(fields[0].1.clone())),
            prefix: o.prefix,
        })
    })
    .unwrap();
    p
}
