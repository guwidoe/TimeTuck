//! List-specific removal, pin preservation and acceptance identities for repair.
use crate::*;
use std::time::Instant;

impl From<ListLocalSearchError> for RuinRecreateError {
    fn from(e: ListLocalSearchError) -> Self {
        Self::ListSearch(e)
    }
}
fn model_error(e: ListError) -> RuinRecreateError {
    ListLocalSearchError::from(ListScoringError::from(e)).into()
}
/// Seeded list-value removal and reconstruction, optionally interleaved with
/// weighted relocations/assignments/unassignments and swaps/replacements.
/// The following sampling rules apply to repairs. Full pins and pinned prefixes
/// cannot be sampled. Unassigned optional values with a legal receiver may also
/// be sampled; other missing values remain unassigned. Unsampled assigned values
/// retain their owners and relative order, but insertion can change their indices.
///
/// Acceptors track changed list entities, a descriptor of their final lists, and
/// value identities whose owner/index changed (including newly unassigned values).
/// Population-relative tabu uses the entity count. Native descriptor equality is
/// not Java ruin/recreate move equality. Shared limits and best retention follow
/// [`ruin_recreate_search_from`]; ValueQueue uses sequential list insertion.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new(vec![0,1,2], vec![ListEntity::new(vec![0,1,2])])?
///     .allowing_unassigned();
/// let mut n = ListConstraintNetwork::new(1,3);
/// n.add_constraint("one", vec![ListScope::Entity(0)], |v| {
///     let ListFact::Entity(list) = v[0] else { unreachable!() };
///     SimpleScore::new(-i64::from(list.len()!=1))
/// })?;
/// let r = ruin_recreate_list_search_from(&p,&n,p.initial_assignment(),
///     RuinRecreateConfig { max_attempts:1, ..Default::default() })?;
/// assert_eq!(r.best.unwrap().assignment.list(0).unwrap().len(),1);
/// # Ok::<(),Box<dyn std::error::Error>>(())
/// ```
pub fn ruin_recreate_list_search_from<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<S>,
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_list_search_from_observed(problem, network, initial, config, None, None)
}

/// Report the initial complete scored solution and each strict retained-best
/// improvement synchronously. No partial repairs, ties or accepted worsening
/// states are reported. Callbacks do not rescore; their time counts toward the
/// search deadline. Cancellation from a callback stops at the next boundary,
/// after retaining that solution. Panics propagate with caller inputs unchanged.
/// Validation failures and skipped initial scoring emit no notifications.
pub fn ruin_recreate_list_search_from_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<S>,
    mut observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_list_search_from_observed(
        problem,
        network,
        initial,
        config,
        Some(&mut observer),
        None,
    )
}
pub(crate) fn ruin_recreate_list_search_from_observed<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, ListAssignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    network
        .validate(problem, &initial)
        .map_err(ListLocalSearchError::from)?;
    run(problem, initial, config, observer, shared, |p, a, c| {
        greedy_list_construction_from(p, network, a, c).map_err(ListLocalSearchError::from)
    })
}
/// Stream counterpart with a fresh graph for initialization and each repair/fine score.
/// Factory work is included in the cooperative time budget. No factories run
/// before validation or when a zero score budget/pre-cancelled token skips scoring.
pub fn ruin_recreate_list_stream_search_from<
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<S>,
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_list_stream_search_from_observed(problem, graph, initial, config, None, None)
}

/// Report the initial complete scored solution and each strict retained-best
/// improvement synchronously. No partial repairs, ties or accepted worsening
/// states are reported. Callbacks do not rescore; their time counts toward the
/// search deadline. Cancellation from a callback stops at the next boundary,
/// after retaining that solution. Panics propagate with caller inputs unchanged.
/// Validation failures and skipped initial scoring emit no notifications.
pub fn ruin_recreate_list_stream_search_from_with_observer<
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<S>,
    mut observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    ruin_recreate_list_stream_search_from_observed(
        problem,
        graph,
        initial,
        config,
        Some(&mut observer),
        None,
    )
}
pub(crate) fn ruin_recreate_list_stream_search_from_observed<
    'g,
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    mut graph: impl FnMut() -> Result<(StreamGraph<'g, S>, ListStreamSources<V>), GraphError>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, ListAssignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    run(problem, initial, config, observer, shared, |p, a, mut c| {
        let start = c.max_time.map(|_| Instant::now());
        let (graph, sources) =
            graph().map_err(|e| ListLocalSearchError::from(ListStreamScoringError::from(e)))?;
        if let (Some(limit), Some(start)) = (c.max_time, start) {
            c.max_time = Some(limit.saturating_sub(start.elapsed()));
        }
        greedy_list_stream_construction_from(p, graph, sources, a, c)
            .map_err(ListLocalSearchError::from)
    })
}
fn run<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    initial: ListAssignment<V>,
    config: RuinRecreateConfig<S>,
    observer: crate::ruin_engine::RepairObserver<'_, ListAssignment<V>, S>,
    shared: Option<&mut dyn crate::time_limit::SearchControl>,
    mut rebuild: impl FnMut(
        &ListProblem<V>,
        ListAssignment<V>,
        ConstructionConfig,
    ) -> Result<ListConstructionResult<V, S>, ListLocalSearchError>,
) -> Result<ListRuinRecreateResult<V, S>, RuinRecreateError> {
    crate::ruin_engine::run(problem, initial, config, observer, shared, |p, a, c| {
        let r = rebuild(p, a, c)?;
        Ok(crate::ruin_engine::Repair {
            assignment: r.assignment,
            score: r.score,
            complete: r.complete,
            evaluations: r.evaluations,
            constraint_evaluations: r.constraint_evaluations,
        })
    })
    .map(|r| r.map(|(assignment, score)| ScoredListAssignment { assignment, score }))
}
impl<V: Eq + Clone> crate::ruin_engine::RepairModel for ListProblem<V> {
    type Assignment = ListAssignment<V>;
    type FineMove = ListMove<V>;
    type Movement = Vec<(usize, Vec<V>)>;
    type Value = V;
    fn validate_complete(&self, a: &ListAssignment<V>) -> Result<(), RuinRecreateError> {
        self.validate(a).map_err(model_error)?;
        if !self.is_complete(a) {
            return Err(RuinRecreateError::IncompleteInitialAssignment);
        }
        Ok(())
    }
    fn initial(&self) -> ListAssignment<V> {
        self.initial_assignment()
    }
    fn candidate_count(&self) -> usize {
        self.values().len()
    }
    fn population(&self) -> usize {
        self.entities().len()
    }
    fn movable(&self, a: &ListAssignment<V>) -> Vec<usize> {
        self.values()
            .iter()
            .enumerate()
            .filter_map(|(i, v)| {
                let eligible = match a.location(v) {
                    Some(pos) => {
                        !self.entities()[pos.entity].is_pinned()
                            && pos.index >= self.entities()[pos.entity].pinned_prefix()
                    }
                    None => self
                        .entities()
                        .iter()
                        .enumerate()
                        .any(|(e, m)| !m.is_pinned() && self.accepts(e, v)),
                };
                eligible.then_some(i)
            })
            .collect()
    }
    fn repair(&self, a: &ListAssignment<V>, selected: &[usize]) -> Result<Self, RuinRecreateError> {
        let selected: Vec<_> = selected.iter().map(|&i| &self.values()[i]).collect();
        let entities = self
            .entities()
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let kept = a.lists()[i]
                    .iter()
                    .filter(|v| !selected.contains(v))
                    .cloned()
                    .collect();
                // Exclude only unselected missing values; changing the registry would
                // invalidate value-scoped constraints and stream identity indices.
                let range = self
                    .values()
                    .iter()
                    .filter(|v| {
                        self.accepts(i, v) && (selected.contains(v) || a.location(v).is_some())
                    })
                    .cloned()
                    .collect();
                ListEntity::new(kept)
                    .with_pinned(m.is_pinned())
                    .with_pinned_prefix(m.pinned_prefix())
                    .with_value_range(range)
            })
            .collect();
        let mut repair = self.rebuild_with_entities(entities).map_err(model_error)?;
        // Repair retains registry identities, including missing values. Preserve
        // fixed edges so reconstruction filters cyclic predecessor orderings too.
        repair.dependencies = self.dependencies.clone();
        Ok(if self.allows_unassigned() {
            repair.allowing_unassigned()
        } else {
            repair
        })
    }
    fn validate_neighborhood(
        &self,
        neighborhood: RepairNeighborhood<'_>,
    ) -> Result<(), RuinRecreateError> {
        neighborhood
            .standard()?
            .validate_list(self)
            .map_err(model_error)
    }
    fn fine_moves<'a>(
        &'a self,
        a: &'a ListAssignment<V>,
        neighborhood: RepairNeighborhood<'a>,
    ) -> Result<crate::ruin_engine::FineMoves<'a, ListMove<V>>, RuinRecreateError> {
        let moves = list_moves(self, a, neighborhood.standard()?).map_err(model_error)?;
        if self.dependencies.is_none() {
            return Ok(crate::ruin_engine::FineMoves::All(Box::new(moves)));
        }
        let checker = self.move_checker(a).map_err(model_error)?;
        Ok(crate::ruin_engine::FineMoves::Checked(Box::new(moves.map(
            move |movement| {
                (!matches!(
                    checker.is_doable(&movement),
                    Err(ListError::DependencyCycle { .. })
                ))
                .then_some(movement)
            },
        ))))
    }
    fn apply_fine(
        &self,
        a: &ListAssignment<V>,
        m: &ListMove<V>,
    ) -> Result<ListAssignment<V>, RuinRecreateError> {
        let mut next = a.clone();
        self.apply(&mut next, m).map_err(model_error)?.commit();
        Ok(next)
    }
    fn metadata(
        &self,
        old: &ListAssignment<V>,
        new: &ListAssignment<V>,
    ) -> (Vec<usize>, Self::Movement, Vec<V>) {
        let changed: Vec<_> = old
            .lists()
            .iter()
            .zip(new.lists())
            .enumerate()
            .filter_map(|(i, (a, b))| (a != b).then_some(i))
            .collect();
        let movement = changed
            .iter()
            .map(|&i| (i, new.lists()[i].clone()))
            .collect();
        let values = self
            .values()
            .iter()
            .filter(|v| old.location(v) != new.location(v))
            .cloned()
            .collect();
        (changed, movement, values)
    }
}
