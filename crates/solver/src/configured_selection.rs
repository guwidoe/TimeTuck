//! Resolve selection configuration and execute it over an owned native source.
use crate::random::SplitMix64;
use crate::{
    CachedSelectionOrder, SelectionCache, SelectionCacheError, SelectionCacheScope,
    SelectionCacheState, SelectionLifecycleEvent, SelectionSource, WeightedSelection,
};
use std::{cmp::Ordering, error::Error, fmt};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionOrder {
    Original,
    Random,
    Sorted,
    Shuffled,
    Probabilistic,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SelectionSortOrder {
    #[default]
    Ascending,
    Descending,
}
/// Cache requirements are explicit native source contracts. For example, use
/// Step for solution entities and Phase for values stable throughout a phase.
/// The source cache can outlive a shorter sorting/shuffling/weighting cache.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfiguredSelectionPolicy {
    pub minimum_scope: SelectionCacheScope,
    pub source_minimum_scope: SelectionCacheScope,
    pub inherited_order: SelectionOrder,
}
impl Default for ConfiguredSelectionPolicy {
    fn default() -> Self {
        Self {
            minimum_scope: SelectionCacheScope::Uncached,
            source_minimum_scope: SelectionCacheScope::Uncached,
            inherited_order: SelectionOrder::Random,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionConfig {
    pub cache_scope: Option<SelectionCacheScope>,
    pub order: Option<SelectionOrder>,
    pub sort_order: Option<SelectionSortOrder>,
    pub maximum_candidates: usize,
    pub seed: u64,
    /// Maximum yielded items per opening, not work rejected inside the source.
    /// ending_iter deliberately ignores this cap.
    pub selected_count_limit: Option<usize>,
}
impl Default for SelectionConfig {
    fn default() -> Self {
        Self {
            cache_scope: None,
            order: None,
            sort_order: None,
            maximum_candidates: 100_000,
            seed: 0,
            selected_count_limit: None,
        }
    }
}
/// Request delivered once to the source factory after configuration validation.
/// Random uncached sources own their draw policy; cached sources must enumerate
/// finite occurrences in their original order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionSourceRequest {
    pub cache_scope: SelectionCacheScope,
    pub random: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionPlan {
    pub cache_scope: SelectionCacheScope,
    pub order: SelectionOrder,
    pub source: SelectionSourceRequest,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectionBuildError {
    RequiresCache(SelectionOrder),
    SortingRequiresSortedOrder,
    MissingComparator,
    WeightingRequiresProbabilisticOrder,
    MissingWeight,
    Cache(SelectionCacheError),
}
impl fmt::Display for SelectionBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "selection configuration: {self:?}")
    }
}
impl Error for SelectionBuildError {}
impl From<SelectionCacheError> for SelectionBuildError {
    fn from(e: SelectionCacheError) -> Self {
        Self::Cache(e)
    }
}
fn wider(a: SelectionCacheScope, b: SelectionCacheScope) -> SelectionCacheScope {
    use SelectionCacheScope::*;
    let rank = |s| match s {
        Uncached => 0,
        Step => 1,
        Phase => 2,
        Solve => 3,
    };
    if rank(a) >= rank(b) { a } else { b }
}
type Comparator<T> = Box<dyn FnMut(&T, &T) -> Ordering>;
type ComparatorFactory<C, T> = Box<dyn FnMut(&C) -> Comparator<T>>;
type ContextComparator<C, T> = Box<dyn FnMut(&C, &T, &T) -> Ordering>;
enum Sorter<C, T> {
    Direct(ContextComparator<C, T>),
    Factory(ComparatorFactory<C, T>),
}
type Weight<C, T> = Box<dyn FnMut(&C, &T) -> f64>;
/// Build the same selection pipeline for entity IDs, values or moves. Rust
/// callbacks replace reflective Java classes. No source is created on invalid
/// configuration. Sorting and weighting callbacks run at their cache boundary.
pub struct SelectionBuilder<C, T> {
    config: SelectionConfig,
    sorter: Option<Sorter<C, T>>,
    weight: Option<Weight<C, T>>,
}
impl<C, T> SelectionBuilder<C, T> {
    pub fn new(config: SelectionConfig) -> Self {
        Self {
            config,
            sorter: None,
            weight: None,
        }
    }
    pub fn with_comparator(
        mut self,
        compare: impl FnMut(&C, &T, &T) -> Ordering + 'static,
    ) -> Self {
        self.sorter = Some(Sorter::Direct(Box::new(compare)));
        self
    }
    /// Called once per sorting-cache build, including empty/singleton sources.
    pub fn with_comparator_factory<F, R>(mut self, mut factory: F) -> Self
    where
        F: FnMut(&C) -> R + 'static,
        R: FnMut(&T, &T) -> Ordering + 'static,
    {
        self.sorter = Some(Sorter::Factory(Box::new(move |c| Box::new(factory(c)))));
        self
    }
    pub fn with_weight(mut self, weight: impl FnMut(&C, &T) -> f64 + 'static) -> Self {
        self.weight = Some(Box::new(weight));
        self
    }
    pub(crate) fn has_replay_overrides(&self) -> bool {
        self.config != SelectionConfig::default() || self.sorter.is_some() || self.weight.is_some()
    }
    pub fn resolve(
        &self,
        policy: ConfiguredSelectionPolicy,
    ) -> Result<SelectionPlan, SelectionBuildError> {
        use SelectionOrder::*;
        let order = self.config.order.unwrap_or(policy.inherited_order);
        let requested = wider(
            self.config.cache_scope.unwrap_or(policy.minimum_scope),
            policy.minimum_scope,
        );
        if order != Sorted && (self.sorter.is_some() || self.config.sort_order.is_some()) {
            return Err(SelectionBuildError::SortingRequiresSortedOrder);
        }
        if order == Sorted && self.sorter.is_none() {
            return Err(SelectionBuildError::MissingComparator);
        }
        if order != Probabilistic && self.weight.is_some() {
            return Err(SelectionBuildError::WeightingRequiresProbabilisticOrder);
        }
        if order == Probabilistic && self.weight.is_none() {
            return Err(SelectionBuildError::MissingWeight);
        }
        // Validate the requested decorator scope before source-cache promotion.
        if requested == SelectionCacheScope::Uncached
            && matches!(order, Sorted | Shuffled | Probabilistic)
        {
            return Err(SelectionBuildError::RequiresCache(order));
        }
        let cache_scope = if matches!(order, Original | Random) {
            wider(requested, policy.source_minimum_scope)
        } else {
            requested
        };
        let source_scope = wider(cache_scope, policy.source_minimum_scope);
        Ok(SelectionPlan {
            cache_scope,
            order,
            source: SelectionSourceRequest {
                cache_scope: source_scope,
                random: cache_scope == SelectionCacheScope::Uncached && order == Random,
            },
        })
    }
    /// Filter occurrences before caching/sorting/weighting. A filtering source
    /// has unknown size until collected. Bound infinite sources before filtering.
    pub fn build_filtered<S: SelectionSource<C, T>, F: FnMut(&C, &T) -> bool>(
        self,
        policy: ConfiguredSelectionPolicy,
        factory: impl FnOnce(SelectionSourceRequest) -> S,
        predicate: F,
    ) -> Result<ConfiguredSelection<C, T, crate::FilteredSelectionSource<S, F>>, SelectionBuildError>
    {
        self.build(policy, |request| {
            crate::FilteredSelectionSource::new(factory(request), predicate)
        })
    }
    pub fn build<S: SelectionSource<C, T>>(
        self,
        policy: ConfiguredSelectionPolicy,
        factory: impl FnOnce(SelectionSourceRequest) -> S,
    ) -> Result<ConfiguredSelection<C, T, S>, SelectionBuildError> {
        let plan = self.resolve(policy)?;
        let source = factory(plan.source);
        let storage = if plan.source.cache_scope == SelectionCacheScope::Uncached {
            Storage::Direct {
                source,
                state: SelectionCacheState::Idle,
            }
        } else {
            Storage::Cached(SelectionCache::uniform(
                source,
                plan.source.cache_scope,
                CachedSelectionOrder::Original,
                self.config.maximum_candidates,
            )?)
        };
        Ok(ConfiguredSelection {
            storage,
            plan,
            sorter: self.sorter,
            weight: self.weight,
            sort_order: self.config.sort_order.unwrap_or_default(),
            random: SplitMix64::new(self.config.seed),
            prepared: None,
            opening_seed: self.config.seed,
            selected_count_limit: self.config.selected_count_limit,
        })
    }
}
type SourceCache<C, T, S> = SelectionCache<C, T, S, fn(&C, &T) -> f64>;
enum Storage<C, T, S> {
    Direct {
        source: S,
        state: SelectionCacheState,
    },
    Cached(SourceCache<C, T, S>),
}
struct Prepared<T> {
    values: WeightedSelection<T>,
    indices: Vec<usize>,
}
/// Executable configuration over one retained source. `source()` always returns
/// that original source, including through cache/order layers. Cached items need
/// not Clone until an owned iterator is requested; only yielded items are cloned.
/// Source cache and order cache may have different lifetimes (phase values with
/// step shuffling, for example). All valid events reach the source exactly once.
///
/// Explicit balanced lifecycle delivery is required. Failed/panicking builds
/// leave the scope active but its output unavailable. End that scope normally;
/// later cache rebuilds can recover. Invalid events change nothing. External
/// callback effects cannot be rolled back. Sort comparators must be consistent.
///
/// Uncached original/random execution delegates directly to the source factory's
/// requested mode without materialization. Bound repeating sources at consumption.
/// Apply FilteredSelectionSource before this builder to filter before caching;
/// filtering/weights then observe the corresponding cache-build context.
pub struct ConfiguredSelection<C, T, S> {
    storage: Storage<C, T, S>,
    plan: SelectionPlan,
    sorter: Option<Sorter<C, T>>,
    weight: Option<Weight<C, T>>,
    sort_order: SelectionSortOrder,
    random: SplitMix64,
    prepared: Option<Prepared<T>>,
    opening_seed: u64,
    selected_count_limit: Option<usize>,
}
fn next_state(
    state: SelectionCacheState,
    event: SelectionLifecycleEvent,
) -> Result<SelectionCacheState, SelectionCacheError> {
    use SelectionCacheState as S;
    use SelectionLifecycleEvent as E;
    match (state, event) {
        (S::Idle, E::SolveStarted) => Ok(S::Solving),
        (S::Solving, E::PhaseStarted) => Ok(S::Phase),
        (S::Phase, E::StepStarted) => Ok(S::Step),
        (S::Step, E::StepEnded) => Ok(S::Phase),
        (S::Phase, E::PhaseEnded) => Ok(S::Solving),
        (S::Solving, E::SolveEnded) => Ok(S::Idle),
        _ => Err(SelectionCacheError::InvalidTransition { state, event }),
    }
}
fn boundary(scope: SelectionCacheScope, event: SelectionLifecycleEvent) -> (bool, bool) {
    use SelectionCacheScope as S;
    use SelectionLifecycleEvent as E;
    (
        matches!(
            (scope, event),
            (S::Solve, E::SolveStarted) | (S::Phase, E::PhaseStarted) | (S::Step, E::StepStarted)
        ),
        matches!(
            (scope, event),
            (S::Solve, E::SolveEnded) | (S::Phase, E::PhaseEnded) | (S::Step, E::StepEnded)
        ),
    )
}
impl<C, T, S: SelectionSource<C, T>> ConfiguredSelection<C, T, S> {
    pub fn plan(&self) -> SelectionPlan {
        self.plan
    }
    pub fn source(&self) -> &S {
        match &self.storage {
            Storage::Direct { source, .. } => source,
            Storage::Cached(c) => c.source(),
        }
    }
    /// Cached membership changes become visible at the next source-cache build.
    pub fn source_mut(&mut self) -> &mut S {
        match &mut self.storage {
            Storage::Direct { source, .. } => source,
            Storage::Cached(c) => c.source_mut(),
        }
    }
    pub fn state(&self) -> SelectionCacheState {
        match &self.storage {
            Storage::Direct { state, .. } => *state,
            Storage::Cached(c) => c.state(),
        }
    }
    pub fn is_never_ending(&self) -> bool {
        self.selected_count_limit.is_none() && self.repeats()
    }
    fn repeats(&self) -> bool {
        if matches!(self.storage, Storage::Direct { .. }) {
            self.source().is_never_ending()
        } else {
            matches!(
                self.plan.order,
                SelectionOrder::Random | SelectionOrder::Probabilistic
            )
        }
    }
    pub fn size(&mut self, context: &C) -> Result<Option<usize>, SelectionCacheError> {
        let repeats = self.repeats();
        let count = match &mut self.storage {
            Storage::Direct { source, state } => {
                if *state != SelectionCacheState::Step {
                    return Err(SelectionCacheError::NotReady);
                }
                if self.selected_count_limit == Some(0) {
                    return Ok(Some(0));
                }
                source.size(context)
            }
            Storage::Cached(_) => {
                let p = self
                    .prepared
                    .as_ref()
                    .ok_or(SelectionCacheError::NotReady)?;
                if self.selected_count_limit.is_some() && repeats && p.values.active_len() == 0 {
                    return Ok(Some(0));
                }
                Some(p.values.len())
            }
        };
        Ok(match self.selected_count_limit {
            Some(0) => Some(0),
            Some(limit) if repeats && count.is_some_and(|n| n > 0) => Some(limit),
            Some(limit) => count.map(|n| n.min(limit)),
            None => count,
        })
    }
    pub fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), SelectionCacheError> {
        let next = next_state(self.state(), event)?;
        if let Storage::Direct { source, state } = &mut self.storage {
            *state = next;
            source.event(event, context);
            return Ok(());
        }
        let (build, release) = boundary(self.plan.cache_scope, event);
        let shuffle = self.plan.order == SelectionOrder::Shuffled
            && event == SelectionLifecycleEvent::StepStarted;
        let previous = if build || release || shuffle {
            self.prepared.take()
        } else {
            None
        };
        let Storage::Cached(cache) = &mut self.storage else {
            unreachable!()
        };
        cache.event(event, context)?;
        if release || !(build || shuffle) {
            return Ok(());
        }
        let mut prepared = if build {
            let values = cache.snapshot()?;
            Prepared {
                indices: if matches!(
                    self.plan.order,
                    SelectionOrder::Sorted | SelectionOrder::Shuffled
                ) {
                    (0..values.len()).collect()
                } else {
                    Vec::new()
                },
                values,
            }
        } else {
            previous.ok_or(SelectionCacheError::NotReady)?
        };
        if build {
            match self.plan.order {
                SelectionOrder::Sorted => {
                    let values = prepared.values.items();
                    let descending = self.sort_order == SelectionSortOrder::Descending;
                    let mut sort = |compare: &mut dyn FnMut(&T, &T) -> Ordering| {
                        prepared.indices.sort_by(|&a, &b| {
                            let result = compare(&values[a], &values[b]);
                            if descending { result.reverse() } else { result }
                        });
                    };
                    match self.sorter.as_mut().expect("validated comparator") {
                        Sorter::Direct(compare) => sort(&mut |a, b| compare(context, a, b)),
                        Sorter::Factory(factory) => sort(&mut factory(context)),
                    }
                }
                SelectionOrder::Probabilistic => {
                    let weight = self.weight.as_mut().expect("validated weights");
                    prepared.values = prepared.values.reweighted(|_, v| weight(context, v))?;
                }
                _ => {}
            }
        }
        if shuffle {
            for bound in (2..=prepared.indices.len()).rev() {
                let i = self.random.below_wide(bound as u128) as usize;
                prepared.indices.swap(bound - 1, i);
            }
        }
        self.prepared = Some(prepared);
        Ok(())
    }
    /// Random cached iterators repeat with replacement using this opening's seed.
    /// Shuffled order is fixed for the step; repeated openings see the same order.
    /// Direct source openings use the source's own random state, not this seed.
    pub fn iter<'a>(
        &'a mut self,
        context: &'a C,
        seed: u64,
    ) -> Result<Box<dyn Iterator<Item = T> + 'a>, SelectionCacheError>
    where
        T: Clone + 'a,
    {
        let limit = self.selected_count_limit;
        if limit == Some(0) {
            if matches!(self.storage, Storage::Direct { .. }) {
                if self.state() != SelectionCacheState::Step {
                    return Err(SelectionCacheError::NotReady);
                }
            } else if self.prepared.is_none() {
                return Err(SelectionCacheError::NotReady);
            }
            return Ok(Box::new(std::iter::empty()));
        }
        let iter = self.raw_iter(context, seed)?;
        Ok(match limit {
            Some(limit) => Box::new(iter.take(limit)),
            None => iter,
        })
    }
    /// Finite traversal ignoring the selected-count cap and random weights.
    /// Cached random selection visits source occurrences once in original order;
    /// sorted/shuffled selection retains its prepared order. An uncached repeating
    /// source has no finite snapshot and returns UnboundedSource.
    pub fn ending_iter<'a>(
        &'a mut self,
        context: &'a C,
    ) -> Result<Box<dyn Iterator<Item = T> + 'a>, SelectionCacheError>
    where
        T: Clone + 'a,
    {
        if let Storage::Direct { source, state } = &mut self.storage {
            if *state != SelectionCacheState::Step {
                return Err(SelectionCacheError::NotReady);
            }
            if source.is_never_ending() {
                return Err(SelectionCacheError::UnboundedSource);
            }
            return Ok(Box::new(source.open(context).fuse()));
        }
        let p = self
            .prepared
            .as_ref()
            .ok_or(SelectionCacheError::NotReady)?;
        if matches!(
            self.plan.order,
            SelectionOrder::Sorted | SelectionOrder::Shuffled
        ) {
            Ok(Box::new(
                p.indices.iter().map(|&i| p.values.items()[i].clone()),
            ))
        } else {
            Ok(Box::new(p.values.items().iter().cloned()))
        }
    }
    fn raw_iter<'a>(
        &'a mut self,
        context: &'a C,
        seed: u64,
    ) -> Result<Box<dyn Iterator<Item = T> + 'a>, SelectionCacheError>
    where
        T: Clone + 'a,
    {
        if let Storage::Direct { source, state } = &mut self.storage {
            if *state != SelectionCacheState::Step {
                return Err(SelectionCacheError::NotReady);
            }
            return Ok(Box::new(source.open(context).fuse()));
        }
        let p = self
            .prepared
            .as_ref()
            .ok_or(SelectionCacheError::NotReady)?;
        if matches!(
            self.plan.order,
            SelectionOrder::Random | SelectionOrder::Probabilistic
        ) {
            let mut random = SplitMix64::new(seed);
            Ok(Box::new(std::iter::from_fn(move || {
                if p.values.active_len() == 0 {
                    return None;
                }
                p.values
                    .choose(random.unit())
                    .expect("valid random draw")
                    .cloned()
            })))
        } else if self.plan.order == SelectionOrder::Original {
            Ok(Box::new(p.values.items().iter().cloned()))
        } else {
            Ok(Box::new(
                p.indices.iter().map(|&i| p.values.items()[i].clone()),
            ))
        }
    }
}

/// Composition adapter: checked inherent methods remain available; the infallible
/// SelectionSource hooks propagate errors as panics instead of empty selections.
/// Each opening advances its seed once, independently of how many items are read.
impl<C, T: Clone, S: SelectionSource<C, T>> SelectionSource<C, T> for ConfiguredSelection<C, T, S> {
    fn is_never_ending(&self) -> bool {
        self.is_never_ending()
    }
    fn event(&mut self, event: SelectionLifecycleEvent, context: &C) {
        self.event(event, context)
            .unwrap_or_else(|e| panic!("configured selection: {e}"));
    }
    fn size(&mut self, context: &C) -> Option<usize> {
        self.size(context)
            .unwrap_or_else(|e| panic!("configured selection: {e}"))
    }
    fn open<'a>(&'a mut self, context: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        let seed = self.opening_seed;
        self.opening_seed = self.opening_seed.wrapping_add(1);
        self.iter(context, seed)
            .unwrap_or_else(|e| panic!("configured selection: {e}"))
    }
}
