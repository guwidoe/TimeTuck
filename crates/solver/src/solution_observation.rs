//! Synchronous notifications of committed construction/local-search states.
use crate::*;

/// A scored state borrowed for one observer call. Construction includes partial
/// initialization; local-search updates are complete retained bests. No candidate
/// trials are exposed. Clone the assignment when sending an owned snapshot to a
/// SolverProgress publisher. Observers must not mutate shared problem facts.
/// Each phase emits its initial scored state even when it equals a prior phase's
/// last event. A composed publisher can filter these duplicates by score.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
/// let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
/// let mut states = Vec::new();
/// let r = greedy_construction_with_observer(&p, &n, ConstructionConfig::default(),
///     |u| states.push((u.assignment.clone(), u.score, u.complete)))?;
/// assert_eq!(states.len(), 2); // Scored partial input, then committed placement.
/// assert!(!states[0].2);
/// assert!(states[1].2);
/// assert_eq!(states[1].0, r.assignment);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct SolutionUpdate<'a, A, S> {
    pub assignment: &'a A,
    pub score: S,
    pub complete: bool,
}
struct Observed<D, F> {
    director: D,
    observer: F,
}

impl<V: Eq + Clone, S: AdditiveScore, D: crate::search_scoring::SearchDirector<V, S>, F>
    crate::search_scoring::SearchDirector<V, S> for Observed<D, F>
where
    F: FnMut(SolutionUpdate<'_, Assignment<V>, S>),
{
    type Error = D::Error;
    fn assignment(&self) -> &Assignment<V> {
        self.director.assignment()
    }
    fn score(&self) -> S {
        self.director.score()
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        self.director.initial_constraint_evaluations()
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.director.assert_consistent()
    }
    fn notify_best(&mut self, complete: impl FnOnce(&Assignment<V>) -> bool) {
        (self.observer)(SolutionUpdate {
            assignment: self.director.assignment(),
            score: self.director.score(),
            complete: complete(self.director.assignment()),
        });
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = Move<V>>,
        max_evaluations: u64,
        allow_rejected: bool,
        accepts: impl FnMut(&Move<V>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<crate::scoring::Selection<V>, Self::Error> {
        self.director.select_best(
            moves,
            max_evaluations,
            allow_rejected,
            accepts,
            quit_early,
            assert_scores,
        )
    }
}

/// Construct with synchronous scored-state notifications. See [`SolutionUpdate`].
/// Emits initial scoring and installed placements (including partial progress).
/// Observer time belongs to the phase deadline; no additional scores are computed.
/// Panics propagate. Invalid input and zero scoring budget emit no updates.
pub fn greedy_construction_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    config: ConstructionConfig,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<ConstructionResult<V, S>, ScoringError> {
    if network.variable_count() != problem.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        });
    }
    crate::construction::construction_impl(problem, config, |initial| {
        ScoreDirector::new(problem, network, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Improve with synchronous initial-best and strict-best notifications. See
/// [`SolutionUpdate`]. Ties, rejected trials and accepted worsening moves emit
/// nothing. Observer time belongs to phase limits; evaluation counts are unchanged.
/// Panics propagate. Validation and cancellation keep the ordinary search rules.
pub fn local_search_from_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    local_search_from_observed(None, problem, network, initial, config, observer)
}

pub(crate) fn local_search_from_observed<V: Eq + Clone, S: AdditiveScore>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.acceptance.validate()?;
    if network.variable_count() != problem.variables().len() {
        return Err(ScoringError::VariableCount {
            expected: network.variable_count(),
            actual: problem.variables().len(),
        }
        .into());
    }
    crate::local_search::search_impl_monitored(shared, problem, Some(initial), config, |initial| {
        ScoreDirector::new(problem, network, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Construct with synchronous scored-state notifications. See [`SolutionUpdate`].
/// Emits initial scoring and installed placements (including partial progress).
/// Observer time belongs to the phase deadline; no additional scores are computed.
/// Panics propagate. Invalid input and zero scoring budget emit no updates.
pub fn greedy_stream_construction_with_observer<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    config: ConstructionConfig,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<ConstructionResult<V, S>, StreamScoringError> {
    crate::stream_scoring::validate_source(&graph, &source)?;
    crate::construction::construction_impl(problem, config, |initial| {
        StreamScoreDirector::new(problem, graph, source, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Improve with synchronous initial-best and strict-best notifications. See
/// [`SolutionUpdate`]. Ties, rejected trials and accepted worsening moves emit
/// nothing. Observer time belongs to phase limits; evaluation counts are unchanged.
/// Panics propagate. Validation and cancellation keep the ordinary search rules.
pub fn stream_local_search_from_with_observer<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    stream_local_search_from_observed(None, problem, graph, source, initial, config, observer)
}

pub(crate) fn stream_local_search_from_observed<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    source: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<LocalSearchResult<V, S>, LocalSearchError> {
    config.acceptance.validate()?;
    crate::stream_scoring::validate_source(&graph, &source)?;
    crate::local_search::search_impl_monitored(shared, problem, Some(initial), config, |initial| {
        StreamScoreDirector::new(problem, graph, source, initial)
            .map(|director| Observed { director, observer })
    })
}

impl<V: Eq + Clone, S: AdditiveScore, D: crate::list_search_scoring::ListSearchDirector<V, S>, F>
    crate::list_search_scoring::ListSearchDirector<V, S> for Observed<D, F>
where
    F: FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
{
    type Error = D::Error;
    fn assignment(&self) -> &ListAssignment<V> {
        self.director.assignment()
    }
    fn score(&self) -> S {
        self.director.score()
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        self.director.initial_constraint_evaluations()
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.director.assert_consistent()
    }
    fn notify_best(&mut self, complete: impl FnOnce(&ListAssignment<V>) -> bool) {
        (self.observer)(SolutionUpdate {
            assignment: self.director.assignment(),
            score: self.director.score(),
            complete: complete(self.director.assignment()),
        });
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = ListMove<V>>,
        max_evaluations: u64,
        allow_rejected: bool,
        accepts: impl FnMut(&ListMove<V>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<crate::list_scoring::ListSelection<V>, Self::Error> {
        self.director.select_best(
            moves,
            max_evaluations,
            allow_rejected,
            accepts,
            quit_early,
            assert_scores,
        )
    }
}

/// Construct with synchronous scored-state notifications. See [`SolutionUpdate`].
/// Emits initial scoring and installed placements (including partial progress).
/// Observer time belongs to the phase deadline; no additional scores are computed.
/// Panics propagate. Invalid input and zero scoring budget emit no updates.
pub fn greedy_list_construction_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    config: ConstructionConfig,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListConstructionResult<V, S>, ListScoringError> {
    let initial = problem.initial_assignment();
    network.validate(problem, &initial)?;
    crate::list_construction::construction_impl(problem, initial, config, |initial| {
        ListScoreDirector::new(problem, network, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Improve with synchronous initial-best and strict-best notifications. See
/// [`SolutionUpdate`]. Ties, rejected trials and accepted worsening moves emit
/// nothing. Observer time belongs to phase limits; evaluation counts are unchanged.
/// Panics propagate. Validation and cancellation keep the ordinary search rules.
pub fn list_local_search_from_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list_local_search_from_observed(None, problem, network, initial, config, observer)
}

pub(crate) fn list_local_search_from_observed<V: Eq + Clone, S: AdditiveScore>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.acceptance.validate()?;
    network.validate(problem, &initial)?;
    crate::list_local_search::search_impl_monitored(shared, problem, initial, config, |initial| {
        ListScoreDirector::new(problem, network, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Construct with synchronous scored-state notifications. See [`SolutionUpdate`].
/// Emits initial scoring and installed placements (including partial progress).
/// Observer time belongs to the phase deadline; no additional scores are computed.
/// Panics propagate. Invalid input and zero scoring budget emit no updates.
pub fn greedy_list_stream_construction_with_observer<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    config: ConstructionConfig,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListConstructionResult<V, S>, ListStreamScoringError> {
    let initial = problem.initial_assignment();
    sources.validate(&graph)?;
    crate::list_construction::construction_impl(problem, initial, config, |initial| {
        ListStreamScoreDirector::new(problem, graph, sources, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Improve with synchronous initial-best and strict-best notifications. See
/// [`SolutionUpdate`]. Ties, rejected trials and accepted worsening moves emit
/// nothing. Observer time belongs to phase limits; evaluation counts are unchanged.
/// Panics propagate. Validation and cancellation keep the ordinary search rules.
pub fn list_stream_local_search_from_with_observer<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    list_stream_local_search_from_observed(None, problem, graph, sources, initial, config, observer)
}

pub(crate) fn list_stream_local_search_from_observed<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: LocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListLocalSearchResult<V, S>, ListLocalSearchError> {
    config.acceptance.validate()?;
    sources.validate(&graph)?;
    crate::list_local_search::search_impl_monitored(shared, problem, initial, config, |initial| {
        ListStreamScoreDirector::new(problem, graph, sources, initial)
            .map(|director| Observed { director, observer })
    })
}

impl<
    B: Eq + Clone,
    L: Eq + Clone,
    S: AdditiveScore,
    D: crate::mixed_search_scoring::MixedSearchDirector<B, L, S>,
    F,
> crate::mixed_search_scoring::MixedSearchDirector<B, L, S> for Observed<D, F>
where
    F: FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
{
    type Error = D::Error;
    fn assignment(&self) -> &MixedAssignment<B, L> {
        self.director.assignment()
    }
    fn score(&self) -> S {
        self.director.score()
    }
    fn initial_constraint_evaluations(&self) -> u128 {
        self.director.initial_constraint_evaluations()
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error> {
        self.director.assert_consistent()
    }
    fn notify_best(&mut self, complete: impl FnOnce(&MixedAssignment<B, L>) -> bool) {
        (self.observer)(SolutionUpdate {
            assignment: self.director.assignment(),
            score: self.director.score(),
            complete: complete(self.director.assignment()),
        });
    }
    fn select_best(
        &mut self,
        moves: impl IntoIterator<Item = MixedMove<B, L>>,
        max_evaluations: u64,
        allow_rejected: bool,
        accepts: impl FnMut(&MixedMove<B, L>, S) -> bool,
        quit_early: impl Fn(S) -> bool,
        assert_scores: bool,
    ) -> Result<crate::mixed_scoring::MixedSelection<B, L>, Self::Error> {
        self.director.select_best(
            moves,
            max_evaluations,
            allow_rejected,
            accepts,
            quit_early,
            assert_scores,
        )
    }
}

/// Construct with synchronous scored-state notifications. See [`SolutionUpdate`].
/// Emits initial scoring and installed placements (including partial progress).
/// Observer time belongs to the phase deadline; no additional scores are computed.
/// Panics propagate. Invalid input and zero scoring budget emit no updates.
pub fn greedy_mixed_construction_with_observer<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    settings: MixedConstructionConfig,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedConstructionResult<B, L, S>, MixedScoringError> {
    let initial = problem.initial_assignment();
    network.validate(problem, &initial)?;
    crate::mixed_construction::construction_impl(problem, initial, settings, |initial| {
        MixedScoreDirector::new(problem, network, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Improve with synchronous initial-best and strict-best notifications. See
/// [`SolutionUpdate`]. Ties, rejected trials and accepted worsening moves emit
/// nothing. Observer time belongs to phase limits; evaluation counts are unchanged.
/// Panics propagate. Validation and cancellation keep the ordinary search rules.
pub fn mixed_local_search_from_with_observer<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    mixed_local_search_from_observed(None, problem, network, initial, settings, observer)
}

pub(crate) fn mixed_local_search_from_observed<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    settings.search.acceptance.validate()?;
    network.validate(problem, &initial)?;
    crate::mixed_local_search::search_impl_monitored(
        shared,
        problem,
        initial,
        settings,
        |initial| {
            MixedScoreDirector::new(problem, network, initial)
                .map(|director| Observed { director, observer })
        },
    )
}

/// Construct with synchronous scored-state notifications. See [`SolutionUpdate`].
/// Emits initial scoring and installed placements (including partial progress).
/// Observer time belongs to the phase deadline; no additional scores are computed.
/// Panics propagate. Invalid input and zero scoring budget emit no updates.
pub fn greedy_mixed_stream_construction_with_observer<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    settings: MixedConstructionConfig,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedConstructionResult<B, L, S>, MixedStreamScoringError> {
    let initial = problem.initial_assignment();
    sources.validate(&graph)?;
    crate::mixed_construction::construction_impl(problem, initial, settings, |initial| {
        MixedStreamScoreDirector::new(problem, graph, sources, initial)
            .map(|director| Observed { director, observer })
    })
}

/// Improve with synchronous initial-best and strict-best notifications. See
/// [`SolutionUpdate`]. Ties, rejected trials and accepted worsening moves emit
/// nothing. Observer time belongs to phase limits; evaluation counts are unchanged.
/// Panics propagate. Validation and cancellation keep the ordinary search rules.
pub fn mixed_stream_local_search_from_with_observer<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    mixed_stream_local_search_from_observed(
        None, problem, graph, sources, initial, settings, observer,
    )
}

pub(crate) fn mixed_stream_local_search_from_observed<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    settings: MixedLocalSearchConfig<S>,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<MixedLocalSearchResult<B, L, S>, MixedLocalSearchError> {
    settings.search.acceptance.validate()?;
    sources.validate(&graph)?;
    crate::mixed_local_search::search_impl_monitored(
        shared,
        problem,
        initial,
        settings,
        |initial| {
            MixedStreamScoreDirector::new(problem, graph, sources, initial)
                .map(|director| Observed { director, observer })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, time::Duration};
    #[test]
    fn construction_observer_time_counts_toward_deadline() {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
        let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
        let now = Cell::new(Duration::ZERO);
        let count = Cell::new(0);
        let r = crate::construction::construction_impl_with_clock(
            &p,
            ConstructionConfig {
                max_time: Some(Duration::from_millis(5)),
                ..Default::default()
            },
            |initial| {
                ScoreDirector::new(&p, &n, initial).map(|director| Observed {
                    director,
                    observer: |u: SolutionUpdate<'_, Assignment<i32>, SimpleScore>| {
                        assert!(!u.complete);
                        count.set(count.get() + 1);
                        now.set(Duration::from_millis(10));
                    },
                })
            },
            || now.get(),
        )
        .unwrap();
        assert_eq!(r.termination, ConstructionTermination::TimeLimit);
        assert_eq!(r.evaluations, 1);
        assert_eq!(count.get(), 1);
        assert!(!r.complete);
    }
    #[test]
    fn local_observer_time_counts_toward_elapsed_and_stagnation_deadlines() {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
        let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
        for stagnation in [false, true] {
            let now = Cell::new(Duration::ZERO);
            let count = Cell::new(0);
            let mut c = LocalSearchConfig::default();
            if stagnation {
                c.max_unimproved_time = Some(Duration::from_millis(5));
            } else {
                c.max_time = Some(Duration::from_millis(5));
            }
            let r = crate::local_search::search_impl_with_clock(
                &p,
                Some(p.initial_assignment()),
                c,
                |initial| {
                    ScoreDirector::new(&p, &n, initial).map(|director| Observed {
                        director,
                        observer: |u: SolutionUpdate<'_, Assignment<i32>, SimpleScore>| {
                            assert!(u.complete);
                            count.set(count.get() + 1);
                            now.set(Duration::from_millis(10));
                        },
                    })
                },
                || now.get(),
            )
            .unwrap();
            assert_eq!(
                r.termination,
                if stagnation {
                    LocalSearchTermination::UnimprovedTimeLimit
                } else {
                    LocalSearchTermination::TimeLimit
                }
            );
            assert_eq!(r.evaluations, 1);
            assert_eq!(r.completed_steps, 0);
            assert_eq!(count.get(), 1);
        }
    }
}
