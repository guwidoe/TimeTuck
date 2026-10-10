//! Native exact phases over complete assignments. Stream candidates roll back to
//! the empty planning sources between scores; immutable graph facts remain intact.
use super::*;
use crate::time_limit::{TimeBudget, monotonic_clock};

type ExactObserver<'a, A, S> = Option<&'a mut dyn FnMut(SolutionUpdate<'_, A, S>)>;

pub(super) fn run_observed<A: Clone, S: AdditiveScore, E>(
    config: ExactPhaseConfig<'_, S>,
    prior: Option<S>,
    control: &mut SolveControl<'_, S>,
    enumerate: impl FnOnce(&mut dyn FnMut(&A) -> bool) -> bool,
    evaluate: impl FnMut(&A) -> Result<(S, u128), E>,
    observer: ExactObserver<'_, A, S>,
) -> Result<PhaseOutput<A, S>, E> {
    let clock = monotonic_clock(config.max_time.is_some());
    run_with_clock_observed(config, prior, control, enumerate, evaluate, clock, observer)
}

pub(super) fn visit_basic_strategy<V: Eq + Clone>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    completion: bool,
    visit: impl FnMut(&Assignment<V>) -> bool,
) -> bool {
    if completion {
        crate::search::visit_basic_from(p, a.clone(), true, visit)
    } else {
        crate::search::visit_assignments(p, visit)
    }
}
pub(super) fn visit_list_strategy<V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
    completion: bool,
    visit: impl FnMut(&ListAssignment<V>) -> bool,
) -> bool {
    if completion {
        crate::list_search::visit_list_from(p, a.clone(), visit)
    } else {
        crate::list_search::visit_list_assignments(p, visit)
    }
}
pub(super) fn visit_mixed_strategy<B: Eq + Clone, L: Eq + Clone>(
    p: &MixedProblem<B, L>,
    a: &MixedAssignment<B, L>,
    completion: bool,
    visit: impl FnMut(&MixedAssignment<B, L>) -> bool,
) -> bool {
    if completion {
        crate::visit_mixed_completions(p, a, visit).expect("validated phase assignment")
    } else {
        visit_mixed(p, visit)
    }
}

#[cfg(test)]
pub(super) fn run_with_clock<A: Clone, S: AdditiveScore, E>(
    config: ExactPhaseConfig<'_, S>,
    prior: Option<S>,
    control: &mut SolveControl<'_, S>,
    enumerate: impl FnOnce(&mut dyn FnMut(&A) -> bool) -> bool,
    evaluate: impl FnMut(&A) -> Result<(S, u128), E>,
    clock: impl FnMut() -> Duration,
) -> Result<PhaseOutput<A, S>, E> {
    run_with_clock_observed(config, prior, control, enumerate, evaluate, clock, None)
}

pub(super) fn run_with_clock_observed<A: Clone, S: AdditiveScore, E>(
    config: ExactPhaseConfig<'_, S>,
    prior: Option<S>,
    control: &mut SolveControl<'_, S>,
    enumerate: impl FnOnce(&mut dyn FnMut(&A) -> bool) -> bool,
    mut evaluate: impl FnMut(&A) -> Result<(S, u128), E>,
    clock: impl FnMut() -> Duration,
    mut observer: ExactObserver<'_, A, S>,
) -> Result<PhaseOutput<A, S>, E> {
    let move_counter = control.move_counter.clone();
    use ExactPhaseTermination as X;
    let mut timer = TimeBudget::new(config.max_time, None, clock).with_shared(control.monitor());
    let exhausted_reason = if matches!(config.strategy, ExactSearchStrategy::Complete) {
        X::CompletionExhausted
    } else {
        X::Exhausted
    };
    let mut out = PhaseOutput {
        assignment: None,
        complete: prior.is_some(),
        score: prior,
        work: SolverWork::default(),
        termination: PhaseTermination::Exact(exhausted_reason),
    };
    timer.start_search();
    let mut error = None;
    let exhausted = enumerate(&mut |a| {
        let reason = if out.work.evaluations >= config.max_evaluations {
            Some(X::EvaluationLimit)
        } else if config
            .max_move_evaluations
            .is_some_and(|n| out.work.move_evaluations >= n)
        {
            Some(X::MoveEvaluationLimit)
        } else if config
            .cancellation
            .is_some_and(CancellationToken::is_cancelled)
        {
            Some(X::Cancelled)
        } else if let Some(reason) = timer.stop() {
            Some(match reason {
                LocalSearchTermination::TimeLimit => X::TimeLimit,
                LocalSearchTermination::UnimprovedTimeLimit => X::UnimprovedTimeLimit,
                LocalSearchTermination::UnimprovedStepLimit => X::UnimprovedStepLimit,
                LocalSearchTermination::StepCondition => X::Condition,
                _ => unreachable!("timer only reports elapsed/stagnation limits"),
            })
        } else if out.score.as_ref().is_some_and(|s| {
            config
                .best_score_limit
                .as_ref()
                .is_some_and(|target| s >= target)
        }) {
            Some(X::BestScoreLimit)
        } else if config.stop_when_feasible && out.score.as_ref().is_some_and(Score::is_feasible) {
            Some(X::FeasibleScore)
        } else {
            None
        };
        if let Some(reason) = reason {
            out.termination = PhaseTermination::Exact(reason);
            return false;
        }
        let (score, callbacks) = match evaluate(a) {
            Ok(value) => value,
            Err(e) => {
                error = Some(e);
                return false;
            }
        };
        out.work.evaluations += 1;
        out.work.move_evaluations += 1;
        if let Some(counter) = &move_counter {
            counter.add(crate::MoveEvaluationKind::ExactAssignment, 1);
        }
        out.work.constraint_evaluations += callbacks;
        if score.structural() >= 0 && out.score.as_ref().is_none_or(|best| score > *best) {
            timer.record_improvement();
            out.assignment = Some(a.clone());
            out.score = Some(score);
            out.complete = true;
            if let Some(observer) = &mut observer {
                observer(SolutionUpdate {
                    assignment: out.assignment.as_ref().unwrap(),
                    score: out.score.as_ref().unwrap().clone(),
                    complete: true,
                });
            }
        }
        true
    });
    if let Some(error) = error {
        return Err(error);
    }
    // Checking controls before the next candidate allows final exhaustion to win
    // over a budget/goal/cancellation reached by the final scoring callback.
    if exhausted {
        out.termination = PhaseTermination::Exact(exhausted_reason);
    }
    Ok(out)
}

pub(super) fn visit_mixed<B: Eq + Clone, L: Eq + Clone>(
    p: &MixedProblem<B, L>,
    mut visit: impl FnMut(&MixedAssignment<B, L>) -> bool,
) -> bool {
    if p.basic().search_space_size() == Some(0) {
        return true;
    }
    crate::list_search::visit_list_assignments(p.lists(), |lists| {
        crate::search::visit_assignments(p.basic(), |basic| {
            visit(&MixedAssignment::new(basic.clone(), lists.clone()))
        })
    })
}
fn put_basic<V: Clone + 'static, S: AdditiveScore + 'static>(
    tx: &mut GraphTransaction<'_, '_, S>,
    source: &PlanningStreamSource<V>,
    a: &Assignment<V>,
) -> Result<(), GraphError> {
    for (i, value) in a.values().iter().enumerate() {
        tx.apply(source, TupleChange::put(i, (i, value.clone())))?;
    }
    Ok(())
}
fn put_lists<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    tx: &mut GraphTransaction<'_, '_, S>,
    sources: &ListStreamSources<V>,
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
) -> Result<(), GraphError> {
    for (i, list) in a.lists().iter().enumerate() {
        tx.apply(&sources.entities, TupleChange::put(i, (i, list.clone())))?;
    }
    let locations = p.value_locations(a);
    for (i, value) in p.values().iter().enumerate() {
        tx.apply(
            &sources.values,
            TupleChange::put(
                i,
                ListValueRow::from_known_position(i, value, a, locations[i]),
            ),
        )?;
    }
    Ok(())
}
pub(super) fn score_basic<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &FiniteProblem<V>,
    g: &mut StreamGraph<'_, S>,
    s: &PlanningStreamSource<V>,
    a: &Assignment<V>,
) -> Result<(S, u128), StreamScoringError> {
    let mut tx = g.transaction();
    put_basic(&mut tx, s, a)?;
    let score = crate::scoring::with_structure(tx.score()?, p, a);
    Ok((score, tx.score_evaluations()))
}
pub(super) fn score_list<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    p: &ListProblem<V>,
    g: &mut StreamGraph<'_, S>,
    s: &ListStreamSources<V>,
    a: &ListAssignment<V>,
) -> Result<(S, u128), ListStreamScoringError> {
    let mut tx = g.transaction();
    put_lists(&mut tx, s, p, a)?;
    let score = crate::list_scoring::with_structure(tx.score()?, p, a);
    Ok((score, tx.score_evaluations()))
}
pub(super) fn score_mixed<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    p: &MixedProblem<B, L>,
    g: &mut StreamGraph<'_, S>,
    s: &MixedStreamSources<B, L>,
    a: &MixedAssignment<B, L>,
) -> Result<(S, u128), MixedStreamScoringError> {
    let mut tx = g.transaction();
    put_basic(&mut tx, &s.basic, a.basic())?;
    put_lists(&mut tx, &s.lists, p.lists(), a.lists())?;
    let score = crate::mixed_scoring::with_structure(tx.score()?, p, a)?;
    Ok((score, tx.score_evaluations()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn row_score(row: &ListValueRow<usize>) -> SimpleScore {
        SimpleScore::new(
            row.position
                .map_or(0, |p| ((p.entity + 1) * 10 + p.index) as i64)
                + row.previous.unwrap_or(0) as i64
                - row.next.unwrap_or(0) as i64,
        )
    }
    #[test]
    fn exact_list_and_mixed_scoring_restore_planning_rows_and_keep_immutable_facts() {
        let lists = ListProblem::new(
            vec![7, 8, 9],
            vec![
                ListEntity::new(vec![7]).with_pinned_prefix(1),
                ListEntity::new(vec![8]).with_value_range(vec![8, 9]),
            ],
        )
        .unwrap()
        .allowing_unassigned();
        let p = MixedProblem::new(
            FiniteProblem::new(vec![Variable::new(vec![0, 1]).allowing_unassigned()]).unwrap(),
            lists,
        );
        let mut b = StreamGraphBuilder::new();
        let s = MixedStreamSources::<usize, usize>::new(&mut b);
        let immutable = b.source::<usize, i64>();
        b.score("fixed", immutable.port(), |v| SimpleScore::new(*v))
            .unwrap();
        b.score("basic", s.basic.port(), |(_, v)| {
            SimpleScore::new(v.unwrap_or(3) as i64)
        })
        .unwrap();
        b.score("values", s.lists.values.port(), row_score).unwrap();
        b.score("lists", s.lists.entities.port(), |(_, v)| {
            SimpleScore::new(v.len() as i64)
        })
        .unwrap();
        let mut g = b.build();
        {
            let mut tx = g.transaction();
            tx.apply(&immutable, TupleChange::put(0, 100)).unwrap();
            tx.commit().unwrap();
        }
        let mut visited = 0;
        assert!(visit_mixed(&p, |a| {
            p.validate(a).unwrap();
            assert!(p.is_complete(a));
            let expected = 100
                + a.basic().values()[0].unwrap_or(3) as i64
                + a.lists()
                    .lists()
                    .iter()
                    .map(|v| v.len() as i64)
                    .sum::<i64>()
                + p.lists()
                    .values()
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        row_score(&ListValueRow::from_assignment(i, v, a.lists())).score()
                    })
                    .sum::<i64>();
            let (score, calls) = score_mixed(&p, &mut g, &s, a).unwrap();
            assert_eq!(score, SimpleScore::new(expected));
            assert_eq!(calls, 6);
            assert!(g.source_rows(&s.basic).unwrap().is_empty());
            assert!(g.source_rows(&s.lists.entities).unwrap().is_empty());
            assert!(g.source_rows(&s.lists.values).unwrap().is_empty());
            assert_eq!(g.source_rows(&immutable).unwrap().len(), 1);
            assert_eq!(g.score(), SimpleScore::new(100));
            let (score, calls) = score_list(p.lists(), &mut g, &s.lists, a.lists()).unwrap();
            assert_eq!(
                score,
                SimpleScore::new(expected - a.basic().values()[0].unwrap_or(3) as i64)
            );
            assert_eq!(calls, 5);
            g.assert_consistent().unwrap();
            visited += 1;
            true
        }));
        assert!(visited > 20);
    }
    #[test]
    fn exact_stream_candidate_errors_rollback_before_later_candidates() {
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2])]).unwrap();
        let mut b = StreamGraphBuilder::new();
        let s = b.source::<usize, (usize, Option<i64>)>();
        b.score("invalid", s.port(), |(_, v)| {
            SimpleScore::new(v.unwrap()).with_structural(if *v == Some(1) { -1 } else { 0 })
        })
        .unwrap();
        let mut g = b.build();
        assert!(crate::search::visit_assignments(&p, |a| {
            let result = score_basic(&p, &mut g, &s, a);
            if a.values()[0] == Some(1) {
                assert!(matches!(
                    result,
                    Err(StreamScoringError::Graph(
                        GraphError::StructuralContribution(_)
                    ))
                ))
            } else {
                assert_eq!(result.unwrap().0, SimpleScore::new(a.values()[0].unwrap()));
            }
            assert!(g.source_rows(&s).unwrap().is_empty());
            assert_eq!(g.score(), SimpleScore::ZERO);
            g.assert_consistent().unwrap();
            true
        }));
    }
}
