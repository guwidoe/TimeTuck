// Adapted Community Jackson/JAXB solution TEST helpers; Apache-2.0.
// This helper module is shared by source cases and original integration controls.
#![allow(dead_code)]
use std::{
    path::PathBuf,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use timetuck::*;
static NEXT: AtomicU64 = AtomicU64::new(0);
pub struct Directory(pub PathBuf);
impl Directory {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "timetuck-round165-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
pub fn original(score: i64) -> BasicSolutionDocument {
    let v1 = Arc::new(PersistedValue { code: "v1".into() });
    let v2 = Arc::new(PersistedValue { code: "v2".into() });
    BasicSolutionDocument {
        code: "s1".into(),
        values: vec![v1.clone(), v2],
        entities: vec![
            PersistedEntity {
                code: "e1".into(),
                value: None,
            },
            PersistedEntity {
                code: "e2".into(),
                value: Some(v1),
            },
            PersistedEntity {
                code: "e3".into(),
                value: None,
            },
        ],
        score: Some(SimpleScore::new(score)),
    }
}
pub fn source_roundtrip<C: SolutionCodec<BasicSolutionDocument>>(codec: C, name: &str, score: i64) {
    let directory = Directory::new();
    let io = SolutionFileIo::new(codec);
    let original = Arc::new(original(score));
    let path = directory.0.join(name);
    io.write(original.as_ref(), &path).unwrap();
    let copy = Arc::new(io.read::<BasicSolutionDocument>(&path).unwrap());
    assert!(!Arc::ptr_eq(&original, &copy));
    assert_eq!(copy.code, "s1");
    assert_eq!(
        copy.values
            .iter()
            .map(|v| v.code.as_str())
            .collect::<Vec<_>>(),
        ["v1", "v2"]
    );
    assert_eq!(
        copy.entities
            .iter()
            .map(|e| e.code.as_str())
            .collect::<Vec<_>>(),
        ["e1", "e2", "e3"]
    );
    let copy_v1 = &copy.values[0];
    let copy_e2 = &copy.entities[1];
    assert_eq!(copy_e2.value.as_ref().unwrap().code, "v1");
    assert!(Arc::ptr_eq(copy_e2.value.as_ref().unwrap(), copy_v1));
    assert_eq!(copy.score, Some(SimpleScore::new(score)));
    // Extra ownership/null controls are independent of the source assertions.
    assert!(!Arc::ptr_eq(&original.values[0], copy_v1));
    assert!(copy.entities[0].value.is_none() && copy.entities[2].value.is_none());
}
pub fn independent_score(s: &BasicSolutionDocument) -> SimpleScore {
    SimpleScore::new(s.entities.iter().filter(|e| e.value.is_some()).count() as i64)
}
/// Actual prepared dense/stream directors live inside an owned manager job.
pub fn solve_owned(input: BasicSolutionDocument, stream: bool) -> BasicSolutionDocument {
    let mut config = SolverSequence::default();
    config.limits.best_score_limit = Some(SimpleScore::new(input.entities.len() as i64));
    for p in &mut config.phases {
        if let SolverPhase::Construction(c) = p {
            c.assert_scores = true
        }
        if let SolverPhase::LocalSearch(c) = p {
            c.max_steps = 40;
            c.max_evaluations = 80;
            c.move_order = MoveOrder::ShuffledVariables { seed: 37 };
            c.assert_scores = true
        }
    }
    let factory = SolverFactory::new(
        config,
        move |mut input: BasicSolutionDocument, config, progress| {
            let model = Rc::new(input.problem()?);
            let result = if stream {
                let recipe = ScoringFactoryRecipe::stream_recipe(
                    |_| Ok::<_, StreamScoringError>(()),
                    |_, _, assignment| {
                        let mut b = StreamGraphBuilder::new();
                        let source = b.source::<usize, (usize, Option<String>)>();
                        b.score("assigned entities", source.port(), |(_, v)| {
                            SimpleScore::new(i64::from(v.is_some()))
                        })?;
                        StreamScoreDirector::new(&model, b.build(), source, assignment)
                    },
                    |d: &StreamScoreDirector<'_, '_, String, SimpleScore>| {
                        Ok(d.assignment().clone())
                    },
                );
                let f = ScoringFactoryDeclaration::default()
                    .with_scoring_recipe(recipe)
                    .prepare_score_factory(
                        model.clone(),
                        Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                        EnvironmentProfile::new(Some(EnvironmentMode::FullAssert), vec![]),
                    )
                    .map_err(|e| e.to_string())?;
                let mut d = f
                    .score_factory_builder(model.initial_assignment())
                    .instantiate_score_director()
                    .map_err(|e| e.to_string())?;
                d.factory_resource_mut()
                    .run_solver(config)
                    .map_err(|e| e.to_string())?
            } else {
                let mut network =
                    ConstraintNetwork::<String, SimpleScore>::new(model.variables().len());
                network
                    .add_constraint(
                        "assigned entities",
                        (0..model.variables().len()).collect(),
                        |v| SimpleScore::new(v.iter().filter(|v| v.is_some()).count() as i64),
                    )
                    .map_err(|e| e.to_string())?;
                let recipe = ScoringFactoryRecipe::incremental_recipe(
                    |_| Ok::<_, ScoringError>(&network),
                    |n: &&ConstraintNetwork<'_, String, SimpleScore>, _, assignment| {
                        ScoreDirector::new(&model, n, assignment)
                    },
                    |d: &ScoreDirector<'_, '_, String, SimpleScore>| Ok(d.assignment().clone()),
                );
                let f = ScoringFactoryDeclaration::default()
                    .with_scoring_recipe(recipe)
                    .prepare_score_factory(
                        model.clone(),
                        Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                        EnvironmentProfile::new(Some(EnvironmentMode::FullAssert), vec![]),
                    )
                    .map_err(|e| e.to_string())?;
                let mut d = f
                    .score_factory_builder(model.initial_assignment())
                    .instantiate_score_director()
                    .map_err(|e| e.to_string())?;
                d.factory_resource_mut()
                    .run_solver(config)
                    .map_err(|e| e.to_string())?
            };
            assert!(result.complete);
            assert!(result.work.evaluations > 0 && result.work.evaluations <= 80);
            input.commit(&result.assignment, result.score.unwrap())?;
            assert_eq!(input.score, Some(independent_score(&input)));
            progress.record_work(result.work);
            Ok::<_, String>(input)
        },
    )
    .unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let job = manager.job_builder(1).with_problem(input).run().unwrap();
    let result = job.wait();
    drop(job);
    manager.shutdown().unwrap();
    drop(manager);
    drop(factory);
    match Arc::try_unwrap(result).ok().unwrap() {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    }
}
