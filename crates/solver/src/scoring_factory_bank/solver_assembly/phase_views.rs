//! Borrowed reads of actual resource lifetimes and fallible solve/phase hooks.
use super::run_limits::FactoryRunTermination;
use super::{
    AssembledFactoryRun, AssembledSolverFactory, AssemblyResource, FactoryAssemblyError,
    FactoryAssemblySolution, FactoryExecutionError, FactoryExecutionFailure,
};
use crate::{
    ContextFailure, ContextScoreResource, ContextScoreWork, FactoryDirectorSettings,
    ScoreFactoryBuildError, ScoringContextError, ScoringContextHandle, SolverPhase,
};

/// Keeps the context identity, never its retired director. Reads return None
/// after replacement/shutdown. Callbacks cannot retain an interior borrow.
pub struct FactoryRuntimeView<'a, M, P, I, D: ContextScoreResource> {
    handle: ScoringContextHandle<AssemblyResource<'a, M, P, I, D>>,
}
impl<M, P, I, D: ContextScoreResource> Clone for FactoryRuntimeView<'_, M, P, I, D> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
        }
    }
}
impl<M, P, I, D: ContextScoreResource> FactoryRuntimeView<'_, M, P, I, D> {
    pub fn same_runtime_view(&self, other: &Self) -> bool {
        self.handle.same_resource(&other.handle)
    }
    pub fn read_runtime_view<R>(&self, read: impl FnOnce(&D) -> R) -> Option<R> {
        let state = self.handle.director()?;
        Some(read(&state.0.resource))
    }
    pub fn runtime_view_closed(&self) -> bool {
        self.handle.director().is_none()
    }
    pub fn runtime_view_calculations(&self) -> u128 {
        self.handle.calculation_count()
    }
    pub fn runtime_view_settings(&self) -> FactoryDirectorSettings {
        let settings = self.handle.settings();
        FactoryDirectorSettings {
            requested_mode: settings.mode,
            matching: settings.matching,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FactoryRunEvent {
    Started,
    PhaseEntered { index: usize },
    PhaseCompleted { index: usize },
    Finished,
}
type FactoryLifecycleFn<'h, H, E> = dyn FnMut(FactoryRunEvent, H) -> Result<(), E> + 'h;
type FactoryFailureFn<'h, H, E> = dyn FnMut(&FactoryExecutionError<E>, H) -> Result<(), E> + 'h;
/// Hooks run outside candidate loops. PhaseEntered follows resource replacement
/// and old-resource release; PhaseCompleted sees successful logical counts.
/// Failure hooks see the current resource before error cleanup. Final release
/// failures necessarily see a closed view. Hook failures are typed, and a failure
/// hook cannot replace the original error. Panics unwind without more callbacks.
pub struct FactoryRunCallbacks<'h, H, E> {
    pub lifecycle: Option<&'h mut FactoryLifecycleFn<'h, H, E>>,
    pub failure: Option<&'h mut FactoryFailureFn<'h, H, E>>,
}
impl<H, E> Default for FactoryRunCallbacks<'_, H, E> {
    fn default() -> Self {
        Self {
            lifecycle: None,
            failure: None,
        }
    }
}
pub(super) type RunCallbacks<'h, 'a, M, P, I, D> =
    FactoryRunCallbacks<'h, FactoryRuntimeView<'a, M, P, I, D>, <D as ContextScoreResource>::Error>;

impl<'a, M: 'a, P: 'a, I: 'a, D: ContextScoreResource + 'a> AssembledSolverFactory<'a, M, P, I, D>
where
    D::Error: 'static,
{
    pub fn start_observed_assembled_run(
        &self,
        input: I,
        callbacks: &mut RunCallbacks<'_, 'a, M, P, I, D>,
    ) -> Result<AssembledFactoryRun<'_, 'a, M, P, I, D>, FactoryExecutionFailure<D::Error>> {
        self.start_observed_assembled_run_with_limits(input, self.limits, callbacks)
    }
    pub fn start_observed_assembled_run_with_limits(
        &self,
        input: I,
        limits: super::run_limits::FactoryRunLimits,
        callbacks: &mut RunCallbacks<'_, 'a, M, P, I, D>,
    ) -> Result<AssembledFactoryRun<'_, 'a, M, P, I, D>, FactoryExecutionFailure<D::Error>> {
        let mut session = self.start_assembled_run_with_limits(input, limits)?;
        if let Err(error) = session.emit_runtime_event(FactoryRunEvent::Started, callbacks) {
            return Err(session.runtime_failure(
                FactoryExecutionError::Context(ScoringContextError::Scoring(
                    FactoryAssemblyError::Resource(error),
                )),
                callbacks,
            ));
        }
        Ok(session)
    }
    pub fn solve_observed_assembled_with<W: ContextScoreWork>(
        &self,
        input: I,
        callbacks: &mut RunCallbacks<'_, 'a, M, P, I, D>,
        mut run: impl FnMut(&mut D, &SolverPhase<'a, D::Score>) -> Result<W, D::Error>,
    ) -> Result<FactoryAssemblySolution<I, D::Score, W>, FactoryExecutionFailure<D::Error>> {
        let mut session = self.start_observed_assembled_run(input, callbacks)?;
        let mut phases = vec![];
        while let Some(work) = session.run_next_observed_phase(callbacks, &mut run)? {
            phases.push(work);
        }
        let termination = session
            .assembled_run_termination()
            .unwrap_or(FactoryRunTermination::PhasesCompleted);
        let (working, score, logical_calculations) =
            session.finish_observed_assembled_state(callbacks)?;
        Ok(FactoryAssemblySolution {
            working,
            score,
            phases,
            logical_calculations,
            termination,
        })
    }
}
impl<'a, M: 'a, P: 'a, I: 'a, D: ContextScoreResource + 'a> AssembledFactoryRun<'_, 'a, M, P, I, D>
where
    D::Error: 'static,
{
    pub fn assembled_runtime_view(&self) -> FactoryRuntimeView<'a, M, P, I, D> {
        FactoryRuntimeView {
            handle: self.manager.handle(),
        }
    }
    fn emit_runtime_event(
        &self,
        event: FactoryRunEvent,
        callbacks: &mut RunCallbacks<'_, 'a, M, P, I, D>,
    ) -> Result<(), D::Error> {
        if let Some(callback) = &mut callbacks.lifecycle {
            callback(event, self.assembled_runtime_view())?;
        }
        Ok(())
    }
    fn runtime_failure(
        &mut self,
        primary: FactoryExecutionError<D::Error>,
        callbacks: &mut RunCallbacks<'_, 'a, M, P, I, D>,
    ) -> FactoryExecutionFailure<D::Error> {
        let mut failure = ContextFailure::new(primary);
        if let Some(callback) = &mut callbacks.failure
            && let Err(error) = callback(&failure.primary, self.assembled_runtime_view())
        {
            failure
                .suppressed
                .push(FactoryAssemblyError::Resource(error));
        }
        self.manager.solving_error(&mut failure);
        failure
    }
    pub fn run_next_observed_phase<W: ContextScoreWork>(
        &mut self,
        callbacks: &mut RunCallbacks<'_, 'a, M, P, I, D>,
        mut run: impl FnMut(&mut D, &SolverPhase<'a, D::Score>) -> Result<W, D::Error>,
    ) -> Result<Option<W>, FactoryExecutionFailure<D::Error>> {
        let Some(index) = self.next_phase_index() else {
            return Ok(None);
        };
        if let Err(e) = self.start_checked_phase(index) {
            return Err(self.runtime_failure(FactoryExecutionError::Context(e), callbacks));
        }
        if let Err(e) = self.emit_runtime_event(FactoryRunEvent::PhaseEntered { index }, callbacks)
        {
            return Err(self.runtime_failure(
                FactoryExecutionError::Context(ScoringContextError::Scoring(
                    FactoryAssemblyError::Resource(e),
                )),
                callbacks,
            ));
        }
        let phase = self.remaining_phase(&self.owner.phases[index]);
        let work = match self
            .manager
            .run(|d| run(&mut d.0.resource, &phase).map_err(FactoryAssemblyError::Resource))
        {
            Ok(work) => work,
            Err(e) => {
                return Err(self.runtime_failure(FactoryExecutionError::Context(e), callbacks));
            }
        };
        if let Err(e) = self.check_completed_phase(index) {
            return Err(self.runtime_failure(FactoryExecutionError::Context(e), callbacks));
        }
        self.next_phase += 1;
        if let Err(e) =
            self.emit_runtime_event(FactoryRunEvent::PhaseCompleted { index }, callbacks)
        {
            return Err(self.runtime_failure(
                FactoryExecutionError::Context(ScoringContextError::Scoring(
                    FactoryAssemblyError::Resource(e),
                )),
                callbacks,
            ));
        }
        Ok(Some(work))
    }
    pub fn finish_observed_assembled_state(
        mut self,
        callbacks: &mut RunCallbacks<'_, 'a, M, P, I, D>,
    ) -> Result<(I, D::Score, u128), FactoryExecutionFailure<D::Error>> {
        let handle = self.manager.handle();
        let Some(state) = handle.director() else {
            return Err(self.runtime_failure(
                FactoryExecutionError::Context(ScoringContextError::Closed),
                callbacks,
            ));
        };
        let input = (self.owner.bank.registry.recipe.child_input)(&state.0.resource);
        let score = state.context_score();
        drop(state);
        let input = match input {
            Ok(input) => input,
            Err(e) => {
                return Err(self.runtime_failure(
                    FactoryExecutionError::Context(ScoringContextError::Scoring(
                        FactoryAssemblyError::Scoring(ScoreFactoryBuildError::ChildInput(e)),
                    )),
                    callbacks,
                ));
            }
        };
        if let Err(e) = self.emit_runtime_event(FactoryRunEvent::Finished, callbacks) {
            return Err(self.runtime_failure(
                FactoryExecutionError::Context(ScoringContextError::Scoring(
                    FactoryAssemblyError::Resource(e),
                )),
                callbacks,
            ));
        }
        let count = self.manager.calculation_count();
        if let Err(e) = self.manager.finish() {
            return Err(self.runtime_failure(FactoryExecutionError::Context(e), callbacks));
        }
        Ok((input, score, count))
    }
}
