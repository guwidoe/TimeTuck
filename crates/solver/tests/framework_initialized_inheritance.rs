// Adapted pinned Community Quarkus AddVar TEST methods and TEST generators.
// Apache-2.0; see compat/provenance.json and THIRD_PARTY_NOTICES.md.
#[path = "fixtures/planning_hierarchy_fixture.rs"]
mod fixture;
use fixture::*;
use timetuck::*;

fn eligible(values: &[Option<&String>]) -> i64 {
    values
        .chunks_exact(2)
        .filter(|entity| entity.iter().all(Option::is_some))
        .count() as i64
}
type ScoringGraph = (
    StreamGraph<'static, SimpleScore>,
    StreamSource<usize, (usize, Option<String>)>,
);
fn graph() -> Result<ScoringGraph, GraphError> {
    let mut builder = StreamGraphBuilder::new();
    let source = builder.source::<usize, (usize, Option<String>)>();
    let rows = builder.collect_list(source.port(), |row| row.clone())?;
    let counts = builder.map(&rows, |rows| {
        let mut values = vec![None; 4];
        for (id, value) in rows.iter() {
            values[*id] = value.as_ref();
        }
        eligible(&values)
    })?;
    // Both TEST providers use forEach(child), so both genuine variables must
    // be initialized before either filtered +1 reward can contribute.
    builder.score("Constraint", &counts, |&count| SimpleScore::new(count))?;
    builder.score("Constraint 2", &counts, |&count| SimpleScore::new(count))?;
    Ok((builder.build(), source))
}
fn factory<S: OwnedScore + Send + Sync + 'static>(
    binding: impl Fn() -> PlanningSolutionBinding<S, String> + Send + Sync + 'static,
    stream: bool,
) -> Result<SolverFactory<S, S, String>, SolverFactoryError> {
    let config = SolverConfig {
        limits: SolverLimits {
            max_evaluations: 100,
            best_score_limit: Some(SimpleScore::ZERO),
            assert_phase_scores: true,
            ..Default::default()
        },
        construction: ConstructionConfig {
            max_steps: Some(4),
            assert_scores: true,
            ..Default::default()
        },
        local_search: vec![LocalSearchConfig {
            max_steps: 16,
            max_evaluations: 80,
            move_order: MoveOrder::Original,
            assert_scores: true,
            ..Default::default()
        }],
    };
    SolverFactory::new(config, move |mut input: S, config, progress| {
        let layout = binding();
        let model = layout.open(&input).map_err(|error| error.to_string())?;
        assert_eq!(model.variables().len(), 4);
        assert_eq!(
            model.initial_assignment().values(),
            &[
                Some("Generated Value 0".to_string()),
                Some("Generated Value2 0".to_string()),
                Some("Generated Value 1".to_string()),
                Some("Generated Value2 1".to_string()),
            ]
        );
        let result = if stream {
            solve_stream(&model, graph, config).map_err(|error| error.to_string())?
        } else {
            let mut network = ConstraintNetwork::new(4);
            for name in ["Constraint", "Constraint 2"] {
                network
                    .add_constraint(name, (0..4).collect(), |values| {
                        SimpleScore::new(eligible(values))
                    })
                    .map_err(|error| error.to_string())?;
            }
            timetuck::solve(&model, &network, config).map_err(|error| error.to_string())?
        };
        assert!(result.complete);
        assert!(result.solution().is_some());
        layout
            .commit(&mut input, &result.assignment)
            .map_err(|error| error.to_string())?;
        input.set_score(result.score.unwrap());
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
}
fn assert_domains(values: &[String], second: &[String]) {
    assert_eq!(values, generated(3, "Value"));
    assert_eq!(second, generated(3, "Value2"));
}
#[test]
fn both_classes_annotated_add_new_variable_initialized() {
    for stream in [false, true] {
        let mut problem = solution(3, 2, "Value2");
        for (id, entity) in problem.entities.iter_mut().enumerate() {
            entity.base.value = Some(problem.values[id % 3].clone());
            entity.extra = Some(problem.extra_values[id % 3].clone());
            assert_eq!(entity.base.id, id);
        }
        assert_domains(&problem.values, &problem.extra_values);
        assert!(problem.score.is_none());
        let factory = factory(|| solution_binding(Some("value2")), stream).unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let result = job.wait();
        let output = result.as_ref().as_ref().unwrap(); // Source assertNotNull.
        assert_eq!(output.score, Some(SimpleScore::new(4)));
        assert_eq!(class_score(output, true), 4);
        assert_domains(&output.values, &output.extra_values);
        assert_eq!(output.entities.len(), 2);
        for (id, entity) in output.entities.iter().enumerate() {
            assert_eq!(entity.base.id, id);
            assert!(output.values.contains(entity.base.value.as_ref().unwrap()));
            assert!(output.extra_values.contains(entity.extra.as_ref().unwrap()));
        }
        manager.shutdown().unwrap();
    }
}
#[test]
fn both_classes_annotated_add_new_variable_base_interface_initialized() {
    for stream in [false, true] {
        let mut problem = interface_solution(3, 2);
        for (id, entity) in problem.entities.iter_mut().enumerate() {
            entity.set_value(Some(problem.values[id % 3].clone()));
            entity.extra = Some(problem.extra_values[id % 3].clone());
            assert_eq!(entity.id, id);
        }
        assert_domains(&problem.values, &problem.extra_values);
        assert!(problem.score.is_none());
        let factory = factory(|| interface_solution_binding(true), stream).unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager.job_builder(1).with_problem(problem).run().unwrap();
        let result = job.wait();
        let output = result.as_ref().as_ref().unwrap(); // Source assertNotNull.
        assert_eq!(output.score, Some(SimpleScore::new(4)));
        assert_eq!(interface_score(output, true), 4);
        assert_domains(&output.values, &output.extra_values);
        assert_eq!(output.entities.len(), 2);
        for (id, entity) in output.entities.iter().enumerate() {
            assert_eq!(entity.id, id);
            assert!(output.values.contains(entity.value().as_ref().unwrap()));
            assert!(output.extra_values.contains(entity.extra.as_ref().unwrap()));
        }
        manager.shutdown().unwrap();
    }
}
