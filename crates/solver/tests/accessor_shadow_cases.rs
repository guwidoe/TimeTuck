// Adapted from Timefold Community method-variable and extended-value tests/fixtures.
// Apache-2.0; see compat/provenance.json for source identities and native adaptations.
use std::{any::Any, collections::BTreeSet, sync::Arc};
use timetuck::*;
struct Solution;
struct Owner;
#[derive(Clone)]
struct BaseValue {
    id: usize,
    code: String,
    previous: Option<Arc<Value>>,
}
#[derive(Clone)]
struct Value {
    base: BaseValue,
}
impl Value {
    fn previous(&self) -> Option<&Value> {
        self.base.previous.as_deref()
    }
    fn code(&self) -> &str {
        &self.base.code
    }
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.base.id == other.base.id
    }
}
impl Eq for Value {}
#[derive(Clone)]
struct World {
    values: Vec<Value>,
    assigned: Vec<usize>,
}
fn world(list: &[usize]) -> World {
    let mut values = ["1", "2", "31"]
        .into_iter()
        .enumerate()
        .map(|(id, code)| Value {
            base: BaseValue {
                id,
                code: code.into(),
                previous: None,
            },
        })
        .collect::<Vec<_>>();
    for (index, &id) in list.iter().enumerate() {
        values[id].base.previous = index
            .checked_sub(1)
            .map(|i| Arc::new(values[list[i]].clone()));
    }
    World {
        values,
        assigned: list.to_vec(),
    }
}
fn solve(getter_list: bool) {
    let schema = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Owner>(vec![PlanningVariableDefinition::list::<Value>(
            "values", false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::shadow::<Value>("previous"),
            PlanningVariableDefinition::shadow::<String>("codeSum"),
        ]),
    ])
    .unwrap();
    let mut paths = SourcePathRegistry::new(&schema);
    paths
        .one("previous", SourceMemberRole::Previous, Value::previous)
        .unwrap();
    paths
        .one(
            "codeSum",
            SourceMemberRole::Declarative,
            |_: &Value| -> Option<&String> { panic!("cached field") },
        )
        .unwrap();
    let path = paths
        .compile::<Value>("codeSum", "previous.codeSum")
        .unwrap();
    let entity = schema.entity::<Value>().unwrap();
    let sum_field = entity.variable("codeSum").unwrap();
    let problem = ListProblem::new(vec![0usize, 1, 2], vec![ListEntity::new(vec![]); 2]).unwrap();
    let factory = || {
        let mut shadows = ShadowSessionBuilder::<usize, World>::new(&schema);
        let mut sums = vec![];
        for id in 0..3 {
            shadows
                .input(entity.variable("previous").unwrap(), id, move |w| {
                    w.values[id].previous().cloned()
                })
                .unwrap();
            let sum = shadows
                .derive(sum_field, id, move |w, i| {
                    let value = &w.values[id];
                    let mut text = if value.previous().is_some() {
                        (*i.for_field::<String>(sum_field)
                            .unwrap()
                            .first()?
                            .as_ref()?)
                        .clone()
                    } else {
                        String::new()
                    };
                    text.push_str(value.code());
                    Some(text)
                })
                .unwrap();
            shadows
                .bind_source_paths(
                    &sum,
                    vec![&path],
                    move |w| &w.values[id],
                    |o: &dyn Any| o.downcast_ref::<Value>().map(|v| v.base.id),
                )
                .unwrap();
            sums.push(sum);
        }
        let mut b = StreamGraphBuilder::<HardSoftScore>::new();
        let sources = ListStreamSources::<usize>::new(&mut b);
        // Both source fixtures share the same supplier/objective. Explicit native
        // adapters preserve the difference between list accessor and field access.
        struct ListAccess<'a>(&'a [usize]);
        impl ListAccess<'_> {
            fn values(&self) -> &[usize] {
                self.0
            }
        }
        let worlds = b.map(sources.entities.port(), move |(_, list)| {
            if getter_list {
                world(ListAccess(list).values())
            } else {
                world(list)
            }
        })?;
        let derived = b.map_shadows(&worlds, shadows, move |w, session| {
            w.assigned
                .iter()
                .map(|&id| {
                    let value = session.read(&sums[id])?;
                    assert!(!value.inconsistent);
                    Ok(value.value.cloned())
                })
                .collect::<Result<Vec<_>, ShadowSessionError>>()
        })?;
        let rows = b.flat_map(&derived, |values| {
            values.iter().flatten().cloned().collect::<Vec<_>>()
        })?;
        b.score("Duplicate characters in code sum", &rows, |text| {
            let distinct = text.chars().collect::<BTreeSet<_>>().len();
            HardSoftScore::new(-((text.len() - distinct) as i64), 0)
        })?;
        b.score("Maximize code sum length", &rows, |text| {
            HardSoftScore::new(0, text.len() as i64)
        })?;
        Ok::<_, GraphError>((b.build(), sources))
    };
    let result = solve_list_stream(
        &problem,
        factory,
        SolverConfig {
            limits: SolverLimits {
                max_evaluations: 1000,
                ..Default::default()
            },
            construction: ConstructionConfig {
                assert_scores: true,
                ..Default::default()
            },
            local_search: vec![LocalSearchConfig {
                assert_scores: true,
                selection: MoveSelection::BestAccepted,
                max_steps: 1000,
                ..Default::default()
            }],
        },
    )
    .unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(HardSoftScore::new(0, 6)));
    // Independently recompute the returned model's getter-defined shadow chain.
    let mut soft = 0;
    for list in result.assignment.lists() {
        let mut text = String::new();
        for &id in list {
            text.push_str(["1", "2", "31"][id]);
            assert_eq!(text.len(), text.chars().collect::<BTreeSet<_>>().len());
            soft += text.len();
        }
    }
    assert_eq!(soft, 6);
}
#[test]
fn method_defined_variables() {
    solve(true);
}
#[test]
fn extended_values() {
    solve(false);
}
