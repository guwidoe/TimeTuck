//! Typed standalone and monitored provider search entry points.
use crate::MoveProvider;
use crate::{
    AdditiveScore, Assignment, ConstraintNetwork, FiniteProblem, ListAssignment,
    ListConstraintNetwork, ListLocalSearchError, ListLocalSearchResult, ListMove, ListProblem,
    ListScoreDirector, ListStreamScoreDirector, ListStreamSources, LocalSearchConfig,
    LocalSearchError, LocalSearchResult, MixedAssignment, MixedConstraintNetwork,
    MixedLocalSearchConfig, MixedLocalSearchError, MixedLocalSearchResult, MixedMove, MixedProblem,
    MixedScoreDirector, MixedStreamScoreDirector, MixedStreamSources, Move, PlanningStreamSource,
    ScoreDirector, ScoringError, StreamGraph, StreamScoreDirector,
};

pub(crate) struct ProviderContext<'a, P, A, M, S> {
    pub provider: crate::MoveProviderBinding<'a, P, A, M>,
    pub control: Option<&'a mut dyn crate::time_limit::SearchControl<S>>,
    pub observer: Option<&'a mut ProviderObserver<'a, A, S>>,
}
pub(crate) fn clock<S>(
    config: &LocalSearchConfig<'_, S>,
    shared: &Option<&mut dyn crate::time_limit::SearchControl<S>>,
) -> impl FnMut() -> std::time::Duration + use<S> {
    crate::solver_clock::monitored_clock(
        config.max_time.is_some()
            || config.max_unimproved_time.is_some()
            || config.diminished_returns.is_some()
            || config.significant_improvement.is_some()
            || config.uses_elapsed_acceptance(),
        shared,
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
    basic(
        p,
        n,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn basic<V: Eq + Clone, S: AdditiveScore>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    context: ProviderContext<'_, FiniteProblem<V>, Assignment<V>, Move<V>, S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.validate_acceptance()?;
    if n.variable_count() != p.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: n.variable_count(),
            actual: p.variables().len(),
        }
        .into());
    }
    let clock = clock(&config, &context.control);
    if let Some(observer) = context.observer {
        crate::local_search::search_impl_with_provider(
            context.control,
            p,
            Some(initial),
            config,
            |a| {
                ScoreDirector::new(p, n, a)
                    .map(|d| crate::solution_observation::Observed::new(d, observer))
            },
            clock,
            Some(context.provider),
        )
    } else {
        crate::local_search::search_impl_with_provider(
            context.control,
            p,
            Some(initial),
            config,
            |a| ScoreDirector::new(p, n, a),
            clock,
            Some(context.provider),
        )
    }
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
    basic_stream_consuming(
        p,
        graph,
        source,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn basic_stream<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    context: ProviderContext<'_, FiniteProblem<V>, Assignment<V>, Move<V>, S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    basic_stream_impl(p, graph, source, initial, config, context, false)
}

fn basic_stream_consuming<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    context: ProviderContext<'_, FiniteProblem<V>, Assignment<V>, Move<V>, S>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    basic_stream_impl(p, graph, source, initial, config, context, true)
}

fn basic_stream_impl<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    context: ProviderContext<'_, FiniteProblem<V>, Assignment<V>, Move<V>, S>,
    consuming: bool,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.validate_acceptance()?;
    crate::stream_scoring::validate_source(&graph, &source)?;
    let clock = clock(&config, &context.control);
    if let Some(observer) = context.observer {
        crate::local_search::search_impl_with_provider(
            context.control,
            p,
            Some(initial),
            config,
            |a| {
                (if consuming {
                    StreamScoreDirector::new_consuming(p, graph, source, a)
                } else {
                    StreamScoreDirector::new(p, graph, source, a)
                })
                .map(|d| crate::solution_observation::Observed::new(d, observer))
            },
            clock,
            Some(context.provider),
        )
    } else {
        crate::local_search::search_impl_with_provider(
            context.control,
            p,
            Some(initial),
            config,
            |a| {
                if consuming {
                    StreamScoreDirector::new_consuming(p, graph, source, a)
                } else {
                    StreamScoreDirector::new(p, graph, source, a)
                }
            },
            clock,
            Some(context.provider),
        )
    }
}
/// Local search using a custom provider and scoped list scoring.
pub fn list_local_search_from_with_provider<V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list(
        p,
        n,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn list<V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    context: ProviderContext<'_, ListProblem<V>, ListAssignment<V>, ListMove<V>, S>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.validate_acceptance()?;
    n.validate(p, &initial)?;
    let clock = clock(&config, &context.control);
    if let Some(observer) = context.observer {
        crate::list_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| {
                ListScoreDirector::new(p, n, a)
                    .map(|d| crate::solution_observation::Observed::new(d, observer))
            },
            clock,
            Some(context.provider),
        )
    } else {
        crate::list_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| ListScoreDirector::new(p, n, a),
            clock,
            Some(context.provider),
        )
    }
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
    list_stream(
        p,
        graph,
        sources,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn list_stream<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    context: ProviderContext<'_, ListProblem<V>, ListAssignment<V>, ListMove<V>, S>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.validate_acceptance()?;
    sources.validate(&graph)?;
    let clock = clock(&config, &context.control);
    if let Some(observer) = context.observer {
        crate::list_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| {
                ListStreamScoreDirector::new(p, graph, sources, a)
                    .map(|d| crate::solution_observation::Observed::new(d, observer))
            },
            clock,
            Some(context.provider),
        )
    } else {
        crate::list_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| ListStreamScoreDirector::new(p, graph, sources, a),
            clock,
            Some(context.provider),
        )
    }
}
/// Local search using a custom provider and scoped mixed scoring.
pub fn mixed_local_search_from_with_provider<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<S>,
    provider: &mut dyn MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    mixed(
        p,
        n,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Infallible(provider),
            control: None,
            observer: None,
        },
    )
}

pub(crate) fn mixed<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<S>,
    context: MixedProviderContext<'_, B, L, S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    config.search.validate_acceptance()?;
    n.validate(p, &initial)?;
    let clock = clock(&config.search, &context.control);
    if let Some(observer) = context.observer {
        crate::mixed_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| {
                MixedScoreDirector::new(p, n, a)
                    .map(|d| crate::solution_observation::Observed::new(d, observer))
            },
            clock,
            Some(context.provider),
        )
    } else {
        crate::mixed_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| MixedScoreDirector::new(p, n, a),
            clock,
            Some(context.provider),
        )
    }
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
    mixed_stream(
        p,
        graph,
        sources,
        initial,
        config,
        ProviderContext {
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
    p: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<S>,
    context: MixedProviderContext<'_, B, L, S>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    config.search.validate_acceptance()?;
    sources.validate(&graph)?;
    let clock = clock(&config.search, &context.control);
    if let Some(observer) = context.observer {
        crate::mixed_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| {
                MixedStreamScoreDirector::new(p, graph, sources, a)
                    .map(|d| crate::solution_observation::Observed::new(d, observer))
            },
            clock,
            Some(context.provider),
        )
    } else {
        crate::mixed_local_search::search_impl_with_provider(
            context.control,
            p,
            initial,
            config,
            |a| MixedStreamScoreDirector::new(p, graph, sources, a),
            clock,
            Some(context.provider),
        )
    }
}

type ProviderObserver<'a, A, S> = dyn FnMut(crate::SolutionUpdate<'_, A, S>) + 'a;
type MixedProviderContext<'a, B, L, S> =
    ProviderContext<'a, MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>, S>;

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn local_search_from_with_fallible_provider<V: Eq + Clone, S: AdditiveScore>(
    p: &FiniteProblem<V>,
    n: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn crate::FallibleMoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    basic(
        p,
        n,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn stream_local_search_from_with_fallible_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn crate::FallibleMoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>,
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    basic_stream_consuming(
        p,
        graph,
        source,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn list_local_search_from_with_fallible_provider<V: Eq + Clone, S: AdditiveScore>(
    p: &ListProblem<V>,
    n: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn crate::FallibleMoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list(
        p,
        n,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn list_stream_local_search_from_with_fallible_provider<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    provider: &mut dyn crate::FallibleMoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>,
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list_stream(
        p,
        graph,
        sources,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn mixed_local_search_from_with_fallible_provider<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
>(
    p: &MixedProblem<B, L>,
    n: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<S>,
    provider: &mut dyn crate::FallibleMoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    mixed(
        p,
        n,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}

/// Run with a fallible provider; an observed error discards the current scan.
#[allow(clippy::too_many_arguments)]
pub fn mixed_stream_local_search_from_with_fallible_provider<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: MixedLocalSearchConfig<S>,
    provider: &mut dyn crate::FallibleMoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>,
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    mixed_stream(
        p,
        graph,
        sources,
        initial,
        config,
        ProviderContext {
            provider: crate::MoveProviderBinding::Fallible(provider),
            control: None,
            observer: None,
        },
    )
}
