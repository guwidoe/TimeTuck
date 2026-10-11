//! Sorted unique entity indices, including no-op touched entities. Calculators
//! tracking values, owners, indices or neighbors must inspect all values in these
//! entities before/after the change; unassigned membership changes follow from
//! their difference. Both callback states are available through the assignment.
//! Incremental calculator scoring for list planning models.
use crate::*;
use crate::{list_scoring::ListSelection, list_search_scoring::ListSearchDirector};
use std::panic::{AssertUnwindSafe, catch_unwind};
crate::calculator_backend::calculator_backend!(
    ListCalculatorScoreDirector,
    ListCalculatorMove,
    [V],
    ListAssignment<V>,
    ListProblem<V>,
    ListMove<V>,
    Vec<usize>,
    ListScoringError,
    list_changes,
    list_structure,
    apply_validated
);
fn list_changes<V>(movement: &ListMove<V>) -> Vec<usize> {
    movement.touched_entities()
}
fn list_structure<V: Eq + Clone, S: AdditiveScore>(
    score: S,
    problem: &ListProblem<V>,
    assignment: &ListAssignment<V>,
) -> Result<S, ListScoringError> {
    Ok(crate::list_scoring::with_structure(
        score, problem, assignment,
    ))
}
impl<V, C, F> ListSearchDirector<V, C::Score> for ListCalculatorScoreDirector<'_, V, C, F>
where
    V: Eq + Clone,
    C: IncrementalCalculator<ListAssignment<V>, Change = Vec<usize>>,
    F: Fn() -> C,
{
    type Error = ListScoringError;
    fn assignment(&self) -> &ListAssignment<V> {
        self.assignment()
    }
    fn score(&self) -> C::Score {
        self.score.clone()
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        1
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.assert_consistent()
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        forager: impl Into<ForagerConfig<C::Score>>,
        mut accepts: impl FnMut(&ListMove<V>, C::Score) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<C::Score>,
        assert_scores: bool,
    ) -> Result<ListSelection<V>, ListScoringError> {
        self.healthy()?;
        let mut result = ListSelection {
            selected_accepted: false,
            selected: None,
            evaluations: 0,
            constraint_evaluations: 0,
            evaluation_limit: false,
        };
        let mut forager = MoveForager::new(forager.into());
        for movement in moves {
            if result.evaluations == max_evaluations {
                result.evaluation_limit = true;
                break;
            }
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
                || -> Result<_, ListScoringError> {
                    Ok((movement.clone(), score.clone(), accepted))
                },
            )?;
            let stop = forager.is_quit_early() || (accepted && quit_early.should_pick(score));
            drop(candidate);
            self.healthy()?;
            if assert_scores {
                self.assert_consistent()?;
            }
            if stop {
                break;
            }
        }
        if !quit_early.can_commit(result.evaluation_limit) {
            return Ok(result);
        }
        if let Some((movement, score, accepted)) = forager.pick() {
            self.apply_with_score(&movement, Some(score))?.commit();
            if assert_scores {
                self.assert_consistent()?;
            }
            result.selected = Some(movement);
            result.selected_accepted = accepted;
        }
        Ok(result)
    }
}

impl<V, C, F> crate::custom_phase::CommandDirector<ListAssignment<V>, ListMove<V>, C::Score>
    for ListCalculatorScoreDirector<'_, V, C, F>
where
    V: Eq + Clone,
    C: IncrementalCalculator<ListAssignment<V>, Change = Vec<usize>>,
    F: Fn() -> C,
{
    type Error = ListScoringError;
    fn assignment(&self) -> &ListAssignment<V> {
        self.assignment()
    }
    fn score(&self) -> C::Score {
        self.score.clone()
    }
    fn initial_evaluations(&self) -> u128 {
        1
    }
    fn assert_consistent(&mut self) -> Result<(), ListScoringError> {
        self.assert_consistent()
    }
    fn trial(
        &mut self,
        movement: &ListMove<V>,
        visit: &mut dyn FnMut(&ListAssignment<V>, C::Score, u128) -> bool,
    ) -> Result<(), ListScoringError> {
        let candidate = self.apply(movement)?;
        if visit(candidate.assignment(), candidate.score(), 1) {
            candidate.commit();
        } else {
            drop(candidate);
        }
        self.healthy()
    }
}

/// Greedy construction using the same placement, interruption and budget policies
/// as scoped scoring. Explanations are disabled during standalone search.
pub fn list_calculator_construction<V, C>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    factory: impl Fn() -> C,
    config: ConstructionConfig<'_>,
) -> Result<ListConstructionResult<V, C::Score>, ListScoringError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<ListAssignment<V>, Change = Vec<usize>>,
{
    problem.validate(&initial)?;
    crate::list_construction::construction_impl_monitored(None, problem, initial, config, |a| {
        ListCalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Native local search with existing acceptance, forager, neighborhood and limit
/// semantics. Use explicit construction first for an incomplete starting state.
pub fn list_calculator_local_search_from<V, C>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    factory: impl Fn() -> C,
    config: LocalSearchConfig<'_, C::Score>,
) -> Result<ListLocalSearchResult<V, C::Score>, ListLocalSearchError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<ListAssignment<V>, Change = Vec<usize>>,
{
    crate::list_local_search::search_impl_monitored(None, problem, initial, config, |a| {
        ListCalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Local search over a caller-defined lazy neighborhood.
pub fn list_calculator_local_search_with_provider<V, C>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    factory: impl Fn() -> C,
    config: LocalSearchConfig<'_, C::Score>,
    provider: &mut dyn MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ListLocalSearchResult<V, C::Score>, ListLocalSearchError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<ListAssignment<V>, Change = Vec<usize>>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.max_time.is_some()
            || config.max_unimproved_time.is_some()
            || config.diminished_returns.is_some()
            || config.significant_improvement.is_some(),
    );
    crate::list_local_search::search_impl_with_provider(
        None,
        problem,
        initial,
        config,
        |a| ListCalculatorScoreDirector::new(problem, a, factory, false),
        clock,
        Some(provider),
    )
}
/// Run bounded custom commands and publish initial/best updates using the shared
/// custom-phase engine. Trial moves use inverse calculator notifications.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn list_calculator_custom_phase<V, C>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    factory: impl Fn() -> C,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, C::Score, ListScoringError>,
    ) -> Result<(), CustomPhaseError<ListScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, C::Score>),
) -> Result<CustomPhaseResult<ListAssignment<V>, C::Score>, CustomPhaseError<ListScoringError>>
where
    V: Eq + Clone,
    C: IncrementalCalculator<ListAssignment<V>, Change = Vec<usize>>,
{
    problem
        .validate(&initial)
        .map_err(|e| CustomPhaseError::Scoring(ListScoringError::Model(e)))?;
    crate::custom_phase::run_monitored(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| ListCalculatorScoreDirector::new(problem, a, factory, false),
        command,
        observer,
        None,
        None,
        false,
    )
}

crate::retained_calculator_phase::retained_calculator!(ListCalculatorScoreDirector, [V], ListAssignment<V>, ListProblem<V>, ListMove<V>, Vec<usize>, ListScoringError, ListLocalSearchError, SolverSequence, list, list_construction, ConstructionConfig<'_>, ListConstructionResult<V, C::Score>, list_local_search, LocalSearchConfig<'_, C::Score>, ListLocalSearchResult<V, C::Score>, list_director, |a| a, []);
