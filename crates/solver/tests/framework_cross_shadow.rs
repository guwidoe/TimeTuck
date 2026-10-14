// Adapted pinned Community Quarkus TEST/helpers; Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
// Native registration replaces annotation scanning/CDI; no framework parity.
use timetuck::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Entity {
    value1: Option<String>,
    value2: Option<String>,
    value1_and_value2: Option<String>,
}
impl Entity {
    fn update_value1_and_value2(&self) -> Option<String> {
        Some(format!(
            "{}+{}",
            self.value1.as_ref()?,
            self.value2.as_ref()?
        ))
    }
}
#[derive(Clone, Debug)]
struct Solution {
    values: Vec<String>,
    entities: Vec<Entity>,
    score: Option<SimpleScore>,
}
fn input() -> Solution {
    Solution {
        values: vec!["v1".into(), "v2".into()],
        entities: vec![Entity::default(); 2],
        score: None,
    }
}
fn binding() -> PlanningSolutionBinding<Solution, String> {
    let entity = PlanningEntityBinding::default()
        .member(
            "value1",
            "valueRange",
            |e: &Entity| e.value1.clone(),
            |e, v| e.value1 = v,
        )
        .unwrap()
        .member(
            "value2",
            "valueRange",
            |e: &Entity| e.value2.clone(),
            |e, v| e.value2 = v,
        )
        .unwrap();
    PlanningSolutionBinding::default().collection(
        entity,
        |s: &Solution| &s.entities,
        |s| &mut s.entities,
        |s, name| (name == "valueRange").then(|| s.values.clone()),
    )
}
fn schema() -> PlanningSchema {
    PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
        PlanningVariableDefinition::basic::<String>("value1", false),
        PlanningVariableDefinition::basic::<String>("value2", false),
        PlanningVariableDefinition::shadow::<String>("value1AndValue2"),
    ])])
    .unwrap()
}
type World = Solution;
type SupplierBuilder<'a> = ShadowSessionBuilder<'a, 'a, usize, World>;
fn supplier_builder(schema: &PlanningSchema) -> (SupplierBuilder<'_>, Vec<ShadowNode<String>>) {
    let mut named = NamedShadowSuppliers::<World>::new(schema).unwrap();
    named
        .register("updateValue1AndValue2", |entity: &Entity, _: &World| {
            entity.update_value1_and_value2()
        })
        .unwrap();
    let fields = schema.entity::<Entity>().unwrap();
    let mut builder = ShadowSessionBuilder::<usize, World>::new(schema);
    let mut nodes = Vec::new();
    for index in 0..2 {
        let a = builder
            .input(fields.variable("value1").unwrap(), index, move |w| {
                w.entities[index].value1.clone()
            })
            .unwrap();
        let b = builder
            .input(fields.variable("value2").unwrap(), index, move |w| {
                w.entities[index].value2.clone()
            })
            .unwrap();
        let supplier = named
            .resolve::<Entity, String>("value1AndValue2", "updateValue1AndValue2")
            .unwrap();
        let target = builder
            .derive(supplier.target(), index, move |w, _| {
                supplier.evaluate(&w.entities[index], w)
            })
            .unwrap();
        builder
            .set_dependencies(&target, &[a.erase(), b.erase()])
            .unwrap();
        nodes.push(target);
    }
    (builder, nodes)
}
fn world(values: &[Option<String>]) -> World {
    assert_eq!(values.len(), 4);
    Solution {
        values: vec!["v1".into(), "v2".into()],
        entities: values
            .chunks_exact(2)
            .map(|pair| Entity {
                value1: pair[0].clone(),
                value2: pair[1].clone(),
                value1_and_value2: None,
            })
            .collect(),
        score: None,
    }
}
fn project(schema: &PlanningSchema, values: &[Option<String>]) -> Vec<Entity> {
    let mut world = world(values);
    let (builder, nodes) = supplier_builder(schema);
    let mut session = builder.build();
    session.update(&world).unwrap();
    for (entity, node) in world.entities.iter_mut().zip(nodes) {
        entity.value1_and_value2 = session.read(&node).unwrap().value.cloned();
    }
    world.entities
}
fn complete(e: &Entity) -> bool {
    e.value1.is_some() && e.value2.is_some()
}
fn shadow_equal(a: &Entity, b: &Entity) -> bool {
    a.value1_and_value2
        .as_ref()
        .expect("supplier before filter")
        == b.value1_and_value2
            .as_ref()
            .expect("supplier before filter")
}
fn objective(entities: &[Entity]) -> SimpleScore {
    let mut penalty = 0;
    for a in entities.iter().filter(|e| complete(e)) {
        for b in entities.iter().filter(|e| complete(e)) {
            if a.value1 == b.value2 && shadow_equal(a, b) {
                penalty += 1;
            }
        }
    }
    SimpleScore::new(-penalty)
}
fn graph(
    schema: &PlanningSchema,
) -> Result<(StreamGraph<'_, SimpleScore>, PlanningStreamSource<String>), GraphError> {
    let mut graph = StreamGraphBuilder::new();
    let source = graph.source::<usize, (usize, Option<String>)>();
    let rows = graph.collect_list(source.port(), |row| row.clone())?;
    let worlds = graph.map(&rows, |rows| {
        let mut values = vec![None; 4];
        for (slot, value) in rows.iter() {
            values[*slot] = value.clone();
        }
        world(&values)
    })?;
    let (builder, nodes) = supplier_builder(schema);
    let projected = graph.map_shadows(&worlds, builder, move |world, session| {
        let mut output = world.entities.clone();
        for (entity, node) in output.iter_mut().zip(&nodes) {
            entity.value1_and_value2 = session.read(node)?.value.cloned();
        }
        Ok(output)
    })?;
    let entities = graph.flat_map(&projected, |world| world.clone())?;
    let eligible = graph.filter(&entities, complete)?;
    let pairs = graph.join_equal(
        &eligible,
        &eligible,
        |e| e.value1.clone(),
        |e| e.value2.clone(),
    )?;
    let matches = graph.filter(&pairs, |(a, b)| shadow_equal(a, b))?;
    graph.score("Don't assign 2 entities the same value.", &matches, |_| {
        SimpleScore::new(-1)
    })?;
    Ok((graph.build(), source))
}
fn factory(stream: bool) -> SolverFactory<Solution, Solution, String> {
    let config = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 128,
            best_score_limit: Some(SimpleScore::ZERO),
            assert_phase_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 16,
            max_evaluations: 64,
            assert_scores: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    SolverFactory::new(config, move |mut input: Solution, config, progress| {
        let layout = binding();
        let model = layout.open(&input).map_err(|e| e.to_string())?;
        let schema = schema();
        assert_eq!(model.variables().len(), 4);
        assert!(
            model
                .initial_assignment()
                .values()
                .iter()
                .all(Option::is_none)
        );
        let result = if stream {
            solve_stream(&model, || graph(&schema), config)
        } else {
            let mut network = ConstraintNetwork::new(4);
            network
                .add_constraint(
                    "Don't assign 2 entities the same value.",
                    (0..4).collect(),
                    |values| {
                        objective(&project(
                            &schema,
                            &values.iter().map(|v| v.cloned()).collect::<Vec<_>>(),
                        ))
                    },
                )
                .unwrap();
            solve(&model, &network, config)
        }
        .map_err(|e| e.to_string())?;
        assert!(result.complete);
        layout
            .commit(&mut input, &result.assignment)
            .map_err(|e| e.to_string())?;
        let projected = project(&schema, result.assignment.values());
        for (entity, projected) in input.entities.iter_mut().zip(projected) {
            assert_eq!(entity.value1, projected.value1);
            assert_eq!(entity.value2, projected.value2);
            entity.value1_and_value2 = projected.value1_and_value2;
        }
        input.score = result.score;
        assert_eq!(input.score, Some(objective(&input.entities)));
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
    .unwrap()
}
#[test]
fn quarkus_two_entity_shadow_variable_solve() {
    for stream in [false, true] {
        let problem = input();
        assert_eq!(problem.values, ["v1", "v2"]);
        assert_eq!(problem.entities, vec![Entity::default(); 2]);
        assert!(problem.score.is_none());
        let factory = factory(stream);
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let result = job.wait();
        let output = result.as_ref().as_ref().unwrap(); // Source assertNotNull.
        assert!(output.score.unwrap().score() >= 0);
        assert_eq!(output.entities.len(), 2);
        assert_eq!(output.values, ["v1", "v2"]);
        for entity in &output.entities {
            assert!(output.values.contains(entity.value1.as_ref().unwrap()));
            assert!(output.values.contains(entity.value2.as_ref().unwrap()));
            assert_eq!(entity.value1_and_value2, entity.update_value1_and_value2());
        }
        manager.shutdown().unwrap();
    }
}
#[test]
fn original_two_entity_self_cross_and_partial_objective_controls() {
    let schema = schema();
    let model = binding().open(&input()).unwrap();
    for (values, expected) in [
        ([Some("v1"), Some("v1"), Some("v1"), Some("v1")], -4),
        ([Some("v1"), Some("v1"), Some("v2"), Some("v2")], -2),
        ([Some("v1"), Some("v2"), Some("v2"), Some("v1")], 0),
        ([Some("v1"), None, Some("v1"), Some("v1")], -1),
        ([None, None, None, None], 0),
    ] {
        let values: Vec<_> = values.into_iter().map(|v| v.map(str::to_string)).collect();
        let entities = project(&schema, &values);
        assert_eq!(objective(&entities), SimpleScore::new(expected));
        for entity in &entities {
            assert_eq!(entity.value1_and_value2, entity.update_value1_and_value2());
        }
        let (graph, source) = graph(&schema).unwrap();
        let mut director =
            StreamScoreDirector::new(&model, graph, source, Assignment::new(values)).unwrap();
        assert_eq!(
            director.score().with_structural(0),
            SimpleScore::new(expected)
        );
        director.assert_consistent().unwrap();
    }
}
