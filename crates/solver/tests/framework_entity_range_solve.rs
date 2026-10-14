// Adapted Timefold Community spring-boot-integration-test TEST/application
// fixtures only. Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Native range bindings, owned jobs and JSON output replace the HTTP/CDI wrapper.
use std::sync::Arc;
use timetuck::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Value {
    id: String,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Entity {
    id: String,
    value: Option<Value>,
    value_list: Vec<Value>,
    shadow_with_param: Option<i32>,
    shadow_without_param: Option<i32>,
}
#[derive(Clone, Debug)]
struct Solution {
    entity_list: Vec<Arc<Entity>>,
    value_list: Vec<Value>,
    score: Option<SimpleScore>,
}
impl Solution {
    fn dummy_shadow_value(&self) -> i32 {
        10
    }
}
fn value(id: &str) -> Value {
    Value { id: id.into() }
}
fn fixture() -> Solution {
    Solution {
        entity_list: (0..3)
            .map(|id| {
                Arc::new(Entity {
                    id: id.to_string(),
                    value: None,
                    value_list: vec![value("1"), value("2")],
                    shadow_with_param: None,
                    shadow_without_param: None,
                })
            })
            .collect(),
        value_list: vec![value("0")],
        score: None,
    }
}
fn schema() -> Arc<PlanningSchema> {
    Arc::new(
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<Value>("value", false),
            PlanningVariableDefinition::shadow::<i32>("shadowVarWithParam"),
            PlanningVariableDefinition::shadow::<i32>("shadowVarWithoutParam"),
        ])])
        .unwrap(),
    )
}
fn accessors(
    schema: Arc<PlanningSchema>,
) -> SolutionEntityRangeAccessors<'static, Solution, Value> {
    let mut accessors = SolutionEntityRangeAccessors::new(
        schema,
        vec![SolutionFieldRange::new::<Entity>(
            "value",
            vec!["valueRange".into(), "valueRangeWithParameter".into()],
        )],
    )
    .unwrap();
    accessors
        .entities(|solution: &Solution| {
            solution
                .entity_list
                .iter()
                .map(|entity| (entity.clone(), vec![("value".into(), entity.value.clone())]))
                .collect()
        })
        .unwrap();
    accessors
        .provider("valueRange", |_: &Solution, entity: &Entity| {
            entity.value_list.clone()
        })
        .unwrap();
    accessors
        .provider(
            "valueRangeWithParameter",
            |solution: &Solution, _: &Entity| solution.value_list.clone(),
        )
        .unwrap();
    accessors
}
fn mismatch(id: &str, value: Option<&Value>) -> SimpleScore {
    // Source forEach excludes unassigned values before evaluating the filter.
    SimpleScore::new(-i64::from(value.is_some_and(|value| value.id != id)))
}
type ScoringGraph = (
    StreamGraph<'static, SimpleScore>,
    StreamSource<usize, (usize, Option<Value>)>,
);
fn graph(ids: Vec<String>) -> Result<ScoringGraph, GraphError> {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (usize, Option<Value>)>();
    builder.score(
        "Entity id do not match value id",
        source.port(),
        move |(slot, value)| mismatch(&ids[*slot], value.as_ref()),
    )?;
    Ok((builder.build(), source))
}
fn publish_shadows(schema: &PlanningSchema, solution: &mut Solution) {
    let mut named = NamedShadowSuppliers::<Solution>::new(schema).unwrap();
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
    let mut builder = ShadowSessionBuilder::<usize, Solution>::new(schema);
    let variable = schema
        .entity::<Entity>()
        .unwrap()
        .variable("value")
        .unwrap();
    let mut shadows = Vec::new();
    for index in 0..solution.entity_list.len() {
        let input = builder
            .input(variable, index, move |solution| {
                solution.entity_list[index].value.clone()
            })
            .unwrap();
        let with = named
            .resolve::<Entity, i32>("shadowVarWithParam", "processShadowVarWithParam")
            .unwrap();
        let without = named
            .resolve::<Entity, i32>("shadowVarWithoutParam", "processShadowVarWithoutParam")
            .unwrap();
        let with = builder
            .derive(with.target(), index, move |solution, _| {
                with.evaluate(solution.entity_list[index].as_ref(), solution)
            })
            .unwrap();
        let without = builder
            .derive(without.target(), index, move |solution, _| {
                without.evaluate(solution.entity_list[index].as_ref(), solution)
            })
            .unwrap();
        builder.set_dependencies(&with, &[input.erase()]).unwrap();
        builder
            .set_dependencies(&without, &[input.erase()])
            .unwrap();
        shadows.push((with, without));
    }
    let mut session = builder.build();
    session.update(solution).unwrap();
    for (index, (with, without)) in shadows.iter().enumerate() {
        let entity = Arc::make_mut(&mut solution.entity_list[index]);
        entity.shadow_with_param = session.read(with).unwrap().value.copied();
        entity.shadow_without_param = session.read(without).unwrap().value.copied();
    }
}
fn factory(stream: bool) -> SolverFactory<Solution, Solution, String> {
    let config = SolverConfig {
        limits: SolverLimits {
            best_score_limit: Some(SimpleScore::ZERO),
            max_evaluations: 1000,
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_steps: Some(3),
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 16,
            max_evaluations: 100,
            move_order: MoveOrder::Original,
            assert_scores: true,
            ..Default::default()
        }],
    };
    SolverFactory::new(config, move |mut input: Solution, config, progress| {
        let schema = schema();
        let accessors = accessors(schema.clone());
        let bound = accessors.bind(&input).map_err(|error| error.to_string())?;
        let problem = bound.problem();
        let mut ids = vec![String::new(); problem.variables().len()];
        for owner in bound.entities() {
            ids[owner.slot("value").unwrap()] = owner.object_ref::<Entity>().unwrap().id.clone();
        }
        let result = if stream {
            solve_stream(problem, || graph(ids.clone()), config)
                .map_err(|error| error.to_string())?
        } else {
            let mut network = ConstraintNetwork::new(problem.variables().len());
            for (slot, id) in ids.iter().enumerate() {
                let id = id.clone();
                network
                    .add_constraint(
                        format!("Entity id do not match value id {slot}"),
                        vec![slot],
                        move |values| mismatch(&id, values[0]),
                    )
                    .map_err(|error| error.to_string())?;
            }
            solve(problem, &network, config).map_err(|error| error.to_string())?
        };
        assert!(result.complete && result.solution().is_some());
        assert!(result.work.evaluations <= 1000);
        problem.validate(&result.assignment).unwrap();
        // Actual bound owner/field metadata drives publication; no expected
        // value IDs are used to construct the solved output.
        for owner in bound.entities() {
            let source = owner.object_ref::<Entity>().unwrap();
            let index = input
                .entity_list
                .iter()
                .position(|entity| entity.id == source.id)
                .unwrap();
            Arc::make_mut(&mut input.entity_list[index]).value =
                result.assignment.values()[owner.slot("value").unwrap()].clone();
        }
        input.score = result.score;
        publish_shadows(&schema, &mut input);
        let rescored = input
            .entity_list
            .iter()
            .map(|entity| mismatch(&entity.id, entity.value.as_ref()).score())
            .sum();
        assert_eq!(input.score, Some(SimpleScore::new(rescored)));
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
    .unwrap()
}
// Native DTO encodes actual owned output fields. This preserves every source
// JSON-path assertion without claiming an HTTP server or Spring/Jackson parity.
fn json_output(solution: &Solution) -> serde_json::Value {
    let output = serde_json::json!({
        "score": solution.score.unwrap().to_string(),
        "entityList": solution.entity_list.iter().map(|entity| serde_json::json!({
            "id": entity.id,
            "value": entity.value.as_ref().map(|value| serde_json::json!({"id": value.id})),
            "valueList": entity.value_list.iter().map(|value| serde_json::json!({"id": value.id})).collect::<Vec<_>>(),
            "shadowVarWithParam": entity.shadow_with_param,
            "shadowVarWithoutParam": entity.shadow_without_param,
        })).collect::<Vec<_>>(),
        "valueList": solution.value_list.iter().map(|value| serde_json::json!({"id": value.id})).collect::<Vec<_>>(),
    });
    let serialized = serde_json::to_string(&output).unwrap();
    serde_json::from_str(&serialized).unwrap()
}
#[test]
fn entity_ranges_plus_solution_parameter_range_solve() {
    for stream in [false, true] {
        let input = fixture();
        let saved = input.clone();
        let factory = factory(stream);
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager.job_builder(1).with_problem(input).run().unwrap();
        let result = job.wait();
        let solution = result.as_ref().as_ref().unwrap();
        let response = json_output(solution);
        assert_eq!(response["score"], "0");
        assert!(response["entityList"].is_array());
        assert!(response["valueList"].is_array());
        assert_eq!(response["entityList"][0]["id"], "0");
        assert_eq!(response["entityList"][0]["value"]["id"], "0");
        assert_eq!(response["entityList"][1]["id"], "1");
        assert_eq!(response["entityList"][1]["value"]["id"], "1");
        assert_eq!(response["entityList"][2]["id"], "2");
        assert_eq!(response["entityList"][2]["value"]["id"], "2");
        assert_eq!(solution.entity_list.len(), 3);
        assert_eq!(response["valueList"][0]["id"], "0");
        for (solved, original) in solution.entity_list.iter().zip(saved.entity_list.iter()) {
            assert_eq!(solved.value_list, vec![value("1"), value("2")]);
            assert_eq!(solved.shadow_with_param, Some(10));
            assert_eq!(solved.shadow_without_param, Some(-1));
            assert!(original.value.is_none());
            assert!(!Arc::ptr_eq(solved, original));
        }
        manager.shutdown().unwrap();
    }
}
#[test]
fn providers_read_actual_entity_and_solution_before_union_and_rebind() {
    let accessors = accessors(schema());
    let input = fixture();
    let bound = accessors.bind(&input).unwrap();
    for variable in bound.problem().variables() {
        assert_eq!(variable.domain(), &[value("1"), value("2"), value("0")]);
        assert!(variable.initial().is_none());
    }
    let mut changed = input.clone();
    changed.value_list = vec![value("2"), value("shared-new")];
    Arc::make_mut(&mut changed.entity_list[0]).value_list = vec![value("local-new")];
    let rebound = accessors.bind(&changed).unwrap();
    assert_eq!(
        rebound.problem().variables()[0].domain(),
        &[value("local-new"), value("2"), value("shared-new")]
    );
    assert_eq!(
        rebound.problem().variables()[1].domain(),
        &[value("1"), value("2"), value("shared-new")]
    );
    assert_eq!(
        bound.problem().variables()[0].domain(),
        &[value("1"), value("2"), value("0")]
    );
    assert_eq!(mismatch("0", None), SimpleScore::ZERO);
    assert_eq!(mismatch("0", Some(&value("wrong"))), SimpleScore::new(-1));
}
