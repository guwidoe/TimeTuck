// Adapted Community inheritance TEST helpers; Apache-2.0.
#![allow(dead_code)]
use std::rc::Rc;
use timetuck::*;
#[derive(Clone, Debug, Default)]
pub struct BaseEntity {
    pub id: usize,
    pub value: Option<String>,
}
#[derive(Clone, Debug, Default)]
pub struct ChildEntity {
    pub base: BaseEntity,
    pub extra: Option<String>,
}
pub trait ValueEntity {
    fn value(&self) -> Option<String>;
    fn set_value(&mut self, v: Option<String>);
}
#[derive(Clone, Debug, Default)]
pub struct InterfaceEntity {
    pub id: usize,
    pub value: Option<String>,
    pub extra: Option<String>,
}
impl ValueEntity for InterfaceEntity {
    fn value(&self) -> Option<String> {
        self.value.clone()
    }
    fn set_value(&mut self, v: Option<String>) {
        self.value = v
    }
}
pub fn base_binding() -> PlanningEntityBinding<BaseEntity, String> {
    PlanningEntityBinding::default()
        .member(
            "value",
            "valueRange",
            |e: &BaseEntity| e.value.clone(),
            |e, v| e.value = v,
        )
        .unwrap()
}
pub fn child_binding(extra: Option<&str>) -> PlanningEntityBinding<ChildEntity, String> {
    let b = PlanningEntityBinding::default()
        .inherit(base_binding(), |e: &ChildEntity| &e.base, |e| &mut e.base)
        .unwrap();
    match extra {
        Some(name) => b
            .member(
                name,
                if name == "subValue" {
                    "subValueRange"
                } else {
                    "valueRange2"
                },
                |e| e.extra.clone(),
                |e, v| e.extra = v,
            )
            .unwrap(),
        None => b,
    }
}
pub fn trait_binding<E: ValueEntity + 'static>() -> PlanningEntityBinding<E, String> {
    PlanningEntityBinding::default()
        .member("value", "valueRange", E::value, E::set_value)
        .unwrap()
}
pub fn interface_binding(extra: bool) -> PlanningEntityBinding<InterfaceEntity, String> {
    let b = trait_binding();
    if extra {
        b.member(
            "value2",
            "valueRange2",
            |e: &InterfaceEntity| e.extra.clone(),
            |e, v| e.extra = v,
        )
        .unwrap()
    } else {
        b
    }
}
#[derive(Clone, Debug, Default)]
pub struct BaseSolution {
    pub values: Vec<String>,
    pub extra_values: Vec<String>,
    pub entities: Vec<ChildEntity>,
    pub score: Option<SimpleScore>,
}
#[derive(Clone, Debug, Default)]
pub struct ExtendedSolution {
    pub base: BaseSolution,
    pub second: Vec<ChildEntity>,
    pub objects: Vec<ChildEntity>,
    pub raw: Vec<ChildEntity>,
}
#[derive(Clone, Debug, Default)]
pub struct InterfaceSolution {
    pub values: Vec<String>,
    pub extra_values: Vec<String>,
    pub entities: Vec<InterfaceEntity>,
    pub score: Option<SimpleScore>,
}
pub fn generated(count: usize, label: &str) -> Vec<String> {
    (0..count)
        .map(|i| format!("Generated {label} {i}"))
        .collect()
}
pub fn solution(values: usize, entities: usize, extra_label: &str) -> BaseSolution {
    BaseSolution {
        values: generated(values, "Value"),
        extra_values: generated(values, extra_label),
        entities: (0..entities)
            .map(|id| ChildEntity {
                base: BaseEntity { id, value: None },
                extra: None,
            })
            .collect(),
        score: None,
    }
}
fn range(s: &BaseSolution, id: &str) -> Option<Vec<String>> {
    match id {
        "valueRange" => Some(s.values.clone()),
        "valueRange2" | "subValueRange" => Some(s.extra_values.clone()),
        _ => None,
    }
}
pub fn solution_binding(extra: Option<&str>) -> PlanningSolutionBinding<BaseSolution, String> {
    PlanningSolutionBinding::default().collection(
        child_binding(extra),
        |s: &BaseSolution| &s.entities,
        |s| &mut s.entities,
        range,
    )
}
pub fn extended_binding(
    collections: usize,
    extra: Option<&str>,
) -> PlanningSolutionBinding<ExtendedSolution, String> {
    let mut b = PlanningSolutionBinding::default().inherit(
        solution_binding(extra),
        |s: &ExtendedSolution| &s.base,
        |s| &mut s.base,
    );
    if collections > 1 {
        b = b.collection(
            child_binding(extra),
            |s| &s.second,
            |s| &mut s.second,
            |s, id| range(&s.base, id),
        )
    }
    if collections > 2 {
        b = b
            .collection(
                child_binding(extra),
                |s| &s.objects,
                |s| &mut s.objects,
                |s, id| range(&s.base, id),
            )
            .collection(
                child_binding(extra),
                |s| &s.raw,
                |s| &mut s.raw,
                |s, id| range(&s.base, id),
            )
    }
    b
}
pub fn interface_solution(values: usize, entities: usize) -> InterfaceSolution {
    InterfaceSolution {
        values: generated(values, "Value"),
        extra_values: generated(values, "Value2"),
        entities: (0..entities)
            .map(|id| InterfaceEntity {
                id,
                ..Default::default()
            })
            .collect(),
        score: None,
    }
}
pub fn interface_solution_binding(
    extra: bool,
) -> PlanningSolutionBinding<InterfaceSolution, String> {
    PlanningSolutionBinding::default().collection(
        interface_binding(extra),
        |s: &InterfaceSolution| &s.entities,
        |s| &mut s.entities,
        |s, id| match id {
            "valueRange" => Some(s.values.clone()),
            "valueRange2" => Some(s.extra_values.clone()),
            _ => None,
        },
    )
}
/// A real factory owns the supplied model and constructs a fresh prepared director
/// inside each job. Constraint scopes are flattened field ordinals from the layout.
pub fn build_factory<S: OwnedScore + Send + Sync + 'static>(
    binding: impl Fn() -> PlanningSolutionBinding<S, String> + Send + Sync + 'static,
    rewarded: Vec<usize>,
    multiplicity: i64,
    goal: SimpleScore,
) -> Result<SolverFactory<S, S, String>, SolverFactoryError> {
    let mut config = SolverSequence::default();
    config.limits.best_score_limit = Some(goal);
    for p in &mut config.phases {
        if let SolverPhase::Construction(c) = p {
            c.assert_scores = true
        }
        if let SolverPhase::LocalSearch(c) = p {
            c.max_steps = 40;
            c.max_evaluations = 80;
            c.assert_scores = true
        }
    }
    SolverFactory::new(config, move |mut input: S, config, progress| {
        let layout = binding();
        let model = Rc::new(layout.open(&input).map_err(|e| e.to_string())?);
        let mut network = ConstraintNetwork::<String, SimpleScore>::new(model.variables().len());
        let width = layout
            .schema()
            .unwrap()
            .genuine_entities()
            .next()
            .unwrap()
            .genuine_variables()
            .count();
        let rewarded = rewarded.clone();
        network
            .add_constraint(
                "assigned inherited values",
                (0..model.variables().len()).collect(),
                move |values| {
                    SimpleScore::new(
                        rewarded
                            .iter()
                            .filter(|&&i| {
                                values[i / width * width..(i / width + 1) * width]
                                    .iter()
                                    .all(|v| v.is_some())
                            })
                            .count() as i64
                            * multiplicity,
                    )
                },
            )
            .map_err(|e| e.to_string())?;
        let recipe = ScoringFactoryRecipe::incremental_recipe(
            |_| Ok::<_, ScoringError>(&network),
            |n: &&ConstraintNetwork<'_, String, SimpleScore>, _, a| {
                ScoreDirector::new(&model, n, a)
            },
            |d: &ScoreDirector<'_, '_, String, SimpleScore>| Ok(d.assignment().clone()),
        );
        let f = ScoringFactoryDeclaration::default()
            .with_scoring_recipe(recipe)
            .prepare_score_factory(
                model.clone(),
                Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
                EnvironmentProfile::new(Some(EnvironmentMode::FullAssert), vec![]),
            )
            .map_err(|e| e.to_string())?;
        let mut d = f
            .score_factory_builder(model.initial_assignment())
            .instantiate_score_director()
            .map_err(|e| e.to_string())?;
        let result = d
            .factory_resource_mut()
            .run_solver(config)
            .map_err(|e| e.to_string())?;
        assert!(result.complete);
        layout
            .commit(&mut input, &result.assignment)
            .map_err(|e| e.to_string())?;
        input.set_score(result.score.unwrap());
        progress.record_work(result.work);
        Ok::<_, String>(input)
    })
}
pub fn solve<S: OwnedScore + Send + Sync + 'static>(
    solution: S,
    binding: impl Fn() -> PlanningSolutionBinding<S, String> + Send + Sync + 'static,
    rewarded: Vec<usize>,
    multiplicity: i64,
) -> S {
    let factory = build_factory(binding, rewarded, multiplicity, SimpleScore::ZERO).unwrap();
    let mut manager = factory.build_manager(1).unwrap();
    let job = manager.job_builder(1).with_problem(solution).run().unwrap();
    // Job result uses Arc; unwrap after dropping job and shutting down manager.
    let result = job.wait();
    drop(job);
    manager.shutdown().unwrap();
    drop(manager);
    drop(factory);
    match std::sync::Arc::try_unwrap(result).ok().unwrap() {
        Ok(s) => s,
        Err(e) => panic!("{e}"),
    }
}
pub fn class_score(s: &BaseSolution, extra_reward: bool) -> i64 {
    s.entities
        .iter()
        .map(|e| {
            i64::from(e.base.value.is_some())
                + if extra_reward {
                    i64::from(e.extra.is_some())
                } else {
                    0
                }
        })
        .sum()
}
pub fn interface_score(s: &InterfaceSolution, extra_reward: bool) -> i64 {
    s.entities
        .iter()
        .map(|e| {
            i64::from(e.value().is_some())
                + if extra_reward {
                    i64::from(e.extra.is_some())
                } else {
                    0
                }
        })
        .sum()
}

pub trait OwnedScore {
    fn set_score(&mut self, score: SimpleScore);
}
impl OwnedScore for BaseSolution {
    fn set_score(&mut self, s: SimpleScore) {
        self.score = Some(s)
    }
}
impl OwnedScore for ExtendedSolution {
    fn set_score(&mut self, s: SimpleScore) {
        self.base.score = Some(s)
    }
}
impl OwnedScore for InterfaceSolution {
    fn set_score(&mut self, s: SimpleScore) {
        self.score = Some(s)
    }
}
