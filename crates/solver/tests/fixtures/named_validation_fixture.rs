// Pinned Community validation TEST/helper adaptation; Apache-2.0; see NOTICE.
#![allow(dead_code)]
#[path = "named_models_fixture.rs"]
pub mod base;
use std::{cell::Cell, rc::Rc};
use timetuck::*;
pub const BASIC: &str =
    "ai.timefold.solver.spring.boot.autoconfigure.basic.domain.TestdataSpringSolution";
pub const OTHER: &str =
    "ai.timefold.solver.spring.boot.autoconfigure.gizmo.domain.TestdataGizmoSpringSolution";
pub const EASY: [&str; 2] = [
    "ai.timefold.solver.spring.boot.autoconfigure.dummy.basic.constraints.easy.DummySpringEasyScore",
    "ai.timefold.solver.spring.boot.autoconfigure.dummy.list.constraints.easy.DummySpringListEasyScore",
];
pub const INCREMENTAL: [&str; 2] = [
    "ai.timefold.solver.spring.boot.autoconfigure.dummy.basic.constraints.incremental.DummySpringIncrementalScore",
    "ai.timefold.solver.spring.boot.autoconfigure.dummy.list.constraints.incremental.DummySpringListIncrementalScore",
];
pub const PROVIDER: [&str; 2] = [
    "ai.timefold.solver.spring.boot.autoconfigure.basic.constraints.TestdataSpringConstraintProvider",
    "ai.timefold.solver.spring.boot.autoconfigure.declarative.constraints.TestdataSpringSupplierVariableConstraintProvider",
];
pub struct OtherModel(pub FiniteProblem<String>);
pub struct OtherEntity;
pub fn other_schema() -> Rc<PlanningSchema> {
    Rc::new(
        PlanningSchema::new::<OtherModel>(vec![PlanningEntityDefinition::of::<OtherEntity>(vec![
            PlanningVariableDefinition::basic::<String>("value", false),
        ])])
        .unwrap(),
    )
}
pub fn requests(
    count: usize,
    model: Option<&str>,
    scoring: Option<&str>,
) -> Vec<(String, NamedSolverConfiguration)> {
    (1..=count)
        .map(|i| {
            (
                format!("solver{i}"),
                NamedSolverConfiguration {
                    model: model.map(str::to_owned),
                    scoring: scoring.map(str::to_owned),
                    ..base::configuration(None, true)
                },
            )
        })
        .collect()
}
fn dummy_score(_working: &Assignment<String>) -> Option<SimpleScore> {
    None
}
pub fn null_recipe(key: &str, calls: &Rc<Cell<usize>>) -> NamedModelScoring {
    let builds = calls.clone();
    NamedModelScoring::new(key, move |config, _| {
        builds.set(builds.get() + 1);
        SolverFactory::new(config, |input: base::BasicModel, _, _| {
            // The TEST dummy calculator's actual callback returns null. Native error
            // replaces a nullable score; registration does not execute this callback.
            let working = input.0.initial_assignment();
            let calculated = dummy_score(&working);
            match calculated {
                None => Err::<base::ResultType, String>(
                    "registered TEST dummy calculator returned no score".into(),
                ),
                Some(_) => unreachable!("TEST dummy callback returns no score"),
            }
        })
        .map_err(|e| e.to_string())
    })
}
pub fn basic_recipe(key: &str, calls: &Rc<Cell<usize>>) -> NamedModelScoring {
    let builds = calls.clone();
    NamedModelScoring::new(key, move |c, p| {
        builds.set(builds.get() + 1);
        base::build_factory(c, p, true)
    })
}
pub fn other_recipe(key: &str, calls: &Rc<Cell<usize>>) -> NamedModelScoring {
    let builds = calls.clone();
    NamedModelScoring::new(key, move |config, _| {
        builds.set(builds.get() + 1);
        SolverFactory::new(config, |input: OtherModel, c, progress| {
            let mut network = ConstraintNetwork::new(input.0.variables().len());
            network
                .add_constraint(
                    "actual other model objective",
                    (0..input.0.variables().len()).collect(),
                    |v| SimpleScore::new(v.iter().filter(|v| v.is_some()).count() as i64),
                )
                .map_err(|e| e.to_string())?;
            let result = solve(&input.0, &network, c).map_err(|e| e.to_string())?;
            progress.record_work(result.work);
            Ok::<_, String>(result)
        })
        .map_err(|e| e.to_string())
    })
}
pub fn catalog(
    keys: [&str; 2],
    calls: &Rc<Cell<usize>>,
    provider: bool,
) -> NamedModelFactoryRegistry {
    let mut r = NamedModelFactoryRegistry::default();
    r.register(BASIC, base::schema(), vec![]).unwrap();
    if provider {
        r.register_global_scoring(basic_recipe(keys[0], calls))
            .unwrap();
        r.register_global_scoring(other_recipe(keys[1], calls))
            .unwrap();
    } else {
        for key in keys {
            r.register_global_scoring(null_recipe(key, calls)).unwrap();
        }
    }
    r
}
pub fn xml_requests(count: usize, xml: &str) -> Vec<(String, NamedSolverConfiguration)> {
    let parsed = SolverXmlConfiguration::parse(xml).unwrap();
    let mut requests = requests(count, None, None);
    for (_, c) in &mut requests {
        c.apply_xml_selection(&parsed).unwrap();
    }
    requests
}
pub const NORMAL_XML: &str = include_str!("named_validation/normalSolverConfig.xml");
pub const EASY_XML: &str = include_str!("named_validation/easyScoreSolverConfig.xml");
pub const INCREMENTAL_XML: &str = include_str!("named_validation/incrementalScoreSolverConfig.xml");
