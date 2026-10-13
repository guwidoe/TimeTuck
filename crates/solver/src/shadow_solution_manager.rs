//! Atomic publication of declarative shadows and scores with session rollback.
use crate::shadow_session::{RefreshMode, update_shadows_mode};
use crate::{
    ConsistentShadowUpdateError, ManagedSolution, ShadowConsistencyPolicy, ShadowSession,
    ShadowSessionCheckpoint, SolutionUpdatePolicy,
};
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum ShadowSolutionError<K, E> {
    Shadows(ConsistentShadowUpdateError<K>),
    Adapter(E),
}
impl<K: fmt::Debug, E: fmt::Display> fmt::Display for ShadowSolutionError<K, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shadows(e) => e.fmt(f),
            Self::Adapter(e) => write!(f, "solution refresh: {e}"),
        }
    }
}
impl<K: fmt::Debug + 'static, E: Error + 'static> Error for ShadowSolutionError<K, E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Shadows(e) => Some(e),
            Self::Adapter(e) => Some(e),
        }
    }
}

type Preflight<'a, A, E> = dyn FnMut(&A) -> Result<(), E> + 'a;

type Project<'a, 's, K, A, H, E> = dyn FnMut(&A, &ShadowSession<'a, 's, K, A>) -> Result<H, E> + 'a;
type Validate<'a, A, H, E> = dyn FnMut(&A, &H) -> Result<(), E> + 'a;
type Calculate<'a, A, H, S, E> = dyn FnMut(&A, &H) -> Result<S, E> + 'a;

/// Own a declarative session and publish its materialized shadows and score together.
/// Everything and ShadowsOnly require registered sources; consistency is required
/// by default and can be relaxed explicitly for domains that score inconsistency.
/// After refresh, `project` builds owned shadows, `validate` checks them, and
/// Everything calculates a score. ScoreOnly validates and scores existing shadows
/// without reading, refreshing or checking the session. ShadowsOnly retains any
/// previous score, which the caller must treat as stale after assignment changes.
///
/// Errors and ordinary callback panics restore the session's cache, dependency
/// edges and pending changes, and leave solution outputs unchanged. This includes
/// failure before the first successful refresh. Assignments, shadows and scores
/// need not implement Clone. Callbacks must not mutate supplied data through
/// interior state; external effects and callback captures are not rolled back.
/// Panicking destructors during successful publication are excluded.
pub struct ShadowSolutionManager<'a, 's, K, A, H, S, E> {
    consistency: ShadowConsistencyPolicy,
    preflight: Vec<Box<Preflight<'a, A, E>>>,
    session: ShadowSession<'a, 's, K, A>,
    project: Box<Project<'a, 's, K, A, H, E>>,
    validate: Box<Validate<'a, A, H, E>>,
    calculate: Box<Calculate<'a, A, H, S, E>>,
}
impl<'a, 's, K: Eq + Clone, A, H, S, E> ShadowSolutionManager<'a, 's, K, A, H, S, E> {
    pub fn new(
        session: ShadowSession<'a, 's, K, A>,
        project: impl FnMut(&A, &ShadowSession<'a, 's, K, A>) -> Result<H, E> + 'a,
        validate: impl FnMut(&A, &H) -> Result<(), E> + 'a,
        calculate: impl FnMut(&A, &H) -> Result<S, E> + 'a,
    ) -> Self {
        Self {
            consistency: ShadowConsistencyPolicy::RequireConsistent,
            preflight: Vec::new(),
            session,
            project: Box::new(project),
            validate: Box::new(validate),
            calculate: Box::new(calculate),
        }
    }
    /// Check the assignment before any graph reader or other adapter runs,
    /// including ScoreOnly. Replaces all previously installed preflight callbacks.
    /// Use `and_preflight` to preserve factory-installed model validation.
    pub fn with_preflight(mut self, check: impl FnMut(&A) -> Result<(), E> + 'a) -> Self {
        self.preflight.clear();
        self.preflight.push(Box::new(check));
        self
    }
    /// Append an assignment check without replacing existing checks, including
    /// the model validation installed by list and mixed manager factories.
    /// Checks run in insertion order under every update policy and stop at the
    /// first error or panic, before any graph or scoring callback. A later
    /// `with_preflight` call explicitly replaces the entire chain.
    pub fn and_preflight(mut self, check: impl FnMut(&A) -> Result<(), E> + 'a) -> Self {
        self.preflight.push(Box::new(check));
        self
    }
    /// Control consistency for graph refreshes; ScoreOnly continues to use the
    /// existing projection without refreshing or checking the graph. With
    /// AllowInconsistent, projection and scoring adapters must handle flags and
    /// missing derived values explicitly. Other errors remain errors.
    pub fn with_consistency_policy(mut self, policy: ShadowConsistencyPolicy) -> Self {
        self.consistency = policy;
        self
    }
    pub fn session(&self) -> &ShadowSession<'a, 's, K, A> {
        &self.session
    }
    /// Edit dependencies or mark fields dirty before the next refresh. Pending
    /// edits remain pending if that refresh fails; they are not silently discarded.
    pub fn session_mut(&mut self) -> &mut ShadowSession<'a, 's, K, A> {
        &mut self.session
    }
    pub(crate) fn score_projection(
        &mut self,
        assignment: &A,
        shadows: &H,
    ) -> Result<S, ShadowSolutionError<K, E>> {
        for check in &mut self.preflight {
            check(assignment).map_err(ShadowSolutionError::Adapter)?;
        }
        (self.validate)(assignment, shadows).map_err(ShadowSolutionError::Adapter)?;
        (self.calculate)(assignment, shadows).map_err(ShadowSolutionError::Adapter)
    }
    pub fn update<'r>(
        &mut self,
        solution: &'r mut ManagedSolution<A, H, S>,
    ) -> Result<Option<&'r S>, ShadowSolutionError<K, E>> {
        self.update_with_policy(solution, SolutionUpdatePolicy::Everything)
    }
    pub fn update_with_policy<'r>(
        &mut self,
        solution: &'r mut ManagedSolution<A, H, S>,
        policy: SolutionUpdatePolicy,
    ) -> Result<Option<&'r S>, ShadowSolutionError<K, E>> {
        self.update_mode(solution, policy, RefreshMode::All)
    }
    /// Publish shadows and score using complete caller notifications, with the
    /// contract of [`ShadowSession::update_notified`]. Notify changed fields via
    /// [`Self::session_mut`] before calling. Failed publication preserves those
    /// notifications for retry. Preflight, projection, validation and scoring
    /// callbacks still run normally; consistency checks inspect the cached graph.
    /// Missing-source diagnostics cover notified bindings only. An independent
    /// [`crate::missing_shadow_sources`] call still resolves the full world.
    pub fn update_notified<'r>(
        &mut self,
        solution: &'r mut ManagedSolution<A, H, S>,
    ) -> Result<Option<&'r S>, ShadowSolutionError<K, E>> {
        self.update_notified_with_policy(solution, SolutionUpdatePolicy::Everything)
    }
    /// Apply an update policy with the notification contract of
    /// [`Self::update_notified`]. ScoreOnly does not refresh the session or consume
    /// notifications; ShadowsOnly retains the previous, potentially stale score.
    pub fn update_notified_with_policy<'r>(
        &mut self,
        solution: &'r mut ManagedSolution<A, H, S>,
        policy: SolutionUpdatePolicy,
    ) -> Result<Option<&'r S>, ShadowSolutionError<K, E>> {
        self.update_mode(solution, policy, RefreshMode::Notified)
    }
    fn update_mode<'r>(
        &mut self,
        solution: &'r mut ManagedSolution<A, H, S>,
        policy: SolutionUpdatePolicy,
        mode: RefreshMode,
    ) -> Result<Option<&'r S>, ShadowSolutionError<K, E>> {
        for check in &mut self.preflight {
            check(&solution.assignment).map_err(ShadowSolutionError::Adapter)?;
        }
        if policy == SolutionUpdatePolicy::ScoreOnly {
            (self.validate)(&solution.assignment, &solution.shadows)
                .map_err(ShadowSolutionError::Adapter)?;
            let score = (self.calculate)(&solution.assignment, &solution.shadows)
                .map_err(ShadowSolutionError::Adapter)?;
            let old_score = solution.score.replace(score);
            drop(old_score);
            return Ok(solution.score.as_ref());
        }
        let saved = self.session.capture();
        let mut guard = RestoreOnDrop {
            session: &mut self.session,
            saved: Some(saved),
        };
        update_shadows_mode(guard.session, &solution.assignment, self.consistency, mode)
            .map_err(ShadowSolutionError::Shadows)?;
        let shadows = (self.project)(&solution.assignment, guard.session)
            .map_err(ShadowSolutionError::Adapter)?;
        (self.validate)(&solution.assignment, &shadows).map_err(ShadowSolutionError::Adapter)?;
        let score = if policy == SolutionUpdatePolicy::Everything {
            Some(
                (self.calculate)(&solution.assignment, &shadows)
                    .map_err(ShadowSolutionError::Adapter)?,
            )
        } else {
            None
        };
        let old_shadows = std::mem::replace(&mut solution.shadows, shadows);
        let old_score = score.and_then(|s| solution.score.replace(s));
        // Disarm before dropping either the saved cache or displaced outputs.
        let saved = guard.saved.take();
        drop((saved, old_shadows, old_score));
        Ok(solution.score.as_ref())
    }
}
struct RestoreOnDrop<'r, 'a, 's, K: Eq, A> {
    session: &'r mut ShadowSession<'a, 's, K, A>,
    saved: Option<ShadowSessionCheckpoint>,
}
impl<K: Eq, A> Drop for RestoreOnDrop<'_, '_, '_, K, A> {
    fn drop(&mut self) {
        if let Some(saved) = &self.saved {
            self.session.install(saved);
        }
    }
}
