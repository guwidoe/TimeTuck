// Adapted Community multiple-solver TEST domain/provider helpers; Apache-2.0.
#![allow(dead_code)]
use std::{rc::Rc, time::Duration};
use timetuck::*;
pub struct Entity;
pub struct BasicModel(pub FiniteProblem<String>);
pub type ResultType = SolverResult<Assignment<String>, SimpleScore>;
pub type Factory = SolverFactory<BasicModel, ResultType, String>;
pub fn schema() -> Rc<PlanningSchema> {
    Rc::new(
        PlanningSchema::new::<BasicModel>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<String>("value", false),
        ])])
        .unwrap(),
    )
}
pub fn problem(s: &PlanningSchema) -> BasicModel {
    let variable = s
        .genuine_entity::<Entity>()
        .unwrap()
        .genuine_variable("value")
        .unwrap();
    let values = vec!["v1".to_string(), "v2".to_string()];
    let ranges = variable.shared_ranges(vec![values], 2).unwrap();
    BasicModel(FiniteProblem::new((0..2).map(|i| ranges.variable(i).unwrap()).collect()).unwrap())
}
pub fn ordered_pairs(values: &[Option<String>]) -> SimpleScore {
    let mut n = 0;
    for (i, a) in values.iter().enumerate() {
        if let Some(a) = a {
            for (j, b) in values.iter().enumerate() {
                if i != j && b.as_ref() == Some(a) {
                    n -= 1;
                }
            }
        }
    }
    SimpleScore::new(n)
}
pub fn scoring(key: &str, stream: bool) -> NamedModelScoring {
    // Two concrete native recipes; the selected callback constructs and uses
    // its own PreparedScoreFactory and fresh actual director in every job.
    NamedModelScoring::new(key, move |config, profile| {
        build_factory(config, profile, stream)
    })
}
pub fn build_factory(
    mut config: SolverSequence<'static>,
    profile: EnvironmentProfile,
    stream: bool,
) -> Result<Factory, String> {
    let mode = profile.global_mode();
    if matches!(
        mode,
        EnvironmentMode::FullAssert | EnvironmentMode::TrackedFullAssert
    ) {
        for p in &mut config.phases {
            match p {
                SolverPhase::Construction(c) => c.assert_scores = true,
                SolverPhase::LocalSearch(c) => c.assert_scores = true,
                _ => {}
            }
        }
    }
    SolverFactory::new(config, move |input: BasicModel, config, progress| {
        let model = Rc::new(input.0);
        let result = if stream {
            let recipe = ScoringFactoryRecipe::stream_recipe(
                |_| Ok::<_, StreamScoringError>(()),
                |_, _, input| {
                    let mut g = StreamGraphBuilder::new();
                    let source = g.source::<usize, (usize, Option<String>)>();
                    let join = g.join_equal(
                        source.port(),
                        source.port(),
                        |(_, v)| v.clone(),
                        |(_, v)| v.clone(),
                    )?;
                    let pairs = g.filter(&join, |((i, a), (j, _))| i != j && a.is_some())?;
                    g.score("Don't assign 2 entities the same value.", &pairs, |_| {
                        SimpleScore::new(-1)
                    })?;
                    StreamScoreDirector::new(&model, g.build(), source, input)
                },
                |d: &StreamScoreDirector<'_, '_, String, SimpleScore>| Ok(d.assignment().clone()),
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
            d.factory_resource_mut()
                .run_solver(config)
                .map_err(|e| e.to_string())?
        } else {
            let count = model.variables().len();
            let mut network = ConstraintNetwork::<String, SimpleScore>::new(count);
            network
                .add_constraint(
                    "Don't assign 2 entities the same value.",
                    (0..count).collect(),
                    |rows| {
                        let v: Vec<_> = rows.iter().map(|v| v.cloned()).collect();
                        ordered_pairs(&v)
                    },
                )
                .unwrap();
            let recipe = ScoringFactoryRecipe::incremental_recipe(
                |_| Ok::<_, ScoringError>(&network),
                |n: &&ConstraintNetwork<'_, String, SimpleScore>, _, input| {
                    ScoreDirector::new(&model, n, input)
                },
                |d: &ScoreDirector<'_, '_, String, SimpleScore>| Ok(d.assignment().clone()),
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
            d.factory_resource_mut()
                .run_solver(config)
                .map_err(|e| e.to_string())?
        };
        progress.record_work(result.work);
        Ok(result)
    })
    .map_err(|e| e.to_string())
}

pub fn registry() -> NamedModelFactoryRegistry {
    let mut r = NamedModelFactoryRegistry::default();
    r.register("basic", schema(), vec![scoring("stream", true)])
        .unwrap();
    r
}
pub fn sequence(time: Option<Duration>, goal: bool) -> SolverSequence<'static> {
    let mut s = SolverSequence::default();
    s.limits.max_time = time;
    s.limits.best_score_limit = goal.then_some(SimpleScore::ZERO);
    for p in &mut s.phases {
        if let SolverPhase::LocalSearch(c) = p {
            c.max_steps = 50;
            c.max_evaluations = 100;
        }
    }
    s
}
pub fn configuration(time: Option<Duration>, goal: bool) -> NamedSolverConfiguration {
    NamedSolverConfiguration {
        sequence: sequence(time, goal),
        ..Default::default()
    }
}
pub fn positive(solution: &ResultType) {
    assert!(solution.complete);
    assert!(solution.score.unwrap().score() >= 0);
    assert_eq!(
        solution.score,
        Some(ordered_pairs(solution.assignment.values()))
    );
}
// Source scope initialized to HardSoft[-1,-1] avoids best-score priority. Native
// job gradients expose the time component; project the negative ordinary score
// to this actual SimpleScore goal before observing that component.
pub fn gradient(job: &ConfiguredSolverJob<ResultType, String>, elapsed: Duration) -> f64 {
    let starting = HardSoftScore::new(-1, -1);
    let best = HardSoftScore::new(-1, -1);
    assert_eq!(starting, best);
    assert!(!starting.is_feasible());
    if let Some(goal) = job.configuration().limits.best_score_limit {
        let negative = SimpleScore::new(best.ordinary_levels().min().unwrap());
        assert!(negative < goal);
    }
    job.configuration()
        .limits
        .time_gradient(elapsed, Duration::ZERO)
        .unwrap()
}
pub fn xml_configuration(name: &str) -> NamedSolverConfiguration {
    let text = match name {
        "ai/timefold/solver/spring/boot/autoconfigure/customSolver1Config.xml" => {
            include_str!("named_models/customSolver1Config.xml")
        }
        "ai/timefold/solver/spring/boot/autoconfigure/customSolver2Config.xml" => {
            include_str!("named_models/customSolver2Config.xml")
        }
        "ai/timefold/solver/spring/boot/autoconfigure/solverConfigWithoutGlobalTermination.xml" => {
            include_str!("named_models/solverConfigWithoutGlobalTermination.xml")
        }
        _ => panic!("unregistered TEST resource"),
    };
    let xml = SolverXmlConfiguration::parse(text).unwrap();
    let seconds = |node: &SolverXmlElement| {
        node.child("termination")
            .and_then(|t| t.value("secondsSpentLimit"))
            .map(|v| Duration::from_secs(v.parse().unwrap()))
    };
    let mut config = configuration(seconds(xml.root()), false);
    if let Some(local) = xml.root().child("localSearch") {
        let SolverPhase::LocalSearch(phase) = &mut config.sequence.phases[1] else {
            unreachable!()
        };
        phase.max_time = seconds(local);
    }
    config
}
