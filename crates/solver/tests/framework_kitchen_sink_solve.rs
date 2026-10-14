// Adapted pinned Timefold Community Quarkus Gizmo kitchen-sink TEST/helpers.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Native tagged domains, owned bindings and shadows replace JVM annotation/CDI.
use timetuck::*;

#[derive(Clone, Debug, Eq, PartialEq)]
enum PlanningValue {
    Integer(i32),
    Name(String),
}
#[derive(Clone, Debug, Default)]
struct Entity {
    group_id: Option<String>,
    int_variable: Option<i32>,
    string_variable: Option<String>,
    declarative_shadow_variable: Option<String>,
    inconsistent: bool,
    pinned: bool,
}
#[derive(Clone, Debug)]
struct Solution {
    planning_entity_property: Option<Entity>,
    planning_entity_list_property: Vec<Entity>,
    problem_fact_property: String,
    problem_fact_list_property: Vec<String>,
    score: HardSoftScore,
}
fn fixture() -> Solution {
    Solution {
        planning_entity_property: Some(Entity::default()),
        planning_entity_list_property: vec![],
        problem_fact_property: "Test".into(),
        problem_fact_list_property: vec![],
        score: HardSoftScore::ZERO,
    }
}
fn entity_binding() -> PlanningEntityBinding<Entity, PlanningValue> {
    PlanningEntityBinding::default()
        .member(
            "intVariable",
            "ints",
            |entity: &Entity| entity.int_variable.map(PlanningValue::Integer),
            |entity, value| {
                entity.int_variable = value.map(|value| {
                    let PlanningValue::Integer(value) = value else {
                        panic!("int domain")
                    };
                    value
                });
            },
        )
        .unwrap()
        .member(
            "stringVariable",
            "names",
            |entity| entity.string_variable.clone().map(PlanningValue::Name),
            |entity, value| {
                entity.string_variable = value.map(|value| {
                    let PlanningValue::Name(value) = value else {
                        panic!("name domain")
                    };
                    value
                });
            },
        )
        .unwrap()
}
fn range(_: &Solution, name: &str) -> Option<Vec<PlanningValue>> {
    // Exact TEST entity providers. These domains are the solver's only source
    // of values; the solved output is produced by construction and native setters.
    match name {
        "ints" => Some(vec![PlanningValue::Integer(1)]),
        "names" => Some(vec![PlanningValue::Name("A".into())]),
        _ => None,
    }
}
fn binding() -> PlanningSolutionBinding<Solution, PlanningValue> {
    PlanningSolutionBinding::default()
        .collection(
            entity_binding(),
            |solution: &Solution| solution.planning_entity_property.as_slice(),
            |solution| solution.planning_entity_property.as_mut_slice(),
            range,
        )
        .collection(
            entity_binding(),
            |solution| &solution.planning_entity_list_property,
            |solution| &mut solution.planning_entity_list_property,
            range,
        )
}
fn publish_shadow(solution: &mut Solution) {
    // The source has one entity with null alignment key, so alignment has no
    // additional members. No multi-entity/group alignment parity is inferred.
    let schema =
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i32>("intVariable", false),
            PlanningVariableDefinition::basic::<String>("stringVariable", false),
            PlanningVariableDefinition::shadow::<String>("declarativeShadowVariable"),
        ])])
        .unwrap();
    let mut named = NamedShadowSuppliers::<Solution>::new(&schema).unwrap();
    named
        .register("copyStringVariable", |entity: &Entity, _: &Solution| {
            entity.string_variable.clone()
        })
        .unwrap();
    let supplier = named
        .resolve::<Entity, String>("declarativeShadowVariable", "copyStringVariable")
        .unwrap();
    let mut builder = ShadowSessionBuilder::<usize, Solution>::new(&schema);
    let source = builder
        .input(
            schema
                .entity::<Entity>()
                .unwrap()
                .variable("stringVariable")
                .unwrap(),
            0,
            |solution| {
                solution
                    .planning_entity_property
                    .as_ref()
                    .unwrap()
                    .string_variable
                    .clone()
            },
        )
        .unwrap();
    let output = builder
        .derive(supplier.target(), 0, move |solution, _| {
            supplier.evaluate(
                solution.planning_entity_property.as_ref().unwrap(),
                solution,
            )
        })
        .unwrap();
    builder
        .set_dependencies(&output, &[source.erase()])
        .unwrap();
    let mut session = builder.build();
    session.update(solution).unwrap();
    let shadow = session.read(&output).unwrap();
    let entity = solution.planning_entity_property.as_mut().unwrap();
    entity.declarative_shadow_variable = shadow.value.cloned();
    entity.inconsistent = shadow.inconsistent;
}
type ScoringGraph = (
    StreamGraph<'static, HardSoftScore>,
    StreamSource<usize, (usize, Option<PlanningValue>)>,
);
fn graph() -> Result<ScoringGraph, GraphError> {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source();
    // DummyConstraintProvider.defineConstraints returns an empty array. Native
    // structural initialization still requires both genuine fields to be assigned.
    Ok((builder.build(), source))
}
fn factory(stream: bool) -> SolverFactory<Solution, Solution, String, HardSoftScore> {
    let config = SolverConfig {
        limits: SolverLimits {
            best_score_limit: Some(HardSoftScore::ZERO),
            max_evaluations: 100,
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_steps: Some(2),
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 4,
            max_evaluations: 20,
            move_order: MoveOrder::Original,
            assert_scores: true,
            ..Default::default()
        }],
    };
    SolverFactory::new(config, move |mut input: Solution, config, progress| {
        let layout = binding();
        let model = layout.open(&input).map_err(|error| error.to_string())?;
        assert_eq!(model.variables().len(), 2);
        let result = if stream {
            solve_stream(&model, graph, config).map_err(|error| error.to_string())?
        } else {
            let network = ConstraintNetwork::<PlanningValue, HardSoftScore>::new(2);
            solve(&model, &network, config).map_err(|error| error.to_string())?
        };
        assert!(result.complete && result.solution().is_some());
        assert!(result.work.evaluations <= 100);
        layout
            .commit(&mut input, &result.assignment)
            .map_err(|error| error.to_string())?;
        input.score = result.score.unwrap();
        publish_shadow(&mut input);
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
    .unwrap()
}
#[test]
fn gizmo_kitchen_sink_native_solve() {
    for stream in [false, true] {
        let problem = fixture();
        let original_entity = problem.planning_entity_property.as_ref().unwrap();
        assert!(
            original_entity.int_variable.is_none() && original_entity.string_variable.is_none()
        );
        assert!(!original_entity.pinned);
        assert!(original_entity.group_id.is_none());
        let mut manager = factory(stream).build_manager(1).unwrap();
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let result = job.wait();
        let solution = result.as_ref().as_ref().unwrap(); // Source assertNotNull.
        let entity = solution.planning_entity_property.as_ref().unwrap();
        assert_eq!(entity.int_variable, Some(1));
        assert_eq!(entity.string_variable.as_deref(), Some("A"));
        // Additional original fixture/shadow checks earn no extra source credit.
        assert_eq!(entity.declarative_shadow_variable, entity.string_variable);
        assert!(!entity.inconsistent && !entity.pinned);
        assert!(entity.group_id.is_none());
        assert!(solution.planning_entity_list_property.is_empty());
        assert_eq!(solution.problem_fact_property, "Test");
        assert!(solution.problem_fact_list_property.is_empty());
        assert_eq!(solution.score, HardSoftScore::ZERO);
        manager.shutdown().unwrap();
    }
}
#[test]
fn typed_fields_keep_separate_domains_and_empty_objective_requires_initialization() {
    let problem = fixture();
    let layout = binding();
    let model = layout.open(&problem).unwrap();
    assert_eq!(model.variables()[0].domain(), &[PlanningValue::Integer(1)]);
    assert_eq!(
        model.variables()[1].domain(),
        &[PlanningValue::Name("A".into())]
    );
    assert!(
        model
            .variables()
            .iter()
            .all(|variable| variable.initial().is_none())
    );
    let network = ConstraintNetwork::<PlanningValue, HardSoftScore>::new(2);
    let dense = ScoreDirector::new(&model, &network, model.initial_assignment()).unwrap();
    let (graph, source) = graph().unwrap();
    let stream =
        StreamScoreDirector::new(&model, graph, source, model.initial_assignment()).unwrap();
    assert_eq!(dense.score().structural(), -2);
    assert_eq!(stream.score(), dense.score());
    assert_eq!(dense.score().with_structural(0), HardSoftScore::ZERO);
}
