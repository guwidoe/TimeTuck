//! Construction from custom move providers. Providers propose a pool at each
//! assignment boundary; construction accepts only monotonic initialization.
//! Unlike provider local search, malformed or non-initializing proposals are
//! skipped and counted. No exhaustion of a custom pool proves infeasibility.
use std::{cell::Cell, time::Duration};

use crate::list_search_scoring::ListSearchDirector;
use crate::mixed_search_scoring::MixedSearchDirector;
use crate::search_scoring::SearchDirector;
use crate::{
    AdditiveScore, Assignment, CancellationToken, ConstraintNetwork, ConstructionPickEarly,
    FiniteProblem, ListAssignment, ListConstraintNetwork, ListMove, ListProblem, ListScoreDirector,
    ListScoringError, ListStreamScoreDirector, ListStreamScoringError, ListStreamSources,
    MixedAssignment, MixedConstraintNetwork, MixedMove, MixedProblem, MixedScoreDirector,
    MixedScoringError, MixedStreamScoreDirector, MixedStreamScoringError, MixedStreamSources, Move,
    MoveProvider, OptionalConstruction, PlanningStreamSource, ScoreDirector, ScoringError,
    StreamGraph, StreamScoreDirector, StreamScoringError,
};

/// Controls for pooled provider construction. Providers determine ordering and
/// may propose composite moves initializing several items atomically.
///
/// Bounds are per call. Score evaluations include initialization; move and scan
/// bounds do not. Scan bounds count every raw proposal, including invalid moves.
/// Factory/iterator/scoring callbacks must return promptly: stopping is cooperative.
#[derive(Clone, Copy, Debug)]
pub struct ProviderConstructionConfig<'a> {
    pub interruption: crate::ConstructionInterruption,
    pub max_evaluations: u64,
    pub max_move_evaluations: Option<u64>,
    pub max_steps: Option<u64>,
    pub max_scanned_moves: u64,
    pub optional: OptionalConstruction,
    pub pick_early: ConstructionPickEarly,
    pub cancellation: Option<&'a CancellationToken>,
    pub max_time: Option<Duration>,
    pub assert_scores: bool,
    /// Check initialization and completed scan/placement boundaries without
    /// candidate-state or rollback rescoring. Full assert_scores takes precedence.
    /// Diagnostic work is excluded from evaluation counters but consumes time.
    pub assert_step_scores: bool,
}
impl Default for ProviderConstructionConfig<'_> {
    fn default() -> Self {
        Self {
            interruption: crate::ConstructionInterruption::KeepBestPrefix,
            max_evaluations: 10_000,
            max_move_evaluations: None,
            max_steps: None,
            max_scanned_moves: 100_000,
            optional: OptionalConstruction::Preserve,
            pick_early: ConstructionPickEarly::Never,
            cancellation: None,
            max_time: None,
            assert_scores: false,
            assert_step_scores: false,
        }
    }
}
/// Why provider construction stopped. NoCandidates may leave a partial assignment
/// even when legal completions exist; it is not an infeasibility certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderConstructionTermination {
    /// No eligible missing items remain; this does not assert hard feasibility.
    Completed,
    /// A solve-wide or phase-local construction condition stopped this phase.
    Condition,
    NoCandidates,
    EvaluationLimit,
    MoveEvaluationLimit,
    ScanLimit,
    StepLimit,
    Cancelled,
    TimeLimit,
}
/// The last committed assignment, including useful partial construction.
#[derive(Clone, Debug)]
pub struct ProviderConstructionResult<A, S> {
    pub assignment: A,
    pub score: Option<S>,
    pub complete: bool,
    pub evaluations: u64,
    pub constraint_evaluations: u128,
    /// Nonempty scans, including all-rejected scans. Interrupted scans count
    /// only under KeepBestPrefix.
    pub completed_steps: u64,
    pub scanned_moves: u64,
    /// Invalid, non-progressing or preservation-violating proposals. Score-based
    /// rejection of an optional initialization is not included here.
    pub rejected_moves: u64,
    pub termination: ProviderConstructionTermination,
}

trait Shape {
    type Problem;
    type Assignment: Clone + Eq;
    type Move;
    fn problem(&self) -> &Self::Problem;
    fn no_change(&self) -> Self::Move;
    fn complete(&self, a: &Self::Assignment) -> bool;
    fn remaining(&self, a: &Self::Assignment, optional: OptionalConstruction) -> usize;
    fn extends(&self, a: &Self::Assignment, m: &Self::Move, optional: OptionalConstruction)
    -> bool;
}
struct BasicShape<'a, V>(&'a FiniteProblem<V>);
struct ListShape<'a, V>(&'a ListProblem<V>);
struct MixedShape<'a, B, L>(&'a MixedProblem<B, L>);

fn basic_remaining<V: Eq>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    o: OptionalConstruction,
) -> usize {
    p.variables()
        .iter()
        .zip(a.values())
        .filter(|(v, value)| {
            value.is_none()
                && !v.is_pinned()
                && (!v.allows_unassigned() || o != OptionalConstruction::Preserve)
        })
        .count()
}
fn basic_preserved<V: Eq>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    b: &Assignment<V>,
    o: OptionalConstruction,
) -> bool {
    p.variables()
        .iter()
        .zip(a.values().iter().zip(b.values()))
        .all(|(v, (old, new))| {
            if old.is_some() || (v.allows_unassigned() && o == OptionalConstruction::Preserve) {
                old == new
            } else {
                true
            }
        })
}
fn list_remaining<V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
    o: OptionalConstruction,
) -> usize {
    if p.allows_unassigned() && o == OptionalConstruction::Preserve {
        0
    } else {
        p.values().len() - a.lists().iter().map(Vec::len).sum::<usize>()
    }
}
fn list_preserved<V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
    b: &ListAssignment<V>,
    o: OptionalConstruction,
) -> bool {
    if p.allows_unassigned() && o == OptionalConstruction::Preserve {
        return a == b;
    }
    // Every old list remains a subsequence of the same owner's new list.
    // New insertions may shift positions, but cannot relocate or reorder old values.
    a.lists().iter().zip(b.lists()).all(|(old, new)| {
        let mut remaining = new.iter();
        old.iter().all(|value| remaining.any(|next| next == value))
    })
}
impl<V: Eq + Clone> Shape for BasicShape<'_, V> {
    type Problem = FiniteProblem<V>;
    type Assignment = Assignment<V>;
    type Move = Move<V>;
    fn no_change(&self) -> Self::Move {
        Move::Composite(vec![])
    }
    fn problem(&self) -> &Self::Problem {
        self.0
    }
    fn complete(&self, a: &Self::Assignment) -> bool {
        self.0.is_complete(a)
    }
    fn remaining(&self, a: &Self::Assignment, o: OptionalConstruction) -> usize {
        basic_remaining(self.0, a, o)
    }
    fn extends(&self, a: &Self::Assignment, m: &Self::Move, o: OptionalConstruction) -> bool {
        let mut b = a.clone();
        let Ok(tx) = self.0.apply(&mut b, m) else {
            return false;
        };
        tx.commit();
        basic_preserved(self.0, a, &b, o) && self.remaining(&b, o) < self.remaining(a, o)
    }
}
impl<V: Eq + Clone> Shape for ListShape<'_, V> {
    type Problem = ListProblem<V>;
    type Assignment = ListAssignment<V>;
    type Move = ListMove<V>;
    fn no_change(&self) -> Self::Move {
        ListMove::Composite(vec![])
    }
    fn problem(&self) -> &Self::Problem {
        self.0
    }
    fn complete(&self, a: &Self::Assignment) -> bool {
        self.0.is_complete(a)
    }
    fn remaining(&self, a: &Self::Assignment, o: OptionalConstruction) -> usize {
        list_remaining(self.0, a, o)
    }
    fn extends(&self, a: &Self::Assignment, m: &Self::Move, o: OptionalConstruction) -> bool {
        let mut b = a.clone();
        let Ok(tx) = self.0.apply(&mut b, m) else {
            return false;
        };
        tx.commit();
        list_preserved(self.0, a, &b, o) && self.remaining(&b, o) < self.remaining(a, o)
    }
}
impl<B: Eq + Clone, L: Eq + Clone> Shape for MixedShape<'_, B, L> {
    type Problem = MixedProblem<B, L>;
    type Assignment = MixedAssignment<B, L>;
    type Move = MixedMove<B, L>;
    fn no_change(&self) -> Self::Move {
        MixedMove {
            basic: Move::Composite(vec![]),
            lists: ListMove::Composite(vec![]),
        }
    }
    fn problem(&self) -> &Self::Problem {
        self.0
    }
    fn complete(&self, a: &Self::Assignment) -> bool {
        self.0.is_complete(a)
    }
    fn remaining(&self, a: &Self::Assignment, o: OptionalConstruction) -> usize {
        basic_remaining(self.0.basic(), a.basic(), o) + list_remaining(self.0.lists(), a.lists(), o)
    }
    fn extends(&self, a: &Self::Assignment, m: &Self::Move, o: OptionalConstruction) -> bool {
        let mut b = a.clone();
        let Ok(tx) = self.0.apply(&mut b, m) else {
            return false;
        };
        tx.commit();
        basic_preserved(self.0.basic(), a.basic(), b.basic(), o)
            && list_preserved(self.0.lists(), a.lists(), b.lists(), o)
            && self.remaining(&b, o) < self.remaining(a, o)
    }
}
struct Selected {
    installed: bool,
    evaluations: u64,
    constraint_evaluations: u128,
}
trait Engine<A, M, S> {
    type Error: From<crate::MoveProviderError>;
    fn assignment(&self) -> &A;
    fn score(&self) -> S;
    fn initial_evaluations(&self) -> u128;
    fn notify_best(&mut self, complete: impl FnOnce(&A) -> bool);
    fn assert_consistent(&mut self) -> Result<(), Self::Error>;
    fn select(
        &mut self,
        moves: impl Iterator<Item = M>,
        limit: u64,
        config: ProviderConstructionConfig,
        can_commit: impl Fn() -> bool,
        move_counter: &Option<crate::move_metrics::MoveCounter>,
        score_counter: &Option<crate::score_evaluations::ScoreCounter>,
    ) -> Result<Selected, Self::Error>;
}
// Wrappers keep this entry point independent of existing search engine behavior.
struct ProviderSelection<S, F> {
    previous: S,
    pick: ConstructionPickEarly,
    can_commit: F,
}
impl<S: AdditiveScore, F: Fn() -> bool> crate::construction::SelectionPolicy<S>
    for ProviderSelection<S, F>
{
    fn should_pick(&self, score: S) -> bool {
        self.pick.should_pick(self.previous.clone(), score)
    }
    fn can_commit(&self, _evaluation_limit: bool) -> bool {
        (self.can_commit)()
    }
}
macro_rules! engine {
    ($wrapper:ident, $search:ident, [$($v:ident),+], $assignment:ty, $movement:ty) => {
        struct $wrapper<D>(D);
        impl<$($v: Eq + Clone,)+ S: AdditiveScore, D: $search<$($v,)+ S>> Engine<$assignment, $movement, S> for $wrapper<D> {
            type Error = D::Error;
            fn assignment(&self) -> &$assignment { self.0.assignment() }
            fn score(&self) -> S { self.0.score() }
            fn initial_evaluations(&self) -> u128 { self.0.initial_constraint_evaluations() }
            fn notify_best(&mut self, complete: impl FnOnce(&$assignment) -> bool) { self.0.notify_best(complete); }
            fn assert_consistent(&mut self) -> Result<(), Self::Error> { self.0.assert_consistent() }
            fn select(&mut self, moves: impl Iterator<Item = $movement>, limit: u64, config: ProviderConstructionConfig, can_commit: impl Fn() -> bool, move_counter: &Option<crate::move_metrics::MoveCounter>, score_counter: &Option<crate::score_evaluations::ScoreCounter>) -> Result<Selected, Self::Error> {
                let previous = self.0.score();
                // Required initialization improves structural score even when the
                // ordinary score worsens; optional-only moves must not worsen it.
                let selection = self.0.select_best(moves, limit, false,
                    |movement, score| { crate::score_evaluations::record(score_counter); crate::move_metrics::record(move_counter, movement); config.optional == OptionalConstruction::EvaluateUnassigned || score >= previous },
                    ProviderSelection { previous: previous.clone(), pick: config.pick_early, can_commit },
                    config.assert_scores)?;
                Ok(Selected { installed: selection.selected.is_some(), evaluations: selection.evaluations,
                    constraint_evaluations: selection.constraint_evaluations })
            }
        }
    }
}
engine!(BasicEngine, SearchDirector, [V], Assignment<V>, Move<V>);
engine!(
    ListEngine,
    ListSearchDirector,
    [V],
    ListAssignment<V>,
    ListMove<V>
);
engine!(MixedEngine, MixedSearchDirector, [B, L], MixedAssignment<B, L>, MixedMove<B, L>);

fn interruption<S>(
    c: ProviderConstructionConfig,
    elapsed: &impl Fn() -> std::time::Duration,
    shared: &mut Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Option<ProviderConstructionTermination> {
    if c.cancellation.is_some_and(CancellationToken::is_cancelled) {
        Some(ProviderConstructionTermination::Cancelled)
    } else if c.max_time.is_some_and(|limit| elapsed() >= limit) {
        Some(ProviderConstructionTermination::TimeLimit)
    } else if shared.as_mut().is_some_and(|s| s.construction_stopped()) {
        Some(ProviderConstructionTermination::Condition)
    } else {
        None
    }
}
fn score_budget(
    c: ProviderConstructionConfig,
    evaluations: u64,
) -> (u64, ProviderConstructionTermination) {
    let scores = c.max_evaluations.saturating_sub(evaluations);
    let moves = c.max_move_evaluations.map_or(u64::MAX, |n| {
        n.saturating_sub(evaluations.saturating_sub(1))
    });
    if moves < scores {
        (moves, ProviderConstructionTermination::MoveEvaluationLimit)
    } else {
        (scores, ProviderConstructionTermination::EvaluationLimit)
    }
}
type Outcome<F, S> = ProviderConstructionResult<<F as Shape>::Assignment, S>;
type Provider<'a, F> = crate::MoveProviderBinding<
    'a,
    <F as Shape>::Problem,
    <F as Shape>::Assignment,
    <F as Shape>::Move,
>;
fn construct<F: Shape, S: AdditiveScore, D: Engine<F::Assignment, F::Move, S>>(
    shape: F,
    initial: F::Assignment,
    config: ProviderConstructionConfig,
    provider: Provider<'_, F>,
    create: impl FnOnce(F::Assignment) -> Result<D, D::Error>,
    mut shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> Result<Outcome<F, S>, D::Error> {
    let move_counter = crate::move_metrics::from_control(&shared);
    let score_counter = crate::score_evaluations::from_control(&shared);
    use ProviderConstructionTermination as Stop;
    let elapsed = crate::solver_clock::monitored_clock(config.max_time.is_some(), &shared);
    let mut result = ProviderConstructionResult {
        complete: shape.complete(&initial),
        assignment: initial,
        score: None,
        evaluations: 0,
        constraint_evaluations: 0,
        completed_steps: 0,
        scanned_moves: 0,
        rejected_moves: 0,
        termination: Stop::EvaluationLimit,
    };
    if config.max_evaluations == 0 {
        return Ok(result);
    }
    if let Some(reason) = interruption(config, &elapsed, &mut shared) {
        result.termination = reason;
        return Ok(result);
    }
    let mut provider = crate::move_provider::ProviderPhase::new(Some(provider));
    let mut director = create(result.assignment.clone())?;
    crate::score_evaluations::record(&score_counter);
    result.evaluations = 1;
    result.constraint_evaluations = director.initial_evaluations();
    if config.assert_scores || config.assert_step_scores {
        director.assert_consistent()?;
    }
    director.notify_best(|a| shape.complete(a));
    crate::time_limit::observe_construction_score(
        &mut shared,
        || director.score(),
        || shape.complete(director.assignment()),
    );
    result.termination = loop {
        if let Some(reason) = interruption(config, &elapsed, &mut shared) {
            break reason;
        }
        if shape.remaining(director.assignment(), config.optional) == 0 {
            break Stop::Completed;
        }
        if config
            .max_steps
            .is_some_and(|limit| result.completed_steps >= limit)
        {
            break Stop::StepLimit;
        }
        let (budget, budget_reason) = score_budget(config, result.evaluations);
        if budget == 0 {
            break budget_reason;
        }
        if result.scanned_moves == config.max_scanned_moves {
            break Stop::ScanLimit;
        }
        if let Some(shared) = &mut shared {
            shared.step_started();
        }
        provider.start_step();
        let base = director.assignment().clone();
        let failure = crate::fallible_move_provider::ProviderFailure::default();
        let mut proposals = provider.provider().expect("construction provider").moves(
            shape.problem(),
            &base,
            result.completed_steps,
            &failure,
        )?;
        let stopped = Cell::new(None);
        let mut no_change = (config.optional == OptionalConstruction::EvaluateUnassigned
            && shape.remaining(&base, config.optional)
                > shape.remaining(&base, OptionalConstruction::Preserve))
        .then(|| shape.no_change());
        let mut emitted = 0;
        let mut ended = false;
        let candidates = std::iter::from_fn(|| {
            if ended {
                return None;
            }
            loop {
                stopped.set(interruption(config, &elapsed, &mut shared));
                if stopped.get().is_none() && emitted == budget {
                    stopped.set(Some(budget_reason));
                }
                if stopped.get().is_none() && result.scanned_moves == config.max_scanned_moves {
                    stopped.set(Some(Stop::ScanLimit));
                }
                if stopped.get().is_some() {
                    ended = true;
                    return None;
                }
                let movement = proposals.next();
                result.scanned_moves += u64::from(movement.is_some());
                if let Some(reason) = interruption(config, &elapsed, &mut shared) {
                    stopped.set(Some(reason));
                    ended = true;
                    return None;
                }
                let Some(movement) = movement else {
                    ended = true;
                    return if failure.failed() {
                        None
                    } else {
                        no_change.take()
                    };
                };
                if shape.extends(&base, &movement, config.optional) {
                    emitted += 1;
                    return Some(movement);
                }
                result.rejected_moves += 1;
            }
        });
        let can_commit = || {
            !failure.failed()
                && (config.interruption == crate::ConstructionInterruption::KeepBestPrefix
                    || stopped.get().is_none())
        };
        let selection = director.select(
            candidates,
            budget,
            config,
            can_commit,
            &move_counter,
            &score_counter,
        )?;
        failure.check()?;
        let installed = selection.installed
            && (config.optional != OptionalConstruction::EvaluateUnassigned
                || director.assignment() != &base);
        result.evaluations += selection.evaluations;
        result.constraint_evaluations += selection.constraint_evaluations;
        let completed = selection.evaluations > 0 && can_commit();
        result.completed_steps += u64::from(completed);
        if let Some(shared) = &mut shared {
            crate::time_limit::finish_step(
                *shared,
                completed,
                || director.score(),
                || shape.complete(director.assignment()),
            );
        }
        if completed && let Some(shared) = &mut shared {
            shared.complete_construction_step();
        }
        if completed {
            crate::time_limit::observe_construction_score(
                &mut shared,
                || director.score(),
                || shape.complete(director.assignment()),
            );
        }

        if config.assert_scores || config.assert_step_scores {
            director.assert_consistent()?;
        }
        drop(proposals);
        let continue_placements = completed
            && provider
                .provider()
                .expect("construction provider")
                .construction_placement_completed(installed);
        provider.end_step();
        if installed {
            director.notify_best(|a| shape.complete(a));
        }
        if let Some(reason) = stopped
            .get()
            .or_else(|| interruption(config, &elapsed, &mut shared))
        {
            break reason;
        }
        if !installed && !continue_placements {
            break Stop::NoCandidates;
        }
    };
    result.complete = shape.complete(director.assignment());
    result.assignment = director.assignment().clone();
    result.score = Some(director.score());
    Ok(result)
}

/// Build an assignment using a custom candidate pool and scoped basic scoring.
/// Existing assignments are preserved; list insertions may shift old indices.
/// Stable ties keep the first candidate. By default the best evaluated prefix
/// is committed when interrupted; DiscardIncompleteStep retains the prior step.
/// No additional provider pull is made to prove exhaustion at
/// an exact limit. Under DiscardIncompleteStep, reaching that boundary discards
/// the scan even if the provider would have ended on its next pull. An explicit
/// early pick still completes the step. EvaluateUnassigned appends a scored
/// no-change after natural provider exhaustion when optional items remain; this
/// synthetic candidate consumes score/move budget but no raw-proposal budget.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![1_i32, 2]); 2])?;
/// let mut n = ConstraintNetwork::new(2);
/// n.add_constraint("sum", vec![0, 1], |v| {
///     SimpleScore::new(v.iter().filter_map(|v| v.copied()).map(i64::from).sum())
/// })?;
/// let mut provider = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
///     a.values().iter().enumerate().filter(|(_, v)| v.is_none())
///         .flat_map(|(variable, _)| p.variables()[variable].domain().iter()
///             .map(move |value| Move::Change { variable, value: Some(*value) }))
///         .collect()
/// };
/// let r = construction_from_with_provider(&p, &n, p.initial_assignment(),
///     ProviderConstructionConfig::default(), &mut provider)?;
/// assert!(r.complete);
/// assert_eq!(r.assignment.values(), &[Some(2), Some(2)]);
/// assert_eq!(r.score, Some(SimpleScore::new(4)));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn construction_from_with_provider<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<ProviderConstructionResult<Assignment<V>, S>, ScoringError> {
    basic(
        problem,
        network,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn basic<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: ProviderConstructionConfig,
    context: crate::provider_search::ProviderContext<
        '_,
        FiniteProblem<V>,
        Assignment<V>,
        Move<V>,
        S,
    >,
) -> Result<ProviderConstructionResult<Assignment<V>, S>, ScoringError> {
    network.validate(problem, &initial)?;
    if let Some(observer) = context.observer {
        construct(
            BasicShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                ScoreDirector::new(problem, network, a)
                    .map(|d| BasicEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            BasicShape(problem),
            initial,
            config,
            context.provider,
            |a| ScoreDirector::new(problem, network, a).map(BasicEngine),
            context.control,
        )
    }
}

/// Provider construction with transactional streams; same controls and
/// preservation rules as [`construction_from_with_provider`].
/// Model and source ownership validate even at zero score budget.
pub fn stream_construction_from_with_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<ProviderConstructionResult<Assignment<V>, S>, StreamScoringError> {
    basic_stream(
        problem,
        graph,
        sources,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn basic_stream<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: ProviderConstructionConfig,
    context: crate::provider_search::ProviderContext<
        '_,
        FiniteProblem<V>,
        Assignment<V>,
        Move<V>,
        S,
    >,
) -> Result<ProviderConstructionResult<Assignment<V>, S>, StreamScoringError> {
    crate::stream_scoring::validate_source(&graph, &sources)?;
    problem.validate(&initial)?;
    if let Some(observer) = context.observer {
        construct(
            BasicShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                StreamScoreDirector::new(problem, graph, sources, a)
                    .map(|d| BasicEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            BasicShape(problem),
            initial,
            config,
            context.provider,
            |a| StreamScoreDirector::new(problem, graph, sources, a).map(BasicEngine),
            context.control,
        )
    }
}

/// Build an assignment using a custom candidate pool and scoped list scoring.
/// Existing assignments are preserved; list insertions may shift old indices.
/// Stable ties keep the first candidate. By default the best evaluated prefix
/// is committed when interrupted; DiscardIncompleteStep retains the prior step.
/// No additional provider pull is made to prove exhaustion at
/// an exact limit. Under DiscardIncompleteStep, reaching that boundary discards
/// the scan even if the provider would have ended on its next pull. An explicit
/// early pick still completes the step. EvaluateUnassigned appends a scored
/// no-change after natural provider exhaustion when optional items remain; this
/// synthetic candidate consumes score/move budget but no raw-proposal budget.
pub fn list_construction_from_with_provider<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ProviderConstructionResult<ListAssignment<V>, S>, ListScoringError> {
    list(
        problem,
        network,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn list<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: ProviderConstructionConfig,
    context: crate::provider_search::ProviderContext<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
    >,
) -> Result<ProviderConstructionResult<ListAssignment<V>, S>, ListScoringError> {
    network.validate(problem, &initial)?;
    if let Some(observer) = context.observer {
        construct(
            ListShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                ListScoreDirector::new(problem, network, a)
                    .map(|d| ListEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            ListShape(problem),
            initial,
            config,
            context.provider,
            |a| ListScoreDirector::new(problem, network, a).map(ListEngine),
            context.control,
        )
    }
}

/// Provider construction with transactional streams; same controls and
/// preservation rules as [`list_construction_from_with_provider`].
/// Model and source ownership validate even at zero score budget.
pub fn list_stream_construction_from_with_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ProviderConstructionResult<ListAssignment<V>, S>, ListStreamScoringError> {
    list_stream(
        problem,
        graph,
        sources,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn list_stream<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: ProviderConstructionConfig,
    context: crate::provider_search::ProviderContext<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
    >,
) -> Result<ProviderConstructionResult<ListAssignment<V>, S>, ListStreamScoringError> {
    sources.validate(&graph)?;
    problem.validate(&initial)?;
    if let Some(observer) = context.observer {
        construct(
            ListShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                ListStreamScoreDirector::new(problem, graph, sources, a)
                    .map(|d| ListEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            ListShape(problem),
            initial,
            config,
            context.provider,
            |a| ListStreamScoreDirector::new(problem, graph, sources, a).map(ListEngine),
            context.control,
        )
    }
}

/// Build an assignment using a custom candidate pool and scoped mixed scoring.
/// Existing assignments are preserved; list insertions may shift old indices.
/// Stable ties keep the first candidate. By default the best evaluated prefix
/// is committed when interrupted; DiscardIncompleteStep retains the prior step.
/// No additional provider pull is made to prove exhaustion at
/// an exact limit. Under DiscardIncompleteStep, reaching that boundary discards
/// the scan even if the provider would have ended on its next pull. An explicit
/// early pick still completes the step. EvaluateUnassigned appends a scored
/// no-change after natural provider exhaustion when optional items remain; this
/// synthetic candidate consumes score/move budget but no raw-proposal budget.
pub fn mixed_construction_from_with_provider<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: ProviderConstructionConfig,
    provider: &mut dyn MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedScoringError> {
    mixed(
        problem,
        network,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

type MixedContext<'a, B, L, S> = crate::provider_search::ProviderContext<
    'a,
    MixedProblem<B, L>,
    MixedAssignment<B, L>,
    MixedMove<B, L>,
    S,
>;
pub(crate) fn mixed<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: ProviderConstructionConfig,
    context: MixedContext<'_, B, L, S>,
) -> Result<ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedScoringError> {
    network.validate(problem, &initial)?;
    if let Some(observer) = context.observer {
        construct(
            MixedShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                MixedScoreDirector::new(problem, network, a)
                    .map(|d| MixedEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            MixedShape(problem),
            initial,
            config,
            context.provider,
            |a| MixedScoreDirector::new(problem, network, a).map(MixedEngine),
            context.control,
        )
    }
}

/// Provider construction with transactional streams; same controls and
/// preservation rules as [`mixed_construction_from_with_provider`].
/// Model and source ownership validate even at zero score budget.
pub fn mixed_stream_construction_from_with_provider<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: ProviderConstructionConfig,
    provider: &mut dyn MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedStreamScoringError> {
    mixed_stream(
        problem,
        graph,
        sources,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn mixed_stream<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: ProviderConstructionConfig,
    context: MixedContext<'_, B, L, S>,
) -> Result<ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedStreamScoringError> {
    sources.validate(&graph)?;
    problem.validate(&initial)?;
    if let Some(observer) = context.observer {
        construct(
            MixedShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                MixedStreamScoreDirector::new(problem, graph, sources, a)
                    .map(|d| MixedEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            MixedShape(problem),
            initial,
            config,
            context.provider,
            |a| MixedStreamScoreDirector::new(problem, graph, sources, a).map(MixedEngine),
            context.control,
        )
    }
}

// Backend-independent entry points for calculator-backed phase providers.
pub(crate) fn basic_director<V: Eq + Clone, S: AdditiveScore, D: SearchDirector<V, S>>(
    problem: &FiniteProblem<V>,
    initial: Assignment<V>,
    config: ProviderConstructionConfig<'_>,
    context: crate::provider_search::ProviderContext<
        '_,
        FiniteProblem<V>,
        Assignment<V>,
        Move<V>,
        S,
    >,
    create: impl FnOnce(Assignment<V>) -> Result<D, D::Error>,
) -> Result<ProviderConstructionResult<Assignment<V>, S>, D::Error>
where
    D::Error: From<crate::ModelError>,
{
    problem.validate(&initial).map_err(D::Error::from)?;
    if let Some(observer) = context.observer {
        construct(
            BasicShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                create(a)
                    .map(|d| BasicEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            BasicShape(problem),
            initial,
            config,
            context.provider,
            |a| create(a).map(BasicEngine),
            context.control,
        )
    }
}

pub(crate) fn list_director<V: Eq + Clone, S: AdditiveScore, D: ListSearchDirector<V, S>>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: ProviderConstructionConfig<'_>,
    context: crate::provider_search::ProviderContext<
        '_,
        ListProblem<V>,
        ListAssignment<V>,
        ListMove<V>,
        S,
    >,
    create: impl FnOnce(ListAssignment<V>) -> Result<D, D::Error>,
) -> Result<ProviderConstructionResult<ListAssignment<V>, S>, D::Error>
where
    D::Error: From<crate::ListError>,
{
    problem.validate(&initial).map_err(D::Error::from)?;
    if let Some(observer) = context.observer {
        construct(
            ListShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                create(a)
                    .map(|d| ListEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            ListShape(problem),
            initial,
            config,
            context.provider,
            |a| create(a).map(ListEngine),
            context.control,
        )
    }
}

pub(crate) fn mixed_director<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: MixedSearchDirector<B, L, S>,
>(
    problem: &MixedProblem<B, L>,
    initial: MixedAssignment<B, L>,
    config: ProviderConstructionConfig<'_>,
    context: MixedContext<'_, B, L, S>,
    create: impl FnOnce(MixedAssignment<B, L>) -> Result<D, D::Error>,
) -> Result<ProviderConstructionResult<MixedAssignment<B, L>, S>, D::Error>
where
    D::Error: From<crate::MixedError>,
{
    problem.validate(&initial).map_err(D::Error::from)?;
    if let Some(observer) = context.observer {
        construct(
            MixedShape(problem),
            initial,
            config,
            context.provider,
            |a| {
                create(a)
                    .map(|d| MixedEngine(crate::solution_observation::Observed::new(d, observer)))
            },
            context.control,
        )
    } else {
        construct(
            MixedShape(problem),
            initial,
            config,
            context.provider,
            |a| create(a).map(MixedEngine),
            context.control,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_limit::SearchControl;
    struct Control {
        polls: usize,
    }
    impl SearchControl for Control {
        fn construction_stopped(&mut self) -> bool {
            self.polls += 1;
            self.polls >= 5
        }
        fn start_search(&mut self) {
            panic!("construction started stagnation clock");
        }
        fn record_improvement(&mut self) {
            panic!("construction reset stagnation");
        }
        fn complete_step(&mut self, _: bool) {
            panic!("construction consumed search step");
        }
        fn stop(&mut self) -> Option<crate::LocalSearchTermination> {
            panic!("construction polled search-only stop");
        }
    }
    #[test]
    fn shared_condition_keeps_scored_prefix_and_observes_only_commits() {
        let p = FiniteProblem::new(vec![crate::Variable::new(vec![1, 2]); 2]).unwrap();
        let n = ConstraintNetwork::<_, crate::SimpleScore>::new(2);
        let mut control = Control { polls: 0 };
        let mut calls = 0;
        let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            calls += 1;
            vec![
                Move::Change {
                    variable: 0,
                    value: Some(1),
                },
                Move::Change {
                    variable: 0,
                    value: Some(2),
                },
            ]
        };
        let mut updates = vec![];
        let mut observer = |u: crate::SolutionUpdate<'_, Assignment<i32>, crate::SimpleScore>| {
            updates.push((u.assignment.clone(), u.complete));
        };
        let r = basic(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig::default(),
            crate::provider_search::ProviderContext {
                provider: crate::MoveProviderBinding::Infallible(&mut provider),
                control: Some(&mut control),
                observer: Some(&mut observer),
            },
        )
        .unwrap();
        assert_eq!(r.termination, ProviderConstructionTermination::Condition);
        assert_eq!(r.assignment.values(), &[Some(1), None]);
        assert_eq!((r.evaluations, r.scanned_moves, calls), (2, 1, 1));
        assert_eq!(
            updates,
            vec![(p.initial_assignment(), false), (r.assignment, false)]
        );
        assert_eq!(control.polls, 5);
    }
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn construction_from_with_fallible_provider<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn crate::FallibleMoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<ProviderConstructionResult<Assignment<V>, S>, ScoringError> {
    basic(
        problem,
        network,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn stream_construction_from_with_fallible_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn crate::FallibleMoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<ProviderConstructionResult<Assignment<V>, S>, StreamScoringError> {
    basic_stream(
        problem,
        graph,
        sources,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn list_construction_from_with_fallible_provider<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn crate::FallibleMoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ProviderConstructionResult<ListAssignment<V>, S>, ListScoringError> {
    list(
        problem,
        network,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn list_stream_construction_from_with_fallible_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: ProviderConstructionConfig,
    provider: &mut dyn crate::FallibleMoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ProviderConstructionResult<ListAssignment<V>, S>, ListStreamScoringError> {
    list_stream(
        problem,
        graph,
        sources,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn mixed_construction_from_with_fallible_provider<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: ProviderConstructionConfig,
    provider: &mut dyn crate::FallibleMoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedScoringError> {
    mixed(
        problem,
        network,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn mixed_stream_construction_from_with_fallible_provider<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: ProviderConstructionConfig,
    provider: &mut dyn crate::FallibleMoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<ProviderConstructionResult<MixedAssignment<B, L>, S>, MixedStreamScoringError> {
    mixed_stream(
        problem,
        graph,
        sources,
        initial,
        config,
        crate::provider_search::ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}
