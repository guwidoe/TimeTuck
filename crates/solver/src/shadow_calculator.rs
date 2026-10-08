//! Use declarative shadow projections in native calculator-backed search.
use crate::{
    AdditiveScore, IncrementalCalculator, ManagedSolution, MatchRegistry, MatchRegistryError,
    ShadowSolutionManager, SolutionUpdatePolicy,
};
use std::{fmt, marker::PhantomData};

/// Bridge an owned shadow manager into basic, list or mixed calculator search.
/// `initial_shadows` supplies an empty projection for each reset; H need not be
/// Clone. Each factory supplied to a search director must build an independent
/// manager/session with the same immutable domain facts and scoring objective.
///
/// Reset and after_change refresh shadows without scoring. This includes inverse
/// notifications during move rollback and replay of a selected move. Scoring is
/// deferred to the calculator score hook, so replay does not score a winner twice.
/// The bridge polls the graph's inputs after every change; Change is a native
/// callback type, not a promise that unmentioned graph inputs stayed unchanged.
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
            state: None,
            ready: false,
            changing: false,
            change_type: PhantomData,
        }
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
    fn reset(
        &mut self,
        solution: &A,
        _: &mut MatchRegistry<S, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.ready = false;
        self.changing = false;
        let mut next = ManagedSolution::new(solution.clone(), (self.initial_shadows)());
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
        _: &C,
        _: &mut MatchRegistry<S, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.ready = false;
        if !self.changing {
            return Err(MatchRegistryError::NeedsReset);
        }
        self.changing = false;
        let state = self.state.as_mut().ok_or(MatchRegistryError::NeedsReset)?;
        state.assignment = solution.clone();
        state.score = None;
        self.manager
            .update_with_policy(state, SolutionUpdatePolicy::ShadowsOnly)
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
