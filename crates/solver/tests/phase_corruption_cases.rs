// Adapted from pinned Community DefaultSolverTest and its corrupted easy-score
// fixture. Apache-2.0; see compat/provenance.json.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
struct Entity;
#[test]
fn solve_corrupted_easy_phase_asserted() {
    let model = Rc::new(FiniteProblem::new(vec![Variable::new(vec!["v1", "v2"]); 2]).unwrap());
    let calls = Cell::new(0i64);
    let mut network = ConstraintNetwork::new(2);
    // Source returns Random.nextInt(1000). Consecutive deterministic scores in
    // that range preserve non-repeatability without accidental random equality.
    network
        .add_constraint("CorruptedEasyScoreCalculator", vec![0, 1], |_| {
            let next = calls.get();
            calls.set(next + 1);
            SimpleScore::new(next % 1000)
        })
        .unwrap();
    let recipe = ScoringFactoryRecipe::full_evaluation_recipe(
        Ok,
        |_: &EnvironmentMode, _settings, input| ScoreDirector::new(&model, &network, input),
        |d: &ScoreDirector<'_, '_, &str, SimpleScore>| Ok(d.assignment().clone()),
    );
    let mut config = SolverAssemblyConfiguration::for_factory_model(model.clone());
    config.schema = Some(Rc::new(
        PlanningSchema::new::<FiniteProblem<&str>>(vec![PlanningEntityDefinition::of::<Entity>(
            vec![PlanningVariableDefinition::basic::<&str>("value", false)],
        )])
        .unwrap(),
    ));
    config.environment = EnvironmentProfile::new(Some(EnvironmentMode::PhaseAssert), vec![]);
    config.phases = vec![
        SolverPhase::Construction(Default::default()),
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        }),
    ];
    let factory = ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .assemble_solver_factory(
            config,
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap();
    let error = factory
        .solve_assembled_with(model.initial_assignment(), |d, p| {
            d.run_solver(SolverSequence {
                limits: Default::default(),
                phases: vec![p.clone()],
            })
            .map(Work)
            .map_err(|e| match e {
                SolverError::InitialScoring {
                    error: LocalSearchError::Scoring(error),
                }
                | SolverError::Phase {
                    error: LocalSearchError::Scoring(error),
                    ..
                } => error,
                other => panic!("unexpected phase error: {other:?}"),
            })
        })
        .err()
        .unwrap();
    let text = match error.primary {
        FactoryExecutionError::Context(e) => e.to_string(),
        other => panic!("unexpected failure: {other:?}"),
    };
    assert!(text.contains("corruption"), "{text}");
    assert!(text.contains("FULL_ASSERT"), "{text}");
    assert!(text.contains("NO_ASSERT"), "{text}");
}
struct Work(SolverResult<Assignment<&'static str>, SimpleScore>);
impl ContextScoreWork for Work {
    fn context_calculations(&self) -> u64 {
        self.0.work.evaluations
    }
}
