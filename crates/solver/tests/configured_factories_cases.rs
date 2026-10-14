// Complete pinned Community operational TEST adaptations; Apache-2.0, NOTICE.
#[path = "fixtures/configured_factories_fixture.rs"]
mod fixture;
use fixture::*;
use std::{cell::Cell, rc::Rc, time::Duration};
use timetuck::*;
#[test]
fn single_solver_config_xml_none() {
    let r = request(false, &[]);
    assert!(r.xml_termination.is_none());
    assert!(r.selected_resource.is_none());
    assert_eq!(r.configuration.sequence.limits.max_time, None);
    execute(r);
}
#[test]
fn single_solve() {
    let r = request(
        false,
        &[("timefold.solver.termination.best-score-limit", "0")],
    );
    assert_eq!(
        r.configuration.sequence.limits.best_score_limit,
        Some(SimpleScore::ZERO)
    );
    execute(r);
}
#[test]
fn single_solve_with_parallel_solver_count() {
    let r = request(
        false,
        &[("timefold.solver-manager.parallel-solver-count", "2")],
    );
    assert_eq!(r.manager_workers, 2);
    execute(r);
}
#[test]
fn single_solve_with_time_override() {
    let r = request(
        false,
        &[
            ("timefold.solver.termination.best-score-limit", "0"),
            ("timefold.solver.termination.spent-limit", "30s"),
        ],
    );
    let bank = registry()
        .prepare_configured([("solver".into(), r.clone())])
        .unwrap();
    metadata(&bank);
    let f = bank
        .unique_factory::<base::BasicModel, base::ResultType, String, SimpleScore>()
        .unwrap();
    assert_eq!(
        f.configuration().limits.max_time,
        Some(Duration::from_secs(30))
    );
    let mut manager = r.build_manager(f).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(base::problem(bank.selection("solver").unwrap().2))
        .with_time_override(SolverTimeOverride {
            max_time: Some(Duration::from_secs(2)),
            ..Default::default()
        })
        .run()
        .unwrap();
    assert_eq!(base::gradient(&job, Duration::from_secs(1)), 0.5);
    base::positive(job.wait().as_ref().as_ref().unwrap());
    assert_eq!(
        f.configuration().limits.max_time,
        Some(Duration::from_secs(30))
    );
    manager.shutdown().unwrap();
}
#[test]
fn xml_solver_config_xml_default() {
    let r = request(true, &[]);
    assert_eq!(r.selected_resource.as_deref(), Some(DEFAULT));
    assert_eq!(
        r.xml_termination
            .as_ref()
            .unwrap()
            .solver
            .as_ref()
            .unwrap()
            .spent
            .units
            .seconds,
        Some(2)
    );
    assert_eq!(
        r.configuration.sequence.limits.max_time,
        Some(Duration::from_secs(2))
    );
    execute(r);
}
#[test]
fn xml_solver_config_xml_property() {
    let r = request(true, &[("timefold.solver-config-xml", CUSTOM)]);
    assert_eq!(r.selected_resource.as_deref(), Some(CUSTOM));
    assert_eq!(
        r.xml_termination
            .as_ref()
            .unwrap()
            .solver
            .as_ref()
            .unwrap()
            .spent
            .units
            .minutes,
        Some(3)
    );
    assert_eq!(
        r.configuration.sequence.limits.max_time,
        Some(Duration::from_secs(180))
    );
    execute(r);
}
#[test]
fn xml_solver_config_xml_solver_property_precedence() {
    let r = request(
        true,
        &[
            ("timefold.solver-config-xml", NO_GLOBAL),
            ("timefold.solver.solver-config-xml", CUSTOM),
        ],
    );
    assert_eq!(r.selected_resource.as_deref(), Some(CUSTOM));
    assert_eq!(
        r.xml_termination
            .as_ref()
            .unwrap()
            .solver
            .as_ref()
            .unwrap()
            .spent
            .units
            .minutes,
        Some(3)
    );
    assert_eq!(
        r.configuration.sequence.limits.max_time,
        Some(Duration::from_secs(180))
    );
    execute(r);
}
#[test]
fn xml_solver_config_xml_property_no_global_termination() {
    let r = request(true, &[("timefold.solver-config-xml", NO_GLOBAL)]);
    assert!(r.xml_termination.as_ref().unwrap().solver.is_none());
    assert_eq!(r.configuration.sequence.limits.max_time, None);
    assert!(
        matches!(&r.configuration.sequence.phases[1],SolverPhase::LocalSearch(c) if c.max_time==Some(Duration::from_secs(3)))
    );
    execute(r);
}
#[test]
fn xml_termination_properties() {
    let r = request(true, &[("timefold.solver.termination.spent-limit", "4h")]);
    assert_eq!(
        r.configuration.sequence.limits.max_time,
        Some(Duration::from_secs(4 * 3600))
    );
    execute(r);
    let r = request(
        true,
        &[("timefold.solver.termination.unimproved-spent-limit", "5h")],
    );
    assert_eq!(
        r.configuration.sequence.limits.max_unimproved_time,
        Some(Duration::from_secs(5 * 3600))
    );
    execute(r);
    let r = request(
        true,
        &[("timefold.solver.termination.best-score-limit", "6")],
    );
    assert_eq!(
        r.configuration
            .sequence
            .limits
            .best_score_limit
            .unwrap()
            .to_string(),
        SimpleScore::new(6).to_string()
    );
    execute(r);
    let r = request(true, &[]);
    assert!(r.xml_termination.as_ref().unwrap().phases.is_empty());
    execute(r);
    let r = request(
        true,
        &[(
            "timefold.solver.termination.diminished-returns.enabled",
            "true",
        )],
    );
    let raw = r
        .runtime_termination
        .termination
        .diminished_returns
        .unwrap();
    assert_eq!(raw.sliding_window_duration, None);
    assert_eq!(raw.minimum_improvement_ratio, None);
    assert!(r.runtime_termination.resolved.is_some());
    execute(r);
    let r = request(
        true,
        &[(
            "timefold.solver.termination.diminished-returns.sliding-window-duration",
            "5m",
        )],
    );
    let raw = r
        .runtime_termination
        .termination
        .diminished_returns
        .unwrap();
    assert_eq!(raw.sliding_window_duration, Some(Duration::from_secs(300)));
    assert_eq!(raw.minimum_improvement_ratio, None);
    assert_eq!(
        r.runtime_termination.resolved.unwrap().window(),
        Duration::from_secs(300)
    );
    execute(r);
    let r = request(
        true,
        &[(
            "timefold.solver.termination.diminished-returns.minimum-improvement-ratio",
            "2.5",
        )],
    );
    let raw = r
        .runtime_termination
        .termination
        .diminished_returns
        .unwrap();
    assert_eq!(raw.sliding_window_duration, None);
    assert_eq!(raw.minimum_improvement_ratio, Some(2.5));
    execute(r);
    let r = request(
        true,
        &[
            (
                "timefold.solver.termination.diminished-returns.enabled",
                "false",
            ),
            (
                "timefold.solver.termination.diminished-returns.minimum-improvement-ratio",
                "2.5",
            ),
        ],
    );
    assert!(
        r.runtime_termination
            .termination
            .diminished_returns
            .is_none()
    );
    assert!(r.runtime_termination.resolved.is_none());
    execute(r);
    let r = request(
        true,
        &[
            ("timefold.solver-config-xml", PHASES),
            (
                "timefold.solver.termination.diminished-returns.enabled",
                "false",
            ),
            (
                "timefold.solver.termination.diminished-returns.minimum-improvement-ratio",
                "2.5",
            ),
        ],
    );
    let xml = r.xml_termination.as_ref().unwrap();
    assert_eq!(xml.phases.len(), 2);
    assert_eq!(xml.phases[0].kind, "constructionHeuristic");
    assert_eq!(xml.phases[1].kind, "localSearch");
    assert!(xml.phases.iter().all(|p| p.settings.is_none()));
    assert!(
        matches!(&r.configuration.sequence.phases[0],SolverPhase::Construction(c) if c.max_time.is_none())
    );
    assert!(
        matches!(&r.configuration.sequence.phases[1],SolverPhase::LocalSearch(c) if c.max_time.is_none()&&c.diminished_returns.is_none())
    );
    execute(r);
}
fn multiple_xml_ambiguity(keys: [&str; 2], provider: bool) {
    let calls = Rc::new(Cell::new(0));
    let registry = validation::catalog(keys, &calls, provider);
    let context = source(false);
    let requests = (1..=2)
        .map(|i| {
            let name = format!("solver{i}");
            let prefix = format!("timefold.solver.{name}");
            let prop = format!("{prefix}.solver-config-xml");
            let r = context
                .resolve(
                    base::configuration(None, false),
                    &properties(&[(&prop, DEFAULT)]),
                    &prefix,
                    &mut |_| {},
                )
                .unwrap();
            assert_eq!(
                r.configuration.sequence.limits.max_time,
                Some(Duration::from_secs(2))
            );
            (name, r)
        })
        .collect::<Vec<_>>();
    let error = registry.prepare_configured(requests).err().unwrap();
    let mut expected: Vec<_> = keys.iter().map(|k| k.to_string()).collect();
    expected.sort();
    assert_eq!(
        error,
        NamedModelFactoryError::AmbiguousScoring {
            model: validation::BASIC.into(),
            solvers: vec!["solver1".into(), "solver2".into()],
            scorings: expected
        }
    );
    for key in keys {
        assert!(error.to_string().contains(key));
    }
    assert_eq!(calls.get(), 0);
}
#[test]
fn multiple_multiple_easy_score_constraints_xml_property() {
    multiple_xml_ambiguity(validation::EASY, false);
}
#[test]
fn multiple_multiple_constraint_provider_constraints_xml_property() {
    multiple_xml_ambiguity(validation::PROVIDER, true);
}
#[test]
fn multiple_multiple_incremental_score_constraints_xml_property() {
    multiple_xml_ambiguity(validation::INCREMENTAL, false);
}
