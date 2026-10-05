//! Lifecycle-scoped domain indexes and immutable candidate views.
use crate::{
    CachedSelectionOrder, FiniteProblem, ListProblem, ReachabilityError, ReachableValues,
    SelectionCache, SelectionCacheError, SelectionCacheScope, SelectionCacheState,
    SelectionLifecycleEvent, SelectionSource, WeightedSelection,
};
use std::{error::Error, fmt};

/// One entity identity, its materialized finite range, and optionality.
pub type ReachabilityRange<E, V> = (E, Vec<V>, bool);
type RangeCache<C, E, V, S> =
    SelectionCache<C, ReachabilityRange<E, V>, S, fn(&C, &ReachabilityRange<E, V>) -> f64>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReachabilityCacheError<E> {
    Cache(SelectionCacheError),
    Domain(ReachabilityError<E>),
}
impl<E: fmt::Debug> fmt::Display for ReachabilityCacheError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "domain selection cache: {self:?}")
    }
}
impl<E: fmt::Debug> Error for ReachabilityCacheError<E> {}
impl<E> From<SelectionCacheError> for ReachabilityCacheError<E> {
    fn from(error: SelectionCacheError) -> Self {
        Self::Cache(error)
    }
}

/// Native selection criteria. CoValues excludes the origin and unassignment;
/// optional transitions remain available through index().destinations().
pub enum ReachabilityQuery<'a, E, V> {
    All,
    Entity(&'a E),
    CoValues(&'a V),
}

/// Capture domain membership at solve/phase/step boundaries. Source rows are
/// collected under maximum_entities; each row already owns a finite value vector.
/// Source events and balanced transitions follow SelectionCache. Rebuilding
/// deduplicates values and retains all owners of shared ranges. Failed builds do
/// not publish an index; held indexes and selections remain independent snapshots.
///
/// select() captures a sorted distinct value view. source_size() reports the full
/// distinct value universe, which may exceed the filtered view's len(). Random
/// sampling is explicitly requested on the returned WeightedSelection. Assignment,
/// pin and position checks remain the responsibility of candidate providers.
pub struct ReachabilityCache<C, E, V, S> {
    cache: RangeCache<C, E, V, S>,
    scope: SelectionCacheScope,
    index: Option<ReachableValues<E, V>>,
}
impl<C, E: Ord + Clone, V: Ord + Clone, S: SelectionSource<C, ReachabilityRange<E, V>>>
    ReachabilityCache<C, E, V, S>
{
    pub fn new(
        source: S,
        scope: SelectionCacheScope,
        maximum_entities: usize,
    ) -> Result<Self, ReachabilityCacheError<E>> {
        Ok(Self {
            cache: SelectionCache::uniform(
                source,
                scope,
                CachedSelectionOrder::Original,
                maximum_entities,
            )?,
            scope,
            index: None,
        })
    }
    pub fn source(&self) -> &S {
        self.cache.source()
    }
    pub fn source_mut(&mut self) -> &mut S {
        self.cache.source_mut()
    }
    pub fn state(&self) -> SelectionCacheState {
        self.cache.state()
    }
    pub fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), ReachabilityCacheError<E>> {
        use SelectionLifecycleEvent as Event;
        let build = matches!(
            (self.scope, event),
            (SelectionCacheScope::Solve, Event::SolveStarted)
                | (SelectionCacheScope::Phase, Event::PhaseStarted)
                | (SelectionCacheScope::Step, Event::StepStarted)
        );
        let release = matches!(
            (self.scope, event),
            (SelectionCacheScope::Solve, Event::SolveEnded)
                | (SelectionCacheScope::Phase, Event::PhaseEnded)
                | (SelectionCacheScope::Step, Event::StepEnded)
        );
        let result = self.cache.event(event, context);
        if matches!(result, Err(SelectionCacheError::InvalidTransition { .. })) {
            return result.map_err(Into::into);
        }
        if build || release {
            self.index = None;
        }
        result?;
        if build {
            let rows = self.cache.snapshot()?;
            let index = ReachableValues::new(rows.items().to_vec())
                .map_err(ReachabilityCacheError::Domain)?;
            self.index = Some(index);
        }
        Ok(())
    }
    pub fn index(&self) -> Result<ReachableValues<E, V>, ReachabilityCacheError<E>> {
        // A panicking source build clears the inner cache before invoking user code.
        self.cache.size()?;
        self.index
            .clone()
            .ok_or(SelectionCacheError::NotReady.into())
    }
    pub fn source_size(&self) -> Result<usize, ReachabilityCacheError<E>> {
        Ok(self.index()?.values().len())
    }
    pub fn select(
        &self,
        query: ReachabilityQuery<'_, E, V>,
    ) -> Result<WeightedSelection<V>, ReachabilityCacheError<E>> {
        let index = self.index()?;
        let values = match query {
            ReachabilityQuery::All => index.values().cloned().collect::<Vec<_>>(),
            ReachabilityQuery::Entity(entity) => index
                .domain(entity)
                .map_or_else(Vec::new, |values| values.to_vec()),
            ReachabilityQuery::CoValues(value) => {
                index.co_values(value).into_iter().cloned().collect()
            }
        };
        WeightedSelection::new(values.into_iter().map(|v| (v, 1.0)).collect())
            .map_err(|e| ReachabilityCacheError::Cache(SelectionCacheError::Weights(e)))
    }
}

/// Domain source for native basic problems; variable IDs are enumeration indices.
#[derive(Clone, Copy, Debug, Default)]
pub struct BasicRangeSource;
impl<V: Ord + Clone> SelectionSource<FiniteProblem<V>, ReachabilityRange<usize, V>>
    for BasicRangeSource
{
    fn size(&mut self, context: &FiniteProblem<V>) -> Option<usize> {
        Some(context.variables().len())
    }
    fn open<'a>(
        &'a mut self,
        context: &'a FiniteProblem<V>,
    ) -> Box<dyn Iterator<Item = ReachabilityRange<usize, V>> + 'a>
    where
        ReachabilityRange<usize, V>: 'a,
    {
        Box::new(
            context
                .variables()
                .iter()
                .enumerate()
                .map(|(i, v)| (i, v.domain().to_vec(), v.allows_unassigned())),
        )
    }
}
/// Domain source for native list problems, expanding unrestricted owner ranges.
#[derive(Clone, Copy, Debug, Default)]
pub struct ListRangeSource;
impl<V: Ord + Clone> SelectionSource<ListProblem<V>, ReachabilityRange<usize, V>>
    for ListRangeSource
{
    fn size(&mut self, context: &ListProblem<V>) -> Option<usize> {
        Some(context.entities().len())
    }
    fn open<'a>(
        &'a mut self,
        context: &'a ListProblem<V>,
    ) -> Box<dyn Iterator<Item = ReachabilityRange<usize, V>> + 'a>
    where
        ReachabilityRange<usize, V>: 'a,
    {
        Box::new(context.entities().iter().enumerate().map(|(i, v)| {
            (
                i,
                v.value_range().unwrap_or(context.values()).to_vec(),
                context.allows_unassigned(),
            )
        }))
    }
}
