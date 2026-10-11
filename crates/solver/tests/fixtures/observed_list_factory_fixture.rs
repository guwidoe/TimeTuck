// Pinned Community TEST fixtures: TestdataListSolution/PlannerTestUtils and
// DefaultSolverTest.InvalidMoveListFactory/InvalidSelectorBasedMove.
// Apache-2.0; see compat/provenance.json. Weighted is an original objective.
#![allow(dead_code)]
use super::basic::Observations;
use std::{io, rc::Rc};
use timetuck::*;
pub type Model = ListProblem<String>;
pub struct Entity;
pub struct Value;
pub struct Domain {
    pub model: Rc<Model>,
    pub network: ListConstraintNetwork<'static, String, SimpleScore>,
    pub weighted: bool,
}
impl Domain {
    pub fn new(values: usize, entities: usize, weighted: bool) -> Self {
        let model = Rc::new(
            Model::new(
                (0..values)
                    .map(|i| format!("Generated Value {i}"))
                    .collect(),
                (0..entities)
                    .map(|i| {
                        ListEntity::new(vec![]).with_range_diagnostics(
                            format!("Generated Entity {i}"),
                            "valueList",
                            String::clone,
                        )
                    })
                    .collect(),
            )
            .unwrap(),
        );
        let mut network = ListConstraintNetwork::new(entities, values);
        if weighted {
            for i in 0..entities {
                network
                    .add_constraint(
                        format!("entity {i}"),
                        vec![ListScope::Entity(i)],
                        move |facts| {
                            let ListFact::Entity(values) = &facts[0] else {
                                unreachable!()
                            };
                            SimpleScore::new((i as i64 + 1) * values.len() as i64)
                        },
                    )
                    .unwrap();
            }
        }
        Self {
            model,
            network,
            weighted,
        }
    }
}
pub enum Engine<'p> {
    Dense(Box<ListScoreDirector<'p, 'static, String, SimpleScore>>),
    Stream(Box<ListStreamScoreDirector<'p, 'static, String, SimpleScore>>),
}
pub struct Resource<'p> {
    pub engine: Engine<'p>,
    pub observations: Rc<Observations>,
    pub settings: FactoryDirectorSettings,
}
impl Resource<'_> {
    pub fn input(&self) -> &ListAssignment<String> {
        match &self.engine {
            Engine::Dense(d) => d.assignment(),
            Engine::Stream(d) => d.assignment(),
        }
    }
    pub fn phase(
        &mut self,
        phase: &SolverPhase<'_, SimpleScore>,
        invalid: bool,
    ) -> Result<Work, io::Error> {
        let plan = SolverSequence {
            limits: Default::default(),
            phases: vec![phase.clone()],
        };
        let mut provider = |_: &Model, a: &ListAssignment<String>, _: u64| {
            vec![ListMove::Assign {
                value: "bad value".into(),
                destination: ListPosition {
                    entity: 0,
                    index: a.list(0).unwrap().len(),
                },
            }]
        };
        macro_rules! run_phase {
            ($director:expr) => {{
                let mut extensions = SolverExtensions::default();
                if invalid && matches!(phase, SolverPhase::LocalSearch(_)) {
                    extensions.move_providers.bind(0, &mut provider);
                }
                $director
                    .run_solver_with_extensions(plan, extensions)
                    .map(Work)
                    .map_err(|e| io::Error::other(e.to_string()))
            }};
        }
        match &mut self.engine {
            Engine::Dense(d) => run_phase!(d),
            Engine::Stream(d) => run_phase!(d),
        }
    }
}
impl ContextScoreResource for Resource<'_> {
    type Score = SimpleScore;
    type Error = io::Error;
    fn context_score(&self) -> SimpleScore {
        match &self.engine {
            Engine::Dense(d) => d.context_score(),
            Engine::Stream(d) => d.context_score(),
        }
    }
    fn context_totals(&self) -> Vec<(String, SimpleScore)> {
        match &self.engine {
            Engine::Dense(d) => d.context_totals(),
            Engine::Stream(d) => d.context_totals(),
        }
    }
    fn check_context(&mut self) -> Result<(), io::Error> {
        match &mut self.engine {
            Engine::Dense(d) => d
                .check_context()
                .map_err(|e| io::Error::other(e.to_string())),
            Engine::Stream(d) => d
                .check_context()
                .map_err(|e| io::Error::other(e.to_string())),
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
pub struct Work(pub SolverResult<ListAssignment<String>, SimpleScore>);
impl ContextScoreWork for Work {
    fn context_calculations(&self) -> u64 {
        self.0.work.evaluations
    }
}
pub struct Prepared<'p> {
    domain: &'p Domain,
    mode: EnvironmentMode,
}
pub type Factory<'p> =
    AssembledSolverFactory<'p, Model, Prepared<'p>, ListAssignment<String>, Resource<'p>>;
pub type View<'p> =
    FactoryRuntimeView<'p, Model, Prepared<'p>, ListAssignment<String>, Resource<'p>>;
pub fn prepare<'p>(
    domain: &'p Domain,
    stream: bool,
    observations: Rc<Observations>,
    modes: Vec<Option<EnvironmentMode>>,
    steps: u64,
) -> Factory<'p> {
    let prep_observations = observations.clone();
    let prepare = move |mode| {
        prep_observations
            .preparations
            .set(prep_observations.preparations.get() + 1);
        Ok(Prepared { domain, mode })
    };
    let build = move |prepared: &Prepared<'p>, settings: FactoryDirectorSettings, input| {
        observations.instances.set(observations.instances.get() + 1);
        observations
            .modes
            .borrow_mut()
            .push(settings.requested_mode);
        let engine = if stream {
            assert_eq!(prepared.mode, settings.requested_mode);
            let mut b = StreamGraphBuilder::new();
            let sources = ListStreamSources::new(&mut b);
            if domain.weighted {
                b.score("weighted", sources.entities.port(), |(i, values)| {
                    SimpleScore::new((*i as i64 + 1) * values.len() as i64)
                })
                .unwrap();
            }
            Engine::Stream(Box::new(
                ListStreamScoreDirector::new(&prepared.domain.model, b.build(), sources, input)
                    .map_err(|e| io::Error::other(e.to_string()))?,
            ))
        } else {
            Engine::Dense(Box::new(
                ListScoreDirector::new(&prepared.domain.model, &prepared.domain.network, input)
                    .map_err(|e| io::Error::other(e.to_string()))?,
            ))
        };
        Ok(Resource {
            engine,
            observations: observations.clone(),
            settings,
        })
    };
    let copy = |r: &Resource<'_>| Ok(r.input().clone());
    let recipe = if stream {
        ScoringFactoryRecipe::stream_recipe(prepare, build, copy)
    } else {
        ScoringFactoryRecipe::full_evaluation_recipe(prepare, build, copy)
    };
    let mut c = SolverAssemblyConfiguration::for_factory_model(domain.model.clone());
    c.schema = Some(Rc::new(
        PlanningSchema::new::<Model>(vec![
            PlanningEntityDefinition::of::<Entity>(vec![
                PlanningVariableDefinition::list::<String>("valueList", false),
            ]),
            PlanningEntityDefinition::of::<Value>(vec![]),
        ])
        .unwrap(),
    ));
    c.environment = EnvironmentProfile::new(None, modes);
    c.phases = vec![
        SolverPhase::Construction(Default::default()),
        SolverPhase::LocalSearch(LocalSearchConfig {
            max_steps: steps,
            ..Default::default()
        }),
    ];
    ScoringFactoryDeclaration::default()
        .with_scoring_recipe(recipe)
        .assemble_solver_factory(c, Rc::new(FactoryScoreShape::for_score::<SimpleScore>()))
        .unwrap()
}
pub fn primary_message(e: &FactoryExecutionFailure<io::Error>) -> &io::Error {
    match &e.primary {
        FactoryExecutionError::Context(ScoringContextError::Scoring(
            FactoryAssemblyError::Resource(error),
        )) => error,
        _ => panic!("unexpected error: {e:?}"),
    }
}
