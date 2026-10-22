//! Incremental calculator scoring for mixed planning models.
use crate::*;
use crate::{mixed_scoring::MixedSelection, mixed_search_scoring::MixedSearchDirector};
use std::panic::{AssertUnwindSafe, catch_unwind};
/// Separate namespaces for the whole move, including no-op touched slots.
/// List entities identify all potentially changed membership/order/neighbor facts.
/// Callbacks see both components in the same complete pre/post state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MixedCalculatorChange {
    pub basic_variables: Vec<usize>,
    pub list_entities: Vec<usize>,
}
crate::calculator_backend::calculator_backend!(MixedCalculatorScoreDirector, MixedCalculatorMove, [B, L], MixedAssignment<B, L>, MixedProblem<B, L>, MixedMove<B, L>, MixedCalculatorChange, MixedScoringError, mixed_changes, mixed_structure, crate::calculator_backend::full_structure_scan, apply_validated);
fn mixed_changes<B, L>(movement: &MixedMove<B, L>) -> MixedCalculatorChange {
    MixedCalculatorChange {
        basic_variables: movement.basic.touched_variables(),
        list_entities: movement.lists.touched_entities(),
    }
}
fn mixed_structure<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    score: S,
    problem: &MixedProblem<B, L>,
    assignment: &MixedAssignment<B, L>,
) -> Result<S, MixedScoringError> {
    crate::mixed_scoring::with_structure(score, problem, assignment)
}
impl<B, L, C, F> MixedSearchDirector<B, L, C::Score>
    for MixedCalculatorScoreDirector<'_, B, L, C, F>
where
    B: Eq + Clone,
    L: Eq + Clone,
    C: IncrementalCalculator<MixedAssignment<B, L>, Change = MixedCalculatorChange>,
    F: Fn() -> C,
{
    type Error = MixedScoringError;
    fn assignment(&self) -> &MixedAssignment<B, L> {
        self.assignment()
    }
    fn score(&self) -> C::Score {
        self.score.clone()
    }
    fn notify_best(&mut self, _complete: impl FnOnce(&MixedAssignment<B, L>) -> bool) {
        self.capture_metrics();
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        1
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = MixedMove<B, L>>,
        max_evaluations: u64,
        forager: impl Into<ForagerConfig<C::Score>>,
        mut accepts: impl FnMut(&MixedMove<B, L>, C::Score) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<C::Score>,
        assert_scores: bool,
    ) -> Result<MixedSelection<B, L>, MixedScoringError> {
        self.healthy()?;
        let mut result = MixedSelection {
            selected_accepted: false,
            selected: None,
            evaluations: 0,
            replay_evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut forager = MoveForager::new(forager.into());
        let rescore_selected =
            self.undo_diagnostics == crate::CalculatorUndoDiagnostics::NonIntrusive;
        let trial_limit = max_evaluations.saturating_sub(u64::from(rescore_selected));
        for movement in moves {
            if result.evaluations == trial_limit {
                result.evaluation_limit = true;
                break;
            }
            let undo_state = self.capture_undo()?;
            let candidate = self.apply(&movement)?;
            result.evaluations += 1;
            result.constraint_evaluations += 1;
            if assert_scores {
                candidate.director.assert_consistent()?;
            }
            let score = candidate.score();
            let accepted = accepts(&movement, score.clone());
            forager.try_add_with(
                score.clone(),
                accepted,
                || -> Result<_, MixedScoringError> {
                    Ok((movement.clone(), score.clone(), accepted))
                },
            )?;
            let stop = forager.is_quit_early() || (accepted && quit_early.should_pick(score));
            drop(candidate);
            self.healthy()?;
            self.verify_undo(undo_state, assert_scores)?;
            if stop {
                break;
            }
        }
        if !quit_early.can_commit(result.evaluation_limit) {
            return Ok(result);
        }
        if let Some((movement, score, accepted)) = forager.pick() {
            if rescore_selected {
                self.apply(&movement)?.commit();
                result.replay_evaluations += 1;
                result.evaluations += 1;
                result.constraint_evaluations += 1;
            } else {
                self.apply_with_score(&movement, Some(score))?.commit();
            }
            if assert_scores {
                self.assert_consistent()?;
            }
            result.selected = Some(movement);
            result.selected_accepted = accepted;
        }
        Ok(result)
    }
}

impl<B, L, C, F>
    crate::custom_phase::CommandDirector<MixedAssignment<B, L>, MixedMove<B, L>, C::Score>
    for MixedCalculatorScoreDirector<'_, B, L, C, F>
where
    B: Eq + Clone,
    L: Eq + Clone,
    C: IncrementalCalculator<MixedAssignment<B, L>, Change = MixedCalculatorChange>,
    F: Fn() -> C,
{
    type Error = MixedScoringError;
    fn assignment(&self) -> &MixedAssignment<B, L> {
        self.assignment()
    }
    fn score(&self) -> C::Score {
        self.score.clone()
    }
    fn notify_best(&mut self, _complete: bool) {
        self.capture_metrics();
    }
    fn initial_evaluations(&self) -> u128 {
        1
    }
    fn assert_consistent(&mut self) -> Result<(), MixedScoringError> {
        self.assert_consistent()
    }
    fn restore(&mut self, assignment: MixedAssignment<B, L>) -> Result<u128, MixedScoringError> {
        self.reset(assignment)?;
        Ok(1)
    }
    fn trial(
        &mut self,
        movement: &MixedMove<B, L>,
        visit: &mut dyn FnMut(&MixedAssignment<B, L>, C::Score, u128) -> bool,
    ) -> Result<(), MixedScoringError> {
        let undo_state = self.capture_undo()?;
        let candidate = self.apply(movement)?;
        if visit(candidate.assignment(), candidate.score(), 1) {
            candidate.commit();
            self.healthy()
        } else {
            drop(candidate);
            self.healthy()?;
            self.verify_undo(undo_state, false)
        }
    }
}

/// Greedy construction using the same placement, interruption and budget policies
/// as scoped scoring. Explanations are disabled during standalone search.
pub fn mixed_calculator_construction<B, L, C>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    factory: impl Fn() -> C,
    config: MixedConstructionConfig<'_>,
) -> Result<MixedConstructionResult<B, L, C::Score>, MixedScoringError>
where
    B: Eq + Clone,
    L: Eq + Clone,
    C: IncrementalCalculator<MixedAssignment<B, L>, Change = MixedCalculatorChange>,
{
    problem.validate(&initial)?;
    crate::mixed_construction::construction_impl_monitored(None, problem, initial, config, |a| {
        MixedCalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Native local search with existing acceptance, forager, neighborhood and limit
/// semantics. Use explicit construction first for an incomplete starting state.
pub fn mixed_calculator_local_search_from<B, L, C>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    factory: impl Fn() -> C,
    config: MixedLocalSearchConfig<'_, C::Score>,
) -> Result<MixedLocalSearchResult<B, L, C::Score>, MixedLocalSearchError>
where
    B: Eq + Clone,
    L: Eq + Clone,
    C: IncrementalCalculator<MixedAssignment<B, L>, Change = MixedCalculatorChange>,
{
    crate::mixed_local_search::search_impl_monitored(None, problem, initial, config, |a| {
        MixedCalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Local search over a caller-defined lazy neighborhood.
pub fn mixed_calculator_local_search_with_provider<B, L, C>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    factory: impl Fn() -> C,
    config: MixedLocalSearchConfig<'_, C::Score>,
    provider: &mut dyn MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<MixedLocalSearchResult<B, L, C::Score>, MixedLocalSearchError>
where
    B: Eq + Clone,
    L: Eq + Clone,
    C: IncrementalCalculator<MixedAssignment<B, L>, Change = MixedCalculatorChange>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.search.max_time.is_some()
            || config.search.max_unimproved_time.is_some()
            || config.search.diminished_returns.is_some()
            || config.search.significant_improvement.is_some()
            || config.search.uses_elapsed_acceptance(),
    );
    crate::mixed_local_search::search_impl_with_provider(
        None,
        problem,
        initial,
        config,
        |a| MixedCalculatorScoreDirector::new(problem, a, factory, false),
        clock,
        Some(crate::MoveProviderBinding::Infallible(provider)),
    )
}
/// Run bounded custom commands and publish initial/best updates using the shared
/// custom-phase engine. Trial moves use inverse calculator notifications.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn mixed_calculator_custom_phase<B, L, C>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    factory: impl Fn() -> C,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<
            '_,
            MixedAssignment<B, L>,
            MixedMove<B, L>,
            C::Score,
            MixedScoringError,
        >,
    ) -> Result<(), CustomPhaseError<MixedScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, C::Score>),
) -> Result<CustomPhaseResult<MixedAssignment<B, L>, C::Score>, CustomPhaseError<MixedScoringError>>
where
    B: Eq + Clone,
    L: Eq + Clone,
    C: IncrementalCalculator<MixedAssignment<B, L>, Change = MixedCalculatorChange>,
{
    problem
        .validate(&initial)
        .map_err(|e| CustomPhaseError::Scoring(MixedScoringError::Model(e)))?;
    crate::custom_phase::run_monitored(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| MixedCalculatorScoreDirector::new(problem, a, factory, false),
        command,
        observer,
        None,
        None,
        false,
    )
}

crate::retained_calculator_phase::retained_calculator!(MixedCalculatorScoreDirector, [B,L], MixedAssignment<B,L>, MixedProblem<B,L>, MixedMove<B,L>, MixedCalculatorChange, MixedScoringError, MixedLocalSearchError, MixedSolverSequence, mixed, mixed_construction, MixedConstructionConfig<'_>, MixedConstructionResult<B,L,C::Score>, mixed_local_search, MixedLocalSearchConfig<'_,C::Score>, MixedLocalSearchResult<B,L,C::Score>, mixed_director, |a| a, [search]);

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn mixed_calculator_local_search_with_fallible_provider<B, L, C>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    factory: impl Fn() -> C,
    config: MixedLocalSearchConfig<'_, C::Score>,
    provider: &mut dyn crate::FallibleMoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<MixedLocalSearchResult<B, L, C::Score>, MixedLocalSearchError>
where
    B: Eq + Clone,
    L: Eq + Clone,
    C: IncrementalCalculator<MixedAssignment<B, L>, Change = MixedCalculatorChange>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.search.max_time.is_some()
            || config.search.max_unimproved_time.is_some()
            || config.search.diminished_returns.is_some()
            || config.search.significant_improvement.is_some(),
    );
    crate::mixed_local_search::search_impl_with_provider(
        None,
        problem,
        initial,
        config,
        |a| MixedCalculatorScoreDirector::new(problem, a, factory, false),
        clock,
        Some(crate::MoveProviderBinding::Fallible(provider)),
    )
}
