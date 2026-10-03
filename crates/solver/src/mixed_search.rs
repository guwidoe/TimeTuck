//! Deterministic exact search over the full basic/list product space.

use crate::{MixedAssignment, MixedProblem, Score, Termination};
use crate::{list_search::visit_list_assignments, search::visit_assignments};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredMixedAssignment<B, L, S> {
    pub assignment: MixedAssignment<B, L>,
    pub score: S,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedSearchResult<B, L, S> {
    pub best: Option<ScoredMixedAssignment<B, L, S>>,
    pub evaluations: u64,
    pub termination: Termination,
}
impl<B, L, S> MixedSearchResult<B, L, S> {
    pub fn is_optimal(&self) -> bool {
        self.termination == Termination::Exhausted && self.best.is_some()
    }
}

/// Maximize one deterministic objective across both kinds of variables.
///
/// Each list assignment is visited in native list enumeration order; for each,
/// all basic assignments follow native domain order. Movable initial values do
/// not constrain the search. Pins, ranges and optionality retain their component
/// semantics. Ties retain the first candidate. Negative structural scores reject
/// a candidate; hard-infeasible candidates remain eligible.
///
/// One shared budget counts calls to the combined scorer. Zero invokes none;
/// exactly visiting the final candidate reports Exhausted. An impossible component
/// makes the whole space empty, even at zero budget. Empty components contribute
/// one empty assignment. Optimality requires exhaustion and a returned candidate.
///
/// This iterative product enumeration uses bounded working storage, but runtime
/// is exponential/factorial. Scoring recomputes the whole objective; no heuristic,
/// incremental-scoring or Java phase-trace equivalence is claimed.
///
/// ```
/// use timetuck::{FiniteProblem, ListEntity, ListProblem, MixedProblem,
///     SimpleScore, Variable, exhaustive_mixed_search};
/// let problem = MixedProblem::new(
///     FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap(),
///     ListProblem::new(vec![7], vec![ListEntity::new(vec![]), ListEntity::new(vec![])]).unwrap(),
/// );
/// let result = exhaustive_mixed_search(&problem, |state| {
///     let owner = state.lists().location(&7).unwrap().entity;
///     SimpleScore::new(if state.basic().values()[0] == Some(owner) { 1 } else { 0 })
/// }, 4);
/// assert!(result.is_optimal());
/// assert_eq!(result.evaluations, 4);
/// assert_eq!(result.best.unwrap().score, SimpleScore::new(1));
/// ```
pub fn exhaustive_mixed_search<B, L, S, F>(
    problem: &MixedProblem<B, L>,
    evaluate: F,
    max_evaluations: u64,
) -> MixedSearchResult<B, L, S>
where
    B: Eq + Clone,
    L: Eq + Clone,
    S: Score,
    F: FnMut(&MixedAssignment<B, L>) -> S,
{
    exhaustive_mixed_search_with_config(
        problem,
        evaluate,
        crate::ExactSearchConfig {
            max_evaluations,
            ..Default::default()
        },
    )
}

/// Exact enumeration with optional cooperative cancellation and elapsed time. See
/// [`crate::ExactSearchConfig`] for polling and termination precedence.
pub fn exhaustive_mixed_search_with_config<B, L, S, F>(
    problem: &MixedProblem<B, L>,
    evaluate: F,
    config: crate::ExactSearchConfig<'_>,
) -> MixedSearchResult<B, L, S>
where
    B: Eq + Clone,
    L: Eq + Clone,
    S: Score,
    F: FnMut(&MixedAssignment<B, L>) -> S,
{
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    exhaustive_mixed_search_with_clock(problem, evaluate, config, clock)
}

pub(crate) fn exhaustive_mixed_search_with_clock<B, L, S, F>(
    problem: &MixedProblem<B, L>,
    mut evaluate: F,
    config: crate::ExactSearchConfig<'_>,
    clock: impl FnMut() -> std::time::Duration,
) -> MixedSearchResult<B, L, S>
where
    B: Eq + Clone,
    L: Eq + Clone,
    S: Score,
    F: FnMut(&MixedAssignment<B, L>) -> S,
{
    let mut timer = crate::time_limit::TimeBudget::new(config.max_time, None, clock);
    let mut result = MixedSearchResult {
        best: None,
        evaluations: 0,
        termination: Termination::Exhausted,
    };
    if problem.basic().search_space_size() == Some(0) {
        return result;
    }
    visit_list_assignments(problem.lists(), |lists| {
        visit_assignments(problem.basic(), |basic| {
            if let Some(reason) = config.stop_before_next(result.evaluations) {
                result.termination = reason;
                return false;
            }
            let assignment = MixedAssignment::new(basic.clone(), lists.clone());
            // Preparing the owned joint candidate belongs to the elapsed budget.
            if timer.expired() {
                result.termination = Termination::TimeLimit;
                return false;
            }
            let score = evaluate(&assignment);
            result.evaluations += 1;
            if score.structural() >= 0 && result.best.as_ref().is_none_or(|best| score > best.score)
            {
                result.best = Some(ScoredMixedAssignment { assignment, score });
            }
            true
        })
    });
    result
}
