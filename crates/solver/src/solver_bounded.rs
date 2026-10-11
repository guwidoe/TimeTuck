//! Ordered bounded phases, using the same tree engine as standalone exact search.
use super::*;
use crate::branch_and_bound::{BranchContext, Retained, TreeEvaluation, TreeSearchError, run_tree};
use crate::branch_tree::BranchTree;
use crate::time_limit::TimeBudget;

type Observer<'a, A, S> = Option<&'a mut dyn FnMut(SolutionUpdate<'_, A, S>)>;
struct Saved<A, S> {
    assignment: A,
    score: S,
}
impl<A, S> Retained<A, S> for Saved<A, S> {
    fn new(assignment: A, score: S) -> Self {
        Self { assignment, score }
    }
    fn score(&self) -> &S {
        &self.score
    }
}
struct Context<'a, 'm, 'o, A, S, F, C> {
    config: ExactPhaseConfig<'a, S>,
    timer: TimeBudget<'m, C, S>,
    evaluate: F,
    observer: Observer<'o, A, S>,
    constraint_evaluations: u128,
    completed_steps: u64,
    expansion_improved: bool,
    move_counter: Option<crate::move_metrics::MoveCounter>,
    score_counter: Option<crate::score_evaluations::ScoreCounter>,
    bound: PartialScoreBound,
}
impl<A, S: AdditiveScore, E, F, C> BranchContext<A, S, Saved<A, S>>
    for Context<'_, '_, '_, A, S, F, C>
where
    F: FnMut(&A) -> Result<(S, u128), E>,
    C: FnMut() -> Duration,
{
    type Error = E;
    fn evaluate(&mut self, a: &A, _: usize) -> Result<TreeEvaluation<S>, E> {
        let (score, callbacks) = (self.evaluate)(a)?;
        crate::score_evaluations::record(&self.score_counter);
        self.constraint_evaluations += callbacks;
        if let Some(counter) = &self.move_counter {
            counter.add(crate::MoveEvaluationKind::ExactNode, 1);
        }
        let optimistic_bound = match &self.bound {
            PartialScoreBound::Unknown => None,
            PartialScoreBound::NonIncreasing => Some(InitializingScoreBound::finite(
                score.clone().with_structural(0),
            )),
            PartialScoreBound::Trend(trend) => Some(
                trend
                    .optimistic_bound(score.clone())
                    .expect("validated constant score-level count"),
            ),
        };
        Ok(TreeEvaluation {
            score,
            optimistic_bound,
        })
    }
    fn stop(&mut self, evaluations: u64, best: Option<&S>) -> Option<BranchTermination> {
        use BranchTermination as B;
        let c = &self.config;
        if evaluations >= c.max_evaluations {
            Some(B::EvaluationLimit)
        } else if c.max_move_evaluations.is_some_and(|n| evaluations >= n) {
            Some(B::MoveEvaluationLimit)
        } else if c.cancellation.is_some_and(CancellationToken::is_cancelled) {
            Some(B::Cancelled)
        } else if let Some(reason) = self.timer.stop() {
            Some(match reason {
                LocalSearchTermination::TimeLimit => B::TimeLimit,
                LocalSearchTermination::UnimprovedTimeLimit => B::UnimprovedTimeLimit,
                LocalSearchTermination::SignificantImprovementTimeLimit => {
                    B::SignificantImprovementTimeLimit
                }
                LocalSearchTermination::UnimprovedStepLimit => B::UnimprovedStepLimit,
                LocalSearchTermination::StepCondition => B::Condition,
                _ => unreachable!("timer only reports elapsed/stagnation limits"),
            })
        } else if best.is_some_and(|s| {
            c.best_score_limit
                .as_ref()
                .is_some_and(|target| s >= target)
        }) {
            Some(B::BestScoreLimit)
        } else if c.stop_when_feasible && best.is_some_and(Score::is_feasible) {
            Some(B::FeasibleScore)
        } else {
            None
        }
    }
    fn expansion_started(&mut self) {
        self.expansion_improved = false;
        self.timer.start_step();
    }
    fn expansion_ended(&mut self, parent_score: &S, completed: bool) {
        if completed {
            self.completed_steps += 1;
            self.timer.complete_step(self.expansion_improved);
        }
        // A step describes expansion of this partial parent, not the last
        // generated sibling or a new score callback. Leaves are never expanded.
        self.timer
            .end_step(completed, || parent_score.clone(), || false);
    }
    fn improved(&mut self, best: &Saved<A, S>) {
        self.expansion_improved = true;
        self.timer.record_improvement();
        self.timer.observe_best_score(&best.score);
        if let Some(observer) = &mut self.observer {
            observer(SolutionUpdate {
                assignment: &best.assignment,
                score: best.score.clone(),
                complete: true,
            });
        }
    }
}
impl From<BranchTermination> for ExactPhaseTermination {
    fn from(reason: BranchTermination) -> Self {
        use BranchTermination as B;
        match reason {
            B::Exhausted => Self::Exhausted,
            B::EvaluationLimit => Self::EvaluationLimit,
            B::MoveEvaluationLimit => Self::MoveEvaluationLimit,
            B::FrontierLimit => Self::FrontierLimit,
            B::Cancelled => Self::Cancelled,
            B::TimeLimit => Self::TimeLimit,
            B::UnimprovedTimeLimit => Self::UnimprovedTimeLimit,
            B::SignificantImprovementTimeLimit => Self::SignificantImprovementTimeLimit,
            B::UnimprovedStepLimit => Self::UnimprovedStepLimit,
            B::Condition => Self::Condition,
            B::BestScoreLimit => Self::BestScoreLimit,
            B::FeasibleScore => Self::FeasibleScore,
        }
    }
}
pub(super) fn error<E, F: From<E>, S: fmt::Debug>(
    phase: usize,
    error: TreeSearchError<E, S>,
) -> SolverError<F> {
    match error {
        TreeSearchError::Evaluation(error) => SolverError::Phase {
            phase,
            error: error.into(),
        },
        TreeSearchError::Trend(error) => SolverError::InvalidScoreTrend { phase, error },
        TreeSearchError::Bound { score, bound } => SolverError::InvalidOptimisticBound {
            phase,
            score: format!("{:?}", score),
            bound: format!("{:?}", bound),
        },
    }
}
pub(super) fn run<T: BranchTree, S: AdditiveScore, E>(
    config: ExactPhaseConfig<'_, S>,
    prior: Option<S>,
    control: &mut SolveControl<'_, S>,
    prepare: impl FnOnce() -> (T, Option<T::Assignment>),
    evaluate: impl FnMut(&T::Assignment) -> Result<(S, u128), E>,
    observer: Observer<'_, T::Assignment, S>,
) -> Result<PhaseOutput<T::Assignment, S>, TreeSearchError<E, S>> {
    let clock = crate::solver_clock::elapsed_clock(
        config.max_time.is_some(),
        control.clock_session.as_ref(),
    );
    run_with_clock(config, prior, control, prepare, evaluate, observer, clock)
}
pub(super) fn run_with_clock<T: BranchTree, S: AdditiveScore, E>(
    config: ExactPhaseConfig<'_, S>,
    prior: Option<S>,
    control: &mut SolveControl<'_, S>,
    prepare: impl FnOnce() -> (T, Option<T::Assignment>),
    evaluate: impl FnMut(&T::Assignment) -> Result<(S, u128), E>,
    observer: Observer<'_, T::Assignment, S>,
    clock: impl FnMut() -> Duration,
) -> Result<PhaseOutput<T::Assignment, S>, TreeSearchError<E, S>> {
    let completion = config.strategy.is_completion();
    let (ExactSearchStrategy::BranchAndBound(branch)
    | ExactSearchStrategy::CompleteBranchAndBound(branch)) = config.strategy.clone()
    else {
        unreachable!("bounded dispatch")
    };
    if let PartialScoreBound::Trend(trend) = &branch.bound {
        trend.validate(&S::ZERO).map_err(TreeSearchError::Trend)?;
    }
    let move_counter = control.move_counter.clone();
    let score_counter = control.score_counter.clone();
    let timer = TimeBudget::new(config.max_time, None, clock).with_shared(control.monitor());
    let mut context = Context {
        config,
        timer,
        evaluate,
        observer,
        constraint_evaluations: 0,
        completed_steps: 0,
        expansion_improved: false,
        move_counter,
        score_counter,
        bound: branch.bound,
    };
    context.timer.start_search();
    if let Some(score) = &prior {
        context.timer.observe_best_score(score);
    }
    let (tree, root) = prepare();
    let result = run_tree(
        &tree,
        root,
        branch.exploration,
        branch.max_frontier,
        prior.clone(),
        &mut context,
    )?;
    let (assignment, score) = match result.best {
        Some(Saved { assignment, score }) => (Some(assignment), Some(score)),
        None => (None, prior),
    };
    Ok(PhaseOutput {
        assignment,
        complete: score.is_some(),
        score,
        work: SolverWork {
            evaluations: result.evaluations,
            move_evaluations: result.evaluations,
            constraint_evaluations: context.constraint_evaluations,
            completed_steps: context.completed_steps,
            ..Default::default()
        },
        termination: PhaseTermination::Exact(
            if completion && result.termination == BranchTermination::Exhausted {
                ExactPhaseTermination::CompletionExhausted
            } else {
                result.termination.into()
            },
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_limit::SearchControl;
    use std::{cell::Cell, convert::Infallible};
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }
    fn control(clock: &mut dyn FnMut() -> Duration) -> SolveControl<'_> {
        SolveControl {
            move_counter: None,
            score_counter: None,
            step_metrics: None,
            metric_phase: None,
            metric_step: 0,
            lifecycle: Default::default(),
            condition: None,
            scored_condition: None,
            stateful_condition: None,
            condition_needs_best: false,
            condition_best: None,
            condition_expired: false,
            condition_uses_moves: false,
            limit: None,
            clock,
            clock_session: None,
            acceptance_clock: None,
            phase_condition: None,
            last: Duration::ZERO,
            shared_bound: false,
            significant: None,
            unimproved_limit: None,
            last_improvement: None,
            unimproved_expired: false,
            unimproved_steps_limit: None,
            steps: StepProgress::default(),
            construction_steps: 0,
            started: false,
            steps_expired: false,
        }
    }
    fn config() -> ExactPhaseConfig<'static> {
        ExactPhaseConfig {
            strategy: ExactSearchStrategy::BranchAndBound(BoundedPhaseConfig::default()),
            ..Default::default()
        }
    }
    #[test]
    fn elapsed_budget_retains_finished_callbacks_and_final_exhaustion_wins() {
        let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2, 3])]).unwrap();
        for cap in 0..=4 {
            let elapsed = Cell::new(0);
            let mut shared_clock = || ms(elapsed.get());
            let mut shared = control(&mut shared_clock);
            let mut c = config();
            c.max_time = Some(ms(cap));
            let r = run_with_clock(
                c,
                None,
                &mut shared,
                || crate::branch_tree::BasicTree::new(&p),
                |a| {
                    elapsed.set(elapsed.get() + 1);
                    Ok::<_, Infallible>((SimpleScore::new(a.values()[0].unwrap_or(0)), 1))
                },
                None,
                || ms(elapsed.get()),
            )
            .unwrap();
            assert_eq!(r.work.evaluations, cap);
            assert_eq!(
                r.score,
                if cap < 2 {
                    None
                } else {
                    Some(SimpleScore::new(cap as i64 - 1))
                }
            );
            assert_eq!(
                r.termination,
                PhaseTermination::Exact(if cap == 4 {
                    ExactPhaseTermination::Exhausted
                } else {
                    ExactPhaseTermination::TimeLimit
                })
            );
        }
    }
    #[test]
    fn shared_stagnation_keeps_late_improvements_and_cannot_be_revived() {
        let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2, 3])]).unwrap();
        let elapsed = Cell::new(0);
        let mut clock = || ms(elapsed.get());
        let mut shared = control(&mut clock);
        shared.unimproved_limit = Some(ms(2));
        let r = run_with_clock(
            config(),
            None,
            &mut shared,
            || crate::branch_tree::BasicTree::new(&p),
            |a| {
                elapsed.set(elapsed.get() + 1);
                Ok::<_, Infallible>((SimpleScore::new(a.values()[0].unwrap_or(0)), 1))
            },
            None,
            || panic!("no phase timer"),
        )
        .unwrap();
        assert_eq!(r.work.evaluations, 2);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(
            r.termination,
            PhaseTermination::Exact(ExactPhaseTermination::UnimprovedTimeLimit)
        );
        assert!(shared.unimproved_expired);
    }
    #[test]
    fn improvements_reset_shared_step_stagnation_without_creating_steps() {
        let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2, 3])]).unwrap();
        let mut clock = || panic!("step-only solve never reads a clock");
        let mut shared = control(&mut clock);
        shared.unimproved_steps_limit = Some(3);
        shared.start_search();
        shared.complete_step(false);
        shared.complete_step(false);
        let mut c = config();
        c.max_evaluations = 2;
        let r = run_with_clock(
            c,
            Some(SimpleScore::ZERO),
            &mut shared,
            || crate::branch_tree::BasicTree::new(&p),
            |a| Ok::<_, Infallible>((SimpleScore::new(a.values()[0].unwrap_or(0)), 1)),
            None,
            || panic!("no phase timer"),
        )
        .unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(r.work.completed_steps, 0);
        assert_eq!(shared.steps.completed_steps(), 2);
        assert_eq!(shared.steps.unimproved_steps(), 0);
        shared.complete_step(false);
        shared.complete_step(false);
        shared.complete_step(false);
        let r = run_with_clock(
            config(),
            None,
            &mut shared,
            || crate::branch_tree::BasicTree::new(&p),
            |_| -> Result<(SimpleScore, u128), Infallible> { panic!("expired shared steps") },
            None,
            || panic!("no phase timer"),
        )
        .unwrap();
        assert_eq!(r.work.evaluations, 0);
        assert_eq!(
            r.termination,
            PhaseTermination::Exact(ExactPhaseTermination::UnimprovedStepLimit)
        );
    }
    #[test]
    fn model_preparation_and_observer_work_belong_to_elapsed_budget() {
        let p = FiniteProblem::new(vec![Variable::new(vec![1_i64, 2])]).unwrap();
        let elapsed = Cell::new(0);
        let mut clock = || ms(elapsed.get());
        let mut shared = control(&mut clock);
        let mut c = config();
        c.max_time = Some(ms(3));
        let r = run_with_clock(
            c.clone(),
            None,
            &mut shared,
            || {
                elapsed.set(3);
                crate::branch_tree::BasicTree::new(&p)
            },
            |_| -> Result<(SimpleScore, u128), Infallible> { panic!("preparation exhausted time") },
            None,
            || ms(elapsed.get()),
        )
        .unwrap();
        assert_eq!(r.work.evaluations, 0);
        elapsed.set(0);
        let r = run_with_clock(
            c,
            None,
            &mut shared,
            || crate::branch_tree::BasicTree::new(&p),
            |a| Ok::<_, Infallible>((SimpleScore::new(a.values()[0].unwrap_or(0)), 1)),
            Some(&mut |_| {
                elapsed.set(3);
            }),
            || ms(elapsed.get()),
        )
        .unwrap();
        assert_eq!(r.work.evaluations, 2);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(
            r.termination,
            PhaseTermination::Exact(ExactPhaseTermination::TimeLimit)
        );
    }
}
