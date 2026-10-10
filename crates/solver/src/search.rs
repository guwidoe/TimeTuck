//! Deterministic exact search for finite basic-variable problems.
//!
//! This is an exhaustive correctness baseline for small problems, not a scalable
//! heuristic. A result is proven optimal only after the entire assignment space
//! is exhausted. Finishing a fixed evaluation budget alone proves no optimality.

use crate::{Score, model::FiniteProblem, moves::Assignment};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Termination {
    /// Unvisited assignments remain and cancellation was observed.
    Cancelled,
    /// Unvisited assignments remain and the cooperative elapsed budget expired.
    TimeLimit,
    /// Every complete assignment was evaluated.
    Exhausted,
    /// Unvisited assignments remain, but no score-evaluation budget remains.
    EvaluationLimit,
}

/// Controls for exhaustive basic, list and mixed search. Each entry point with
/// a `_with_config` suffix accepts these controls; the original budget-only
/// entry points retain their behavior.
///
/// Cancellation is checked before scoring each complete assignment, after the
/// evaluation budget and before elapsed time. An active callback finishes and its eligible result is
/// retained. Enumeration itself is not preempted. If the final assignment has
/// been evaluated, exhaustion wins even if that callback requested cancellation;
/// an empty search space also reports exhaustion. Only actual exhaustion can
/// prove optimality. Elapsed time starts before model setup and enumeration and is
/// polled at the same complete-candidate boundaries. Zero time invokes no scorer
/// for a nonempty space. Evaluation budget precedes cancellation, which precedes
/// time. Final exhaustion wins over all three. User callbacks and value operations
/// are not preempted, so there is no hard bound on cancellation or time overshoot.
///
/// ```
/// use timetuck::{CancellationToken, ExactSearchConfig, FiniteProblem, Variable,
///     SimpleScore, Termination, exhaustive_search_with_config};
/// let problem = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2])])?;
/// let token = CancellationToken::new();
/// let result = exhaustive_search_with_config(&problem, |assignment| {
///     token.cancel(); // A controller thread may request this instead.
///     SimpleScore::new(assignment.values()[0].unwrap())
/// }, ExactSearchConfig { cancellation: Some(&token), ..Default::default() });
/// assert_eq!(result.termination, Termination::Cancelled);
/// assert_eq!(result.best.unwrap().score, SimpleScore::new(1));
/// # Ok::<(), timetuck::ModelError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactSearchConfig<'a> {
    pub max_evaluations: u64,
    pub cancellation: Option<&'a crate::CancellationToken>,
    /// Elapsed time from entry, including setup and enumeration. Disabled by default.
    ///
    /// ```
    /// use std::time::Duration;
    /// use timetuck::{ExactSearchConfig, FiniteProblem, Variable, SimpleScore,
    ///     Termination, exhaustive_search_with_config};
    /// let p = FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap();
    /// let r = exhaustive_search_with_config(&p, |_| SimpleScore::ZERO,
    ///     ExactSearchConfig { max_time: Some(Duration::ZERO), ..Default::default() });
    /// assert_eq!(r.termination, Termination::TimeLimit);
    /// assert_eq!(r.evaluations, 0);
    /// assert!(!r.is_optimal());
    /// ```
    pub max_time: Option<std::time::Duration>,
}
impl Default for ExactSearchConfig<'_> {
    fn default() -> Self {
        Self {
            max_evaluations: 10_000,
            cancellation: None,
            max_time: None,
        }
    }
}
impl ExactSearchConfig<'_> {
    pub(crate) fn stop_before_next(self, evaluations: u64) -> Option<Termination> {
        if crate::CountLimit::new(self.max_evaluations).is_reached(evaluations) {
            Some(Termination::EvaluationLimit)
        } else if self
            .cancellation
            .is_some_and(crate::CancellationToken::is_cancelled)
        {
            Some(Termination::Cancelled)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoredAssignment<V, S> {
    pub assignment: Assignment<V>,
    pub score: S,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResult<V, S> {
    /// Best structurally sound complete assignment seen, even if hard-infeasible.
    pub best: Option<ScoredAssignment<V, S>>,
    pub evaluations: u64,
    pub termination: Termination,
}
impl<V, S> SearchResult<V, S> {
    pub fn is_optimal(&self) -> bool {
        self.termination == Termination::Exhausted && self.best.is_some()
    }
}

/// Maximize a deterministic score over all complete assignments until exhausted
/// or `max_evaluations` scores have been calculated. `u64::MAX` is the largest
/// supported budget. A zero budget is valid and does not invoke the scorer.
///
/// Domain order is preserved, with `None` last for optional variables. Pinned
/// values stay fixed. Initial values of movable variables do not restrict search.
/// Ties retain the first complete assignment encountered. Negative structural
/// scores are rejected; negative hard scores are retained as infeasible candidates.
/// The callback must be deterministic and may not mutate problem facts through
/// interior mutability. Optimality claims depend on that scoring contract.
///
/// The working assignment and best solution are owned here; the caller's model
/// is never mutated. Enumeration is iterative, using linear auxiliary memory.
pub fn exhaustive_search<V, S, F>(
    problem: &FiniteProblem<V>,
    evaluate: F,
    max_evaluations: u64,
) -> SearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&Assignment<V>) -> S,
{
    exhaustive_search_with_config(
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
pub fn exhaustive_search_with_config<V, S, F>(
    problem: &FiniteProblem<V>,
    evaluate: F,
    config: crate::ExactSearchConfig<'_>,
) -> SearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&Assignment<V>) -> S,
{
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    exhaustive_search_with_clock(problem, evaluate, config, clock)
}

/// Exact search with a synchronous notification for each strict retained-best
/// improvement, including the first eligible candidate. Ties and structurally
/// rejected candidates emit nothing. The observer borrows an owned snapshot;
/// clone it to publish through a [`crate::SolverProgress`]. Observer work is
/// included in elapsed time but not evaluation counts. Panics propagate to the
/// caller (a managed job catches them). Existing cancellation polling applies.
pub fn exhaustive_search_with_observer<V, S, F>(
    problem: &FiniteProblem<V>,
    evaluate: F,
    config: crate::ExactSearchConfig<'_>,
    observer: impl FnMut(&ScoredAssignment<V, S>),
) -> SearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&Assignment<V>) -> S,
{
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    exhaustive_search_observed(problem, evaluate, config, clock, observer)
}

pub(crate) fn exhaustive_search_with_clock<V, S, F>(
    problem: &FiniteProblem<V>,
    evaluate: F,
    config: crate::ExactSearchConfig<'_>,
    clock: impl FnMut() -> std::time::Duration,
) -> SearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&Assignment<V>) -> S,
{
    exhaustive_search_observed(problem, evaluate, config, clock, |_| {})
}

fn exhaustive_search_observed<V, S, F>(
    problem: &FiniteProblem<V>,
    mut evaluate: F,
    config: crate::ExactSearchConfig<'_>,
    clock: impl FnMut() -> std::time::Duration,
    mut observer: impl FnMut(&ScoredAssignment<V, S>),
) -> SearchResult<V, S>
where
    V: Eq + Clone,
    S: Score,
    F: FnMut(&Assignment<V>) -> S,
{
    let mut timer = crate::time_limit::TimeBudget::<_, S>::new(config.max_time, None, clock);
    let mut result = SearchResult {
        best: None,
        evaluations: 0,
        termination: Termination::Exhausted,
    };
    visit_assignments(problem, |working| {
        if let Some(reason) = config
            .stop_before_next(result.evaluations)
            .or_else(|| timer.expired().then_some(Termination::TimeLimit))
        {
            result.termination = reason;
            return false;
        }
        let score = evaluate(working);
        result.evaluations += 1;
        if score.structural() >= 0 && result.best.as_ref().is_none_or(|best| score > best.score) {
            result.best = Some(ScoredAssignment {
                assignment: working.clone(),
                score,
            });
            observer(result.best.as_ref().unwrap());
        }
        true
    });
    result
}

// Return true only when every assignment was visited. Returning false from the
// visitor stops enumeration immediately, including when nested in mixed search.
pub(crate) fn visit_assignments<V: Eq + Clone>(
    problem: &FiniteProblem<V>,
    visit: impl FnMut(&Assignment<V>) -> bool,
) -> bool {
    visit_basic_from(problem, problem.initial_assignment(), false, visit)
}

pub(crate) fn visit_basic_from<V: Eq + Clone>(
    problem: &FiniteProblem<V>,
    mut working: Assignment<V>,
    only_missing: bool,
    mut visit: impl FnMut(&Assignment<V>) -> bool,
) -> bool {
    if problem.search_space_size() == Some(0) {
        return true;
    }
    let variables = problem.variables();
    let mut choices = vec![0_usize; variables.len()];
    let movable: Vec<_> = variables
        .iter()
        .enumerate()
        .filter_map(|(i, v)| {
            (!v.is_pinned() && (!only_missing || working.values[i].is_none())).then_some(i)
        })
        .collect();
    for &index in &movable {
        working.values[index] = variables[index].domain().first().cloned();
    }
    loop {
        if !visit(&working) {
            return false;
        }
        let mut advanced = false;
        for &index in movable.iter().rev() {
            let variable = &variables[index];
            let last = if variable.allows_unassigned() {
                variable.domain().len()
            } else {
                variable.domain().len() - 1
            };
            if choices[index] < last {
                choices[index] += 1;
                working.values[index] = variable.domain().get(choices[index]).cloned();
                advanced = true;
                break;
            }
            choices[index] = 0;
            working.values[index] = variable.domain().first().cloned();
        }
        if !advanced {
            return true;
        }
    }
}
