// Complete native adaptations of operational Community multiple-solver TESTs.
#[path = "fixtures/named_models_fixture.rs"]
mod fixture;
use fixture::*;
use std::time::Duration;
use timetuck::*;
#[test]
fn solve_named_models() {
    let bank = registry()
        .prepare([
            ("solver1".into(), configuration(None, true)),
            ("solver2".into(), configuration(None, true)),
        ])
        .unwrap();
    for name in ["solver1", "solver2"] {
        let factory = bank
            .factory::<BasicModel, ResultType, String, SimpleScore>(name)
            .unwrap();
        assert_eq!(
            factory.configuration().limits.best_score_limit,
            Some(SimpleScore::ZERO)
        );
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(problem(bank.selection(name).unwrap().2))
            .run()
            .unwrap();
        positive(job.wait().as_ref().as_ref().unwrap());
        manager.shutdown().unwrap();
    }
}
#[test]
fn solve_with_time_override() {
    let bank = registry()
        .prepare([
            (
                "solver1".into(),
                configuration(Some(Duration::from_secs(30)), true),
            ),
            (
                "solver2".into(),
                configuration(Some(Duration::from_secs(30)), true),
            ),
        ])
        .unwrap();
    for name in ["solver1", "solver2"] {
        let factory = bank
            .factory::<BasicModel, ResultType, String, SimpleScore>(name)
            .unwrap();
        assert_eq!(
            factory.configuration().limits.max_time,
            Some(Duration::from_secs(30))
        );
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(problem(bank.selection(name).unwrap().2))
            .with_time_override(SolverTimeOverride {
                max_time: Some(Duration::from_secs(2)),
                ..Default::default()
            })
            .run()
            .unwrap();
        assert_eq!(
            job.configuration().limits.max_time,
            Some(Duration::from_secs(2))
        );
        assert_eq!(gradient(&job, Duration::from_secs(1)), 0.5);
        positive(job.wait().as_ref().as_ref().unwrap());
        assert_eq!(
            factory.configuration().limits.max_time,
            Some(Duration::from_secs(30))
        );
        manager.shutdown().unwrap();
    }
}
#[test]
fn solver_config_xml_none() {
    let bank = registry()
        .prepare([
            (
                "solver1".into(),
                configuration(Some(Duration::from_secs(10)), false),
            ),
            (
                "solver2".into(),
                configuration(Some(Duration::from_secs(20)), false),
            ),
        ])
        .unwrap();
    for (name, expected) in [("solver1", 0.5), ("solver2", 0.25)] {
        let factory = bank
            .factory::<BasicModel, ResultType, String, SimpleScore>(name)
            .unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(problem(bank.selection(name).unwrap().2))
            .run()
            .unwrap();
        assert_eq!(gradient(&job, Duration::from_secs(5)), expected);
        job.wait();
        manager.shutdown().unwrap();
    }
}
#[test]
fn solver_config_xml_property() {
    let bank = registry()
        .prepare([
            (
                "solver1".into(),
                xml_configuration(
                    "ai/timefold/solver/spring/boot/autoconfigure/customSolver1Config.xml",
                ),
            ),
            (
                "solver2".into(),
                xml_configuration(
                    "ai/timefold/solver/spring/boot/autoconfigure/customSolver2Config.xml",
                ),
            ),
        ])
        .unwrap();
    for (name, expected) in [("solver1", 0.5), ("solver2", 0.25)] {
        let factory = bank
            .factory::<BasicModel, ResultType, String, SimpleScore>(name)
            .unwrap();
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(problem(bank.selection(name).unwrap().2))
            .run()
            .unwrap();
        assert_eq!(gradient(&job, Duration::from_secs(5)), expected);
        job.wait();
        manager.shutdown().unwrap();
    }
}
#[test]
fn solver_config_xml_property_no_global_termination() {
    let path =
        "ai/timefold/solver/spring/boot/autoconfigure/solverConfigWithoutGlobalTermination.xml";
    let bank = registry()
        .prepare([
            ("solver1".into(), xml_configuration(path)),
            ("solver2".into(), xml_configuration(path)),
        ])
        .unwrap();
    for name in ["solver1", "solver2"] {
        let factory = bank
            .factory::<BasicModel, ResultType, String, SimpleScore>(name)
            .unwrap();
        assert_eq!(factory.configuration().limits.max_time, None);
        assert!(
            matches!(&factory.configuration().phases[1],SolverPhase::LocalSearch(c) if c.max_time==Some(Duration::from_secs(3)))
        );
        let mut manager = factory.build_manager(1).unwrap();
        let job = manager
            .job_builder(1)
            .with_problem(problem(bank.selection(name).unwrap().2))
            .run()
            .unwrap();
        positive(job.wait().as_ref().as_ref().unwrap());
        manager.shutdown().unwrap();
    }
}
