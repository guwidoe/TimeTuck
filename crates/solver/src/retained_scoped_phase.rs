//! Retained scoped director phase adapters.
pub(crate) struct ScopedSnapshot<A, S> {
    pub(crate) assignment: A,
    pub(crate) contributions: Vec<S>,
    pub(crate) score: S,
}
macro_rules! retained_scoped {
    ($director:ident, [$($v:ident),+], $assignment:ty, $problem:ty, $movement:ty,
     $error:ty, $local_error:ty, $sequence:ident, $solver:ident,
     $construction:ident, $construction_config:ty, $construction_result:ty,
     $search:ident, $search_config:ty, $search_result:ty, $provider_constructor:ident,
     $initial:expr, [$($clock_field:ident)?]) => {
impl<'p, 'n, $($v: Eq + Clone,)+ S: crate::AdditiveScore> $director<'p, 'n, $($v,)+ S>
{
    /// Run a plan on this initialized scoped contribution cache. Earlier setup is
    /// outside this call's work. Cached phase entry uses one logical evaluation
    /// and no scoring callbacks; best snapshots restore without rescoring.
    ///
    /// Constraints must depend only on their declared scopes and immutable facts.
    /// Ordinary handoffs preserve the scoring payloads of the working solution.
    /// Every exact phase fully refreshes its first candidate before caching deltas
    /// among that traversal's fixed value representatives. Equality may therefore
    /// describe planning identity without treating external payloads as score-equal.
    /// Repair rebuild boundaries also refresh all contributions: rejected repairs
    /// may change payload representatives even when their planning IDs compare equal.
    /// Successful calls retain the best solution. Errors/panics leave the last
    /// committed state; an unsuccessful scoring transition never installs a cache.
    pub fn run_solver<'c>(&mut self, config: impl Into<crate::$sequence<'c, S>>)
        -> Result<crate::SolverResult<$assignment, S>, crate::SolverError<$local_error>>
    where S: 'c,
    {
        self.run_solver_with_extensions(config, Default::default())
    }
    /// Retained plans with providers, commands, lifecycle/events, metrics and
    /// independent score diagnostics, using the same controls as ordinary solves.
    pub fn run_solver_with_extensions<'c>(
        &mut self,
        config: impl Into<crate::$sequence<'c, S>>,
        extensions: crate::SolverExtensions<'_, $problem, $assignment, $movement, S, $error>,
    ) -> Result<crate::SolverResult<$assignment, S>, crate::SolverError<$local_error>>
    where S: 'c,
    {
        crate::solver::scoped_retained::$solver(self.problem, self, config.into(), extensions)
    }
    pub(crate) fn capture_retained(&self) -> crate::retained_scoped_phase::ScopedSnapshot<$assignment, S> {
        crate::retained_scoped_phase::ScopedSnapshot {
            assignment: self.assignment.clone(), contributions: self.contributions.clone(), score: self.score.clone(),
        }
    }
    pub(crate) fn install_retained_state(&mut self, snapshot: crate::retained_scoped_phase::ScopedSnapshot<$assignment, S>) {
        self.assignment = snapshot.assignment;
        self.contributions = snapshot.contributions;
        self.score = snapshot.score;
    }
    pub(crate) fn prepare_retained_phase(&mut self, initial: $assignment) -> Result<u128, $error> {
        // Only engine-produced handoffs from this same session reach this method.
        // Unlike the first exact candidate, they retain its payload representatives.
        if self.assignment == initial { return Ok(0); }
        self.replace_retained_assignment(initial, false)
    }
    pub(crate) fn run_construction_monitored(
        &mut self,
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<$assignment>,
        mut observer: crate::retained_stream_phase::Observer<'_, $assignment, S>,
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
        shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
        initial: Option<$assignment>,
        mut observer: crate::retained_stream_phase::Observer<'_, $assignment, S>,
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
        let clock = crate::provider_search::clock(&config$(.$clock_field)?);
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
        context: crate::provider_search::ProviderContext<'_, $problem, $assignment, $movement, S>,
    ) -> Result<crate::ProviderConstructionResult<$assignment, S>, $error> {
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
pub(crate) use retained_scoped;
