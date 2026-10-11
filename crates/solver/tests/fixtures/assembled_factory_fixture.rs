// Community TEST-visible registration, objectives and phase roles adapted from
// DefaultSolverFactoryTest and its test helpers. Apache-2.0; see provenance.
#![allow(dead_code)]
use std::{
    cell::{Cell, RefCell},
    io,
    rc::Rc,
};
use timetuck::*;
pub type Model = FiniteProblem<i64>;
pub struct Entity;
#[derive(Clone, Copy)]
pub enum Objective {
    Zero,
    FirstPresence,
    AlwaysAssigned,
    Collisions,
    Weighted,
}
#[derive(Default)]
pub struct Observations {
    pub preparations: Cell<usize>,
    pub instances: Cell<usize>,
    pub releases: Cell<usize>,
    pub checks: Cell<usize>,
    pub modes: RefCell<Vec<EnvironmentMode>>,
    pub fail_prepare: Cell<bool>,
    pub fail_release: Cell<bool>,
}
pub struct Domain {
    pub model: Rc<Model>,
    pub network: ConstraintNetwork<'static, i64, SimpleScore>,
    pub objective: Objective,
}
impl Domain {
    pub fn new(values: usize, entities: usize, objective: Objective) -> Self {
        let model = Rc::new(
            Model::new(
                (0..entities)
                    .map(|i| {
                        Variable::new((0..values as i64).collect())
                            .with_initial((i % values) as i64)
                    })
                    .collect(),
            )
            .unwrap(),
        );
        let mut network = ConstraintNetwork::new(entities);
        if !matches!(objective, Objective::Zero) {
            network
                .add_constraint("fixture", (0..entities).collect(), move |rows| {
                    SimpleScore::new(match objective {
                        Objective::Zero => 0,
                        Objective::FirstPresence => {
                            -i64::from(rows.iter().any(|r| r.is_some_and(|v| *v == 0)))
                        }
                        Objective::AlwaysAssigned => {
                            -(rows.iter().filter(|r| r.is_some()).count() as i64)
                        }
                        Objective::Collisions => {
                            -(rows
                                .iter()
                                .enumerate()
                                .map(|(i, a)| {
                                    rows.iter()
                                        .enumerate()
                                        .filter(|(j, b)| *j != i && a.is_some() && a == *b)
                                        .count()
                                })
                                .sum::<usize>() as i64)
                        }
                        Objective::Weighted => rows
                            .iter()
                            .enumerate()
                            .map(|(i, v)| (i as i64 + 1) * v.copied().unwrap_or(0))
                            .sum(),
                    })
                })
                .unwrap();
        }
        Self {
            model,
            network,
            objective,
        }
    }
}
pub struct Prepared<'p> {
    pub domain: &'p Domain,
    pub mode: EnvironmentMode,
}
pub enum Engine<'p> {
    Dense(Box<ScoreDirector<'p, 'static, i64, SimpleScore>>),
    Stream(Box<StreamScoreDirector<'p, 'static, i64, SimpleScore>>),
}
pub struct Resource<'p> {
    pub engine: Engine<'p>,
    pub observations: Rc<Observations>,
    pub settings: FactoryDirectorSettings,
}
impl Resource<'_> {
    pub fn input(&self) -> &Assignment<i64> {
        match &self.engine {
            Engine::Dense(d) => d.assignment(),
            Engine::Stream(d) => d.assignment(),
        }
    }
    pub fn phase(&mut self, phase: &SolverPhase<'_, SimpleScore>) -> Result<Work, io::Error> {
        let plan = SolverSequence {
            limits: Default::default(),
            phases: vec![phase.clone()],
        };
        match &mut self.engine {
            Engine::Dense(d) => d
                .run_solver(plan)
                .map(Work)
                .map_err(|e| io::Error::other(e.to_string())),
            Engine::Stream(d) => d
                .run_solver(plan)
                .map(Work)
                .map_err(|e| io::Error::other(e.to_string())),
        }
    }
}
impl ContextScoreResource for Resource<'_> {
    type Score = SimpleScore;
    type Error = io::Error;
    fn context_score(&self) -> SimpleScore {
        match &self.engine {
            Engine::Dense(d) => d.score(),
            Engine::Stream(d) => d.score(),
        }
    }
    fn check_context(&mut self) -> Result<(), io::Error> {
        self.observations
            .checks
            .set(self.observations.checks.get() + 1);
        match &mut self.engine {
            Engine::Dense(d) => d
                .assert_consistent()
                .map_err(|e| io::Error::other(e.to_string())),
            Engine::Stream(d) => d
                .assert_consistent()
                .map_err(|e| io::Error::other(e.to_string())),
        }
    }
    fn context_totals(&self) -> Vec<(String, SimpleScore)> {
        match &self.engine {
            Engine::Dense(d) => d.constraint_scores().map(|(n, s)| (n.into(), s)).collect(),
            Engine::Stream(d) => d.constraint_scores().map(|(n, s)| (n.into(), s)).collect(),
        }
    }
    fn release_context(&mut self) -> Result<(), io::Error> {
        self.observations
            .releases
            .set(self.observations.releases.get() + 1);
        if self.observations.fail_release.get() {
            Err(io::Error::other("release fixture"))
        } else {
            Ok(())
        }
    }
}
pub struct Work(pub SolverResult<Assignment<i64>, SimpleScore>);
impl ContextScoreWork for Work {
    fn context_calculations(&self) -> u64 {
        self.0.work.evaluations
    }
}
pub type Declaration<'p> =
    ScoringFactoryDeclaration<'p, Prepared<'p>, Assignment<i64>, Resource<'p>, io::Error>;
pub type Factory<'p> =
    AssembledSolverFactory<'p, Model, Prepared<'p>, Assignment<i64>, Resource<'p>>;
pub fn declaration(
    domain: &Domain,
    stream: bool,
    observations: Rc<Observations>,
) -> Declaration<'_> {
    let prep_observations = observations.clone();
    let prepare = move |mode| {
        prep_observations
            .preparations
            .set(prep_observations.preparations.get() + 1);
        if prep_observations.fail_prepare.get() {
            return Err(io::Error::other("prepare fixture"));
        }
        Ok(Prepared { domain, mode })
    };
    let build = move |prepared: &Prepared<'_>, settings: FactoryDirectorSettings, input| {
        observations.instances.set(observations.instances.get() + 1);
        observations
            .modes
            .borrow_mut()
            .push(settings.requested_mode);
        let engine = if stream {
            assert_eq!(prepared.mode, settings.requested_mode);
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i64>)>();
            match domain.objective {
                Objective::Zero => {}
                Objective::AlwaysAssigned => {
                    b.score("fixture", source.port(), |(_, v)| {
                        SimpleScore::new(-i64::from(v.is_some()))
                    })
                    .unwrap();
                }
                Objective::Weighted => {
                    b.score("fixture", source.port(), |(i, v)| {
                        SimpleScore::new((*i as i64 + 1) * v.unwrap_or(0))
                    })
                    .unwrap();
                }
                _ => panic!("fixture stream objective not bound"),
            }
            Engine::Stream(Box::new(
                StreamScoreDirector::new(&domain.model, b.build(), source, input)
                    .map_err(|e| io::Error::other(e.to_string()))?,
            ))
        } else {
            Engine::Dense(Box::new(
                ScoreDirector::new(&domain.model, &domain.network, input)
                    .map_err(|e| io::Error::other(e.to_string()))?,
            ))
        };
        Ok(Resource {
            engine,
            settings,
            observations: observations.clone(),
        })
    };
    let copy = |d: &Resource<'_>| Ok(d.input().clone());
    let recipe = if stream {
        ScoringFactoryRecipe::stream_recipe(prepare, build, copy)
    } else {
        ScoringFactoryRecipe::full_evaluation_recipe(prepare, build, copy)
    };
    Declaration::default().with_scoring_recipe(recipe)
}
pub fn schema() -> Rc<PlanningSchema> {
    Rc::new(
        PlanningSchema::new::<Model>(vec![PlanningEntityDefinition::of::<Entity>(vec![
            PlanningVariableDefinition::basic::<i64>("value", false),
        ])])
        .unwrap(),
    )
}
pub fn configuration(
    domain: &Domain,
    global: Option<EnvironmentMode>,
    modes: Vec<Option<EnvironmentMode>>,
    steps: usize,
) -> SolverAssemblyConfiguration<'_, Model, SimpleScore> {
    let mut c = SolverAssemblyConfiguration::for_factory_model(domain.model.clone());
    c.schema = Some(schema());
    c.environment = EnvironmentProfile::new(global, modes);
    c.phases = vec![
        SolverPhase::Construction(Default::default()),
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: steps as u64,
            ..Default::default()
        }),
    ];
    c
}
pub fn prepare<'p>(
    domain: &'p Domain,
    stream: bool,
    global: Option<EnvironmentMode>,
    modes: Vec<Option<EnvironmentMode>>,
) -> Factory<'p> {
    declaration(domain, stream, Rc::new(Observations::default()))
        .assemble_solver_factory(
            configuration(domain, global, modes, 10),
            Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
        )
        .unwrap()
}
