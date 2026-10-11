//! Prepared scoring recipes shared by independent native runtime resources.
pub(crate) mod solver_assembly;
use crate::{
    ContextMatchPolicy, ContextMetrics, EnvironmentMode, EnvironmentProfile,
    EnvironmentProfileError, InitializingScoreTrend, ScoreDefinition, ScoreTrendError,
};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
    rc::Rc,
};

/// Selection of an actual recipe. MultiModeStream is a view over a stream recipe
/// whose declared phases request another mode, not another scoring implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoringFactoryStrategy {
    FullEvaluation,
    Incremental,
    Stream,
    MultiModeStream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FactoryDirectorSettings {
    pub requested_mode: EnvironmentMode,
    pub matching: ContextMatchPolicy,
}

/// Explicit builder policy. Factory construction itself does not turn reporting
/// on. Full assertion modes or constraint-total metrics request named matches.
/// This policy is separate from retained ScoringContextManager settings.
pub fn decide_score_factory_matching(
    mode: EnvironmentMode,
    metrics: ContextMetrics,
) -> ContextMatchPolicy {
    if metrics.best_constraint_totals
        || matches!(
            mode,
            EnvironmentMode::NonIntrusiveFullAssert
                | EnvironmentMode::FullAssert
                | EnvironmentMode::TrackedFullAssert
        )
    {
        ContextMatchPolicy::Enabled
    } else {
        ContextMatchPolicy::Disabled
    }
}

/// Shared native score-shape identity, independent of any working score/cache.
#[derive(Debug)]
pub struct FactoryScoreShape {
    labels: Vec<String>,
    hard_levels: usize,
}
impl FactoryScoreShape {
    pub fn for_score<S: ScoreDefinition>() -> Self {
        Self {
            labels: S::level_labels(),
            hard_levels: S::feasible_level_count(),
        }
    }
    pub fn factory_level_labels(&self) -> &[String] {
        &self.labels
    }
    pub fn factory_hard_levels(&self) -> usize {
        self.hard_levels
    }
}

#[derive(Debug)]
pub enum ScoreFactoryBuildError<E> {
    MissingRecipe,
    ConflictingRecipes,
    NestedAssertion,
    AssertionRequiresStepAssert,
    EmptyScoreShape,
    Trend(ScoreTrendError),
    Profile(EnvironmentProfileError),
    RecursivePreparation { mode: EnvironmentMode },
    Preparation { mode: EnvironmentMode, error: E },
    Instantiation { mode: EnvironmentMode, error: E },
    ChildInput(E),
}
impl<E: fmt::Display> fmt::Display for ScoreFactoryBuildError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRecipe => f.write_str("scoring factory lacks configuration"),
            Self::ConflictingRecipes => {
                f.write_str("scoring factory recipes cannot be supplied together")
            }
            Self::NestedAssertion => f.write_str(
                "assertion scoring factory cannot have a non-null assertion scoring factory",
            ),
            Self::AssertionRequiresStepAssert => f.write_str(
                "assertion scoring factory requires an environmentMode of StepAssert or stronger",
            ),
            Self::EmptyScoreShape => {
                f.write_str("scoring factory requires at least one score level")
            }
            Self::Trend(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::RecursivePreparation { mode } => {
                write!(f, "recursive scoring preparation for {mode:?}")
            }
            Self::Preparation { mode, error } => {
                write!(f, "scoring preparation for {mode:?}: {error}")
            }
            Self::Instantiation { mode, error } => {
                write!(f, "scoring instantiation for {mode:?}: {error}")
            }
            Self::ChildInput(e) => write!(f, "scoring child working state: {e}"),
        }
    }
}
impl<E: Error + 'static> Error for ScoreFactoryBuildError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Trend(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Preparation { error, .. }
            | Self::Instantiation { error, .. }
            | Self::ChildInput(error) => Some(error),
            _ => None,
        }
    }
}
type PreparationFn<'a, P, E> = dyn Fn(EnvironmentMode) -> Result<P, E> + 'a;
type InstantiationFn<'a, P, I, D, E> = dyn Fn(&P, FactoryDirectorSettings, I) -> Result<D, E> + 'a;
type ChildInputFn<'a, I, D, E> = dyn Fn(&D) -> Result<I, E> + 'a;

/// Preparation builds immutable scoring data. Instantiation allocates independent
/// mutable runtime state using that data and the supplied working input. The child
/// callback snapshots the parent's *current* working input, not its initial one.
/// Callbacks may borrow external models/networks; no Clone bound is imposed on
/// preparations or directors. They must not put shared mutable scoring state in P.
/// No reflection or Java calculator-property injection is performed.
pub struct ScoringFactoryRecipe<'a, P, I, D, E> {
    strategy: ScoringFactoryStrategy,
    prepare: Box<PreparationFn<'a, P, E>>,
    instantiate: Box<InstantiationFn<'a, P, I, D, E>>,
    child_input: Box<ChildInputFn<'a, I, D, E>>,
}
impl<'a, P, I, D, E> ScoringFactoryRecipe<'a, P, I, D, E> {
    fn recipe(
        strategy: ScoringFactoryStrategy,
        prepare: impl Fn(EnvironmentMode) -> Result<P, E> + 'a,
        instantiate: impl Fn(&P, FactoryDirectorSettings, I) -> Result<D, E> + 'a,
        child_input: impl Fn(&D) -> Result<I, E> + 'a,
    ) -> Self {
        Self {
            strategy,
            prepare: Box::new(prepare),
            instantiate: Box::new(instantiate),
            child_input: Box::new(child_input),
        }
    }
    pub fn full_evaluation_recipe(
        prepare: impl Fn(EnvironmentMode) -> Result<P, E> + 'a,
        instantiate: impl Fn(&P, FactoryDirectorSettings, I) -> Result<D, E> + 'a,
        child_input: impl Fn(&D) -> Result<I, E> + 'a,
    ) -> Self {
        Self::recipe(
            ScoringFactoryStrategy::FullEvaluation,
            prepare,
            instantiate,
            child_input,
        )
    }
    pub fn incremental_recipe(
        prepare: impl Fn(EnvironmentMode) -> Result<P, E> + 'a,
        instantiate: impl Fn(&P, FactoryDirectorSettings, I) -> Result<D, E> + 'a,
        child_input: impl Fn(&D) -> Result<I, E> + 'a,
    ) -> Self {
        Self::recipe(
            ScoringFactoryStrategy::Incremental,
            prepare,
            instantiate,
            child_input,
        )
    }
    pub fn stream_recipe(
        prepare: impl Fn(EnvironmentMode) -> Result<P, E> + 'a,
        instantiate: impl Fn(&P, FactoryDirectorSettings, I) -> Result<D, E> + 'a,
        child_input: impl Fn(&D) -> Result<I, E> + 'a,
    ) -> Self {
        Self::recipe(
            ScoringFactoryStrategy::Stream,
            prepare,
            instantiate,
            child_input,
        )
    }
}

/// Exactly one recipe is required. Invalid configuration is rejected before any
/// user preparation runs. An assertion declaration must be concrete/non-nested;
/// its default director mode is NonReproducible as an independent checking route.
pub struct ScoringFactoryDeclaration<'a, P, I, D, E> {
    pub recipes: Vec<ScoringFactoryRecipe<'a, P, I, D, E>>,
    pub trend: Option<String>,
    pub assertion: Option<Box<Self>>,
}
impl<P, I, D, E> Default for ScoringFactoryDeclaration<'_, P, I, D, E> {
    fn default() -> Self {
        Self {
            recipes: vec![],
            trend: None,
            assertion: None,
        }
    }
}
impl<'a, P, I, D, E> ScoringFactoryDeclaration<'a, P, I, D, E> {
    pub fn with_scoring_recipe(mut self, recipe: ScoringFactoryRecipe<'a, P, I, D, E>) -> Self {
        self.recipes.push(recipe);
        self
    }
    fn validate_factory_declaration(
        &self,
        mode: EnvironmentMode,
        levels: usize,
    ) -> Result<(), ScoreFactoryBuildError<E>> {
        match self.recipes.len() {
            0 => return Err(ScoreFactoryBuildError::MissingRecipe),
            1 => {}
            _ => return Err(ScoreFactoryBuildError::ConflictingRecipes),
        }
        if levels == 0 {
            return Err(ScoreFactoryBuildError::EmptyScoreShape);
        }
        InitializingScoreTrend::parse(self.trend.as_deref().unwrap_or("ANY"), levels)
            .map_err(ScoreFactoryBuildError::Trend)?;
        if let Some(assertion) = &self.assertion {
            if assertion.assertion.is_some() {
                return Err(ScoreFactoryBuildError::NestedAssertion);
            }
            if mode < EnvironmentMode::StepAssert {
                return Err(ScoreFactoryBuildError::AssertionRequiresStepAssert);
            }
            assertion.validate_factory_declaration(EnvironmentMode::NonReproducible, levels)?;
        }
        Ok(())
    }
    pub fn prepare_score_factory<M>(
        self,
        model: Rc<M>,
        shape: Rc<FactoryScoreShape>,
        profile: EnvironmentProfile,
    ) -> Result<PreparedScoreFactory<'a, M, P, I, D, E>, ScoreFactoryBuildError<E>> {
        profile
            .validate()
            .map_err(ScoreFactoryBuildError::Profile)?;
        self.validate_factory_declaration(profile.global_mode(), shape.labels.len())?;
        self.prepare_validated_factory(model, shape, profile)
    }
    fn prepare_validated_factory<M>(
        self,
        model: Rc<M>,
        shape: Rc<FactoryScoreShape>,
        profile: EnvironmentProfile,
    ) -> Result<PreparedScoreFactory<'a, M, P, I, D, E>, ScoreFactoryBuildError<E>> {
        let trend = InitializingScoreTrend::parse(
            self.trend.as_deref().unwrap_or("ANY"),
            shape.labels.len(),
        )
        .map_err(ScoreFactoryBuildError::Trend)?;
        let assertion = self
            .assertion
            .map(|a| {
                a.prepare_validated_factory(
                    model.clone(),
                    shape.clone(),
                    EnvironmentProfile::new(Some(EnvironmentMode::NonReproducible), vec![]),
                )
            })
            .transpose()?;
        let recipe = self.recipes.into_iter().next().expect("validated recipe");
        let multi_mode = recipe.strategy == ScoringFactoryStrategy::Stream
            && profile.overrides().next().is_some();
        let registry = Rc::new(FactoryRegistry {
            recipe,
            model,
            shape,
            trend,
            assertion,
            global: profile.global_mode(),
            multi_mode,
            cache: RefCell::new(BTreeMap::new()),
            preparing: RefCell::new(BTreeSet::new()),
        });
        let prepared = registry.prepare_scoring_mode(registry.global)?;
        Ok(PreparedScoreFactory { registry, prepared })
    }
}
struct PreparedMode<P> {
    mode: EnvironmentMode,
    data: P,
}
struct FactoryRegistry<'a, M, P, I, D, E> {
    recipe: ScoringFactoryRecipe<'a, P, I, D, E>,
    model: Rc<M>,
    shape: Rc<FactoryScoreShape>,
    trend: InitializingScoreTrend,
    global: EnvironmentMode,
    multi_mode: bool,
    assertion: Option<PreparedScoreFactory<'a, M, P, I, D, E>>,
    cache: RefCell<BTreeMap<EnvironmentMode, Rc<PreparedMode<P>>>>,
    preparing: RefCell<BTreeSet<EnvironmentMode>>,
}
struct PreparationGuard<'a> {
    keys: &'a RefCell<BTreeSet<EnvironmentMode>>,
    mode: EnvironmentMode,
}
impl Drop for PreparationGuard<'_> {
    fn drop(&mut self) {
        self.keys.borrow_mut().remove(&self.mode);
    }
}
impl<M, P, I, D, E> FactoryRegistry<'_, M, P, I, D, E> {
    fn prepare_scoring_mode(
        &self,
        requested: EnvironmentMode,
    ) -> Result<Rc<PreparedMode<P>>, ScoreFactoryBuildError<E>> {
        let mode = if self.recipe.strategy == ScoringFactoryStrategy::Stream {
            requested
        } else {
            self.global
        };
        if let Some(p) = self.cache.borrow().get(&mode) {
            return Ok(p.clone());
        }
        if !self.preparing.borrow_mut().insert(mode) {
            return Err(ScoreFactoryBuildError::RecursivePreparation { mode });
        }
        let _guard = PreparationGuard {
            keys: &self.preparing,
            mode,
        };
        let data = (self.recipe.prepare)(mode)
            .map_err(|error| ScoreFactoryBuildError::Preparation { mode, error })?;
        let prepared = Rc::new(PreparedMode { mode, data });
        self.cache.borrow_mut().insert(mode, prepared.clone());
        Ok(prepared)
    }
}

/// A bank view with shared model/shape/trend and one prepared recipe identity.
/// Stream modes cache distinct preparations; full/incremental recipes share one
/// mode-independent preparation while passing every requested mode to the runtime.
/// Successful preparation is cached once; failures/panics are not cached. A failed
/// instantiation does not invalidate an already valid preparation. This bank is
/// single-threaded like native stream graphs (Rc); it does not launch worker threads.
/// It caches P, not mutable graph nodes or director state. Stream instantiation
/// still allocates its own nodes/indexes from the application's prepared recipe.
///
/// ```
/// use std::rc::Rc;
/// use timetuck::*;
/// let model=Rc::new(FiniteProblem::new(vec![Variable::new(vec![1_i64,2]).with_initial(1)])?);
/// let network=ConstraintNetwork::<i64,SimpleScore>::new(1);
/// let recipe=ScoringFactoryRecipe::incremental_recipe(
///     |_| Ok::<_,ScoringError>(&network),
///     |network,_,input| ScoreDirector::new(&model,*network,input),
///     |director:&ScoreDirector<'_, '_,i64,SimpleScore>| Ok(director.assignment().clone()));
/// let factory=ScoringFactoryDeclaration::default().with_scoring_recipe(recipe)
///     .prepare_score_factory(model.clone(),Rc::new(FactoryScoreShape::for_score::<SimpleScore>()),
///         EnvironmentProfile::default())?;
/// let director=factory.score_factory_builder(model.initial_assignment()).instantiate_score_director()?;
/// let child=director.create_scoring_child()?;
/// assert!(factory.same_prepared_factory(child.scoring_factory()));
/// assert_eq!(child.factory_resource().score(),SimpleScore::ZERO);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub struct PreparedScoreFactory<'a, M, P, I, D, E> {
    registry: Rc<FactoryRegistry<'a, M, P, I, D, E>>,
    prepared: Rc<PreparedMode<P>>,
}
impl<M, P, I, D, E> Clone for PreparedScoreFactory<'_, M, P, I, D, E> {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            prepared: self.prepared.clone(),
        }
    }
}
impl<'a, M, P, I, D, E> PreparedScoreFactory<'a, M, P, I, D, E> {
    pub fn scoring_factory_strategy(&self) -> ScoringFactoryStrategy {
        if self.registry.multi_mode && self.prepared.mode == self.registry.global {
            ScoringFactoryStrategy::MultiModeStream
        } else {
            self.registry.recipe.strategy
        }
    }
    pub fn same_prepared_factory(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.registry, &other.registry) && Rc::ptr_eq(&self.prepared, &other.prepared)
    }
    pub fn factory_model(&self) -> &Rc<M> {
        &self.registry.model
    }
    pub fn factory_score_shape(&self) -> &Rc<FactoryScoreShape> {
        &self.registry.shape
    }
    pub fn factory_score_trend(&self) -> &InitializingScoreTrend {
        &self.registry.trend
    }
    pub fn factory_preparation(&self) -> &P {
        &self.prepared.data
    }
    pub fn prepared_factory_count(&self) -> usize {
        self.registry.cache.borrow().len()
    }
    pub fn assertion_score_factory(&self) -> Option<&Self> {
        self.registry.assertion.as_ref()
    }
    pub fn select_scoring_factory(
        &self,
        requested: EnvironmentMode,
    ) -> Result<Self, ScoreFactoryBuildError<E>> {
        Ok(Self {
            registry: self.registry.clone(),
            prepared: self.registry.prepare_scoring_mode(requested)?,
        })
    }
    /// The default mode belongs to this view. An explicitly selected stream view
    /// opens in that mode; shared full/incremental views default to the global mode.
    pub fn score_factory_builder(&self, input: I) -> FactoryDirectorBuilder<'a, M, P, I, D, E> {
        self.score_factory_builder_in(input, self.prepared.mode)
    }
    pub fn score_factory_builder_in(
        &self,
        input: I,
        mode: EnvironmentMode,
    ) -> FactoryDirectorBuilder<'a, M, P, I, D, E> {
        FactoryDirectorBuilder {
            factory: self.clone(),
            input,
            settings: FactoryDirectorSettings {
                requested_mode: mode,
                matching: ContextMatchPolicy::Disabled,
            },
        }
    }
}
pub struct FactoryDirectorBuilder<'a, M, P, I, D, E> {
    factory: PreparedScoreFactory<'a, M, P, I, D, E>,
    input: I,
    settings: FactoryDirectorSettings,
}
impl<'a, M, P, I, D, E> FactoryDirectorBuilder<'a, M, P, I, D, E> {
    pub fn with_factory_matching(mut self, policy: ContextMatchPolicy) -> Self {
        self.settings.matching = policy;
        self
    }
    pub fn instantiate_score_director(
        self,
    ) -> Result<FactoryScoreDirector<'a, M, P, I, D, E>, ScoreFactoryBuildError<E>> {
        let factory = self
            .factory
            .select_scoring_factory(self.settings.requested_mode)?;
        let resource = (factory.registry.recipe.instantiate)(
            &factory.prepared.data,
            self.settings,
            self.input,
        )
        .map_err(|error| ScoreFactoryBuildError::Instantiation {
            mode: self.settings.requested_mode,
            error,
        })?;
        Ok(FactoryScoreDirector {
            factory,
            settings: self.settings,
            resource,
        })
    }
}
/// Owns the independent native resource returned by the instantiator. Accessors
/// expose it for actual scoring/search/editing; no scores or results are fabricated.
/// Drop releases the resource normally. Child resources copy current working input
/// through the recipe and retain this requested mode and explicit match policy.
pub struct FactoryScoreDirector<'a, M, P, I, D, E> {
    // Release mutable runtime state before the last shared preparation owner.
    resource: D,
    factory: PreparedScoreFactory<'a, M, P, I, D, E>,
    settings: FactoryDirectorSettings,
}
impl<'a, M, P, I, D, E> FactoryScoreDirector<'a, M, P, I, D, E> {
    pub fn scoring_factory(&self) -> &PreparedScoreFactory<'a, M, P, I, D, E> {
        &self.factory
    }
    pub fn factory_director_settings(&self) -> FactoryDirectorSettings {
        self.settings
    }
    pub fn factory_resource(&self) -> &D {
        &self.resource
    }
    pub fn factory_resource_mut(&mut self) -> &mut D {
        &mut self.resource
    }
    pub fn into_factory_resource(self) -> D {
        self.resource
    }
    pub fn create_scoring_child(&self) -> Result<Self, ScoreFactoryBuildError<E>> {
        let input = (self.factory.registry.recipe.child_input)(&self.resource)
            .map_err(ScoreFactoryBuildError::ChildInput)?;
        self.factory
            .score_factory_builder_in(input, self.settings.requested_mode)
            .with_factory_matching(self.settings.matching)
            .instantiate_score_director()
    }
}
