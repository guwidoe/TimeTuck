// Adapted pinned Community TEST models/providers; Apache-2.0. See NOTICE.
#![allow(dead_code)]
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};
use timetuck::*;
pub const FAILING_VALUE: &str = "fail";
pub const FAILURE_MESSAGE: &str = "Deliberate failure while calculating the score.";
#[derive(Clone, Default, Debug)]
pub struct Entity {
    pub value: Option<String>,
}
#[derive(Clone, Default, Debug)]
pub struct Solution {
    pub values: Vec<String>,
    pub entities: Vec<Entity>,
    pub score: Option<SimpleScore>,
}
#[derive(Clone, Debug)]
pub struct ExtendedSolution {
    pub base: Solution,
    pub extra_data: String,
}
pub fn problem(values: &[&str], count: usize) -> Solution {
    Solution {
        values: values.iter().map(|s| (*s).into()).collect(),
        entities: vec![Entity::default(); count],
        score: None,
    }
}
pub fn binding() -> PlanningSolutionBinding<Solution, String> {
    PlanningSolutionBinding::default().collection(
        PlanningEntityBinding::default()
            .member(
                "value",
                "valueRange",
                |e: &Entity| e.value.clone(),
                |e, v| e.value = v,
            )
            .unwrap(),
        |s: &Solution| &s.entities,
        |s| &mut s.entities,
        |s, name| (name == "valueRange").then(|| s.values.clone()),
    )
}
pub fn extended_binding() -> PlanningSolutionBinding<ExtendedSolution, String> {
    PlanningSolutionBinding::default().inherit(
        binding(),
        |s: &ExtendedSolution| &s.base,
        |s| &mut s.base,
    )
}
pub trait ValueEntity: Send + Sync {
    fn value(&self) -> Option<i32>;
    fn set_value(&mut self, v: Option<i32>);
    fn clone_entity(&self) -> Box<dyn ValueEntity>;
}
#[derive(Clone, Default)]
pub struct InterfaceImplementation {
    pub value: Option<i32>,
}
impl ValueEntity for InterfaceImplementation {
    fn value(&self) -> Option<i32> {
        self.value
    }
    fn set_value(&mut self, v: Option<i32>) {
        self.value = v;
    }
    fn clone_entity(&self) -> Box<dyn ValueEntity> {
        Box::new(self.clone())
    }
}
impl Clone for Box<dyn ValueEntity> {
    fn clone(&self) -> Self {
        self.clone_entity()
    }
}
#[derive(Clone)]
pub struct InterfaceSolution {
    pub values: Vec<i32>,
    pub entities: Vec<Box<dyn ValueEntity>>,
    pub score: Option<SimpleScore>,
}
pub fn interface_problem() -> InterfaceSolution {
    InterfaceSolution {
        values: vec![0, 1, 2],
        entities: (0..2)
            .map(|_| Box::new(InterfaceImplementation::default()) as Box<dyn ValueEntity>)
            .collect(),
        score: None,
    }
}
pub fn interface_binding() -> PlanningSolutionBinding<InterfaceSolution, i32> {
    PlanningSolutionBinding::default().collection(
        PlanningEntityBinding::<Box<dyn ValueEntity>, i32>::default()
            .member("value", "valueRange", |e| e.value(), |e, v| e.set_value(v))
            .unwrap(),
        |s: &InterfaceSolution| &s.entities,
        |s| &mut s.entities,
        |s, name| (name == "valueRange").then(|| s.values.clone()),
    )
}
#[derive(Default)]
pub struct Audit {
    pub opened: AtomicUsize,
    pub dropped: AtomicUsize,
    pub assigned_failures: AtomicUsize,
}
struct ScoreResource(Arc<Audit>);
impl ScoreResource {
    fn new(a: Arc<Audit>) -> Self {
        a.opened.fetch_add(1, Ordering::SeqCst);
        Self(a)
    }
}
impl Drop for ScoreResource {
    fn drop(&mut self) {
        self.0.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
pub trait Constraint<V>: Send + Sync + 'static {
    fn dense(&self, rows: &[Option<&V>]) -> SimpleScore;
    fn stream<'a>(
        &'a self,
        graph: &mut StreamGraphBuilder<'a, SimpleScore>,
        source: &StreamSource<usize, (usize, Option<V>)>,
    ) -> Result<(), GraphError>
    where
        V: Clone + 'static;
}
pub struct DistinctValues;
impl Constraint<String> for DistinctValues {
    fn dense(&self, rows: &[Option<&String>]) -> SimpleScore {
        let mut score = 0;
        for (i, a) in rows.iter().enumerate() {
            for (j, b) in rows.iter().enumerate() {
                if i != j && a.is_some() && a == b {
                    score -= 1;
                }
            }
        }
        SimpleScore::new(score)
    }
    fn stream<'a>(
        &'a self,
        g: &mut StreamGraphBuilder<'a, SimpleScore>,
        s: &StreamSource<usize, (usize, Option<String>)>,
    ) -> Result<(), GraphError> {
        let join = g.join_equal(s.port(), s.port(), |(_, v)| v.clone(), |(_, v)| v.clone())?;
        let pairs = g.filter(&join, |((i, a), (j, _))| i != j && a.is_some())?;
        g.score("Don't assign 2 entities the same value.", &pairs, |_| {
            SimpleScore::new(-1)
        })?;
        Ok(())
    }
}
pub struct DeliberateFailure(pub Arc<Audit>);
impl Constraint<String> for DeliberateFailure {
    fn dense(&self, rows: &[Option<&String>]) -> SimpleScore {
        for value in rows.iter().flatten() {
            if value.as_str() == FAILING_VALUE {
                self.0.assigned_failures.fetch_add(1, Ordering::SeqCst);
                panic!("{FAILURE_MESSAGE}");
            }
        }
        SimpleScore::ZERO
    }
    fn stream<'a>(
        &'a self,
        g: &mut StreamGraphBuilder<'a, SimpleScore>,
        s: &StreamSource<usize, (usize, Option<String>)>,
    ) -> Result<(), GraphError> {
        let filter = g.filter(s.port(), move |(_, v)| {
            if v.as_deref() == Some(FAILING_VALUE) {
                self.0.assigned_failures.fetch_add(1, Ordering::SeqCst);
                panic!("{FAILURE_MESSAGE}");
            }
            false
        })?;
        g.score("Deliberate failure", &filter, |_| SimpleScore::new(-1))?;
        Ok(())
    }
}
pub struct MinimizeIntegers;
impl Constraint<i32> for MinimizeIntegers {
    fn dense(&self, rows: &[Option<&i32>]) -> SimpleScore {
        SimpleScore::new(-rows.iter().flatten().map(|v| i64::from(**v)).sum::<i64>())
    }
    fn stream<'a>(
        &'a self,
        g: &mut StreamGraphBuilder<'a, SimpleScore>,
        s: &StreamSource<usize, (usize, Option<i32>)>,
    ) -> Result<(), GraphError> {
        let assigned = g.filter(s.port(), |(_, v)| v.is_some())?;
        g.score("Minimize value", &assigned, |(_, v)| {
            SimpleScore::new(-i64::from(v.unwrap()))
        })?;
        Ok(())
    }
}
pub fn configuration() -> SolverSequence<'static> {
    let mut s = SolverSequence::default();
    s.limits.best_score_limit = Some(SimpleScore::ZERO);
    for p in &mut s.phases {
        match p {
            SolverPhase::Construction(c) => c.assert_scores = true,
            SolverPhase::LocalSearch(c) => {
                c.assert_scores = true;
                c.max_steps = 40;
                c.max_evaluations = 100;
            }
            _ => {}
        }
    }
    s
}
pub fn session_with_configuration<S, V, C>(
    config: SolverSequence<'static>,
    binding: PlanningSolutionBinding<S, V>,
    constraint: C,
    stream: bool,
    audit: Arc<Audit>,
    set_score: impl Fn(&mut S, SimpleScore) + Send + Sync + 'static,
) -> ModelSolverSession<S, V, String>
where
    S: Clone + Send + Sync + 'static,
    V: Clone + Eq + 'static,
    C: Constraint<V>,
{
    ModelSolverSession::new(
        binding,
        config,
        move |model, config, _| {
            let model = Rc::new(model);
            let result = if stream {
                let recipe = ScoringFactoryRecipe::stream_recipe(
                    |_| Ok::<_, StreamScoringError>(ScoreResource::new(audit.clone())),
                    |_, _, input| {
                        let mut g = StreamGraphBuilder::new();
                        let source = g.source::<usize, (usize, Option<V>)>();
                        constraint.stream(&mut g, &source)?;
                        StreamScoreDirector::new(&model, g.build(), source, input)
                    },
                    |d: &StreamScoreDirector<'_, '_, V, SimpleScore>| Ok(d.assignment().clone()),
                );
                let prepared = ScoringFactoryDeclaration::default()
                    .with_scoring_recipe(recipe)
                    .prepare_score_factory(
                        model.clone(),
                        Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                        EnvironmentProfile::new(Some(EnvironmentMode::FullAssert), vec![]),
                    )
                    .map_err(|e| e.to_string())?;
                let mut director = prepared
                    .score_factory_builder(model.initial_assignment())
                    .instantiate_score_director()
                    .map_err(|e| e.to_string())?;
                director
                    .factory_resource_mut()
                    .run_solver(config)
                    .map_err(|e| e.to_string())?
            } else {
                let mut network = ConstraintNetwork::<V, SimpleScore>::new(model.variables().len());
                network
                    .add_constraint(
                        "source constraint",
                        (0..model.variables().len()).collect(),
                        |rows| constraint.dense(rows),
                    )
                    .map_err(|e| e.to_string())?;
                let recipe = ScoringFactoryRecipe::incremental_recipe(
                    |_| Ok::<_, ScoringError>((&network, ScoreResource::new(audit.clone()))),
                    |resource: &(&ConstraintNetwork<'_, V, SimpleScore>, ScoreResource),
                     _,
                     input| ScoreDirector::new(&model, resource.0, input),
                    |d: &ScoreDirector<'_, '_, V, SimpleScore>| Ok(d.assignment().clone()),
                );
                let prepared = ScoringFactoryDeclaration::default()
                    .with_scoring_recipe(recipe)
                    .prepare_score_factory(
                        model.clone(),
                        Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                        EnvironmentProfile::new(Some(EnvironmentMode::FullAssert), vec![]),
                    )
                    .map_err(|e| e.to_string())?;
                let mut director = prepared
                    .score_factory_builder(model.initial_assignment())
                    .instantiate_score_director()
                    .map_err(|e| e.to_string())?;
                director
                    .factory_resource_mut()
                    .run_solver(config)
                    .map_err(|e| e.to_string())?
            };
            Ok(result)
        },
        set_score,
    )
    .unwrap()
}
/// Independent model readback: value multiplicities instead of scorer pair loops
/// or stream joins. Include actual unassigned structural state after cancellation.
pub fn duplicate_score(s: &Solution) -> SimpleScore {
    let mut counts = std::collections::BTreeMap::<&str, i64>::new();
    let mut missing = 0;
    for entity in &s.entities {
        if let Some(value) = &entity.value {
            *counts.entry(value).or_default() += 1;
        } else {
            missing += 1;
        }
    }
    SimpleScore::new(-counts.values().map(|n| n * (n - 1)).sum::<i64>()).with_structural(-missing)
}

pub fn session<S, V, C>(
    binding: PlanningSolutionBinding<S, V>,
    constraint: C,
    stream: bool,
    audit: Arc<Audit>,
    set_score: impl Fn(&mut S, SimpleScore) + Send + Sync + 'static,
) -> ModelSolverSession<S, V, String>
where
    S: Clone + Send + Sync + 'static,
    V: Clone + Eq + 'static,
    C: Constraint<V>,
{
    session_with_configuration(
        configuration(),
        binding,
        constraint,
        stream,
        audit,
        set_score,
    )
}
/// A real native abstract provider contract; only its concrete implementation
/// is an executable constructor in the catalog. No abstract-class token exists.
pub trait MultipleProviderBase {
    fn inherited_dense(&self, rows: &[Option<&String>]) -> SimpleScore {
        DistinctValues.dense(rows)
    }
}
pub struct ConcreteMultipleProvider;
impl MultipleProviderBase for ConcreteMultipleProvider {}
impl Constraint<String> for ConcreteMultipleProvider {
    fn dense(&self, rows: &[Option<&String>]) -> SimpleScore {
        self.inherited_dense(rows)
    }
    fn stream<'a>(
        &'a self,
        g: &mut StreamGraphBuilder<'a, SimpleScore>,
        s: &StreamSource<usize, (usize, Option<String>)>,
    ) -> Result<(), GraphError> {
        let join = g.join_equal(s.port(), s.port(), |(_, v)| v.clone(), |(_, v)| v.clone())?;
        let pairs = g.filter(&join, |((i, a), (j, _))| i != j && a.is_some())?;
        g.score_weighted(
            "Don't assign 2 entities the same value.",
            &pairs,
            SimpleScore::new(-1),
            |_| 1,
        )?;
        Ok(())
    }
}
pub fn concrete_provider_bank(stream: bool) -> NamedSolverFactoryBank {
    let schema = binding().schema().unwrap();
    let input_schema = PlanningSchema::new::<Arc<Solution>>(
        schema.entities().map(|e| e.definition().clone()).collect(),
    )
    .unwrap();
    let mut registry = NamedModelFactoryRegistry::default();
    registry
        .register("multiple", Rc::new(input_schema), vec![])
        .unwrap();
    registry
        .register_global_scoring(NamedModelScoring::new(
            "concrete provider",
            move |config, _| {
                Ok::<_, String>(
                    session_with_configuration(
                        config,
                        binding(),
                        ConcreteMultipleProvider,
                        stream,
                        Arc::default(),
                        |s, score| s.score = Some(score),
                    )
                    .factory()
                    .clone(),
                )
            },
        ))
        .unwrap();
    registry
        .prepare([(
            "native".into(),
            NamedSolverConfiguration {
                sequence: configuration(),
                ..Default::default()
            },
        )])
        .unwrap()
}
