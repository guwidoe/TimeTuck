//! Use declarative shadow projections in native calculator-backed search.
use crate::{
    AdditiveScore, IncrementalCalculator, ManagedSolution, MatchRegistry, MatchRegistryError,
    ShadowSession, ShadowSessionCheckpoint, ShadowSolutionManager, SolutionUpdatePolicy,
};
use std::{fmt, marker::PhantomData};

type ChangeNotifier<'a, 's, K, A, E, C> =
    dyn FnMut(&A, &A, &C, &mut ShadowSession<'a, 's, K, A>) -> Result<(), E> + 'a;

type UndoStateCapture<'a, A, H> =
    dyn Fn(&A, &H) -> Result<crate::CalculatorStateSnapshot, MatchRegistryError> + 'a;

/// Bridge an owned shadow manager into basic, list or mixed calculator search.
/// `initial_shadows` supplies an empty projection for each reset; H need not be
/// Clone. Each factory supplied to a search director must build an independent
/// manager/session with the same immutable domain facts and scoring objective.
///
/// Reset and after_change refresh shadows without scoring. This includes inverse
/// notifications during move rollback and replay of a selected move. Scoring is
/// deferred to the calculator score hook, so replay does not score a winner twice.
/// By default the bridge polls the graph's inputs after every change; Change is
/// a native callback type, not a promise that unmentioned graph inputs stayed
/// unchanged. [`Self::with_change_notifier`] opts into complete notifications.
/// Assignments are cloned into the manager's owned working state on each change;
/// the graph retains its incremental caches. Structural penalties are supplied by
/// the search director, and nonzero structural scores from the adapter are rejected.
///
/// Errors and callback panics make the calculator unreadable until reset. Manager
/// errors become MatchRegistryError::Calculator diagnostics. This bridge does not
/// register constraint matches or advertise explanation support. Captured callback
/// effects/interior mutation and panicking destructors retain the manager's limits.
/// The infallible calculate_score trait hook panics on error; CalculatorSession
/// and native directors use the fallible hook instead.
pub struct ShadowCalculator<'a, 's, K, A, H, S, E, C = Vec<usize>> {
    manager: ShadowSolutionManager<'a, 's, K, A, H, S, E>,
    initial_shadows: Box<dyn FnMut() -> H + 'a>,
    undo_diagnostics: crate::CalculatorUndoDiagnostics,
    recompute_live_projection: bool,
    undo_capture: Option<Box<UndoStateCapture<'a, A, H>>>,
    notifier: Option<Box<ChangeNotifier<'a, 's, K, A, E, C>>>,
    state: Option<ManagedSolution<A, H, S>>,
    ready: bool,
    changing: bool,
    change_type: PhantomData<fn(C)>,
}
impl<'a, 's, K: Eq + Clone + fmt::Debug, A: Clone, H, S: AdditiveScore, E: fmt::Display, C>
    ShadowCalculator<'a, 's, K, A, H, S, E, C>
{
    pub fn new(
        manager: ShadowSolutionManager<'a, 's, K, A, H, S, E>,
        initial_shadows: impl FnMut() -> H + 'a,
    ) -> Self {
        Self {
            manager,
            initial_shadows: Box::new(initial_shadows),
            notifier: None,
            undo_capture: None,
            undo_diagnostics: crate::CalculatorUndoDiagnostics::Default,
            recompute_live_projection: false,
            state: None,
            ready: false,
            changing: false,
            change_type: PhantomData,
        }
    }
    /// Configure fresh native searches using this calculator without requiring
    /// a cloneable shadow projection or changing the calculator factory interface.
    pub fn with_undo_diagnostics(mut self, policy: crate::CalculatorUndoDiagnostics) -> Self {
        self.undo_diagnostics = policy;
        self
    }
    /// Use a fresh manager's scorer on the actual live shadow projection during
    /// consistency checks, without resetting or refreshing that projection.
    /// This checks scoring independently of shadow recomputation. Explicit undo
    /// diagnostics still compare the restored score and tracked fields. The
    /// manager factory must create independently initialized scorer state.
    pub fn with_live_shadow_score_recomputation(mut self) -> Self {
        self.recompute_live_projection = true;
        self
    }
    /// Supply immutable owned field captures from the actual live assignment and
    /// shadow projection for Tracked calculator undo checks. Called only when
    /// explicitly requested; no scoring, refresh or reset is performed here.
    /// Captures must not alias mutable model/projection storage.
    pub fn with_undo_state_capture(
        mut self,
        capture: impl Fn(&A, &H) -> Result<crate::CalculatorStateSnapshot, MatchRegistryError> + 'a,
    ) -> Self {
        self.undo_capture = Some(Box::new(capture));
        self
    }
    /// Opt into notified shadow refreshes after each matched change callback.
    /// The notifier receives the calculator's last published assignment, the new
    /// assignment, and the native change payload. It must mark every changed
    /// input, binding target and direct external-fact supplier as required by
    /// [`ShadowSession::update_notified`]. It may also edit manual dependencies.
    /// Payloads alone are not a complete graph change set; domain adapters must
    /// account for indirect effects such as list predecessor changes.
    ///
    /// The notifier runs on forward moves, inverse moves during rollback, and
    /// winner replay. Before/after assignments describe that actual direction;
    /// identical assignments and no-op changes may still receive a callback.
    /// Reset invalidates every field and performs a full refresh without calling
    /// the notifier, including after failure. Scoring never calls the notifier.
    ///
    /// A notifier error or panic restores its session edits and makes this
    /// calculator unreadable until reset, like other callback failures. Captured
    /// callback state and external effects are not rolled back. A failure later
    /// in projection or validation retains the successful notifications for reset.
    /// Session checkpoints, graph bookkeeping and projection still perform work
    /// beyond the notified fields; this reduces input/binding callback work, not
    /// necessarily total refresh time. Replaces any previously installed notifier.
    pub fn with_change_notifier(
        mut self,
        notify: impl FnMut(&A, &A, &C, &mut ShadowSession<'a, 's, K, A>) -> Result<(), E> + 'a,
    ) -> Self {
        self.notifier = Some(Box::new(notify));
        self
    }
    /// Current assignment and projection, available after reset or after_change.
    /// Score is None until scoring succeeds, and is cleared after each change.
    pub fn solution(&self) -> Result<&ManagedSolution<A, H, S>, MatchRegistryError> {
        self.require_ready()?;
        self.state.as_ref().ok_or(MatchRegistryError::NeedsReset)
    }
    fn require_ready(&self) -> Result<(), MatchRegistryError> {
        if self.ready {
            Ok(())
        } else {
            Err(MatchRegistryError::NeedsReset)
        }
    }
    fn try_score(&mut self) -> Result<S, MatchRegistryError> {
        self.changing = false;
        self.require_ready()?;
        self.ready = false;
        let state = self.state.as_mut().ok_or(MatchRegistryError::NeedsReset)?;
        let score = self
            .manager
            .update_with_policy(state, SolutionUpdatePolicy::ScoreOnly)
            .map_err(|e| MatchRegistryError::Calculator(e.to_string()))?
            .ok_or(MatchRegistryError::NeedsReset)?
            .clone();
        if score.structural() != 0 {
            return Err(MatchRegistryError::StructuralContribution);
        }
        self.ready = true;
        Ok(score)
    }
}
impl<K: Eq + Clone + fmt::Debug, A: Clone, H, S: AdditiveScore, E: fmt::Display, C>
    IncrementalCalculator<A> for ShadowCalculator<'_, '_, K, A, H, S, E, C>
{
    type Score = S;
    type Justification = ();
    type Change = C;
    fn undo_diagnostics(&self) -> crate::CalculatorUndoDiagnostics {
        self.undo_diagnostics
    }
    fn recompute_score_from(&mut self, working: &Self) -> Result<Option<S>, MatchRegistryError> {
        if !self.recompute_live_projection {
            return Ok(None);
        }
        working.require_ready()?;
        let state = working
            .state
            .as_ref()
            .ok_or(MatchRegistryError::NeedsReset)?;
        self.manager
            .score_projection(&state.assignment, &state.shadows)
            .map(Some)
            .map_err(|e| MatchRegistryError::Calculator(e.to_string()))
    }
    fn capture_undo_state(
        &self,
    ) -> Result<Option<crate::CalculatorStateSnapshot>, MatchRegistryError> {
        self.require_ready()?;
        let Some(capture) = &self.undo_capture else {
            return Ok(None);
        };
        let state = self.state.as_ref().ok_or(MatchRegistryError::NeedsReset)?;
        capture(&state.assignment, &state.shadows).map(Some)
    }
    fn reset(
        &mut self,
        solution: &A,
        _: &mut MatchRegistry<S, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.ready = false;
        self.changing = false;
        let mut next = ManagedSolution::new(solution.clone(), (self.initial_shadows)());
        if self.notifier.is_some() {
            self.manager.session_mut().invalidate_all();
        }
        self.manager
            .update_with_policy(&mut next, SolutionUpdatePolicy::ShadowsOnly)
            .map_err(|e| MatchRegistryError::Calculator(e.to_string()))?;
        self.state = Some(next);
        self.ready = true;
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &A,
        _: &C,
        _: &mut MatchRegistry<S, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.changing = false;
        self.require_ready()?;
        self.ready = false;
        self.changing = true;
        Ok(())
    }
    fn after_change(
        &mut self,
        solution: &A,
        change: &C,
        _: &mut MatchRegistry<S, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.ready = false;
        if !self.changing {
            return Err(MatchRegistryError::NeedsReset);
        }
        self.changing = false;
        let state = self.state.as_mut().ok_or(MatchRegistryError::NeedsReset)?;
        let next_assignment = solution.clone();
        state.score = None;
        if let Some(notify) = &mut self.notifier {
            let session = self.manager.session_mut();
            let saved = session.capture();
            let mut guard = NotificationGuard {
                session,
                saved: Some(saved),
            };
            notify(&state.assignment, &next_assignment, change, guard.session)
                .map_err(|e| MatchRegistryError::Calculator(e.to_string()))?;
            guard.saved = None;
        }
        state.assignment = next_assignment;
        if self.notifier.is_some() {
            self.manager
                .update_notified_with_policy(state, SolutionUpdatePolicy::ShadowsOnly)
        } else {
            self.manager
                .update_with_policy(state, SolutionUpdatePolicy::ShadowsOnly)
        }
        .map_err(|e| MatchRegistryError::Calculator(e.to_string()))?;
        self.ready = true;
        Ok(())
    }
    fn calculate_score(&mut self) -> S {
        self.try_score()
            .expect("declarative scoring failed; use the fallible calculator hook")
    }
    fn calculate_score_with_matches(
        &mut self,
        _: &mut MatchRegistry<S, ()>,
    ) -> Result<S, MatchRegistryError> {
        self.try_score()
    }
}

struct NotificationGuard<'r, 'a, 's, K: Eq, A> {
    session: &'r mut ShadowSession<'a, 's, K, A>,
    saved: Option<ShadowSessionCheckpoint>,
}
impl<K: Eq, A> Drop for NotificationGuard<'_, '_, '_, K, A> {
    fn drop(&mut self) {
        if let Some(saved) = &self.saved {
            self.session.install(saved);
        }
    }
}
