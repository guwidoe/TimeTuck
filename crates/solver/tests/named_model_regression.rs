#[path = "fixtures/named_models_fixture.rs"]
mod fixture;
use fixture::*;
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use timetuck::*;
struct ListEntityType;
struct ListModel(ListProblem<i32>);
type ListResult = SolverResult<ListAssignment<i32>, SimpleScore>;
fn list_schema() -> Rc<PlanningSchema> {
    Rc::new(
        PlanningSchema::new::<ListModel>(vec![PlanningEntityDefinition::of::<ListEntityType>(
            vec![PlanningVariableDefinition::list::<i32>("route", false)],
        )])
        .unwrap(),
    )
}
fn list_recipe() -> NamedModelScoring {
    NamedModelScoring::new("weighted", |config, profile| {
        let mode = profile.global_mode();
        SolverFactory::new(config, move |input: ListModel, config, progress| {
            let model = Rc::new(input.0);
            let mut n = ListConstraintNetwork::new(2, 4);
            for e in 0..2 {
                n.add_constraint(
                    format!("route{e}"),
                    vec![ListScope::Entity(e)],
                    move |facts| {
                        let ListFact::Entity(row) = facts[0] else {
                            unreachable!()
                        };
                        SimpleScore::new(
                            row.iter()
                                .enumerate()
                                .map(|(i, &v)| (e as i64 + 1) * (i as i64 + 1) * i64::from(v))
                                .sum(),
                        )
                    },
                )
                .unwrap();
            }
            let recipe = ScoringFactoryRecipe::incremental_recipe(
                |_| Ok::<_, ListScoringError>(&n),
                |n: &&ListConstraintNetwork<'_, i32, SimpleScore>, _, a| {
                    ListScoreDirector::new(&model, n, a)
                },
                |d: &ListScoreDirector<'_, '_, i32, SimpleScore>| Ok(d.assignment().clone()),
            );
            let f = ScoringFactoryDeclaration::default()
                .with_scoring_recipe(recipe)
                .prepare_score_factory(
                    model.clone(),
                    Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                    EnvironmentProfile::new(Some(mode), vec![]),
                )
                .map_err(|e| e.to_string())?;
            let mut d = f
                .score_factory_builder(model.initial_assignment())
                .instantiate_score_director()
                .map_err(|e| e.to_string())?;
            let result = d
                .factory_resource_mut()
                .run_solver(config)
                .map_err(|e| e.to_string())?;
            progress.record_work(result.work);
            Ok::<_, String>(result)
        })
        .map_err(|e| e.to_string())
    })
}
fn counted(key: &str, calls: &Rc<Cell<usize>>) -> NamedModelScoring {
    let calls = calls.clone();
    NamedModelScoring::new(key, move |c, p| {
        calls.set(calls.get() + 1);
        build_factory(c, p, false)
    })
}
fn request(model: Option<&str>, score: Option<&str>) -> NamedSolverConfiguration {
    NamedSolverConfiguration {
        model: model.map(str::to_owned),
        scoring: score.map(str::to_owned),
        ..configuration(None, true)
    }
}
#[test]
fn preflight_ambiguity_missing_unused_and_duplicates_never_call_builders() {
    let calls = Rc::new(Cell::new(0));
    let mut r = NamedModelFactoryRegistry::default();
    r.register(
        "basic",
        schema(),
        vec![counted("dense", &calls), counted("stream", &calls)],
    )
    .unwrap();
    let err = r
        .prepare([
            ("one".into(), request(None, None)),
            ("two".into(), request(None, None)),
        ])
        .err()
        .unwrap();
    assert_eq!(
        err,
        NamedModelFactoryError::AmbiguousScoring {
            model: "basic".into(),
            solvers: vec!["one".into(), "two".into()],
            scorings: vec!["dense".into(), "stream".into()]
        }
    );
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        r.prepare([("one".into(), request(None, Some("dense")))]),
        Err(NamedModelFactoryError::UnusedScoring { .. })
    ));
    assert!(matches!(
        r.prepare([("one".into(), request(None, Some("missing")))]),
        Err(NamedModelFactoryError::MissingScoring { .. })
    ));
    assert!(matches!(
        r.prepare([("one".into(), request(Some("missing"), None))]),
        Err(NamedModelFactoryError::MissingModel { .. })
    ));
    assert!(matches!(
        r.prepare([
            ("one".into(), request(None, None)),
            ("one".into(), request(None, None))
        ]),
        Err(NamedModelFactoryError::DuplicateSolver(_))
    ));
    r.register("list", list_schema(), vec![list_recipe()])
        .unwrap();
    let err = r
        .prepare([
            ("one".into(), request(None, None)),
            ("two".into(), request(None, None)),
        ])
        .err()
        .unwrap();
    assert_eq!(
        err,
        NamedModelFactoryError::AmbiguousModels {
            solvers: vec!["one".into(), "two".into()],
            models: vec!["basic".into(), "list".into()]
        }
    );
    assert!(matches!(
        r.prepare([("one".into(), request(Some("basic"), Some("dense")))]),
        Err(NamedModelFactoryError::UnusedModels(_))
    ));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        r.register("basic", schema(), vec![counted("new", &calls)]),
        Err(NamedModelFactoryError::DuplicateModel(_))
    ));
    let mut bad = NamedModelFactoryRegistry::default();
    assert!(matches!(
        bad.register("wrong", list_schema(), vec![counted("dense", &calls)]),
        Err(NamedModelFactoryError::ModelType { .. })
    ));
    assert!(matches!(
        bad.prepare([("one".into(), request(None, None))]),
        Err(NamedModelFactoryError::MissingModels)
    ));
    let empty = Rc::new(PlanningSchema::new::<BasicModel>(vec![]).unwrap());
    bad.register("empty", empty, vec![counted("dense", &calls)])
        .unwrap();
    assert!(matches!(
        bad.prepare([("one".into(), request(None, None))]),
        Err(NamedModelFactoryError::MissingEntities(_))
    ));
    assert_eq!(calls.get(), 0);
}
#[test]
fn preflight_configuration_and_failed_factory_build_publish_no_partial_bank() {
    let calls = Rc::new(Cell::new(0));
    let mut r = NamedModelFactoryRegistry::default();
    r.register("basic", schema(), vec![counted("dense", &calls)])
        .unwrap();
    let mut bad = request(None, None);
    bad.sequence.phases.clear();
    assert!(matches!(
        r.prepare([("a".into(), request(None, None)), ("z".into(), bad)]),
        Err(NamedModelFactoryError::Configuration { .. })
    ));
    assert_eq!(calls.get(), 0);
    let c = calls.clone();
    let failing = NamedModelScoring::new::<BasicModel, ResultType, String>(
        "bad",
        move |mut config, profile| {
            c.set(c.get() + 1);
            config.phases.clear();
            build_factory(config, profile, false)
        },
    );
    let mut r = NamedModelFactoryRegistry::default();
    r.register("basic", schema(), vec![counted("dense", &calls), failing])
        .unwrap();
    assert!(matches!(
        r.prepare([
            ("a".into(), request(None, Some("dense"))),
            ("z".into(), request(None, Some("bad")))
        ]),
        Err(NamedModelFactoryError::Build { .. })
    ));
    assert_eq!(calls.get(), 2);
}
#[test]
fn heterogeneous_models_and_concrete_dense_stream_choices_execute_independent_factories() {
    let mut r = NamedModelFactoryRegistry::default();
    r.register(
        "basic",
        schema(),
        vec![scoring("dense", false), scoring("stream", true)],
    )
    .unwrap();
    r.register("list", list_schema(), vec![list_recipe()])
        .unwrap();
    let mut dense = request(Some("basic"), Some("dense"));
    dense.environment = EnvironmentProfile::new(Some(EnvironmentMode::FullAssert), vec![]);
    let stream = request(Some("basic"), Some("stream"));
    let list = NamedSolverConfiguration {
        model: Some("list".into()),
        scoring: Some("weighted".into()),
        sequence: SolverSequence {
            phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                max_evaluations: 1000,
                ..Default::default()
            })],
            ..Default::default()
        },
        ..Default::default()
    };
    let bank = r
        .prepare([
            ("dense".into(), dense),
            ("stream".into(), stream),
            ("list".into(), list),
        ])
        .unwrap();
    assert_eq!(
        bank.names().collect::<Vec<_>>(),
        vec!["dense", "list", "stream"]
    );
    assert!(matches!(
        bank.unique_factory::<BasicModel, ResultType, String, SimpleScore>(),
        Err(NamedModelFactoryError::AmbiguousSolver(_))
    ));
    assert!(matches!(
        bank.factory::<ListModel, ListResult, String, SimpleScore>("dense"),
        Err(NamedModelFactoryError::FactoryType(_))
    ));
    for name in ["dense", "stream"] {
        let f = bank
            .factory::<BasicModel, ResultType, String, SimpleScore>(name)
            .unwrap();
        if name == "dense" {
            assert!(
                matches!(&f.configuration().phases[1],SolverPhase::LocalSearch(c) if c.assert_scores)
            );
        }
        let mut manager = f.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(problem(bank.selection(name).unwrap().2))
            .run()
            .unwrap();
        positive(job.wait().as_ref().as_ref().unwrap());
        manager.shutdown().unwrap();
    }
    let f = bank
        .factory::<ListModel, ListResult, String, SimpleScore>("list")
        .unwrap();
    let mut manager = f.build_manager(1).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(ListModel(
            ListProblem::new(
                vec![1, 2, 3, 4],
                vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
            )
            .unwrap(),
        ))
        .run()
        .unwrap();
    let owned = job.wait();
    let answer = owned.as_ref().as_ref().unwrap();
    let score: i64 = answer
        .assignment
        .lists()
        .iter()
        .enumerate()
        .map(|(e, row)| {
            row.iter()
                .enumerate()
                .map(|(i, &v)| (e as i64 + 1) * (i as i64 + 1) * i64::from(v))
                .sum::<i64>()
        })
        .sum();
    assert_eq!(answer.score, Some(SimpleScore::new(score)));
    assert!(answer.is_optimal());
    manager.shutdown().unwrap();
}
#[test]
fn job_cancellation_and_sparse_limits_do_not_cross_named_factory_boundaries() {
    let (entered, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = Arc::new(Mutex::new(released));
    let mut r = NamedModelFactoryRegistry::default();
    let a = entered.clone();
    let b = released.clone();
    let score = NamedModelScoring::new("blocked", move |c, _| {
        let a = a.clone();
        let b = b.clone();
        SolverFactory::new(c, move |input: BasicModel, c, progress| {
            a.send(()).map_err(|e| e.to_string())?;
            b.lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(10))
                .map_err(|e| e.to_string())?;
            let mut n = ConstraintNetwork::<_, SimpleScore>::new(2);
            n.add_constraint("pairs", vec![0, 1], |v| {
                ordered_pairs(&v.iter().map(|x| x.map(Clone::clone)).collect::<Vec<_>>())
            })
            .unwrap();
            let result = solve(&input.0, &n, c).map_err(|e| e.to_string())?;
            progress.record_work(result.work);
            Ok::<_, String>(result)
        })
        .map_err(|e| e.to_string())
    });
    r.register("basic", schema(), vec![score, scoring("normal", true)])
        .unwrap();
    let bank = r
        .prepare([
            ("blocked".into(), request(None, Some("blocked"))),
            ("normal".into(), request(None, Some("normal"))),
        ])
        .unwrap();
    let mut blocked = bank
        .factory::<BasicModel, ResultType, String, SimpleScore>("blocked")
        .unwrap()
        .build_manager(1)
        .unwrap();
    let mut normal = bank
        .factory::<BasicModel, ResultType, String, SimpleScore>("normal")
        .unwrap()
        .build_manager(1)
        .unwrap();
    let job = blocked
        .job_builder(1)
        .with_problem(problem(bank.selection("blocked").unwrap().2))
        .run()
        .unwrap();
    observed.recv_timeout(Duration::from_secs(10)).unwrap();
    job.cancel();
    let independent = normal
        .job_builder(1)
        .with_problem(problem(bank.selection("normal").unwrap().2))
        .with_time_override(SolverTimeOverride {
            max_time: Some(Duration::from_secs(2)),
            ..Default::default()
        })
        .run()
        .unwrap();
    positive(independent.wait().as_ref().as_ref().unwrap());
    assert_eq!(
        independent.configuration().limits.max_time,
        Some(Duration::from_secs(2))
    );
    assert_eq!(job.configuration().limits.max_time, None);
    release.send(()).unwrap();
    let cancelled = job.wait();
    let answer = cancelled.as_ref().as_ref().unwrap();
    assert_eq!(answer.termination, SolverTermination::Cancelled);
    blocked.shutdown().unwrap();
    normal.shutdown().unwrap();
}
#[test]
fn standard_xsi_hints_preserve_executable_settings_without_external_resolution() {
    let xml = SolverXmlConfiguration::parse("<solver xmlns='https://timefold.ai/xsd/solver' xmlns:hint='http://www.w3.org/2001/XMLSchema-instance' hint:schemaLocation='urn:native file:///nonexistent-schema.xsd' hint:noNamespaceSchemaLocation='http://127.0.0.1:1/not-read'><termination><secondsSpentLimit>7</secondsSpentLimit></termination><constructionHeuristic/><localSearch><termination><secondsSpentLimit>3</secondsSpentLimit></termination></localSearch></solver>").unwrap();
    assert!(xml.root().attributes.is_empty());
    assert_eq!(
        xml.root()
            .child("termination")
            .unwrap()
            .value("secondsSpentLimit"),
        Some("7")
    );
    let mut c = configuration(
        Some(Duration::from_secs(
            xml.root()
                .child("termination")
                .unwrap()
                .value("secondsSpentLimit")
                .unwrap()
                .parse()
                .unwrap(),
        )),
        false,
    );
    let seconds = xml
        .root()
        .child("localSearch")
        .unwrap()
        .child("termination")
        .unwrap()
        .value("secondsSpentLimit")
        .unwrap()
        .parse()
        .unwrap();
    let SolverPhase::LocalSearch(local) = &mut c.sequence.phases[1] else {
        unreachable!()
    };
    local.max_time = Some(Duration::from_secs(seconds));
    let bank = registry().prepare([("one".into(), c)]).unwrap();
    let f = bank
        .unique_factory::<BasicModel, ResultType, String, SimpleScore>()
        .unwrap();
    assert_eq!(
        f.configuration().limits.max_time,
        Some(Duration::from_secs(7))
    );
    assert!(
        matches!(&f.configuration().phases[1],SolverPhase::LocalSearch(c) if c.max_time==Some(Duration::from_secs(3)))
    );
    let mut manager = f.build_manager(1).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(problem(bank.selection("one").unwrap().2))
        .run()
        .unwrap();
    positive(job.wait().as_ref().as_ref().unwrap());
    manager.shutdown().unwrap();
    let reread = SolverXmlConfiguration::parse(&xml.to_xml()).unwrap();
    assert_eq!(reread.root(), xml.root());
}
#[test]
fn external_hints_do_not_relax_unknown_attributes_or_namespaces() {
    for input in [
        "<solver schemaLocation='x'/>",
        "<solver noNamespaceSchemaLocation='x'/>",
        "<solver xmlns:x='foreign' x:schemaLocation='x'/>",
        "<solver xmlns:x='foreign' x:noNamespaceSchemaLocation='x'/>",
        "<solver xmlns:x='http://www.w3.org/2001/XMLSchema-instance' x:unknown='x'/>",
        "<solver xmlns='foreign' xmlns:x='http://www.w3.org/2001/XMLSchema-instance' x:schemaLocation='x'/>",
        "<solver xmlns:x='http://www.w3.org/2001/XMLSchema-instance' x:schemaLocation='x' unknown='x'/>",
    ] {
        assert!(SolverXmlConfiguration::parse(input).is_err(), "{input}");
    }
}
