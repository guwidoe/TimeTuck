// Community TEST-visible DefaultSolverTest/PlannerTestUtils/TestdataSolution
// fixture roles adapted to native owned data. Apache-2.0; see provenance.
#![allow(dead_code)]
use std::{io, rc::Rc};
use timetuck::*;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Solution {
    pub values: Vec<i64>,
    pub assignment: Vec<Option<i64>>,
}
impl Solution {
    pub fn initialized(values: usize, entities: usize) -> Self {
        Self {
            values: (0..values as i64).collect(),
            assignment: (0..entities).map(|i| Some((i % values) as i64)).collect(),
        }
    }
    pub fn model(&self) -> FiniteProblem<i64> {
        FiniteProblem::new(
            self.assignment
                .iter()
                .map(|v| {
                    let var = Variable::new(self.values.clone());
                    match v {
                        Some(v) => var.with_initial(*v),
                        None => var,
                    }
                })
                .collect(),
        )
        .unwrap()
    }
}
pub struct Entity;
pub struct EasyCalculator {
    weighted: bool,
    score: SimpleScore,
}
impl IncrementalCalculator<Solution> for EasyCalculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = ();
    fn reset(
        &mut self,
        input: &Solution,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.score = SimpleScore::new(if self.weighted {
            input
                .assignment
                .iter()
                .enumerate()
                .map(|(i, v)| (i as i64 + 1) * v.unwrap_or(0))
                .sum()
        } else {
            0
        });
        Ok(())
    }
    fn after_change(
        &mut self,
        input: &Solution,
        _: &(),
        registry: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.reset(input, registry)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.score
    }
}
/// The actual scoring resource owns its calculator/session and complete working
/// solution. It borrows no model, so its closed runtime views may be retained on
/// the worker thread without extending a model's lifetime or leaking fixtures.
pub struct Resource {
    pub session: CalculatorSession<Solution, EasyCalculator>,
    score: SimpleScore,
    weighted: bool,
}
impl Resource {
    pub fn input(&self) -> &Solution {
        self.session.solution()
    }
    pub fn change(&mut self, update: impl FnOnce(&mut Solution)) -> Result<(), io::Error> {
        self.session.change(&(), update).map_err(io::Error::other)?;
        self.score = self
            .session
            .score()
            .map_err(io::Error::other)?
            .with_structural(
                -(self
                    .input()
                    .assignment
                    .iter()
                    .filter(|v| v.is_none())
                    .count() as i64),
            );
        Ok(())
    }
    pub fn phase(
        &mut self,
        phase: &SolverPhase<'_, SimpleScore>,
        token: &CancellationToken,
    ) -> Result<Work, io::Error> {
        let model = self.input().model();
        let mut network = ConstraintNetwork::<_, SimpleScore>::new(self.input().assignment.len());
        if self.weighted {
            network
                .add_constraint(
                    "weighted values",
                    (0..self.input().assignment.len()).collect(),
                    |values| {
                        SimpleScore::new(
                            values
                                .iter()
                                .enumerate()
                                .map(|(i, v)| (i as i64 + 1) * v.copied().unwrap_or(0))
                                .sum(),
                        )
                    },
                )
                .unwrap();
        }
        let result = solve(
            &model,
            &network,
            SolverSequence {
                limits: SolverLimits {
                    cancellation: Some(token),
                    ..Default::default()
                },
                phases: vec![phase.clone()],
            },
        )
        .map_err(|e| io::Error::other(e.to_string()))?;
        let mut input = self.input().clone();
        input.assignment = result.assignment.values().to_vec();
        self.session.reset(input).map_err(io::Error::other)?;
        self.score = result.score.unwrap_or(SimpleScore::ZERO);
        Ok(Work(result))
    }
}
impl ContextScoreResource for Resource {
    type Score = SimpleScore;
    type Error = io::Error;
    fn context_score(&self) -> SimpleScore {
        self.score
    }
    fn check_context(&mut self) -> Result<(), io::Error> {
        let score = self
            .session
            .score()
            .map_err(io::Error::other)?
            .with_structural(
                -(self
                    .input()
                    .assignment
                    .iter()
                    .filter(|v| v.is_none())
                    .count() as i64),
            );
        if score != self.score {
            return Err(io::Error::other("owned calculator score corruption"));
        }
        Ok(())
    }
    fn context_totals(&self) -> Vec<(String, SimpleScore)> {
        vec![]
    }
}
pub struct Work(pub SolverResult<Assignment<i64>, SimpleScore>);
impl ContextScoreWork for Work {
    fn context_calculations(&self) -> u64 {
        self.0.work.evaluations
    }
}
pub type Factory = AssembledSolverFactory<'static, Solution, EnvironmentMode, Solution, Resource>;
pub type View = FactoryRuntimeView<'static, Solution, EnvironmentMode, Solution, Resource>;
pub fn factory(input: Solution) -> Factory {
    factory_with_objective(input, false)
}
// Original nonzero control, not adapted from the reference calculator.
pub fn factory_with_objective(input: Solution, weighted: bool) -> Factory {
    let recipe = ScoringFactoryRecipe::full_evaluation_recipe(
        Ok,
        move |_: &EnvironmentMode, _, input: Solution| {
            let missing = input.assignment.iter().filter(|v| v.is_none()).count();
            let mut session = CalculatorSession::new(
                input,
                EasyCalculator {
                    weighted,
                    score: SimpleScore::ZERO,
                },
                false,
            )
            .map_err(io::Error::other)?;
            let score = session
                .score()
                .map_err(io::Error::other)?
                .with_structural(-(missing as i64));
            Ok(Resource {
                session,
                score,
                weighted,
            })
        },
        |r: &Resource| Ok(r.input().clone()),
    );
    let mut config = SolverAssemblyConfiguration::for_factory_model(Rc::new(input));
    config.schema = Some(Rc::new(
        PlanningSchema::new::<Solution>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i64>("value", false),
        ])])
        .unwrap(),
    ));
    config.environment = EnvironmentProfile::new(
        Some(EnvironmentMode::NoAssert),
        vec![None, Some(EnvironmentMode::FullAssert)],
    );
    config.phases = vec![
        SolverPhase::Construction(Default::default()),
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: 10,
            ..Default::default()
        }),
    ];
    ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .assemble_solver_factory(
            config,
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap()
}
