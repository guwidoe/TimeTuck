// Adapted pinned Community dedicated Quarkus integration-test app TEST fixtures.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Native jobs/DTOs replace HTTP, Jackson, CDI and reflective discovery.
use std::{sync::Arc, time::Duration};
use timetuck::*;

#[derive(Clone, Debug)]
struct JsonEntity {
    value: Option<String>,
}
#[derive(Clone, Debug)]
struct JsonSolution {
    values: Vec<String>,
    entities: Vec<JsonEntity>,
    score: Option<SimpleScore>,
}
fn decode_json(input: &str) -> JsonSolution {
    let input: serde_json::Value = serde_json::from_str(input).unwrap();
    JsonSolution {
        values: input["valueList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        entities: input["entityList"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entity| JsonEntity {
                value: entity
                    .get("value")
                    .map(|value| value.as_str().unwrap().into()),
            })
            .collect(),
        score: input
            .get("score")
            .map(|score| score.as_str().unwrap().parse().unwrap()),
    }
}
fn encode_json(solution: &JsonSolution) -> String {
    // The source asserts the entire body including this declaration order.
    // Values and strings are encoded by serde_json, never inserted unescaped.
    format!(
        "{{\"valueList\":{},\"entityList\":{},\"score\":{}}}",
        serde_json::to_string(&solution.values).unwrap(),
        serde_json::to_string(
            &solution
                .entities
                .iter()
                .map(|entity| serde_json::json!({"value":entity.value}))
                .collect::<Vec<_>>()
        )
        .unwrap(),
        serde_json::to_string(&solution.score.unwrap().to_string()).unwrap()
    )
}
fn json_binding() -> PlanningSolutionBinding<JsonSolution, String> {
    let entity = PlanningEntityBinding::default()
        .member(
            "value",
            "valueRange",
            |entity: &JsonEntity| entity.value.clone(),
            |entity, value| entity.value = value,
        )
        .unwrap();
    PlanningSolutionBinding::default().collection(
        entity,
        |solution: &JsonSolution| &solution.entities,
        |solution| &mut solution.entities,
        |solution, name| (name == "valueRange").then(|| solution.values.clone()),
    )
}
fn duplicates(values: &[Option<&String>]) -> i64 {
    values
        .iter()
        .enumerate()
        .map(|(left, value)| {
            values
                .iter()
                .enumerate()
                .filter(|(right, other)| left != *right && value.is_some() && value == *other)
                .count() as i64
        })
        .sum()
}
type JsonGraph = (
    StreamGraph<'static, SimpleScore>,
    StreamSource<usize, (usize, Option<String>)>,
);
fn json_graph() -> Result<JsonGraph, GraphError> {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (usize, Option<String>)>();
    let rows = builder.collect_list(source.port(), |row| row.clone())?;
    builder.score("Don't assign 2 entities the same value.", &rows, |rows| {
        let values = rows
            .iter()
            .map(|(_, value)| value.as_ref())
            .collect::<Vec<_>>();
        SimpleScore::new(-duplicates(&values))
    })?;
    Ok((builder.build(), source))
}
fn json_factory(stream: bool) -> SolverFactory<JsonSolution, JsonSolution, String> {
    let config = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 1000,
            best_score_limit: Some(SimpleScore::ZERO),
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_steps: Some(2),
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 16,
            move_order: MoveOrder::Original,
            assert_scores: true,
            ..Default::default()
        }],
    };
    SolverFactory::new(config, move |mut input: JsonSolution, config, progress| {
        let binding = json_binding();
        let model = binding.open(&input).map_err(|error| error.to_string())?;
        let result = if stream {
            solve_stream(&model, json_graph, config).map_err(|error| error.to_string())?
        } else {
            let mut network = ConstraintNetwork::new(2);
            network
                .add_constraint(
                    "Don't assign 2 entities the same value.",
                    vec![0, 1],
                    |values| SimpleScore::new(-duplicates(values)),
                )
                .unwrap();
            solve(&model, &network, config).map_err(|error| error.to_string())?
        };
        assert!(result.complete && result.work.evaluations <= 1000);
        binding
            .commit(&mut input, &result.assignment)
            .map_err(|error| error.to_string())?;
        input.score = result.score;
        assert_eq!(
            input.score,
            Some(SimpleScore::new(-duplicates(
                &input
                    .entities
                    .iter()
                    .map(|entity| entity.value.as_ref())
                    .collect::<Vec<_>>()
            )))
        );
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
    .unwrap()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Shadows,
    Reflection,
}
#[derive(Clone, Debug)]
struct Entity {
    value: Option<String>,
    field_value: Option<String>,
    local: Vec<String>,
    length: Option<i32>,
    with_param: Option<i32>,
    without_param: Option<i32>,
}
#[derive(Clone, Debug)]
struct Solution {
    kind: Kind,
    entities: Vec<Arc<Entity>>,
    values: Vec<String>,
    method_values: Vec<String>,
    score: Option<HardSoftScore>,
}
impl Solution {
    fn dummy_shadow_value(&self) -> i32 {
        10
    }
}
fn fixture(kind: Kind) -> Solution {
    Solution {
        kind,
        entities: (0..2)
            .map(|_| {
                Arc::new(Entity {
                    value: None,
                    field_value: None,
                    local: if kind == Kind::Shadows {
                        vec!["ccc".into()]
                    } else {
                        vec![]
                    },
                    length: None,
                    with_param: None,
                    without_param: None,
                })
            })
            .collect(),
        values: if kind == Kind::Shadows {
            ["a", "bb"].map(String::from).to_vec()
        } else {
            ["a", "bb", "ccc"].map(String::from).to_vec()
        },
        method_values: if kind == Kind::Reflection {
            ["a", "bb", "ccc", "ddd"].map(String::from).to_vec()
        } else {
            vec![]
        },
        score: None,
    }
}
fn schema(kind: Kind) -> Arc<PlanningSchema> {
    let mut fields = vec![];
    if kind == Kind::Reflection {
        fields.push(PlanningVariableDefinition::basic::<String>(
            "fieldValue",
            false,
        ));
    }
    fields.push(PlanningVariableDefinition::basic::<String>("value", false));
    if kind == Kind::Shadows {
        fields.extend([
            PlanningVariableDefinition::shadow::<i32>("length"),
            PlanningVariableDefinition::shadow::<i32>("withParam"),
            PlanningVariableDefinition::shadow::<i32>("withoutParam"),
        ]);
    }
    Arc::new(
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(fields)])
            .unwrap(),
    )
}
fn accessors(kind: Kind) -> SolutionEntityRangeAccessors<'static, Solution, String> {
    let fields = if kind == Kind::Shadows {
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["local".into(), "shared".into()],
        )]
    } else {
        vec![
            SolutionFieldRange::new::<Entity>("fieldValue", vec!["fields".into()]),
            SolutionFieldRange::new::<Entity>("value", vec!["methods".into()]),
        ]
    };
    let mut accessors = SolutionEntityRangeAccessors::new(schema(kind), fields).unwrap();
    accessors
        .entities(move |solution: &Solution| {
            solution
                .entities
                .iter()
                .map(|entity| {
                    let mut values = vec![];
                    if kind == Kind::Reflection {
                        values.push(("fieldValue".into(), entity.field_value.clone()));
                    }
                    values.push(("value".into(), entity.value.clone()));
                    (entity.clone(), values)
                })
                .collect()
        })
        .unwrap();
    if kind == Kind::Shadows {
        accessors
            .provider("local", |_: &Solution, entity: &Entity| {
                entity.local.clone()
            })
            .unwrap();
        accessors
            .provider("shared", |solution: &Solution, _: &Entity| {
                solution.values.clone()
            })
            .unwrap();
    } else {
        accessors
            .provider("fields", |solution: &Solution, _: &Entity| {
                solution.values.clone()
            })
            .unwrap();
        accessors
            .provider("methods", |solution: &Solution, _: &Entity| {
                solution.method_values.clone()
            })
            .unwrap();
    }
    accessors
}
fn publish_shadows(solution: &mut Solution) {
    if solution.kind != Kind::Shadows {
        return;
    }
    let schema = schema(Kind::Shadows);
    let mut named = NamedShadowSuppliers::<Solution>::new(&schema).unwrap();
    named
        .register("updateLength", |entity: &Entity, _: &Solution| {
            Some(entity.value.as_ref().map_or(0, |value| value.len() as i32))
        })
        .unwrap();
    named
        .register(
            "processShadowVarWithParam",
            |_: &Entity, solution: &Solution| Some(solution.dummy_shadow_value()),
        )
        .unwrap();
    named
        .register(
            "processShadowVarWithoutParam",
            |_: &Entity, _: &Solution| Some(-1_i32),
        )
        .unwrap();
    let mut builder = ShadowSessionBuilder::<usize, Solution>::new(&schema);
    let mut outputs = vec![];
    for index in 0..2 {
        let input = builder
            .input(
                schema
                    .entity::<Entity>()
                    .unwrap()
                    .variable("value")
                    .unwrap(),
                index,
                move |solution| solution.entities[index].value.clone(),
            )
            .unwrap();
        let mut fields = vec![];
        for (field, name) in [
            ("length", "updateLength"),
            ("withParam", "processShadowVarWithParam"),
            ("withoutParam", "processShadowVarWithoutParam"),
        ] {
            let supplier = named.resolve::<Entity, i32>(field, name).unwrap();
            let output = builder
                .derive(supplier.target(), index, move |solution, _| {
                    supplier.evaluate(solution.entities[index].as_ref(), solution)
                })
                .unwrap();
            builder.set_dependencies(&output, &[input.erase()]).unwrap();
            fields.push(output);
        }
        outputs.push(fields);
    }
    let mut session = builder.build();
    session.update(solution).unwrap();
    for (entity, fields) in solution.entities.iter_mut().zip(outputs) {
        let entity = Arc::make_mut(entity);
        entity.length = session.read(&fields[0]).unwrap().value.copied();
        entity.with_param = session.read(&fields[1]).unwrap().value.copied();
        entity.without_param = session.read(&fields[2]).unwrap().value.copied();
    }
}
fn projected(input: &Solution, values: &[Option<&String>]) -> Solution {
    let mut output = input.clone();
    let width = if input.kind == Kind::Reflection { 2 } else { 1 };
    for (index, entity) in output.entities.iter_mut().enumerate() {
        let entity = Arc::make_mut(entity);
        entity.value = values[index * width + width - 1].cloned();
        if width == 2 {
            entity.field_value = values[index * width].cloned();
        }
    }
    publish_shadows(&mut output);
    output
}
fn objective(solution: &Solution) -> HardSoftScore {
    let mut hard = 0;
    let mut soft = 0;
    for left in &solution.entities {
        if left.value.is_none() || (solution.kind == Kind::Reflection && left.field_value.is_none())
        {
            continue;
        }
        soft += if solution.kind == Kind::Shadows {
            i64::from(left.length.unwrap())
        } else {
            left.value.as_ref().unwrap().len() as i64
        };
        for right in &solution.entities {
            if right.value.is_none()
                || (solution.kind == Kind::Reflection && right.field_value.is_none())
            {
                continue;
            }
            if solution.kind == Kind::Shadows {
                hard -= i64::from(!Arc::ptr_eq(left, right) && left.value == right.value);
            } else {
                hard -=
                    i64::from(left.value == right.value && left.field_value != right.field_value);
            }
        }
    }
    HardSoftScore::new(hard, soft)
}
type HardGraph = (
    StreamGraph<'static, HardSoftScore>,
    StreamSource<usize, (usize, Option<String>)>,
);
fn hard_graph(input: Solution) -> Result<HardGraph, GraphError> {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (usize, Option<String>)>();
    let rows = builder.collect_list(source.port(), |row| row.clone())?;
    builder.score("TEST constraint provider", &rows, move |rows| {
        let width = if input.kind == Kind::Reflection { 2 } else { 1 };
        let mut values = vec![None; 2 * width];
        for (slot, value) in rows.iter() {
            values[*slot] = value.as_ref();
        }
        objective(&projected(&input, &values))
    })?;
    Ok((builder.build(), source))
}
fn hard_factory(
    kind: Kind,
    stream: bool,
) -> SolverFactory<Solution, Solution, String, HardSoftScore> {
    let goal = HardSoftScore::new(0, if kind == Kind::Shadows { 5 } else { 6 });
    let config = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 1000,
            best_score_limit: Some(goal),
            max_time: (kind == Kind::Shadows).then_some(Duration::from_secs(30)),
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_steps: Some(4),
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 32,
            move_order: MoveOrder::Original,
            assert_scores: true,
            ..Default::default()
        }],
    };
    SolverFactory::new(config, move |mut input: Solution, config, progress| {
        let accessors = accessors(kind);
        let bound = accessors.bind(&input).map_err(|error| error.to_string())?;
        let model = bound.problem();
        let result = if stream {
            solve_stream(model, || hard_graph(input.clone()), config)
                .map_err(|error| error.to_string())?
        } else {
            let mut network = ConstraintNetwork::new(model.variables().len());
            let captured = input.clone();
            network
                .add_constraint(
                    "TEST constraint provider",
                    (0..model.variables().len()).collect(),
                    move |values| objective(&projected(&captured, values)),
                )
                .unwrap();
            solve(model, &network, config).map_err(|error| error.to_string())?
        };
        assert!(result.complete && result.work.evaluations <= 1000);
        model.validate(&result.assignment).unwrap();
        for (index, owner) in bound.entities().iter().enumerate() {
            let entity = Arc::make_mut(&mut input.entities[index]);
            entity.value = result.assignment.values()[owner.slot("value").unwrap()].clone();
            if kind == Kind::Reflection {
                entity.field_value =
                    result.assignment.values()[owner.slot("fieldValue").unwrap()].clone();
            }
        }
        publish_shadows(&mut input);
        input.score = result.score;
        assert_eq!(input.score, Some(objective(&input)));
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
    .unwrap()
}
fn dto_score(solution: &Solution) -> String {
    let dto = serde_json::json!({"score":solution.score.unwrap().to_string(),
        "entityList":solution.entities.iter().map(|entity|serde_json::json!({"value":entity.value,"fieldValue":entity.field_value,"length":entity.length})).collect::<Vec<_>>()});
    let output: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&dto).unwrap()).unwrap();
    output["score"].as_str().unwrap().into()
}
#[test]
fn jackson_resource_input_and_complete_output_body() {
    for stream in [false, true] {
        let input = decode_json(r#"{"valueList":["v1","v2"],"entityList":[{},{}]}"#);
        let mut manager = json_factory(stream).build_manager(1).unwrap();
        let job = manager.job_builder(1).with_problem(input).run().unwrap();
        let result = job.wait();
        let solution = result.as_ref().as_ref().unwrap();
        let output = encode_json(solution);
        assert_eq!(
            output,
            r#"{"valueList":["v1","v2"],"entityList":[{"value":"v1"},{"value":"v2"}],"score":"0"}"#
        );
        let decoded = decode_json(&output);
        assert_eq!(decoded.score, solution.score);
        assert_eq!(decoded.entities[0].value, solution.entities[0].value);
        manager.shutdown().unwrap();
    }
}
#[test]
fn quarkus_resource_live_shadow_score() {
    for stream in [false, true] {
        let mut manager = hard_factory(Kind::Shadows, stream)
            .build_manager(1)
            .unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(fixture(Kind::Shadows))
            .run()
            .unwrap();
        let result = job.wait();
        let solution = result.as_ref().as_ref().unwrap();
        assert_eq!(dto_score(solution), "0hard/5soft");
        assert!(solution.entities.iter().all(|entity| entity.length
            == Some(entity.value.as_ref().unwrap().len() as i32)
            && entity.with_param == Some(10)
            && entity.without_param == Some(-1)));
        manager.shutdown().unwrap();
    }
}
#[test]
fn quarkus_resource_actual_time_override_and_formatted_gradient() {
    for stream in [false, true] {
        let factory = hard_factory(Kind::Shadows, stream);
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(fixture(Kind::Shadows))
            .with_time_limit(Some(Duration::from_secs(10)))
            .run()
            .unwrap();
        let result = job.wait();
        let solution = result.as_ref().as_ref().unwrap();
        let gradient = job
            .configuration()
            .limits
            .time_gradient(Duration::from_secs(5), Duration::ZERO)
            .unwrap();
        assert_eq!(
            format!("{},{gradient:.2}", dto_score(solution)),
            "0hard/5soft,0.50"
        );
        assert_eq!(
            job.configuration().limits.max_time,
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            factory.configuration().limits.max_time,
            Some(Duration::from_secs(30))
        );
        manager.shutdown().unwrap();
    }
}
#[test]
fn reflection_resource_separate_field_and_method_ranges() {
    for stream in [false, true] {
        let mut manager = hard_factory(Kind::Reflection, stream)
            .build_manager(1)
            .unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(fixture(Kind::Reflection))
            .run()
            .unwrap();
        let result = job.wait();
        let solution = result.as_ref().as_ref().unwrap();
        assert_eq!(dto_score(solution), "0hard/6soft");
        for entity in &solution.entities {
            assert!(
                solution
                    .values
                    .contains(entity.field_value.as_ref().unwrap())
            );
            assert!(
                solution
                    .method_values
                    .contains(entity.value.as_ref().unwrap())
            );
        }
        manager.shutdown().unwrap();
    }
}

#[test]
fn source_join_penalties_and_json_escaping_have_real_controls() {
    for kind in [Kind::Shadows, Kind::Reflection] {
        let initial = fixture(kind);
        let assigned = if kind == Kind::Shadows {
            ["ccc", "ccc"].map(String::from).to_vec()
        } else {
            ["a", "ccc", "bb", "ccc"].map(String::from).to_vec()
        };
        let assigned = assigned.iter().map(Some).collect::<Vec<_>>();
        let input = projected(&initial, &assigned);
        assert_eq!(objective(&input), HardSoftScore::new(-2, 6));
        let bound = accessors(kind).bind(&input).unwrap();
        let (graph, source) = hard_graph(input.clone()).unwrap();
        let director = StreamScoreDirector::new(
            bound.problem(),
            graph,
            source,
            bound.problem().initial_assignment(),
        )
        .unwrap();
        assert_eq!(director.score(), HardSoftScore::new(-2, 6));
        let mut partial = assigned;
        partial[0] = None;
        let partial = projected(&initial, &partial);
        assert_eq!(objective(&partial), HardSoftScore::new(0, 3));
    }
    let duplicated = ["v1".to_string(), "v1".to_string()];
    assert_eq!(
        duplicates(&duplicated.iter().map(Some).collect::<Vec<_>>()),
        2
    );
    assert_eq!(duplicates(&[Some(&duplicated[0]), None]), 0);
    let encoded = encode_json(&JsonSolution {
        values: vec!["quote\" slash\\".into()],
        entities: vec![JsonEntity {
            value: Some("quote\" slash\\".into()),
        }],
        score: Some(SimpleScore::ZERO),
    });
    let decoded = decode_json(&encoded);
    assert_eq!(decoded.entities[0].value, Some("quote\" slash\\".into()));
    assert_eq!(decoded.values, ["quote\" slash\\"]);
}
