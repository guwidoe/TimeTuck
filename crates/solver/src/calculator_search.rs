//! Basic-variable search using application-defined incremental calculators.
//!
//! ```
//! use timetuck::*;
//! #[derive(Default)]
//! struct Sum(i64);
//! impl IncrementalCalculator<Assignment<i64>> for Sum {
//!     type Score = SimpleScore;
//!     type Justification = ();
//!     type Change = Vec<usize>;
//!     fn reset(&mut self, a: &Assignment<i64>, _: &mut MatchRegistry<SimpleScore, ()>)
//!         -> Result<(), MatchRegistryError> {
//!         self.0 = a.values().iter().flatten().sum();
//!         Ok(())
//!     }
//!     fn before_change(&mut self, a: &Assignment<i64>, changed: &Vec<usize>,
//!         _: &mut MatchRegistry<SimpleScore, ()>) -> Result<(), MatchRegistryError> {
//!         for &i in changed { self.0 -= a.values()[i].unwrap_or(0); }
//!         Ok(())
//!     }
//!     fn after_change(&mut self, a: &Assignment<i64>, changed: &Vec<usize>,
//!         _: &mut MatchRegistry<SimpleScore, ()>) -> Result<(), MatchRegistryError> {
//!         for &i in changed { self.0 += a.values()[i].unwrap_or(0); }
//!         Ok(())
//!     }
//!     fn calculate_score(&mut self) -> SimpleScore { SimpleScore::new(self.0) }
//! }
//! let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]); 2])?;
//! let built = calculator_construction(&p, p.initial_assignment(), Sum::default,
//!     ConstructionConfig::default())?;
//! let improved = calculator_local_search_from(&p, built.assignment, Sum::default,
//!     LocalSearchConfig { assert_scores: true, ..Default::default() })?;
//! assert_eq!(improved.best.unwrap().score, SimpleScore::new(4));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
use crate::*;
use crate::{scoring::Selection, search_scoring::SearchDirector};
use std::panic::{AssertUnwindSafe, catch_unwind};

crate::calculator_backend::calculator_backend!(
    CalculatorScoreDirector,
    CalculatorMove,
    [V],
    Assignment<V>,
    FiniteProblem<V>,
    Move<V>,
    Vec<usize>,
    ScoringError,
    basic_changes,
    basic_structure,
    basic_structural_delta,
    apply_validated
);
fn basic_changes<V>(movement: &Move<V>) -> Vec<usize> {
    movement.touched_variables()
}
/// Structural change of a move: each touched required variable contributes -1 when
/// it becomes unassigned and +1 when it becomes assigned. Touched indices are unique.
fn basic_structural_delta<V: Eq>(
    problem: &FiniteProblem<V>,
    before: &Assignment<V>,
    after: &Assignment<V>,
    touched: &[usize],
) -> Option<i64> {
    Some(
        touched
            .iter()
            .filter(|&&v| !problem.variables()[v].allows_unassigned())
            .map(|&v| {
                i64::from(before.values()[v].is_none()) - i64::from(after.values()[v].is_none())
            })
            .sum(),
    )
}
fn basic_structure<V: Eq, S: AdditiveScore>(
    score: S,
    problem: &FiniteProblem<V>,
    assignment: &Assignment<V>,
) -> Result<S, ScoringError> {
    Ok(crate::scoring::with_structure(score, problem, assignment))
}

impl<V, C, F> SearchDirector<V, C::Score> for CalculatorScoreDirector<'_, V, C, F>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
    F: Fn() -> C,
{
    type Error = ScoringError;
    fn assignment(&self) -> &Assignment<V> {
        self.assignment()
    }
    fn score(&self) -> C::Score {
        self.score.clone()
    }
    fn notify_best(&mut self, _complete: impl FnOnce(&Assignment<V>) -> bool) {
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
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        forager: impl Into<ForagerConfig<C::Score>>,
        mut accepts: impl FnMut(&Move<V>, C::Score) -> bool,
        quit_early: impl crate::construction::SelectionPolicy<C::Score>,
        assert_scores: bool,
    ) -> Result<Selection<V>, ScoringError> {
        self.healthy()?;
        let mut result = Selection {
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
            forager.try_add_with(score.clone(), accepted, || -> Result<_, ScoringError> {
                Ok((movement.clone(), score.clone(), accepted))
            })?;
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

impl<V, C, F> crate::custom_phase::CommandDirector<Assignment<V>, Move<V>, C::Score>
    for CalculatorScoreDirector<'_, V, C, F>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
    F: Fn() -> C,
{
    type Error = ScoringError;
    fn assignment(&self) -> &Assignment<V> {
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
    fn assert_consistent(&mut self) -> Result<(), ScoringError> {
        self.assert_consistent()
    }
    fn restore(&mut self, assignment: Assignment<V>) -> Result<u128, ScoringError> {
        self.reset(assignment)?;
        Ok(1)
    }
    fn trial(
        &mut self,
        movement: &Move<V>,
        visit: &mut dyn FnMut(&Assignment<V>, C::Score, u128) -> bool,
    ) -> Result<(), ScoringError> {
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
pub fn calculator_construction<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: ConstructionConfig<'_>,
) -> Result<ConstructionResult<V, C::Score>, ScoringError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    problem.validate(&initial)?;
    crate::construction::construction_impl_monitored(None, problem, initial, config, |a| {
        CalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Basic local search with existing acceptance, forager, neighborhood and limit
/// semantics. Use explicit construction first for an incomplete starting state.
pub fn calculator_local_search_from<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: LocalSearchConfig<'_, C::Score>,
) -> Result<LocalSearchResult<V, C::Score>, LocalSearchError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    crate::local_search::search_impl_monitored(None, problem, Some(initial), config, |a| {
        CalculatorScoreDirector::new(problem, a, factory, false)
    })
}
/// Local search over a caller-defined lazy neighborhood.
pub fn calculator_local_search_with_provider<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: LocalSearchConfig<'_, C::Score>,
    provider: &mut dyn MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<LocalSearchResult<V, C::Score>, LocalSearchError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.max_time.is_some()
            || config.max_unimproved_time.is_some()
            || config.diminished_returns.is_some()
            || config.significant_improvement.is_some()
            || config.uses_elapsed_acceptance(),
    );
    crate::local_search::search_impl_with_provider(
        None,
        problem,
        Some(initial),
        config,
        |a| CalculatorScoreDirector::new(problem, a, factory, false),
        clock,
        Some(crate::MoveProviderBinding::Infallible(provider)),
    )
}
/// Run bounded custom commands and publish initial/best updates using the shared
/// custom-phase engine. Trial moves use inverse calculator notifications.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn calculator_custom_phase<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, C::Score, ScoringError>,
    ) -> Result<(), CustomPhaseError<ScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, C::Score>),
) -> Result<CustomPhaseResult<Assignment<V>, C::Score>, CustomPhaseError<ScoringError>>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    problem
        .validate(&initial)
        .map_err(|e| CustomPhaseError::Scoring(ScoringError::Model(e)))?;
    crate::custom_phase::run_monitored(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| CalculatorScoreDirector::new(problem, a, factory, false),
        command,
        observer,
        None,
        None,
        false,
    )
}

crate::retained_calculator_phase::retained_calculator!(CalculatorScoreDirector, [V], Assignment<V>, FiniteProblem<V>, Move<V>, Vec<usize>, ScoringError, LocalSearchError, SolverSequence, basic, construction, ConstructionConfig<'_>, ConstructionResult<V, C::Score>, local_search, LocalSearchConfig<'_, C::Score>, LocalSearchResult<V, C::Score>, basic_director, Some, []);

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn calculator_local_search_with_fallible_provider<V, C>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    factory: impl Fn() -> C,
    config: LocalSearchConfig<'_, C::Score>,
    provider: &mut dyn crate::FallibleMoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<LocalSearchResult<V, C::Score>, LocalSearchError>
where
    V: Eq + Clone,
    C: IncrementalCalculator<Assignment<V>, Change = Vec<usize>>,
{
    let clock = crate::time_limit::monotonic_clock(
        config.max_time.is_some()
            || config.max_unimproved_time.is_some()
            || config.diminished_returns.is_some()
            || config.significant_improvement.is_some(),
    );
    crate::local_search::search_impl_with_provider(
        None,
        problem,
        Some(initial),
        config,
        |a| CalculatorScoreDirector::new(problem, a, factory, false),
        clock,
        Some(crate::MoveProviderBinding::Fallible(provider)),
    )
}
