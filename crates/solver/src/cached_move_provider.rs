//! Lifecycle-owned selection caches feeding current-assignment move generation.
use crate::{
    CachedSelectionOrder, MoveProvider, MoveProviderEvent, SelectionCache, SelectionCacheError,
    SelectionCacheScope, SelectionCacheState, SelectionLifecycleEvent, SelectionSource,
};
use std::{error::Error, fmt};

/// Cache duration inside one provider-backed engine phase. Whole-solve caching
/// needs a solver-level owner and is deliberately not represented by this type.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProviderCacheScope {
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
/// Phase scope retains keys/weights until phase end; Step rebuilds every scan.
/// Choose Step if key membership or weights depend on changing assignment facts.
///
/// Construction and local search deliver lifecycle automatically. The private
/// selection session starts lazily on its first nonzero-draw opening and closes
/// at provider phase end. Source SolveStarted/Ended bracket this private session,
/// not the enclosing solver sequence. Unopened scans/phases emit no source events.
/// Each source StepEnded uses the same captured pre-scan context as its start;
/// the next scan captures the updated assignment. PhaseEnded/SolveEnded use the
/// last captured context. Phase snapshots release at phase end, step snapshots at
/// scan end. Owned snapshots explicitly obtained through cache() remain valid.
///
/// Source size/weight/collection failures must not masquerade as no candidates.
/// try_moves returns those errors to direct callers; the infallible MoveProvider
/// interface panics with the error. Engines skip callbacks on panic; reset closes
/// any partially opened source lifecycle before reuse. Source callbacks and mapper
/// panics propagate, and external side effects cannot be rolled back. Callbacks
/// must return promptly. Rejected or capped output is not an exhaustion proof.
pub struct CachedMoveProvider<C, T, S, W, F, G> {
    cache: SelectionCache<C, T, S, W>,
    context: Option<C>,
    capture: F,
    mapper: G,
    config: CachedProviderConfig,
    phase: bool,
    step: bool,
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
            context: None,
            capture,
            mapper,
            config,
            phase: false,
            step: false,
        })
    }
    pub fn cache(&self) -> &SelectionCache<C, T, S, W> {
        &self.cache
    }
    /// Edits affect the next cache build, not an already retained snapshot.
    pub fn source_mut(&mut self) -> &mut S {
        self.cache.source_mut()
    }
    fn source_event(&mut self, event: SelectionLifecycleEvent) -> Result<(), SelectionCacheError> {
        self.cache.event(
            event,
            self.context.as_ref().expect("opened selection context"),
        )
    }
    /// End a private session, including one interrupted by a caught panic or a
    /// failed cache build. Valid end events update cache state before callbacks;
    /// if a callback panics, calling reset again continues from the remaining state.
    /// Do not reset while an engine is using the provider.
    pub fn reset(&mut self) -> Result<(), SelectionCacheError> {
        loop {
            let event = match self.cache.state() {
                SelectionCacheState::Step => SelectionLifecycleEvent::StepEnded,
                SelectionCacheState::Phase => SelectionLifecycleEvent::PhaseEnded,
                SelectionCacheState::Solving => SelectionLifecycleEvent::SolveEnded,
                SelectionCacheState::Idle => break,
            };
            self.source_event(event)?;
        }
        self.context = None;
        self.phase = false;
        self.step = false;
        Ok(())
    }
    fn lifecycle(&mut self, event: MoveProviderEvent) {
        match event {
            MoveProviderEvent::PhaseStarted => {
                assert!(
                    !self.phase && self.cache.state() == SelectionCacheState::Idle,
                    "cached provider needs reset before a new phase"
                );
                self.phase = true;
            }
            MoveProviderEvent::StepStarted => {
                assert!(
                    self.phase && !self.step,
                    "cached provider scan already active or phase missing"
                );
                self.step = true;
            }
            MoveProviderEvent::StepEnded => {
                assert!(self.phase && self.step, "cached provider scan missing");
                self.step = false;
                if self.cache.state() == SelectionCacheState::Step {
                    self.source_event(SelectionLifecycleEvent::StepEnded)
                        .expect("balanced source scan");
                }
            }
            MoveProviderEvent::PhaseEnded => {
                assert!(
                    self.phase && !self.step,
                    "cached provider phase ended during a scan"
                );
                self.reset().expect("balanced source session");
            }
        }
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
        if !self.phase || !self.step {
            return Err(CachedProviderError::OutsideStep);
        }
        if self.config.maximum_draws == 0 {
            return Ok(Box::new(std::iter::empty()));
        }
        if self.cache.state() != SelectionCacheState::Step {
            self.context = Some((self.capture)(p, a, step));
            if self.cache.state() == SelectionCacheState::Idle {
                self.source_event(SelectionLifecycleEvent::SolveStarted)?;
            }
            if self.cache.state() == SelectionCacheState::Solving {
                self.source_event(SelectionLifecycleEvent::PhaseStarted)?;
            }
            self.source_event(SelectionLifecycleEvent::StepStarted)?;
        }
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
        self.lifecycle(event);
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.try_moves(p, a, step)
            .unwrap_or_else(|error| panic!("cached move provider: {error}"))
    }
}
