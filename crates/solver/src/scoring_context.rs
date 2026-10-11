//! Retained scoring resources and explicit runtime phase transitions.
use crate::{AdditiveScore, EnvironmentMode};
use std::{
    cell::{Ref, RefCell},
    error::Error,
    fmt,
    rc::Rc,
};

pub type ContextRebuilder<D> =
    fn(&mut D, ContextSettings) -> Result<D, <D as ContextScoreResource>::Error>;
pub type ContextFactory<'a, D> =
    Box<dyn FnMut(&mut D, ContextSettings) -> Result<D, <D as ContextScoreResource>::Error> + 'a>;

/// Whether consumers may request named constraint totals. Incremental caches
/// remain available internally even when reporting is disabled.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextMatchPolicy {
    Disabled,
    Enabled,
}

/// Metrics requested before creating a runtime context.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContextMetrics {
    pub best_score: bool,
    pub best_constraint_totals: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextSettings {
    pub mode: EnvironmentMode,
    pub matching: ContextMatchPolicy,
}
impl ContextSettings {
    fn new(mode: EnvironmentMode, metrics: ContextMetrics) -> Self {
        Self {
            mode,
            matching: if metrics.best_constraint_totals || mode >= EnvironmentMode::PhaseAssert {
                ContextMatchPolicy::Enabled
            } else {
                ContextMatchPolicy::Disabled
            },
        }
    }
}

/// A live scoring resource. Implementations must release external resources in
/// `release_context` (or Drop); a failed release is still followed by Drop. Native dense
/// and stream directors implement this interface without an extra candidate hook.
pub trait ContextScoreResource {
    type Score: AdditiveScore;
    type Error: Error;
    fn context_score(&self) -> Self::Score;
    fn check_context(&mut self) -> Result<(), Self::Error>;
    fn context_totals(&self) -> Vec<(String, Self::Score)>;
    fn release_context(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Successful native phase work, including its initial logical evaluation.
pub trait ContextScoreWork {
    fn context_calculations(&self) -> u64;
}
impl<V, S> ContextScoreWork for crate::ConstructionResult<V, S> {
    fn context_calculations(&self) -> u64 {
        self.evaluations
    }
}
impl<V, S> ContextScoreWork for crate::LocalSearchResult<V, S> {
    fn context_calculations(&self) -> u64 {
        self.evaluations
    }
}

struct Resource<D> {
    director: Option<D>,
    settings: ContextSettings,
    count: u128,
}

/// An observable resource handle. A retained old handle never keeps its scoring
/// director alive after replacement or shutdown. `director()` then returns None.
pub struct ScoringContextHandle<D>(Rc<RefCell<Resource<D>>>);
impl<D> Clone for ScoringContextHandle<D> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<D> ScoringContextHandle<D> {
    pub fn same_resource(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
    pub fn director(&self) -> Option<Ref<'_, D>> {
        Ref::filter_map(self.0.borrow(), |state| state.director.as_ref()).ok()
    }
    pub fn settings(&self) -> ContextSettings {
        self.0.borrow().settings
    }
    pub fn calculation_count(&self) -> u128 {
        self.0.borrow().count
    }
}

/// Editing access tied to the current resource. Old editing handles reject edits
/// after replacement. Edits are distinct from logical score calculation calls;
/// use the director's transactional move/edit APIs inside the callback.
pub struct ContextChangeDirector<D> {
    identity: Rc<()>,
    handle: ScoringContextHandle<D>,
}
impl<D> Clone for ContextChangeDirector<D> {
    fn clone(&self) -> Self {
        Self {
            identity: self.identity.clone(),
            handle: self.handle.clone(),
        }
    }
}
impl<D: ContextScoreResource> ContextChangeDirector<D> {
    fn new(handle: ScoringContextHandle<D>) -> Self {
        Self {
            identity: Rc::new(()),
            handle,
        }
    }
    pub fn same_resource(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.identity, &other.identity)
    }
    pub fn edit<R>(
        &self,
        edit: impl FnOnce(&mut D) -> Result<R, D::Error>,
    ) -> Result<R, ScoringContextError<D::Error>> {
        let mut state = self.handle.0.borrow_mut();
        let director = state.director.as_mut().ok_or(ScoringContextError::Closed)?;
        edit(director).map_err(ScoringContextError::Scoring)
    }
}

#[derive(Debug)]
pub enum ScoringContextError<E> {
    Closed,
    NotStarted,
    PhaseIndex { phase: usize, count: usize },
    Scoring(E),
    Release(E),
}
impl<E: fmt::Display> fmt::Display for ScoringContextError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => f.write_str("scoring context is closed"),
            Self::NotStarted => f.write_str("scoring context has not started"),
            Self::PhaseIndex { phase, count } => {
                write!(f, "phase {phase} is outside {count} configured phases")
            }
            Self::Scoring(e) => write!(f, "scoring context failed: {e}"),
            Self::Release(e) => write!(f, "scoring context release failed: {e}"),
        }
    }
}
impl<E: Error + 'static> Error for ScoringContextError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoring(e) | Self::Release(e) => Some(e),
            _ => None,
        }
    }
}

/// The primary error remains owned by the caller. Cleanup appends the actual
/// release error, without replacing, cloning or stringifying either error.
#[derive(Debug)]
pub struct ContextFailure<P, E> {
    pub primary: P,
    pub suppressed: Vec<E>,
}
impl<P, E> ContextFailure<P, E> {
    pub fn new(primary: P) -> Self {
        Self {
            primary,
            suppressed: Vec::new(),
        }
    }
}

/// Runtime mode scheduling, separate from EnvironmentProfile validation. A
/// direct schedule may include weaker modes; validated applications should pass
/// PreparedEnvironmentProfile::phase_modes().
///
/// The replacement factory must prepare all fallible work before transferring
/// working state, leaving the old director usable on error. Native into_context
/// constructors do this and move the assignment buffer without cloning it.
/// Same-mode transitions call neither the factory nor release. A mode change
/// creates a new handle and editing resource, carries the calculation count,
/// then releases the prior director. Returning to an earlier mode builds afresh.
///
/// Logical counts cover calculate_score and successful run work. Diagnostic
/// recomputations and factory initialization are excluded. Failed arbitrary
/// callbacks have no recoverable work count. Full/step modes check at managed
/// calculation/work boundaries; per-step verification within a phase requires
/// independent SolverExtensions score checks. This is not Java lifecycle/ABI
/// parity or a replacement for whole-solve termination scheduling.
///
/// ```
/// use timetuck::*;
/// let problem = FiniteProblem::new(vec![Variable::new(vec![1, 2])])?;
/// let network = ConstraintNetwork::<_, SimpleScore>::new(1);
/// let director = ScoreDirector::new(&problem, &network, problem.initial_assignment())?;
/// let mut context = director.into_context(EnvironmentMode::NoAssert,
///     vec![EnvironmentMode::NoAssert, EnvironmentMode::FullAssert],
///     ContextMetrics { best_score: true, ..Default::default() });
/// context.start()?;
/// context.start_phase(0)?;
/// let old = context.handle();
/// let result = context.run(|d| d.run_construction(ConstructionConfig::default()))?;
/// context.start_phase(1)?;
/// assert!(old.director().is_none());
/// assert_eq!(context.calculation_count(), u128::from(result.evaluations));
/// context.finish()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct ScoringContextManager<D: ContextScoreResource, F> {
    current: ScoringContextHandle<D>,
    changes: ContextChangeDirector<D>,
    phases: Vec<EnvironmentMode>,
    metrics: ContextMetrics,
    factory: F,
    started: bool,
    best_score: Option<D::Score>,
}
impl<D: ContextScoreResource, F> ScoringContextManager<D, F> {
    pub fn new(
        director: D,
        global: EnvironmentMode,
        phases: Vec<EnvironmentMode>,
        metrics: ContextMetrics,
        factory: F,
    ) -> Self {
        let best_score = metrics.best_score.then(|| director.context_score());
        let current = ScoringContextHandle(Rc::new(RefCell::new(Resource {
            director: Some(director),
            settings: ContextSettings::new(global, metrics),
            count: 0,
        })));
        Self {
            changes: ContextChangeDirector::new(current.clone()),
            current,
            phases,
            metrics,
            factory,
            started: false,
            best_score,
        }
    }
    pub fn handle(&self) -> ScoringContextHandle<D> {
        self.current.clone()
    }
    pub fn change_director(&self) -> ContextChangeDirector<D> {
        self.changes.clone()
    }
    pub fn calculation_count(&self) -> u128 {
        self.current.calculation_count()
    }
    /// Best observed managed score, including the initialized resource. None
    /// when that metric was not requested. Retained after shutdown.
    pub fn best_score(&self) -> Option<&D::Score> {
        self.best_score.as_ref()
    }
    pub fn start(&mut self) -> Result<(), ScoringContextError<D::Error>> {
        if self.current.director().is_none() {
            return Err(ScoringContextError::Closed);
        }
        self.started = true;
        Ok(())
    }
    pub fn calculate_score(&mut self) -> Result<D::Score, ScoringContextError<D::Error>> {
        let mut state = self.current.0.borrow_mut();
        let mode = state.settings.mode;
        let director = state.director.as_mut().ok_or(ScoringContextError::Closed)?;
        if mode >= EnvironmentMode::StepAssert {
            director
                .check_context()
                .map_err(ScoringContextError::Scoring)?;
        }
        let score = director.context_score();
        state.count = state.count.saturating_add(1);
        if self.metrics.best_score && self.best_score.as_ref().is_none_or(|best| score > *best) {
            self.best_score = Some(score.clone());
        }
        Ok(score)
    }
    /// Named totals are fetched only on demand, outside the candidate loop.
    /// Disabled matching returns None, rather than a misleading empty report.
    pub fn constraint_totals(&self) -> Option<Vec<(String, D::Score)>> {
        let state = self.current.0.borrow();
        if state.settings.matching == ContextMatchPolicy::Disabled {
            return None;
        }
        state
            .director
            .as_ref()
            .map(ContextScoreResource::context_totals)
    }
    /// Run actual director work and retain its committed state. Counts come from
    /// native result fields, not elapsed time or a caller-provided estimate.
    pub fn run<R: ContextScoreWork, E: From<D::Error>>(
        &mut self,
        work: impl FnOnce(&mut D) -> Result<R, E>,
    ) -> Result<R, ScoringContextError<E>> {
        if !self.started {
            return Err(ScoringContextError::NotStarted);
        }
        let mut state = self.current.0.borrow_mut();
        let mode = state.settings.mode;
        let director = state.director.as_mut().ok_or(ScoringContextError::Closed)?;
        let result = work(director).map_err(ScoringContextError::Scoring)?;
        // Count completed work even if a subsequent diagnostic fails.
        state.count = state
            .count
            .saturating_add(u128::from(result.context_calculations()));
        if mode >= EnvironmentMode::StepAssert {
            state
                .director
                .as_mut()
                .unwrap()
                .check_context()
                .map_err(|error| ScoringContextError::Scoring(E::from(error)))?;
        }
        if self.metrics.best_score {
            let score = state.director.as_ref().unwrap().context_score();
            if self.best_score.as_ref().is_none_or(|best| score > *best) {
                self.best_score = Some(score);
            }
        }
        Ok(result)
    }
    pub fn finish(&mut self) -> Result<(), ScoringContextError<D::Error>> {
        let director = self.current.0.borrow_mut().director.take();
        if let Some(mut director) = director {
            director
                .release_context()
                .map_err(ScoringContextError::Release)?;
        }
        Ok(())
    }
    pub fn solving_error<P>(&mut self, failure: &mut ContextFailure<P, D::Error>) {
        if let Err(ScoringContextError::Release(error)) = self.finish() {
            failure.suppressed.push(error);
        }
    }
}
impl<D, F> ScoringContextManager<D, F>
where
    D: ContextScoreResource,
    F: FnMut(&mut D, ContextSettings) -> Result<D, D::Error>,
{
    pub fn start_phase(&mut self, phase: usize) -> Result<(), ScoringContextError<D::Error>> {
        if !self.started {
            return Err(ScoringContextError::NotStarted);
        }
        let mode = *self
            .phases
            .get(phase)
            .ok_or(ScoringContextError::PhaseIndex {
                phase,
                count: self.phases.len(),
            })?;
        let settings = ContextSettings::new(mode, self.metrics);
        let mut old = self.current.0.borrow_mut();
        if old.director.is_none() {
            return Err(ScoringContextError::Closed);
        }
        // Phase mode checks run on the outgoing committed state before transfer.
        if old.settings.mode == EnvironmentMode::PhaseAssert {
            old.director
                .as_mut()
                .unwrap()
                .check_context()
                .map_err(ScoringContextError::Scoring)?;
        }
        if old.settings == settings {
            return Ok(());
        }
        let next = (self.factory)(old.director.as_mut().unwrap(), settings)
            .map_err(ScoringContextError::Scoring)?;
        let count = old.count;
        let mut retired = old.director.take().unwrap();
        drop(old);
        self.current = ScoringContextHandle(Rc::new(RefCell::new(Resource {
            director: Some(next),
            settings,
            count,
        })));
        self.changes = ContextChangeDirector::new(self.current.clone());
        retired
            .release_context()
            .map_err(ScoringContextError::Release)
    }
}
impl<D: ContextScoreResource, F> Drop for ScoringContextManager<D, F> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

impl<V: Eq + Clone, S: AdditiveScore> ContextScoreResource for crate::ScoreDirector<'_, '_, V, S> {
    type Score = S;
    type Error = crate::ScoringError;
    fn context_score(&self) -> S {
        self.score()
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn context_totals(&self) -> Vec<(String, S)> {
        self.constraint_scores()
            .map(|(name, score)| (name.to_owned(), score))
            .collect()
    }
}
impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static> ContextScoreResource
    for crate::StreamScoreDirector<'_, '_, V, S>
{
    type Score = S;
    type Error = crate::StreamScoringError;
    fn context_score(&self) -> S {
        self.score()
    }
    fn check_context(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn context_totals(&self) -> Vec<(String, S)> {
        self.constraint_scores()
            .map(|(name, score)| (name.to_owned(), score))
            .collect()
    }
}
