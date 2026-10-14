#[path = "fixtures/configured_factories_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc, time::Duration};
use timetuck::*;

fn owned_source(xml: &str) -> NativeSolverConfigurationSource {
    let mut source = source(false);
    source
        .register_resource("native", SolverXmlConfiguration::parse(xml).unwrap())
        .unwrap();
    source.default_resource(Some("native".into()));
    source
}

#[test]
fn owned_resources_create_independent_dense_stream_jobs_and_effective_overrides() {
    let xml = format!(
        "<solver><solutionClass>{BASIC}</solutionClass><entityClass>{ENTITY}</entityClass><termination><secondsSpentLimit>8</secondsSpentLimit></termination><constructionHeuristic/><localSearch><termination><stepCountLimit>4</stepCountLimit></termination></localSearch></solver>"
    );
    let source = owned_source(&xml);
    let mut registry = NamedModelFactoryRegistry::default();
    registry
        .register(
            BASIC,
            base::schema(),
            vec![base::scoring("dense", false), base::scoring("stream", true)],
        )
        .unwrap();
    let requests: Vec<_> = ["dense", "stream"].into_iter().map(|key| {
        let mut config = base::configuration(None, false);
        config.scoring = Some(key.into());
        let request = source.resolve(config, &properties(&[("native.termination.spent-limit", "6s"), ("timefold.solver-manager.parallel-solver-count", "2")]), "native", &mut |_| {}).unwrap();
        assert_eq!(request.manager_workers, 2);
        assert_eq!(request.configuration.entity_types, Some(vec![PlanningType::of::<base::Entity>()]));
        assert!(matches!(&request.configuration.sequence.phases[1], SolverPhase::LocalSearch(c) if c.max_steps == 4));
        (key.to_owned(), request)
    }).collect();
    // Source XML/resources/properties need not survive publication or execution.
    drop(source);
    let bank = registry.prepare_configured(requests.clone()).unwrap();
    for (key, request) in requests {
        let factory = bank
            .factory::<base::BasicModel, base::ResultType, String, SimpleScore>(&key)
            .unwrap();
        assert_eq!(
            factory.configuration().limits.max_time,
            Some(Duration::from_secs(6))
        );
        let mut manager = request.build_manager(factory).unwrap();
        let first = manager
            .job_builder(1)
            .with_problem(base::BasicModel(
                FiniteProblem::new(vec![
                    Variable::new(vec!["a".into(), "b".into(), "c".into()]);
                    3
                ])
                .unwrap(),
            ))
            .with_time_override(SolverTimeOverride {
                max_time: Some(Duration::from_secs(2)),
                ..Default::default()
            })
            .run()
            .unwrap();
        let second = manager
            .job_builder(2)
            .with_problem(base::BasicModel(
                FiniteProblem::new(vec![Variable::new(vec!["one".into()]); 2]).unwrap(),
            ))
            .run()
            .unwrap();
        assert_eq!(
            first
                .configuration()
                .limits
                .time_gradient(Duration::from_secs(1), Duration::ZERO),
            Some(0.5)
        );
        assert_eq!(
            second
                .configuration()
                .limits
                .time_gradient(Duration::from_secs(1), Duration::ZERO),
            Some(1.0 / 6.0)
        );
        let first = first.wait();
        let first = first.as_ref().as_ref().unwrap();
        assert!(first.complete);
        assert_eq!(first.assignment.values().len(), 3);
        assert_eq!(
            first.score,
            Some(base::ordered_pairs(first.assignment.values()))
        );
        assert_eq!(first.score, Some(SimpleScore::ZERO));
        let second = second.wait();
        let second = second.as_ref().as_ref().unwrap();
        assert!(second.complete);
        assert_eq!(second.assignment.values().len(), 2);
        assert_eq!(
            second.score,
            Some(base::ordered_pairs(second.assignment.values()))
        );
        assert_eq!(second.score, Some(SimpleScore::new(-2)));
        assert_eq!(
            factory.configuration().limits.max_time,
            Some(Duration::from_secs(6))
        );
        manager.shutdown().unwrap();
    }
}

#[test]
fn typed_entity_mismatch_preflights_the_entire_set_before_any_builder() {
    struct OtherEntity;
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let mut registry = NamedModelFactoryRegistry::default();
    registry
        .register(
            BASIC,
            base::schema(),
            vec![NamedModelScoring::new(PROVIDER, move |config, profile| {
                count.set(count.get() + 1);
                base::build_factory(config, profile, false)
            })],
        )
        .unwrap();
    let mut source = owned_source(&format!(
        "<solver><solutionClass>{BASIC}</solutionClass><entityClass>other</entityClass></solver>"
    ));
    source.register_entity::<OtherEntity>("other").unwrap();
    let invalid = source
        .resolve(
            base::configuration(None, false),
            &properties(&[]),
            "native",
            &mut |_| {},
        )
        .unwrap();
    let valid = request(false, &[]);
    assert!(
        matches!(registry.prepare_configured([("a-valid".into(), valid), ("z-invalid".into(), invalid)]), Err(NamedModelFactoryError::EntityTypes { model, registered, declared }) if model == BASIC && registered == vec![PlanningType::of::<base::Entity>()] && declared == vec![PlanningType::of::<OtherEntity>()])
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn invalid_selected_resources_and_properties_leave_owned_template_untouched() {
    let base = base::configuration(Some(Duration::from_secs(17)), true);
    let before = base.sequence.clone();
    for (xml, values) in [
        (
            "<solver><entityClass>unknown</entityClass></solver>",
            vec![],
        ),
        (
            "<solver><termination><bestScoreLimit>bad-score</bestScoreLimit></termination></solver>",
            vec![],
        ),
        ("<solver><localSearch/></solver>", vec![]),
        (
            "<solver/>",
            vec![("native.termination.spent-limit", "bad-duration")],
        ),
        (
            "<solver/>",
            vec![("native.termination.best-score-limit", "bad-score")],
        ),
        (
            "<solver/>",
            vec![(
                "native.termination.diminished-returns.minimum-improvement-ratio",
                "NaN",
            )],
        ),
        (
            "<solver/>",
            vec![("timefold.solver-manager.parallel-solver-count", "0")],
        ),
        (
            "<solver/>",
            vec![("timefold.solver-manager.parallel-solver-count", "129")],
        ),
        (
            "<solver/>",
            vec![("native.solver-config-xml", "missing-resource")],
        ),
    ] {
        assert!(
            owned_source(xml)
                .resolve(base.clone(), &properties(&values), "native", &mut |_| {})
                .is_err()
        );
        assert_eq!(base.sequence, before);
        assert_eq!(base.model, None);
        assert_eq!(base.scoring, None);
        assert_eq!(base.entity_types, None);
    }
    let source = source(true);
    assert!(
        source
            .resolve(
                base.clone(),
                &properties(&[("timefold.solver.solver-config-xml", "")]),
                "timefold.solver",
                &mut |_| {}
            )
            .is_err()
    );
    assert_eq!(base.sequence, before);
    for (kind, phase) in [
        (
            "exhaustiveSearch",
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ),
        (
            "customPhase",
            SolverPhase::Custom(CustomPhaseConfig::default()),
        ),
    ] {
        let mut template = base.clone();
        template.sequence.phases = vec![phase];
        for control in [
            "<bestScoreLimit>0</bestScoreLimit>",
            "<stepCountLimit>5</stepCountLimit>",
            "<unimprovedStepCountLimit>6</unimprovedStepCountLimit>",
        ] {
            let xml =
                format!("<solver><{kind}><termination>{control}</termination></{kind}></solver>");
            assert!(
                owned_source(&xml)
                    .resolve(template.clone(), &properties(&[]), "native", &mut |_| {})
                    .is_err()
            );
        }
        let xml = format!(
            "<solver><{kind}><termination><secondsSpentLimit>3</secondsSpentLimit></termination></{kind}></solver>"
        );
        let request = owned_source(&xml)
            .resolve(template, &properties(&[]), "native", &mut |_| {})
            .unwrap();
        match &request.configuration.sequence.phases[0] {
            SolverPhase::Exact(config) => assert_eq!(config.max_time, Some(Duration::from_secs(3))),
            SolverPhase::Custom(config) => {
                assert_eq!(config.max_time, Some(Duration::from_secs(3)))
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn explicit_phase_policies_and_static_termination_survive_owned_binding() {
    static LIMIT: SearchTermination<'static> = SearchTermination::Condition(SearchLimit::Steps(12));
    let xml = "<solver><termination><secondsSpentLimit>7</secondsSpentLimit><unimprovedStepCountLimit>9</unimprovedStepCountLimit><diminishedReturns><slidingWindowSeconds>4</slidingWindowSeconds></diminishedReturns></termination><constructionHeuristic><termination><stepCountLimit>1</stepCountLimit></termination></constructionHeuristic><localSearch><termination><secondsSpentLimit>3</secondsSpentLimit><diminishedReturns><slidingWindowSeconds>2</slidingWindowSeconds></diminishedReturns></termination></localSearch></solver>";
    let mut base = base::configuration(None, false);
    base.sequence.limits.termination = Some(&LIMIT);
    let policy = DiminishedReturnsConfig::new(Duration::from_secs(11), 1, 2).unwrap();
    let SolverPhase::LocalSearch(local) = &mut base.sequence.phases[1] else {
        unreachable!()
    };
    local.diminished_returns = Some(policy);
    let request = owned_source(xml)
        .resolve(
            base,
            &properties(&[(
                "native.termination.diminished-returns.sliding-window-duration",
                "5s",
            )]),
            "native",
            &mut |_| {},
        )
        .unwrap();
    assert!(std::ptr::eq(
        request.configuration.sequence.limits.termination.unwrap(),
        &LIMIT
    ));
    assert_eq!(
        request.configuration.sequence.limits.max_time,
        Some(Duration::from_secs(7))
    );
    assert_eq!(
        request.configuration.sequence.limits.max_unimproved_steps,
        Some(9)
    );
    assert!(
        matches!(&request.configuration.sequence.phases[0], SolverPhase::Construction(c) if c.max_steps == Some(1))
    );
    assert!(
        matches!(&request.configuration.sequence.phases[1], SolverPhase::LocalSearch(c) if c.max_time == Some(Duration::from_secs(3)) && c.diminished_returns == Some(policy))
    );
    let request = owned_source(xml)
        .resolve(
            base::configuration(None, false),
            &properties(&[]),
            "native",
            &mut |_| {},
        )
        .unwrap();
    assert!(
        matches!(&request.configuration.sequence.phases[1], SolverPhase::LocalSearch(c) if c.diminished_returns.unwrap().window() == Duration::from_secs(2))
    );
}

#[test]
fn borrowed_solve_wide_step_expression_is_rejected_by_owned_factory_profile() {
    let xml = "<solver><termination><stepCountLimit>5</stepCountLimit></termination></solver>";
    let source = owned_source(xml);
    assert!(
        source
            .resolve(
                base::configuration(None, false),
                &properties(&[]),
                "native",
                &mut |_| {}
            )
            .unwrap_err()
            .to_string()
            .contains("with_sequence")
    );
    let plan = SolverXmlConfiguration::parse(xml)
        .unwrap()
        .native_termination_plan()
        .unwrap();
    let called = Cell::new(false);
    plan.with_sequence(base::sequence(None, false), |sequence| {
        called.set(true);
        assert!(sequence.limits.termination.is_some());
    })
    .unwrap();
    assert!(called.get());
}

#[test]
fn specific_resources_win_before_global_and_default_with_live_legacy_diagnostics() {
    let mut source = source(true);
    source.property_rules(
        PropertyFallbackRules::new().prefix("timefold.solver", vec!["optaplanner.solver".into()]),
    );
    let mut warnings = vec![];
    let request = source
        .resolve(
            base::configuration(None, false),
            &properties(&[
                ("timefold.solver-config-xml", NO_GLOBAL),
                ("optaplanner.solver.solver-config-xml", CUSTOM),
            ]),
            "timefold.solver",
            &mut |warning| warnings.push(warning),
        )
        .unwrap();
    assert_eq!(request.selected_resource.as_deref(), Some(CUSTOM));
    assert_eq!(
        request.configuration.sequence.limits.max_time,
        Some(Duration::from_secs(180))
    );
    assert_eq!(warnings.len(), 1);
    let request = source
        .resolve(
            base::configuration(None, false),
            &properties(&[("timefold.solver-config-xml", NO_GLOBAL)]),
            "timefold.solver",
            &mut |_| {},
        )
        .unwrap();
    assert_eq!(request.selected_resource.as_deref(), Some(NO_GLOBAL));
    assert_eq!(request.configuration.sequence.limits.max_time, None);
}
