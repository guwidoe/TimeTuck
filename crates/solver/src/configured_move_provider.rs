//! Configured selection directly feeding current-state solver moves.
use crate::provider_selection_session::ProviderSelectionSession;
use crate::{
    CachedProviderError, ConfiguredSelection, MoveProvider, MoveProviderEvent, SelectionCacheError,
    SelectionSource,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfiguredProviderConfig {
    /// Maximum selected occurrences per scan, before mapper rejection. Zero
    /// avoids opening the source. Also bounds repeating random selection.
    pub maximum_draws: usize,
    /// Each opening uses seed + completed_steps with wrapping addition.
    pub seed: u64,
}
impl Default for ConfiguredProviderConfig {
    fn default() -> Self {
        Self {
            maximum_draws: 100_000,
            seed: 0,
        }
    }
}
/// Connect any configured selection order to construction or local search.
/// Sorted/comparator-factory, shuffled, original, random and probabilistic orders
/// keep their configured source/order cache scopes and selected-count limits.
/// There is no second cache and neither context nor keys need Clone.
///
/// The capture callback owns the context for each scan; the mapper borrows each
/// selected key alongside the current assignment. Return None to reject it. Sort
/// and weight callbacks use their cache-build context, while moves always use the
/// current assignment. Phase/solve caches may intentionally retain old keys.
///
/// Lifecycle and failure behavior match CachedMoveProvider: scopes open lazily,
/// standalone calls own one phase, shared solver bindings retain solve caches,
/// and reset closes scopes after a caught panic. Direct try_moves calls require
/// engine step events. Selection errors are returned there and panic through the
/// infallible MoveProvider interface. Callbacks must return promptly.
pub struct ConfiguredMoveProvider<C, T, S, F, G> {
    selection: ConfiguredSelection<C, T, S>,
    session: ProviderSelectionSession<C>,
    capture: F,
    mapper: G,
    config: ConfiguredProviderConfig,
}
impl<C, T, S: SelectionSource<C, T>, F, G> ConfiguredMoveProvider<C, T, S, F, G> {
    pub fn new(
        selection: ConfiguredSelection<C, T, S>,
        config: ConfiguredProviderConfig,
        capture: F,
        mapper: G,
    ) -> Self {
        Self {
            selection,
            session: Default::default(),
            capture,
            mapper,
            config,
        }
    }
    pub fn selection(&self) -> &ConfiguredSelection<C, T, S> {
        &self.selection
    }
    /// Edits affect the next source-cache build, not a retained snapshot.
    pub fn source_mut(&mut self) -> &mut S {
        self.selection.source_mut()
    }
    pub fn reset(&mut self) -> Result<(), SelectionCacheError> {
        self.session.reset(&mut self.selection)
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
            .open(&mut self.selection, || (self.capture)(p, a, step))?;
        let context = self
            .session
            .context
            .as_ref()
            .expect("opened selection context");
        let mapper = &mut self.mapper;
        let selected =
            self.selection
                .map_iter(context, self.config.seed.wrapping_add(step), move |key| {
                    mapper(p, a, step, key)
                })?;
        Ok(Box::new(selected.take(self.config.maximum_draws).flatten()))
    }
}
impl<P, A, M, C, T, S: SelectionSource<C, T>, F, G> MoveProvider<P, A, M>
    for ConfiguredMoveProvider<C, T, S, F, G>
where
    F: FnMut(&P, &A, u64) -> C,
    G: FnMut(&P, &A, u64, &T) -> Option<M>,
{
    fn event(&mut self, event: MoveProviderEvent) {
        self.session.lifecycle(&mut self.selection, event);
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.try_moves(p, a, step)
            .unwrap_or_else(|e| panic!("configured move provider: {e}"))
    }
}
