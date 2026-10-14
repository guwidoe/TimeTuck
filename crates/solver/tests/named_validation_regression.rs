#[path = "fixtures/named_validation_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc};
use timetuck::*;
#[test]
fn global_catalog_selects_real_typed_factories_and_independent_jobs() {
    let calls = Rc::new(Cell::new(0));
    let mut r = NamedModelFactoryRegistry::default();
    r.register(BASIC, base::schema(), vec![]).unwrap();
    r.register(OTHER, other_schema(), vec![]).unwrap();
    r.register_global_scoring(basic_recipe(PROVIDER[0], &calls))
        .unwrap();
    r.register_global_scoring(other_recipe(PROVIDER[1], &calls))
        .unwrap();
    let bank = r
        .prepare([
            (
                "first".into(),
                NamedSolverConfiguration {
                    model: Some(BASIC.into()),
                    scoring: Some(PROVIDER[0].into()),
                    ..base::configuration(None, true)
                },
            ),
            (
                "second".into(),
                NamedSolverConfiguration {
                    model: Some(OTHER.into()),
                    scoring: Some(PROVIDER[1].into()),
                    ..base::configuration(None, false)
                },
            ),
        ])
        .unwrap();
    assert_eq!(calls.get(), 2);
    let mut manager = bank
        .factory::<base::BasicModel, base::ResultType, String, SimpleScore>("first")
        .unwrap()
        .build_manager(2)
        .unwrap();
    for id in [1, 2] {
        let job = manager
            .job_builder(id)
            .with_problem(base::problem(bank.selection("first").unwrap().2))
            .run()
            .unwrap();
        base::positive(job.wait().as_ref().as_ref().unwrap());
    }
    manager.shutdown().unwrap();
    let mut manager = bank
        .factory::<OtherModel, base::ResultType, String, SimpleScore>("second")
        .unwrap()
        .build_manager(1)
        .unwrap();
    let model = OtherModel(
        FiniteProblem::new(vec![Variable::new(vec!["x".into(), "y".into()]); 3]).unwrap(),
    );
    let job = manager.job_builder(3).with_problem(model).run().unwrap();
    let outcome = job.wait();
    let outcome = outcome.as_ref().as_ref().unwrap();
    assert!(outcome.complete);
    assert_eq!(outcome.score, Some(SimpleScore::new(3)));
    assert_eq!(outcome.assignment.values().len(), 3);
    manager.shutdown().unwrap();
    assert!(matches!(
        bank.factory::<OtherModel, base::ResultType, String, SimpleScore>("first"),
        Err(NamedModelFactoryError::FactoryType(_))
    ));
}
#[test]
fn catalog_selected_type_missing_duplicate_and_xml_errors_precede_callbacks() {
    let calls = Rc::new(Cell::new(0));
    let mut r = NamedModelFactoryRegistry::default();
    r.register(BASIC, base::schema(), vec![]).unwrap();
    r.register_global_scoring(other_recipe("wrong-input", &calls))
        .unwrap();
    assert!(matches!(
        r.prepare(requests(1, None, None)),
        Err(NamedModelFactoryError::ModelType { .. })
    ));
    assert_eq!(calls.get(), 0);
    assert!(matches!(
        r.register_global_scoring(other_recipe("wrong-input", &calls)),
        Err(NamedModelFactoryError::DuplicateScoring { .. })
    ));
    assert!(matches!(
        r.prepare(requests(1, None, Some("not-registered"))),
        Err(NamedModelFactoryError::MissingScoring { .. })
    ));
    assert_eq!(calls.get(), 0);
    let xml=SolverXmlConfiguration::parse("<solver><solutionClass>changed</solutionClass><scoreDirectorFactory><easyScoreCalculatorClass>a</easyScoreCalculatorClass><constraintProviderClass>b</constraintProviderClass></scoreDirectorFactory></solver>").unwrap();
    let mut c = base::configuration(None, true);
    let before = c.sequence.clone();
    assert!(c.apply_xml_selection(&xml).is_err());
    assert_eq!(c.model, None);
    assert_eq!(c.scoring, None);
    assert_eq!(c.sequence, before);
}
#[test]
fn actual_registered_dummy_callback_fails_only_when_executed() {
    let calls = Rc::new(Cell::new(0));
    let mut r = NamedModelFactoryRegistry::default();
    r.register(BASIC, base::schema(), vec![]).unwrap();
    r.register_global_scoring(null_recipe(EASY[0], &calls))
        .unwrap();
    let bank = r.prepare(requests(1, None, None)).unwrap();
    assert_eq!(calls.get(), 1);
    let factory = bank
        .factory::<base::BasicModel, base::ResultType, String, SimpleScore>("solver1")
        .unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(base::problem(bank.selection("solver1").unwrap().2))
        .run()
        .unwrap();
    assert!(
        job.wait()
            .as_ref()
            .as_ref()
            .unwrap_err()
            .to_string()
            .contains("returned no score")
    );
    manager.shutdown().unwrap();
}
