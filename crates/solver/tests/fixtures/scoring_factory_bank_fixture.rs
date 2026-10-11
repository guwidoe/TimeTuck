// Test-visible zero calculators/property values and generated solution adapted
// from pinned Community ScoreDirectorFactoryFactoryTest/TestdataSolution.
// Apache-2.0; see compat/provenance.json. Native resource wiring is independent.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
pub type Model = FiniteProblem<i64>;
pub type Failure = String;
#[derive(Clone, Default)]
pub struct Properties {
    pub string: String,
    pub number: i32,
}
pub struct PreparedFixture {
    pub properties: Properties,
    pub columns: Vec<usize>,
    pub mode: EnvironmentMode,
}
#[derive(Clone)]
pub struct DummyCalculator {
    pub properties: Properties,
}
impl IncrementalCalculator<Assignment<i64>> for DummyCalculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = ();
    fn reset(
        &mut self,
        _: &Assignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
pub enum Resource<'p> {
    Full {
        problem: &'p Model,
        input: Assignment<i64>,
        network: ConstraintNetwork<'static, i64, SimpleScore>,
        settings: FactoryDirectorSettings,
    },
    Incremental {
        session: CalculatorSession<Assignment<i64>, DummyCalculator>,
        settings: FactoryDirectorSettings,
    },
    Stream {
        director: Box<StreamScoreDirector<'p, 'static, i64, SimpleScore>>,
        settings: FactoryDirectorSettings,
    },
}
impl Resource<'_> {
    pub fn resource_settings(&self) -> FactoryDirectorSettings {
        match self {
            Self::Full { settings, .. }
            | Self::Incremental { settings, .. }
            | Self::Stream { settings, .. } => *settings,
        }
    }
    pub fn working_input(&self) -> &Assignment<i64> {
        match self {
            Self::Full { input, .. } => input,
            Self::Incremental { session, .. } => session.solution(),
            Self::Stream { director, .. } => director.assignment(),
        }
    }
    pub fn actual_score(&mut self) -> SimpleScore {
        match self {
            Self::Full {
                network,
                problem,
                input,
                ..
            } => network.calculate(problem, input).unwrap(),
            Self::Incremental { session, .. } => session.score().unwrap(),
            Self::Stream { director, .. } => director.score(),
        }
    }
    pub fn source_properties(&self) -> &Properties {
        let Self::Incremental { session, .. } = self else {
            panic!("not incremental")
        };
        &session.calculator().properties
    }
}
pub type Recipe<'p> =
    ScoringFactoryRecipe<'p, PreparedFixture, Assignment<i64>, Resource<'p>, Failure>;
pub type Declaration<'p> =
    ScoringFactoryDeclaration<'p, PreparedFixture, Assignment<i64>, Resource<'p>, Failure>;
pub type Factory<'p> =
    PreparedScoreFactory<'p, Model, PreparedFixture, Assignment<i64>, Resource<'p>, Failure>;
pub fn model() -> Rc<Model> {
    Rc::new(
        Model::new(
            (0..7)
                .map(|i| Variable::new((0..5).collect()).with_initial(i % 5))
                .collect(),
        )
        .unwrap(),
    )
}
pub fn shape() -> Rc<FactoryScoreShape> {
    Rc::new(FactoryScoreShape::for_score::<SimpleScore>())
}
pub fn recipe<'p>(
    problem: &'p Model,
    strategy: ScoringFactoryStrategy,
    properties: Properties,
    calls: Rc<Cell<usize>>,
) -> Recipe<'p> {
    let prepare = move |mode| {
        calls.set(calls.get() + 1);
        Ok(PreparedFixture {
            properties: properties.clone(),
            columns: (0..problem.variables().len()).collect(),
            mode,
        })
    };
    let instantiate =
        move |prepared: &PreparedFixture, settings: FactoryDirectorSettings, input| {
            assert_eq!(prepared.columns.len(), problem.variables().len());
            Ok(match strategy {
                ScoringFactoryStrategy::FullEvaluation => Resource::Full {
                    problem,
                    input,
                    network: ConstraintNetwork::new(prepared.columns.len()),
                    settings,
                },
                ScoringFactoryStrategy::Incremental => {
                    problem.validate(&input).map_err(|e| e.to_string())?;
                    Resource::Incremental {
                        session: CalculatorSession::new(
                            input,
                            DummyCalculator {
                                properties: prepared.properties.clone(),
                            },
                            settings.matching == ContextMatchPolicy::Enabled,
                        )
                        .map_err(|e| e.to_string())?,
                        settings,
                    }
                }
                ScoringFactoryStrategy::Stream => {
                    assert_eq!(prepared.mode, settings.requested_mode);
                    let mut builder = StreamGraphBuilder::new();
                    let source = builder.source();
                    Resource::Stream {
                        director: Box::new(
                            StreamScoreDirector::new(problem, builder.build(), source, input)
                                .map_err(|e| e.to_string())?,
                        ),
                        settings,
                    }
                }
                ScoringFactoryStrategy::MultiModeStream => panic!("bank view, not recipe"),
            })
        };
    let copy = |d: &Resource<'p>| Ok(d.working_input().clone());
    match strategy {
        ScoringFactoryStrategy::FullEvaluation => {
            ScoringFactoryRecipe::full_evaluation_recipe(prepare, instantiate, copy)
        }
        ScoringFactoryStrategy::Incremental => {
            ScoringFactoryRecipe::incremental_recipe(prepare, instantiate, copy)
        }
        ScoringFactoryStrategy::Stream => {
            ScoringFactoryRecipe::stream_recipe(prepare, instantiate, copy)
        }
        ScoringFactoryStrategy::MultiModeStream => panic!("bank view, not recipe"),
    }
}
pub fn declaration(problem: &Model, strategy: ScoringFactoryStrategy) -> Declaration<'_> {
    Declaration::default().with_scoring_recipe(recipe(
        problem,
        strategy,
        Properties::default(),
        Rc::new(Cell::new(0)),
    ))
}
pub fn factory<'p>(
    problem: &'p Rc<Model>,
    strategy: ScoringFactoryStrategy,
    global: EnvironmentMode,
    phases: Vec<Option<EnvironmentMode>>,
) -> Factory<'p> {
    declaration(problem, strategy)
        .prepare_score_factory(
            problem.clone(),
            shape(),
            EnvironmentProfile::new(Some(global), phases),
        )
        .unwrap()
}
pub fn assert_opens(factory: &Factory<'_>, input: Assignment<i64>) {
    let mut d = factory
        .score_factory_builder(input)
        .instantiate_score_director()
        .unwrap();
    assert_eq!(d.factory_resource_mut().actual_score(), SimpleScore::ZERO);
    assert_eq!(
        d.factory_resource().resource_settings(),
        d.factory_director_settings()
    );
}
