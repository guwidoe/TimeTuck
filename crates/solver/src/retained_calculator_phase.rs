//! Retained calculator director phase adapters.
macro_rules! retained_calculator {
    ($director:ident, [$($v:ident),+], $assignment:ty, $problem:ty, $movement:ty,
     $change:ty, $error:ty, $local_error:ty, $sequence:ident, $solver:ident,
     $construction:ident, $construction_config:ty, $construction_result:ty,
     $search:ident, $search_config:ty, $search_result:ty, $provider_constructor:ident,
     $initial:expr, [$($clock_field:ident)?]) => {
impl<'p, $($v: Eq + Clone,)+ C, F> $director<'p, $($v,)+ C, F>
where C: IncrementalCalculator<$assignment, Change = $change> + Clone,
      F: Fn() -> C,
{
    /// Run a plan on this initialized calculator, retaining its caches and match
    /// collection setting. Earlier initialization is outside this call's work.
    /// Cached phase-entry scores consume the usual evaluation budget but no
    /// calculator invocation. A changed entry assignment is reset and scored.
    ///
    /// `Clone` must snapshot mutable scoring state independently. Shared external
    /// instrumentation is allowed only when it cannot affect scoring. Best-state
    /// snapshots include the assignment, calculator and match registry; restoring
    /// them does not rescore or reset. Registration IDs are never recycled after
    /// rollback. Factories are used only for explicit consistency diagnostics.
    /// Errors/panics retain the last committed state, which may require reset if
    /// a calculator callback failed. Successful calls retain the best solution.
    pub fn run_solver<'c>(&mut self, config: impl Into<crate::$sequence<'c, C::Score>>)
        -> Result<crate::SolverResult<$assignment, C::Score>, crate::SolverError<$local_error>>
    where C::Score: 'c,
    {
        self.run_solver_with_extensions(config, Default::default())
    }
    /// Retained plans with providers, commands, lifecycle/events, metrics and
    /// independent score diagnostics, using the same controls as factory solves.
    pub fn run_solver_with_extensions<'c>(
        &mut self,
        config: impl Into<crate::$sequence<'c, C::Score>>,
        extensions: crate::SolverExtensions<'_, $problem, $assignment, $movement, C::Score, $error>,
    ) -> Result<crate::SolverResult<$assignment, C::Score>, crate::SolverError<$local_error>>
    where C::Score: 'c,
    {
        crate::solver::calculator_retained::$solver(self.problem, self, config.into(), extensions)
    }
    pub(crate) fn run_construction_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<C::Score>>,
        initial: Option<$assignment>,
        mut observer: crate::retained_stream_phase::Observer<'_, $assignment, C::Score>,
        config: $construction_config,
    ) -> Result<$construction_result, $error> {
        let problem = self.problem;
        let initial = initial.unwrap_or_else(|| self.assignment().clone());
        let observed = observer.is_some();
        let mut notify = |director: &Self, complete| {
            if let Some(observer) = &mut observer {
                observer(crate::SolutionUpdate {
                    assignment: director.assignment(),
                    score: director.score.clone(), complete,
                });
            }
        };
        crate::$construction::construction_impl_monitored(shared, problem, initial, config, |initial| {
            let initialization_work = self.prepare_retained_phase(initial)?;
            Ok(crate::retained_stream_phase::RetainedPhase {
                director: self, initialization_work, on_best: observed.then_some(&mut notify),
            })
        })
    }
    pub(crate) fn run_local_search_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<C::Score>>,
        initial: Option<$assignment>,
        mut observer: crate::retained_stream_phase::Observer<'_, $assignment, C::Score>,
        config: $search_config,
        provider: Option<&mut (dyn crate::MoveProvider<$problem, $assignment, $movement> + '_)>,
    ) -> Result<$search_result, $local_error> {
        let problem = self.problem;
        let initial = initial.unwrap_or_else(|| self.assignment().clone());
        let mut best = None;
        let mut save = |director: &Self, complete| {
            best = Some(director.capture_retained());
            if let Some(observer) = &mut observer {
                observer(crate::SolutionUpdate {
                    assignment: director.assignment(), score: director.score.clone(), complete,
                });
            }
        };
        let clock = crate::provider_search::clock(&config$(.$clock_field)?, &shared);
        let result = crate::$search::search_impl_with_provider(shared, problem, ($initial)(initial), config, |initial| {
            let initialization_work = self.prepare_retained_phase(initial)?;
            Ok(crate::retained_stream_phase::RetainedPhase {
                director: &mut *self, initialization_work, on_best: Some(&mut save),
            })
        }, clock, provider.map(|p| p as &mut dyn crate::MoveProvider<$problem, $assignment, $movement>))?;
        if let Some(best) = best { self.install_retained_state(best); }
        Ok(result)
    }
    pub(crate) fn run_provider_construction_monitored(
        &mut self, initial: $assignment, config: crate::ProviderConstructionConfig<'_>,
        context: crate::provider_search::ProviderContext<'_, $problem, $assignment, $movement, C::Score>,
    ) -> Result<crate::ProviderConstructionResult<$assignment, C::Score>, $error> {
        crate::provider_construction::$provider_constructor(self.problem, initial, config, context, |initial| {
            let initialization_work = self.prepare_retained_phase(initial)?;
            Ok(crate::retained_stream_phase::RetainedPhase {
                director: self, initialization_work, on_best: None,
            })
        })
    }
}
    };
}
pub(crate) use retained_calculator;
