//! Lifecycle-owned selection caches feeding current-assignment move generation.
use crate::{
    CachedSelectionOrder, MoveProvider, MoveProviderEvent, SelectionCache, SelectionCacheError,
    SelectionCacheScope, SelectionSource,
};
use std::{error::Error, fmt};

/// Cache duration inside provider-backed execution. A standalone engine call
/// has a single-phase selection session; solver sequences own the complete solve.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProviderCacheScope {
    Solve,
    Phase,
    #[default]
    Step,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CachedProviderConfig {
    pub scope: ProviderCacheScope,
    pub order: CachedSelectionOrder,
    /// Maximum source occurrences retained when a cache is built.
    pub maximum_candidates: usize,
    /// Maximum draws per scan, including mapper rejections. Bounds random sources
    /// even when every selected key maps to None. Zero does not open the source.
    pub maximum_draws: usize,
    /// Random openings use seed + completed_steps, with wrapping addition.
    pub seed: u64,
}
impl Default for CachedProviderConfig {
    fn default() -> Self {
        Self {
            scope: ProviderCacheScope::Step,
            order: CachedSelectionOrder::Original,
            maximum_candidates: 100_000,
            maximum_draws: 100_000,
            seed: 0,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CachedProviderError {
    OutsideStep,
    Cache(SelectionCacheError),
}
impl From<SelectionCacheError> for CachedProviderError {
    fn from(error: SelectionCacheError) -> Self {
        Self::Cache(error)
    }
}
impl fmt::Display for CachedProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutsideStep => f.write_str("cached provider requires an active engine scan"),
            Self::Cache(error) => error.fmt(f),
        }
    }
}
impl Error for CachedProviderError {}

/// Adapt a SelectionSource into a solver MoveProvider without retaining moves
/// derived from an old assignment. The context callback captures an owned C at
/// the first opening of each scan; the source and weights use that context. The
/// mapper receives the current problem, assignment and step plus a borrowed key.
/// Returning None rejects that occurrence. Neither C nor the keys need Clone.
/// Solve/phase scope retains keys and weights until its end; Step rebuilds each
/// scan. Choose Step if membership or weights depend on changing assignment facts.
///
/// Source scopes open lazily on the first nonzero-draw scan. A solver sequence
/// keeps the solve scope across phases; bind_shared reuses this provider in several
/// phases without resetting its solve cache. Standalone engine calls retain the
/// single-phase private-session behavior. Unopened scans/phases emit no source
/// events. Each source StepEnded uses its captured pre-scan context; phase/solve
/// ends use the last capture. Keys release at their scope's end. Explicitly held
/// snapshots remain valid. An opened solve cache survives unbound intervening
/// phases and closes even if a later phase fails before opening this provider.
///
/// Source size/weight/collection failures must not masquerade as no candidates.
/// try_moves returns those errors to direct callers; the infallible MoveProvider
/// interface panics with the error. Engines skip callbacks on panic; reset closes
/// any partially opened source lifecycle before reuse. Source callbacks and mapper
/// panics propagate, and external side effects cannot be rolled back. Callbacks
/// must return promptly. Rejected or capped output is not an exhaustion proof.
pub struct CachedMoveProvider<C, T, S, W, F, G> {
    cache: SelectionCache<C, T, S, W>,
    session: crate::provider_selection_session::ProviderSelectionSession<C>,
    capture: F,
    mapper: G,
    config: CachedProviderConfig,
}
impl<C, T, S: SelectionSource<C, T>, F, G> CachedMoveProvider<C, T, S, fn(&C, &T) -> f64, F, G> {
    pub fn uniform(
        source: S,
        config: CachedProviderConfig,
        capture: F,
        mapper: G,
    ) -> Result<Self, SelectionCacheError> {
        Self::new(source, config, |_, _| 1.0, capture, mapper)
    }
}
impl<C, T, S: SelectionSource<C, T>, W: FnMut(&C, &T) -> f64, F, G>
    CachedMoveProvider<C, T, S, W, F, G>
{
    pub fn new(
        source: S,
        config: CachedProviderConfig,
        weight: W,
        capture: F,
        mapper: G,
    ) -> Result<Self, SelectionCacheError> {
        let scope = match config.scope {
            ProviderCacheScope::Solve => SelectionCacheScope::Solve,
            ProviderCacheScope::Phase => SelectionCacheScope::Phase,
            ProviderCacheScope::Step => SelectionCacheScope::Step,
        };
        Ok(Self {
            cache: SelectionCache::new(
                source,
                scope,
                config.order,
                config.maximum_candidates,
                weight,
            )?,
            session: Default::default(),
            capture,
            mapper,
            config,
        })
    }
    pub fn cache(&self) -> &SelectionCache<C, T, S, W> {
        &self.cache
    }
    /// Edits affect the next cache build, not an already retained snapshot.
    pub fn source_mut(&mut self) -> &mut S {
        self.cache.source_mut()
    }
    /// Close active source scopes, including a session interrupted by a panic.
    pub fn reset(&mut self) -> Result<(), SelectionCacheError> {
        self.session.reset(&mut self.cache)
    }
    pub fn try_moves<'a, P, A, M: 'a>(
        &'a mut self,
        p: &'a P,
        a: &'a A,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = M> + 'a>, CachedProviderError>
    where
        F: FnMut(&P, &A, u64) -> C,
        G: FnMut(&P, &A, u64, &T) -> Option<M>,
    {
        self.session.check_step()?;
        if self.config.maximum_draws == 0 {
            return Ok(Box::new(std::iter::empty()));
        }
        self.session
            .open(&mut self.cache, || (self.capture)(p, a, step))?;
        let selections = self.cache.iter(self.config.seed.wrapping_add(step))?;
        let mapper = &mut self.mapper;
        Ok(Box::new(
            selections
                .take(self.config.maximum_draws)
                .filter_map(move |key| mapper(p, a, step, key)),
        ))
    }
}
impl<P, A, M, C, T, S: SelectionSource<C, T>, W: FnMut(&C, &T) -> f64, F, G> MoveProvider<P, A, M>
    for CachedMoveProvider<C, T, S, W, F, G>
where
    F: FnMut(&P, &A, u64) -> C,
    G: FnMut(&P, &A, u64, &T) -> Option<M>,
{
    fn event(&mut self, event: MoveProviderEvent) {
        self.session.lifecycle(&mut self.cache, event);
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.try_moves(p, a, step)
            .unwrap_or_else(|error| panic!("cached move provider: {error}"))
    }
}
