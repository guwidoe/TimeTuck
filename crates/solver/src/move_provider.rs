//! Problem-specific candidate generation using the existing local-search engine.
use crate::{
    AdditiveScore, Assignment, ConstraintNetwork, FiniteProblem, ListAssignment,
    ListConstraintNetwork, ListLocalSearchError, ListLocalSearchResult, ListMove, ListProblem,
    ListScoreDirector, ListStreamScoreDirector, ListStreamSources, LocalSearchConfig,
    LocalSearchError, LocalSearchResult, MixedAssignment, MixedConstraintNetwork,
    MixedLocalSearchConfig, MixedLocalSearchError, MixedLocalSearchResult, MixedMove, MixedProblem,
    MixedScoreDirector, MixedStreamScoreDirector, MixedStreamSources, Move, PlanningStreamSource,
    ScoreDirector, ScoringError, StreamGraph, StreamScoreDirector,
};

/// Supply candidates for one immutable assignment snapshot at a completed-step
/// boundary. The solver recreates this iterator each step and applies its normal
/// scoring, acceptance, tabu, foraging, rollback and stopping rules.
///
/// Providers replace built-in neighborhoods, union settings and mixed joint-move
/// generation. Variable shuffling does not reorder provider inputs; candidate
/// shuffling and tie breaking still apply. Invalid moves fail the search using
/// the existing model/scoring error. Returning no moves ends an ordinary scan.
/// Providers may retain state, but must not mutate the model or snapshot.
///
/// Iterator work is cooperative: factory/next calls must return promptly. Only
/// scored proposals count toward evaluation limits; candidate shuffling may
/// prefetch a bounded window. Infinite iterators require a search budget.
///
/// Closures returning Vec<M> implement this trait for convenience. Implement the
/// trait directly to return lazy iterators borrowing the current snapshot.
pub trait MoveProvider<P, A, M> {
    fn moves<'a>(
        &'a mut self,
        problem: &'a P,
        assignment: &'a A,
        completed_steps: u64,
    ) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a;
}
impl<P, A, M, F> MoveProvider<P, A, M> for F
where
    F: FnMut(&P, &A, u64) -> Vec<M>,
{
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        Box::new(self(p, a, step).into_iter())
    }
}

pub(crate) enum ProvidedMoves<'a, M, I> {
    BuiltIn(I),
    Custom(Box<dyn Iterator<Item = M> + 'a>),
}
impl<M, I: Iterator<Item = M>> Iterator for ProvidedMoves<'_, M, I> {
    type Item = M;
    fn next(&mut self) -> Option<M> {
        match self {
            Self::BuiltIn(i) => i.next(),
            Self::Custom(i) => i.next(),
        }
    }
}
fn clock<S>(config: &LocalSearchConfig<'_, S>) -> impl FnMut() -> std::time::Duration + use<S> {
    crate::time_limit::monotonic_clock(
        config.max_time.is_some()
            || config.max_unimproved_time.is_some()
            || config.diminished_returns.is_some()
            || config.significant_improvement.is_some(),
    )
}

/// Local search using a custom provider and scoped basic scoring.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0_i32,1,2]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(i64::from(*v[0].unwrap())))?;
/// let mut provider = |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
///     if a.values()[0] == Some(2) { vec![] } else {
///         vec![Move::Change { variable: 0, value: Some(2) }]
///     }
/// };
/// let result = local_search_from_with_provider(&p, &n, p.initial_assignment(),
///     LocalSearchConfig::default(), &mut provider)?;
/// assert_eq!(result.best.unwrap().score, SimpleScore::new(2));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn local_search_from_with_provider<V: Eq + Clone, S: AdditiveScore>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.validate_acceptance()?;
    if n.variable_count() != p.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: n.variable_count(),
            actual: p.variables().len(),
        }
        .into());
    }
    let clock = clock(&config);
    crate::local_search::search_impl_with_provider(
        None,
        p,
        Some(initial),
        config,
        |a| ScoreDirector::new(p, n, a),
        clock,
        Some(provider),
    )
}
/// Local search using a custom provider and stream basic scoring.
pub fn stream_local_search_from_with_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.validate_acceptance()?;
    crate::stream_scoring::validate_source(&graph, &source)?;
    let clock = clock(&config);
    crate::local_search::search_impl_with_provider(
        None,
        p,
        Some(initial),
        config,
        |a| StreamScoreDirector::new(p, graph, source, a),
        clock,
        Some(provider),
    )
}
/// Local search using a custom provider and scoped list scoring.
pub fn list_local_search_from_with_provider<V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.validate_acceptance()?;
    n.validate(p, &initial)?;
    let clock = clock(&config);
    crate::list_local_search::search_impl_with_provider(
        None,
        p,
        initial,
        config,
        |a| ListScoreDirector::new(p, n, a),
        clock,
        Some(provider),
    )
}
/// Local search using a custom provider and stream list scoring.
pub fn list_stream_local_search_from_with_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.validate_acceptance()?;
    sources.validate(&graph)?;
    let clock = clock(&config);
    crate::list_local_search::search_impl_with_provider(
        None,
        p,
        initial,
        config,
        |a| ListStreamScoreDirector::new(p, graph, sources, a),
        clock,
        Some(provider),
    )
}
/// Local search using a custom provider and scoped mixed scoring.
pub fn mixed_local_search_from_with_provider<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    config.search.validate_acceptance()?;
    n.validate(p, &initial)?;
    let clock = clock(&config.search);
    crate::mixed_local_search::search_impl_with_provider(
        None,
        p,
        initial,
        config,
        |a| MixedScoreDirector::new(p, n, a),
        clock,
        Some(provider),
    )
}
/// Local search using a custom provider and stream mixed scoring.
pub fn mixed_stream_local_search_from_with_provider<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    config.search.validate_acceptance()?;
    sources.validate(&graph)?;
    let clock = clock(&config.search);
    crate::mixed_local_search::search_impl_with_provider(
        None,
        p,
        initial,
        config,
        |a| MixedStreamScoreDirector::new(p, graph, sources, a),
        clock,
        Some(provider),
    )
}
