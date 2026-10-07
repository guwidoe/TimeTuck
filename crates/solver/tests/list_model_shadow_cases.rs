// Adapted from Timefold Community ListModelTest and simple_list fixtures.
// Apache-2.0; see compat/provenance.json for exact source identities.
use std::{any::Any, sync::Arc};
use timetuck::*;
struct Solution;
#[derive(Clone, PartialEq, Eq)]
struct Entity {
    position: i32,
    start_time: i32,
}
#[derive(Clone)]
struct Value {
    id: usize,
    position: i32,
    duration: i32,
    entity: Option<Entity>,
    previous: Option<Arc<Value>>,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
type Times = Vec<(Option<i32>, Option<i32>)>;
fn world(list: &[usize]) -> Vec<Value> {
    let mut values = [(1, 60), (2, 120), (3, 30)]
        .into_iter()
        .enumerate()
        .map(|(id, (position, duration))| Value {
            id,
            position,
            duration,
            entity: None,
            previous: None,
        })
        .collect::<Vec<_>>();
    for (index, &id) in list.iter().enumerate() {
        values[id].entity = Some(Entity {
            position: 0,
            start_time: 0,
        });
        values[id].previous = index
            .checked_sub(1)
            .map(|i| Arc::new(values[list[i]].clone()));
    }
    values
}
#[test]
fn simple_list() {
    let schema = PlanningSchema::new::<Solution>(vec![
        PlanningEntityDefinition::of::<Entity>(vec![PlanningVariableDefinition::list::<Value>(
            "values", false,
        )]),
        PlanningEntityDefinition::of::<Value>(vec![
            PlanningVariableDefinition::shadow::<Entity>("entity"),
            PlanningVariableDefinition::shadow::<Value>("previous"),
            PlanningVariableDefinition::shadow::<i32>("startTime"),
            PlanningVariableDefinition::shadow::<i32>("endTime"),
        ]),
    ])
    .unwrap();
    let mut paths = SourcePathRegistry::new(&schema);
    paths
        .one("entity", SourceMemberRole::Variable, |v: &Value| {
            v.entity.as_ref()
        })
        .unwrap();
    paths
        .one("previous", SourceMemberRole::Previous, |v: &Value| {
            v.previous.as_deref()
        })
        .unwrap();
    for name in ["startTime", "endTime"] {
        paths
            .one(
                name,
                SourceMemberRole::Declarative,
                |_: &Value| -> Option<&i32> {
                    panic!("terminal shadow values must come from the session")
                },
            )
            .unwrap();
    }
    let owner = paths.compile::<Value>("startTime", "entity").unwrap();
    let predecessor = paths
        .compile::<Value>("startTime", "previous.endTime")
        .unwrap();
    let start_path = paths.compile::<Value>("endTime", "startTime").unwrap();
    let value = schema.entity::<Value>().unwrap();
    let start_field = value.variable("startTime").unwrap();
    let end_field = value.variable("endTime").unwrap();
    let factory = || -> Result<_, GraphError> {
        let mut shadows = ShadowSessionBuilder::<usize, Vec<Value>>::new(&schema);
        let mut starts = vec![];
        let mut ends = vec![];
        for id in 0..3 {
            shadows
                .input(value.variable("entity").unwrap(), id, move |w| {
                    w[id].entity.clone()
                })
                .unwrap();
            shadows
                .input(value.variable("previous").unwrap(), id, move |w| {
                    w[id].previous.as_deref().cloned()
                })
                .unwrap();
            let start = shadows
                .derive(start_field, id, move |w, inputs| {
                    let v = &w[id];
                    let entity = v.entity.as_ref()?;
                    if let Some(previous) = &v.previous {
                        let end = inputs.for_field::<i32>(end_field).unwrap();
                        Some(**end.first()?.as_ref()? + (v.position - previous.position).abs())
                    } else {
                        Some(entity.start_time + (v.position - entity.position).abs())
                    }
                })
                .unwrap();
            let end = shadows
                .derive(end_field, id, move |w, inputs| {
                    let starts = inputs.for_field::<i32>(start_field).unwrap();
                    Some(**starts.first()?.as_ref()? + w[id].duration)
                })
                .unwrap();
            let key = |object: &dyn Any| object.downcast_ref::<Value>().map(|v| v.id);
            shadows
                .bind_source_paths(&start, vec![&owner, &predecessor], move |w| &w[id], key)
                .unwrap();
            shadows
                .bind_source_paths(&end, vec![&start_path], move |w| &w[id], key)
                .unwrap();
            starts.push(start);
            ends.push(end);
        }
        let mut b = StreamGraphBuilder::<SimpleScore>::new();
        let sources = ListStreamSources::new(&mut b);
        let worlds = b.map(sources.entities.port(), |(_, list)| world(list))?;
        let times = b.map_shadows(&worlds, shadows, move |_, session| {
            starts
                .iter()
                .zip(&ends)
                .map(|(start, end)| {
                    let start = session.read(start)?;
                    let end = session.read(end)?;
                    assert!(!start.inconsistent && !end.inconsistent);
                    Ok((start.value.copied(), end.value.copied()))
                })
                .collect::<Result<Times, ShadowSessionError>>()
        })?;
        let matches = b.score("Minimize end time", &times, |times| {
            SimpleScore::new(
                -times
                    .iter()
                    .filter_map(|(_, end)| *end)
                    .map(i64::from)
                    .sum::<i64>(),
            )
        })?;
        Ok((b.build(), sources, matches))
    };
    let problem = ListProblem::new(vec![0usize, 1, 2], vec![ListEntity::new(vec![])]).unwrap();
    let config = SolverConfig {
        limits: SolverLimits {
            best_score_limit: Some(SimpleScore::new(-344)),
            max_evaluations: 1000,
            ..Default::default()
        },
        construction: ConstructionConfig {
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 32,
            assert_scores: true,
            ..Default::default()
        }],
    };
    let result = solve_list_stream(&problem, || factory().map(|(g, s, _)| (g, s)), config).unwrap();
    assert!(result.complete);
    assert_eq!(result.score, Some(SimpleScore::new(-344)));
    assert_eq!(result.assignment.lists()[0], vec![2, 0, 1]);
    let (graph, sources, matches) = factory().unwrap();
    let mut director =
        ListStreamScoreDirector::new(&problem, graph, sources, result.assignment).unwrap();
    let times = &director.graph().constraint_matches(&matches).unwrap()[&0].0;
    assert_eq!(times[2], (Some(3), Some(33)));
    assert_eq!(times[0], (Some(35), Some(95)));
    assert_eq!(times[1], (Some(96), Some(216)));
    assert_eq!(director.score(), SimpleScore::new(-344));
    director.assert_consistent().unwrap();
}
