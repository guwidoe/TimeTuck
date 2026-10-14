//! Original actual solve, ownership, warm-start and preflight controls.
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use timetuck::*;
#[derive(Clone, Debug)]
struct Model {
    tag: String,
    domains: Vec<Vec<i32>>,
    values: Vec<Option<i32>>,
    score: Option<SimpleScore>,
}
#[derive(Debug)]
struct Config {
    steps: u64,
}
fn model(tag: &str) -> Arc<Model> {
    Arc::new(Model {
        tag: tag.into(),
        domains: vec![(0..=6).collect(); 2],
        values: vec![Some(0); 2],
        score: None,
    })
}
fn independent(values: &[Option<i32>]) -> SimpleScore {
    SimpleScore::new(
        values
            .iter()
            .enumerate()
            .map(|(i, v)| (i as i64 + 1) * i64::from(v.unwrap_or(0)))
            .sum(),
    )
}
fn factory(
    stream: bool,
    calls: Arc<AtomicUsize>,
) -> SolverFactory<PreparedModelConversion<Model, Config>, Arc<Model>, String> {
    let config = SolverSequence {
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            max_evaluations: 100,
            max_steps: 10,
            selection: MoveSelection::BestAccepted,
            ..Default::default()
        })],
        ..Default::default()
    };
    SolverFactory::new(
        config,
        move |prepared: PreparedModelConversion<Model, Config>, mut config, progress| {
            let input = prepared.model_input;
            let initial = if let Some(previous) = prepared.last_model_output {
                if previous.tag != input.tag || previous.domains != input.domains {
                    return Err("previous output belongs to a different problem".into());
                }
                previous.values.clone()
            } else {
                input.values.clone()
            };
            if initial.len() != input.domains.len() {
                return Err("model value/domain count mismatch".into());
            }
            let problem = FiniteProblem::new(
                input
                    .domains
                    .iter()
                    .zip(initial)
                    .map(|(domain, initial)| {
                        let variable = Variable::new(domain.clone());
                        initial.map_or(variable.clone(), |v| variable.with_initial(v))
                    })
                    .collect(),
            )
            .map_err(|e| e.to_string())?;
            if let SolverPhase::LocalSearch(local) = &mut config.phases[0] {
                local.max_steps = prepared.model_config.steps;
            }
            calls.fetch_add(1, Ordering::SeqCst);
            let result = if stream {
                solve_stream(
                    &problem,
                    || {
                        let mut builder = StreamGraphBuilder::new();
                        let source = builder.source::<usize, (usize, Option<i32>)>();
                        builder.score("weighted", source.port(), |(i, v)| {
                            SimpleScore::new((*i as i64 + 1) * i64::from(v.unwrap_or(0)))
                        })?;
                        Ok((builder.build(), source))
                    },
                    config,
                )
                .map_err(|e| e.to_string())?
            } else {
                let mut network = ConstraintNetwork::new(input.domains.len());
                for i in 0..input.domains.len() {
                    network
                        .add_constraint(format!("variable{i}"), vec![i], move |values| {
                            SimpleScore::new((i as i64 + 1) * i64::from(*values[0].unwrap_or(&0)))
                        })
                        .map_err(|e| e.to_string())?;
                }
                solve(&problem, &network, config).map_err(|e| e.to_string())?
            };
            assert_eq!(result.score, Some(independent(result.assignment.values())));
            problem
                .validate(&result.assignment)
                .map_err(|e| e.to_string())?;
            progress.record_work(result.work);
            Ok(Arc::new(Model {
                tag: input.tag.clone(),
                domains: input.domains.clone(),
                values: result.assignment.values().to_vec(),
                score: result.score,
            }))
        },
    )
    .unwrap()
}
#[test]
fn actual_managers_consume_config_and_previous_outputs_in_both_backends() {
    for stream in [false, true] {
        let calls = Arc::new(AtomicUsize::new(0));
        let factory = factory(stream, calls.clone());
        let mut manager = factory.build_manager(1).unwrap();
        let convertor = TrivialModelConvertor::<Model, Config>::new();
        let input = model("owned-problem");
        let config = Arc::new(Config { steps: 1 });
        let request = ModelConversionRequest::new(input.clone(), config.clone(), None);
        let first = convertor
            .solve(&request, |prepared| {
                assert!(Arc::ptr_eq(&prepared.model_input, &input));
                assert!(Arc::ptr_eq(&prepared.model_config, &config));
                let job = manager
                    .job_builder(1)
                    .with_problem(prepared)
                    .run()
                    .map_err(|e| e.to_string())?;
                assert!(job.wait().as_ref().as_ref().unwrap().score.is_some());
                job.wait()
                    .as_ref()
                    .as_ref()
                    .map(Arc::clone)
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        assert!(Arc::ptr_eq(&first.model_output, &first.replacement_input));
        assert_eq!(first.model_output.score, Some(SimpleScore::new(12)));
        assert_eq!(input.values, vec![Some(0); 2]);
        assert!(input.score.is_none());
        let request =
            ModelConversionRequest::new(input.clone(), config, Some(first.model_output.clone()));
        let second = convertor
            .solve(&request, |prepared| {
                assert!(Arc::ptr_eq(
                    prepared.last_model_output.as_ref().unwrap(),
                    &first.model_output
                ));
                let job = manager
                    .job_builder(2)
                    .with_problem(prepared)
                    .run()
                    .map_err(|e| e.to_string())?;
                job.wait()
                    .as_ref()
                    .as_ref()
                    .map(Arc::clone)
                    .map_err(|e| e.to_string())
            })
            .unwrap();
        assert_eq!(second.model_output.values, vec![Some(6); 2]);
        assert_eq!(second.model_output.score, Some(SimpleScore::new(18)));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(factory.configuration().phases.len(), 1);
        assert!(
            matches!(&factory.configuration().phases[0],SolverPhase::LocalSearch(local) if local.max_steps==10)
        );
        manager.shutdown().unwrap();
    }
}
#[test]
fn missing_arguments_fail_before_callbacks_and_failed_jobs_publish_no_replacement() {
    let convertor = TrivialModelConvertor::<Model, Config>::new();
    let input = model("actual");
    let config = Arc::new(Config { steps: 1 });
    for key in ["modelInput", "modelConfig", "lastModelOutput"] {
        let mut request = ModelConversionRequest::new(input.clone(), config.clone(), None);
        match key {
            "modelInput" => request.model_input = None,
            "modelConfig" => request.model_config = None,
            _ => request.last_model_output = None,
        }
        let result = convertor.solve::<String>(&request, |_| {
            panic!("invalid boundary reached solve callback")
        });
        assert!(
            matches!(result,Err(ConvertingSolveError::Conversion(ModelConversionError::MissingArgument(actual))) if actual==key)
        );
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let f = factory(false, calls.clone());
    let mut manager = f.build_manager(1).unwrap();
    let request = ModelConversionRequest::new(input.clone(), config, Some(model("different")));
    let result = convertor.solve(&request, |prepared| {
        let job = manager
            .job_builder(3)
            .with_problem(prepared)
            .run()
            .map_err(|e| e.to_string())?;
        job.wait()
            .as_ref()
            .as_ref()
            .map(Arc::clone)
            .map_err(|e| e.to_string())
    });
    assert!(
        matches!(result,Err(ConvertingSolveError::Solve(message)) if message.contains("previous output belongs to a different problem"))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(input.values, vec![Some(0); 2]);
    assert!(input.score.is_none());
    manager.shutdown().unwrap();
}
