//! Shared lazy lifecycle for provider-owned selection pipelines.
use crate::{
    CachedProviderError, ConfiguredSelection, MoveProviderEvent, SelectionCache,
    SelectionCacheError, SelectionCacheState, SelectionLifecycleEvent, SelectionSource,
};
pub(crate) trait ProviderSelectionCache<C> {
    fn state(&self) -> SelectionCacheState;
    fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), SelectionCacheError>;
}
impl<C, T, S: SelectionSource<C, T>, W: FnMut(&C, &T) -> f64> ProviderSelectionCache<C>
    for SelectionCache<C, T, S, W>
{
    fn state(&self) -> SelectionCacheState {
        self.state()
    }
    fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), SelectionCacheError> {
        self.event(event, context)
    }
}
impl<C, T, S: SelectionSource<C, T>> ProviderSelectionCache<C> for ConfiguredSelection<C, T, S> {
    fn state(&self) -> SelectionCacheState {
        self.state()
    }
    fn event(
        &mut self,
        event: SelectionLifecycleEvent,
        context: &C,
    ) -> Result<(), SelectionCacheError> {
        self.event(event, context)
    }
}
pub(crate) struct ProviderSelectionSession<C> {
    pub(crate) context: Option<C>,
    solve: bool,
    phase: bool,
    step: bool,
}
impl<C> Default for ProviderSelectionSession<C> {
    fn default() -> Self {
        Self {
            context: None,
            solve: false,
            phase: false,
            step: false,
        }
    }
}
impl<C> ProviderSelectionSession<C> {
    pub(crate) fn check_step(&self) -> Result<(), CachedProviderError> {
        if self.phase && self.step {
            Ok(())
        } else {
            Err(CachedProviderError::OutsideStep)
        }
    }
    pub(crate) fn open<K: ProviderSelectionCache<C>>(
        &mut self,
        cache: &mut K,
        capture: impl FnOnce() -> C,
    ) -> Result<(), SelectionCacheError> {
        if cache.state() != SelectionCacheState::Step {
            self.context = Some(capture());
            if cache.state() == SelectionCacheState::Idle {
                self.source_event(cache, SelectionLifecycleEvent::SolveStarted)?;
            }
            if cache.state() == SelectionCacheState::Solving {
                self.source_event(cache, SelectionLifecycleEvent::PhaseStarted)?;
            }
            self.source_event(cache, SelectionLifecycleEvent::StepStarted)?;
        }
        Ok(())
    }
    fn source_event<K: ProviderSelectionCache<C>>(
        &mut self,
        cache: &mut K,
        event: SelectionLifecycleEvent,
    ) -> Result<(), SelectionCacheError> {
        cache.event(
            event,
            self.context.as_ref().expect("opened selection context"),
        )
    }
    /// End a private session, including one interrupted by a caught panic or a
    /// failed cache build. Valid end events update cache state before callbacks;
    /// if a callback panics, calling reset again continues from the remaining state.
    /// Do not reset while an engine is using the provider.
    pub(crate) fn reset<K: ProviderSelectionCache<C>>(
        &mut self,
        cache: &mut K,
    ) -> Result<(), SelectionCacheError> {
        loop {
            let event = match cache.state() {
                SelectionCacheState::Step => SelectionLifecycleEvent::StepEnded,
                SelectionCacheState::Phase => SelectionLifecycleEvent::PhaseEnded,
                SelectionCacheState::Solving => SelectionLifecycleEvent::SolveEnded,
                SelectionCacheState::Idle => break,
            };
            self.source_event(cache, event)?;
        }
        self.context = None;
        self.solve = false;
        self.phase = false;
        self.step = false;
        Ok(())
    }
    pub(crate) fn lifecycle<K: ProviderSelectionCache<C>>(
        &mut self,
        cache: &mut K,
        event: MoveProviderEvent,
    ) {
        match event {
            MoveProviderEvent::SolveStarted => {
                assert!(
                    !self.solve && !self.phase && cache.state() == SelectionCacheState::Idle,
                    "cached provider needs reset before a new solve"
                );
                self.solve = true;
            }
            MoveProviderEvent::SolveEnded => {
                assert!(
                    self.solve && !self.phase && !self.step,
                    "cached provider solve ended during a phase"
                );
                self.reset(cache).expect("balanced source solve");
            }
            MoveProviderEvent::PhaseStarted => {
                assert!(
                    !self.phase
                        && (cache.state() == SelectionCacheState::Idle
                            || (self.solve && cache.state() == SelectionCacheState::Solving)),
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
                if cache.state() == SelectionCacheState::Step {
                    self.source_event(cache, SelectionLifecycleEvent::StepEnded)
                        .expect("balanced source scan");
                }
            }
            MoveProviderEvent::PhaseEnded => {
                assert!(
                    self.phase && !self.step,
                    "cached provider phase ended during a scan"
                );
                if self.solve {
                    if cache.state() == SelectionCacheState::Phase {
                        self.source_event(cache, SelectionLifecycleEvent::PhaseEnded)
                            .expect("balanced source phase");
                    }
                    self.phase = false;
                } else {
                    self.reset(cache).expect("balanced source session");
                }
            }
        }
    }
}
