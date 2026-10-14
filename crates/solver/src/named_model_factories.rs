//! Whole-set selection over explicit typed model schemas and real solver factories.
use crate::{
    EnvironmentProfile, PlanningSchema, PlanningType, SimpleScore, SolverFactory, SolverPhase,
    SolverSequence, SolverXmlConfiguration,
};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    rc::Rc,
};
type Build<S> =
    dyn Fn(SolverSequence<'static, S>, EnvironmentProfile) -> Result<Box<dyn Any>, String>;
/// A concrete scoring/factory builder. P is the actual owned problem input type,
/// not a caller-supplied type label. The callback must assemble its chosen native
/// scoring recipe and pass the supplied configuration to the existing solver.
/// It builds a factory, never launches a job during registration/preflight.
pub struct NamedModelScoring<S: 'static = SimpleScore> {
    key: String,
    model_type: PlanningType,
    build: Box<Build<S>>,
}
impl<S: 'static> NamedModelScoring<S> {
    pub fn new<P: 'static, R: 'static, E: 'static>(
        key: impl Into<String>,
        build: impl Fn(
            SolverSequence<'static, S>,
            EnvironmentProfile,
        ) -> Result<SolverFactory<P, R, E, S>, String>
        + 'static,
    ) -> Self {
        Self {
            key: key.into(),
            model_type: PlanningType::of::<P>(),
            build: Box::new(move |config, profile| {
                build(config, profile).map(|f| Box::new(f) as Box<dyn Any>)
            }),
        }
    }
}
#[derive(Clone, Debug)]
pub struct NamedSolverConfiguration<S: 'static = SimpleScore> {
    pub model: Option<String>,
    pub scoring: Option<String>,
    pub sequence: SolverSequence<'static, S>,
    pub environment: EnvironmentProfile,
}
impl<S: 'static> Default for NamedSolverConfiguration<S> {
    fn default() -> Self {
        Self {
            model: None,
            scoring: None,
            sequence: SolverSequence::default(),
            environment: EnvironmentProfile::default(),
        }
    }
}
impl<S: 'static> NamedSolverConfiguration<S> {
    /// Read explicit registered model/scoring keys from declarative XML. This
    /// does not load Java classes or interpret unrelated solver declarations.
    pub fn apply_xml_selection(
        &mut self,
        xml: &SolverXmlConfiguration,
    ) -> Result<(), NamedModelFactoryError> {
        let root = xml.root();
        let model = root.value("solutionClass").map(str::to_owned);
        let scores: Vec<_> = root
            .child("scoreDirectorFactory")
            .into_iter()
            .flat_map(|director| {
                [
                    "easyScoreCalculatorClass",
                    "constraintProviderClass",
                    "incrementalScoreCalculatorClass",
                ]
                .into_iter()
                .filter_map(move |key| director.value(key))
            })
            .collect();
        if scores.len() > 1 {
            return Err(NamedModelFactoryError::Configuration {
                solver: "XML selection".into(),
                reason: "multiple scoring selection declarations".into(),
            });
        }
        if let Some(model) = model {
            self.model = Some(model);
        }
        if let Some(score) = scores.first() {
            self.scoring = Some((*score).to_owned());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NamedModelFactoryError {
    EmptyName(&'static str),
    DuplicateModel(String),
    DuplicateScoring {
        model: String,
        scoring: String,
    },
    MissingModels,
    MissingEntities(String),
    ModelType {
        model: String,
        registered: PlanningType,
        recipe: PlanningType,
    },
    DuplicateSolver(String),
    MissingModel {
        solver: String,
        model: String,
    },
    AmbiguousModels {
        solvers: Vec<String>,
        models: Vec<String>,
    },
    MissingScoring {
        solver: String,
        model: String,
        scoring: Option<String>,
    },
    AmbiguousScoring {
        model: String,
        solvers: Vec<String>,
        scorings: Vec<String>,
    },
    UnusedModels(Vec<String>),
    UnusedScoring {
        model: String,
        scorings: Vec<String>,
    },
    Configuration {
        solver: String,
        reason: String,
    },
    Build {
        solver: String,
        reason: String,
    },
    UnknownSolver(String),
    AmbiguousSolver(Vec<String>),
    FactoryType(String),
}
impl fmt::Display for NamedModelFactoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingModels => f.write_str("no registered planning models"),
            Self::MissingEntities(model) => write!(
                f,
                "model {model} has no registered genuine planning entities"
            ),
            Self::AmbiguousModels { solvers, models } => write!(
                f,
                "solver configs {solvers:?} omit a model selection with multiple registered models {models:?}"
            ),
            Self::AmbiguousScoring {
                model,
                solvers,
                scorings,
            } => write!(
                f,
                "solver configs {solvers:?} omit scoring selection for model {model} with multiple recipes {scorings:?}"
            ),
            Self::UnusedModels(names) => write!(f, "unused registered models {names:?}"),
            Self::UnusedScoring { model, scorings } => {
                write!(f, "unused scoring recipes for {model}: {scorings:?}")
            }
            _ => write!(f, "named model factory: {self:?}"),
        }
    }
}
impl Error for NamedModelFactoryError {}
struct Model<S: 'static> {
    schema: Rc<PlanningSchema>,
    scorings: BTreeMap<String, NamedModelScoring<S>>,
}
/// Explicit native registrations; no package scans, reflection or class loading.
/// Each scoring recipe is tied to the same real owned input type as its schema.
/// Preparation requires every registered model and recipe to be selected at least
/// once, catching accidental unused registrations before any builder callback.
pub struct NamedModelFactoryRegistry<S: 'static = SimpleScore> {
    models: BTreeMap<String, Model<S>>,
    global_scorings: BTreeMap<String, NamedModelScoring<S>>,
}
impl<S: 'static> Default for NamedModelFactoryRegistry<S> {
    fn default() -> Self {
        Self {
            models: BTreeMap::new(),
            global_scorings: BTreeMap::new(),
        }
    }
}
impl<S: 'static> NamedModelFactoryRegistry<S> {
    pub fn register(
        &mut self,
        name: impl Into<String>,
        schema: Rc<PlanningSchema>,
        scorings: Vec<NamedModelScoring<S>>,
    ) -> Result<(), NamedModelFactoryError> {
        let name = name.into();
        if name.is_empty() {
            return Err(NamedModelFactoryError::EmptyName("model"));
        }
        if self.models.contains_key(&name) {
            return Err(NamedModelFactoryError::DuplicateModel(name));
        }
        let mut recipes = BTreeMap::new();
        for recipe in scorings {
            if recipe.key.is_empty() {
                return Err(NamedModelFactoryError::EmptyName("scoring"));
            }
            if recipe.model_type != schema.solution_type() {
                return Err(NamedModelFactoryError::ModelType {
                    model: name,
                    registered: schema.solution_type(),
                    recipe: recipe.model_type,
                });
            }
            if recipes.contains_key(&recipe.key) {
                return Err(NamedModelFactoryError::DuplicateScoring {
                    model: name,
                    scoring: recipe.key,
                });
            }
            recipes.insert(recipe.key.clone(), recipe);
        }
        self.models.insert(
            name,
            Model {
                schema,
                scorings: recipes,
            },
        );
        Ok(())
    }
    /// Optional discovered heterogeneous scoring catalog. Actual input types and
    /// executable builders are retained. Model-owned recipes take precedence;
    /// catalog ambiguity is resolved before selected recipe type validation.
    pub fn register_global_scoring(
        &mut self,
        recipe: NamedModelScoring<S>,
    ) -> Result<(), NamedModelFactoryError> {
        if recipe.key.is_empty() {
            return Err(NamedModelFactoryError::EmptyName("scoring"));
        }
        if self.global_scorings.contains_key(&recipe.key) {
            return Err(NamedModelFactoryError::DuplicateScoring {
                model: "global".into(),
                scoring: recipe.key,
            });
        }
        self.global_scorings.insert(recipe.key.clone(), recipe);
        Ok(())
    }
    /// All membership, ambiguity, unused, schema and common configuration checks
    /// finish before the first factory builder. Successful factories publish only
    /// after every builder succeeds. Builders validate their own scoring/model
    /// specifics; a failed builder publishes no partial bank.
    pub fn prepare(
        &self,
        configs: impl IntoIterator<Item = (String, NamedSolverConfiguration<S>)>,
    ) -> Result<NamedSolverFactoryBank, NamedModelFactoryError> {
        let mut requests = BTreeMap::new();
        for (name, c) in configs {
            if name.is_empty() {
                return Err(NamedModelFactoryError::EmptyName("solver"));
            }
            if requests.contains_key(&name) {
                return Err(NamedModelFactoryError::DuplicateSolver(name));
            }
            requests.insert(name, c);
        }
        if self.models.is_empty() {
            return Err(NamedModelFactoryError::MissingModels);
        }
        let missing: Vec<_> = requests
            .iter()
            .filter(|(_, c)| c.model.is_none())
            .map(|(n, _)| n.clone())
            .collect();
        if self.models.len() > 1 && !missing.is_empty() {
            return Err(NamedModelFactoryError::AmbiguousModels {
                solvers: missing,
                models: self.models.keys().cloned().collect(),
            });
        }
        let mut selected = BTreeMap::new();
        let mut used = BTreeMap::<String, BTreeSet<String>>::new();
        let mut global_used = BTreeSet::new();
        for (solver, c) in &requests {
            let model = c
                .model
                .as_ref()
                .unwrap_or_else(|| self.models.keys().next().unwrap());
            let m = self
                .models
                .get(model)
                .ok_or_else(|| NamedModelFactoryError::MissingModel {
                    solver: solver.clone(),
                    model: model.clone(),
                })?;
            if m.schema.genuine_entities().next().is_none() {
                return Err(NamedModelFactoryError::MissingEntities(model.clone()));
            }
            let recipes = if m.scorings.is_empty() {
                &self.global_scorings
            } else {
                &m.scorings
            };
            if recipes.is_empty() {
                return Err(NamedModelFactoryError::MissingScoring {
                    solver: solver.clone(),
                    model: model.clone(),
                    scoring: c.scoring.clone(),
                });
            }
            if c.scoring.is_none() && recipes.len() > 1 {
                let solvers = requests
                    .iter()
                    .filter(|(_, r)| {
                        r.scoring.is_none()
                            && (r.model.as_ref() == Some(model)
                                || r.model.is_none() && self.models.len() == 1)
                    })
                    .map(|(n, _)| n.clone())
                    .collect();
                return Err(NamedModelFactoryError::AmbiguousScoring {
                    model: model.clone(),
                    solvers,
                    scorings: recipes.keys().cloned().collect(),
                });
            }
            let scoring = c
                .scoring
                .as_ref()
                .unwrap_or_else(|| recipes.keys().next().unwrap());
            if !recipes.contains_key(scoring) {
                return Err(NamedModelFactoryError::MissingScoring {
                    solver: solver.clone(),
                    model: model.clone(),
                    scoring: Some(scoring.clone()),
                });
            }
            let recipe = &recipes[scoring];
            if recipe.model_type != m.schema.solution_type() {
                return Err(NamedModelFactoryError::ModelType {
                    model: model.clone(),
                    registered: m.schema.solution_type(),
                    recipe: recipe.model_type,
                });
            }
            if m.scorings.is_empty() {
                global_used.insert(scoring.clone());
            }
            validate_configuration(solver, c)?;
            selected.insert(solver.clone(), (model.clone(), scoring.clone()));
            used.entry(model.clone())
                .or_default()
                .insert(scoring.clone());
        }
        let unused: Vec<_> = self
            .models
            .keys()
            .filter(|n| !used.contains_key(*n))
            .cloned()
            .collect();
        if !unused.is_empty() {
            return Err(NamedModelFactoryError::UnusedModels(unused));
        }
        for (name, model) in &self.models {
            let unused: Vec<_> = model
                .scorings
                .keys()
                .filter(|k| !used[name].contains(*k))
                .cloned()
                .collect();
            if !unused.is_empty() {
                return Err(NamedModelFactoryError::UnusedScoring {
                    model: name.clone(),
                    scorings: unused,
                });
            }
        }
        let global_unused: Vec<_> = self
            .global_scorings
            .keys()
            .filter(|key| !global_used.contains(*key))
            .cloned()
            .collect();
        if !global_unused.is_empty() {
            return Err(NamedModelFactoryError::UnusedScoring {
                model: "global".into(),
                scorings: global_unused,
            });
        }
        let mut factories = BTreeMap::new();
        for (name, c) in requests {
            let (model, scoring) = &selected[&name];
            let m = &self.models[model];
            let recipes = if m.scorings.is_empty() {
                &self.global_scorings
            } else {
                &m.scorings
            };
            let factory =
                (recipes[scoring].build)(c.sequence, c.environment).map_err(|reason| {
                    NamedModelFactoryError::Build {
                        solver: name.clone(),
                        reason,
                    }
                })?;
            factories.insert(
                name,
                NamedPreparedFactory {
                    schema: m.schema.clone(),
                    model: model.clone(),
                    scoring: scoring.clone(),
                    factory,
                },
            );
        }
        Ok(NamedSolverFactoryBank { factories })
    }
}
fn validate_configuration<S>(
    name: &str,
    c: &NamedSolverConfiguration<S>,
) -> Result<(), NamedModelFactoryError> {
    let error = |reason: String| NamedModelFactoryError::Configuration {
        solver: name.into(),
        reason,
    };
    c.environment.validate().map_err(|e| error(e.to_string()))?;
    if c.sequence.phases.is_empty() {
        return Err(error("at least one phase is required".into()));
    }
    if c.sequence.limits.cancellation.is_some() {
        return Err(error("cancellation belongs to each job".into()));
    }
    for (i, p) in c.sequence.phases.iter().enumerate() {
        let token = match p {
            SolverPhase::Construction(c) => c.cancellation,
            SolverPhase::LocalSearch(c) => c.cancellation,
            SolverPhase::Exact(c) => c.cancellation,
            SolverPhase::Custom(c) => c.cancellation,
            SolverPhase::RuinRecreate(c) => c.cancellation,
        };
        if token.is_some() {
            return Err(error(format!("phase {i} cancellation belongs to each job")));
        }
    }
    Ok(())
}
struct NamedPreparedFactory {
    schema: Rc<PlanningSchema>,
    model: String,
    scoring: String,
    factory: Box<dyn Any>,
}
/// Heterogeneous typed factory lookup. The input/result/error/score types must
/// match the actual registered factory; diagnostic names never establish identity.
pub struct NamedSolverFactoryBank {
    factories: BTreeMap<String, NamedPreparedFactory>,
}
impl NamedSolverFactoryBank {
    pub fn names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.factories.keys().map(String::as_str)
    }
    pub fn selection(
        &self,
        name: &str,
    ) -> Result<(&str, &str, &PlanningSchema), NamedModelFactoryError> {
        let f = self
            .factories
            .get(name)
            .ok_or_else(|| NamedModelFactoryError::UnknownSolver(name.into()))?;
        Ok((&f.model, &f.scoring, &f.schema))
    }
    pub fn factory<P: 'static, R: 'static, E: 'static, S: 'static>(
        &self,
        name: &str,
    ) -> Result<&SolverFactory<P, R, E, S>, NamedModelFactoryError> {
        self.factories
            .get(name)
            .ok_or_else(|| NamedModelFactoryError::UnknownSolver(name.into()))?
            .factory
            .downcast_ref()
            .ok_or_else(|| NamedModelFactoryError::FactoryType(name.into()))
    }
    pub fn unique_factory<P: 'static, R: 'static, E: 'static, S: 'static>(
        &self,
    ) -> Result<&SolverFactory<P, R, E, S>, NamedModelFactoryError> {
        if self.factories.len() != 1 {
            return Err(NamedModelFactoryError::AmbiguousSolver(
                self.names().map(str::to_owned).collect(),
            ));
        }
        self.factory(self.names().next().unwrap())
    }
}
