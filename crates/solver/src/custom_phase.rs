//! Bounded user-defined optimization using the existing transactional score caches.
use crate::*;
use std::{error::Error, fmt, time::Duration};

/// Cooperative controls for a custom command. Initial scoring consumes one
/// evaluation. A command must return or poll the context to observe termination;
/// user code and scoring callbacks cannot be preempted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CustomPhaseConfig<'a> {
    pub max_evaluations: u64,
    pub max_move_evaluations: Option<u64>,
    pub max_time: Option<Duration>,
    pub cancellation: Option<&'a CancellationToken>,
}
impl Default for CustomPhaseConfig<'_> {
    fn default() -> Self {
        Self {
            max_evaluations: 10_000,
            max_move_evaluations: None,
            max_time: None,
            cancellation: None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustomPhaseTermination {
    Completed,
    EvaluationLimit,
    MoveEvaluationLimit,
    TimeLimit,
    Cancelled,
    UnimprovedTimeLimit,
    SignificantImprovementTimeLimit,
    UnimprovedStepLimit,
    Condition,
    BestScoreLimit,
    FeasibleScore,
}
#[derive(Debug)]
pub enum CustomPhaseError<E> {
    Scoring(E),
    Command(String),
    /// The command caught an operation error and returned success. A failed
    /// scoring operation cannot establish a valid result for this invocation.
    IgnoredOperationError,
}
impl<E: fmt::Display> fmt::Display for CustomPhaseError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scoring(e) => e.fmt(f),
            Self::Command(e) => f.write_str(e),
            Self::IgnoredOperationError => {
                f.write_str("custom command ignored a failed scoring operation")
            }
        }
    }
}
impl<E: Error + 'static> Error for CustomPhaseError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scoring(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CustomPhaseWork {
    /// Initial scoring plus attempted candidate evaluations, including unscored
    /// dependency-flaw inspections. Constraint work counts only actual callbacks.
    pub evaluations: u64,
    pub move_evaluations: u64,
    pub constraint_evaluations: u128,
    pub committed_moves: u64,
}
/// Retained best committed state. A temporary trial never becomes the best.
/// Complete is independent of feasibility; no custom command proves optimality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomPhaseResult<A, S> {
    pub assignment: A,
    pub score: Option<S>,
    pub complete: bool,
    pub work: CustomPhaseWork,
    pub termination: CustomPhaseTermination,
}

/// One scored conditional trial. Rejection leaves the working assignment and
/// score unchanged; `score` still reports the rejected candidate's score.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomTrialOutcome<S> {
    pub score: S,
    pub committed: bool,
}

pub(crate) trait CommandDirector<A, M, S> {
    fn notify_best(&mut self, _complete: bool) {}
    type Error;
    fn assignment(&self) -> &A;
    fn score(&self) -> S;
    fn initial_evaluations(&self) -> u128;
    /// Some backends can inspect dependency flaws without installing/scoring.
    /// Basic-only models have no dependency graph and use the ordinary trial.
    fn preview(&self, _movement: &M) -> Result<Option<MoveInspection<A>>, Self::Error> {
        Ok(None)
    }
    fn assert_consistent(&mut self) -> Result<(), Self::Error>;
    /// The visitor returns whether to commit. An unwind drops the move guard.
    fn trial(
        &mut self,
        movement: &M,
        visit: &mut dyn FnMut(&A, S, u128) -> bool,
    ) -> Result<(), Self::Error>;
}
macro_rules! director {
    ([$($generics:tt)*] $director:ty, $assignment:ty, $movement:ty, $error:ty, $search:ident $(, $inspect:ident)?) => {
        impl<$($generics)*> CommandDirector<$assignment,$movement,S> for $director {
            type Error=$error;
            $(fn preview(&self,movement:&$movement)->Result<Option<MoveInspection<$assignment>>,Self::Error> {
                self.$inspect(movement).map(Some)
            })?
            fn assignment(&self)->&$assignment {self.assignment()}
            fn score(&self)->S {self.score()}
            fn initial_evaluations(&self)->u128 {$search::initial_constraint_evaluations(self)}
            fn assert_consistent(&mut self)->Result<(),Self::Error>{<$director>::assert_consistent(self)}
            fn trial(&mut self,movement:&$movement,visit:&mut dyn FnMut(&$assignment,S,u128)->bool)->Result<(),Self::Error>{
                let candidate=self.apply(movement)?;
                if visit(candidate.assignment(),candidate.score(),candidate.constraint_evaluations() as u128) {
                    candidate.commit();
                }
                Ok(())
            }
        }
    }
}
use crate::{
    list_search_scoring::ListSearchDirector, mixed_search_scoring::MixedSearchDirector,
    search_scoring::SearchDirector,
};
director!(['p,'c,V:Eq+Clone,S:AdditiveScore] ScoreDirector<'p,'c,V,S>, Assignment<V>, Move<V>, ScoringError, SearchDirector);
director!(['p,'c,V:Eq+Clone+'static,S:AdditiveScore+'static] StreamScoreDirector<'p,'c,V,S>, Assignment<V>, Move<V>, StreamScoringError, SearchDirector);
director!(['p,'c,V:Eq+Clone,S:AdditiveScore] ListScoreDirector<'p,'c,V,S>, ListAssignment<V>, ListMove<V>, ListScoringError, ListSearchDirector, inspect_move);
director!(['p,'c,V:Eq+Clone+'static,S:AdditiveScore+'static] ListStreamScoreDirector<'p,'c,V,S>, ListAssignment<V>, ListMove<V>, ListStreamScoringError, ListSearchDirector, inspect_move);
director!(['p,'c,B:Eq+Clone,L:Eq+Clone,S:AdditiveScore] MixedScoreDirector<'p,'c,B,L,S>, MixedAssignment<B,L>, MixedMove<B,L>, MixedScoringError, MixedSearchDirector, inspect_move);
director!(['p,'c,B:Eq+Clone+'static,L:Eq+Clone+'static,S:AdditiveScore+'static] MixedStreamScoreDirector<'p,'c,B,L,S>, MixedAssignment<B,L>, MixedMove<B,L>, MixedStreamScoringError, MixedSearchDirector, inspect_move);

/// Execute a native command against a bounded transactional scoring session.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)])?;
/// let mut n = ConstraintNetwork::new(1);
/// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap()))?;
/// let r = custom_phase(&p, &n, p.initial_assignment(), CustomPhaseConfig::default(), |c| {
///     let trial = c.evaluate(&Move::Change { variable: 0, value: Some(2) }, |_, s| s)?;
///     assert_eq!(trial, Some(SimpleScore::new(2)));
///     assert_eq!(c.score(), SimpleScore::ZERO); // Temporary move was undone.
///     c.execute(&Move::Change { variable: 0, value: Some(1) })?;
///     c.execute(&Move::Change { variable: 0, value: Some(0) })?;
///     Ok(())
/// })?;
/// assert_eq!(r.assignment.values(), &[Some(1)]); // Retained committed best.
/// assert_eq!(r.work.evaluations, 4);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Borrowed command session. Read-only assignments protect the contribution cache;
/// execute/execute_if/evaluate change or inspect candidate state.
/// Execute and ordinary evaluate calculate a fresh score. Inspected evaluation can
/// instead report dependency flaws without scoring. Partial assignments are allowed.
/// Do not mutate captured model facts or constraint data during a command.
///
/// Execute can deliberately worsen the working score; only a strict improvement
/// (or completion of a partial best) replaces the retained best. Evaluate rolls
/// back on return or unwind and never publishes progress. Caught scoring errors
/// invalidate the phase, as do caught scoring panics; caught visitor panics roll
/// back and consume their work.
pub struct CustomPhaseContext<'a, A, M, S, E> {
    director: &'a mut dyn CommandDirector<A, M, S, Error = E>,
    config: CustomPhaseConfig<'a>,
    clock: &'a mut dyn FnMut() -> Duration,
    elapsed: Duration,
    complete: &'a dyn Fn(&A) -> bool,
    observer: &'a mut dyn FnMut(SolutionUpdate<'_, A, S>),
    best_assignment: A,
    best_score: S,
    best_complete: bool,
    work: CustomPhaseWork,
    stop: Option<CustomPhaseTermination>,
    failed: bool,
    shared: Option<&'a mut dyn crate::time_limit::SearchControl<S>>,
    move_counter: Option<crate::move_metrics::MoveCounter>,
    score_counter: Option<crate::score_evaluations::ScoreCounter>,
    target: Option<S>,
    stop_when_feasible: bool,
}
impl<A: Clone, M, S: Score, E> CustomPhaseContext<'_, A, M, S, E> {
    pub fn assignment(&self) -> &A {
        self.director.assignment()
    }
    pub fn score(&self) -> S {
        self.director.score()
    }
    pub fn best_assignment(&self) -> &A {
        &self.best_assignment
    }
    pub fn best_score(&self) -> S {
        self.best_score.clone()
    }
    pub fn work(&self) -> CustomPhaseWork {
        self.work
    }
    /// Latches termination; cancellation, time, score cap, then candidate cap.
    pub fn is_terminated(&mut self) -> bool {
        if self.stop.is_none() {
            if let Some(limit) = self.config.max_time {
                self.elapsed = self.elapsed.max((self.clock)());
                if self.elapsed >= limit {
                    self.stop = Some(CustomPhaseTermination::TimeLimit);
                }
            }
            if self
                .config
                .cancellation
                .is_some_and(CancellationToken::is_cancelled)
            {
                self.stop = Some(CustomPhaseTermination::Cancelled);
            } else if self.stop.is_none() && self.work.evaluations >= self.config.max_evaluations {
                self.stop = Some(CustomPhaseTermination::EvaluationLimit);
            } else if self.stop.is_none()
                && self
                    .config
                    .max_move_evaluations
                    .is_some_and(|n| self.work.move_evaluations >= n)
            {
                self.stop = Some(CustomPhaseTermination::MoveEvaluationLimit);
            }
        }
        if self.stop.is_none() {
            self.stop = self.shared.as_mut().and_then(|m| match m.stop() {
                Some(LocalSearchTermination::SignificantImprovementTimeLimit) => {
                    Some(CustomPhaseTermination::SignificantImprovementTimeLimit)
                }
                Some(LocalSearchTermination::UnimprovedTimeLimit) => {
                    Some(CustomPhaseTermination::UnimprovedTimeLimit)
                }
                Some(LocalSearchTermination::StepCondition) => {
                    Some(CustomPhaseTermination::Condition)
                }
                Some(LocalSearchTermination::UnimprovedStepLimit) => {
                    Some(CustomPhaseTermination::UnimprovedStepLimit)
                }
                _ => None,
            });
        }
        if self.stop.is_none() && self.best_complete {
            if self.target.as_ref().is_some_and(|s| self.best_score >= *s) {
                self.stop = Some(CustomPhaseTermination::BestScoreLimit);
            } else if self.stop_when_feasible && self.best_score.is_feasible() {
                self.stop = Some(CustomPhaseTermination::FeasibleScore);
            }
        }
        self.stop.is_some() || self.failed
    }
    pub fn termination(&mut self) -> Option<CustomPhaseTermination> {
        self.is_terminated();
        self.stop
    }
    fn begin(&mut self) -> bool {
        if self.is_terminated() {
            return false;
        }
        crate::score_evaluations::record(&self.score_counter);
        self.work.evaluations += 1;
        self.work.move_evaluations += 1;
        if let Some(counter) = &self.move_counter {
            counter.add(crate::MoveEvaluationKind::Custom, 1);
        }
        true
    }
    fn operation(&mut self, result: Result<(), E>) -> Result<(), CustomPhaseError<E>> {
        result.map_err(|e| {
            self.failed = true;
            CustomPhaseError::Scoring(e)
        })
    }
    /// Apply and commit one validated move. None means a limit prevented work.
    /// An error invalidates this phase even if the command catches that error.
    pub fn execute(&mut self, movement: &M) -> Result<Option<S>, CustomPhaseError<E>> {
        if !self.begin() {
            return Ok(None);
        }
        // A caught lifecycle/scoring/commit panic must not hide unknown work.
        self.failed = true;
        if let Some(shared) = &mut self.shared {
            shared.step_started();
        }
        let work = &mut self.work;
        let result = self.director.trial(movement, &mut |_, _, count| {
            work.constraint_evaluations += count;
            true
        });
        self.operation(result)?;
        self.failed = false;
        self.work.committed_moves += 1;
        let score = self.director.score();
        self.finish_committed(&score);
        Ok(Some(score))
    }
    fn finish_committed(&mut self, score: &S) {
        let complete = (self.complete)(self.director.assignment());
        if (complete && !self.best_complete) || score > &self.best_score {
            self.best_assignment = self.director.assignment().clone();
            self.best_score = score.clone();
            self.best_complete = complete;
            if let Some(shared) = &mut self.shared {
                shared.record_improvement();
                if complete {
                    shared.observe_complete_best_score(score);
                } else {
                    shared.observe_best_score(score);
                }
            }
            self.failed = true;
            self.director.notify_best(complete);
            (self.observer)(SolutionUpdate {
                assignment: &self.best_assignment,
                score: score.clone(),
                complete,
            });
            self.failed = false;
        }
        self.failed = true;
        if let Some(shared) = &mut self.shared {
            crate::time_limit::finish_step(*shared, true, || score.clone(), || complete);
        }
        self.failed = false;
    }
    /// Score one transactional candidate and let `decide` choose whether to keep
    /// it. The callback borrows the applied assignment and freshly calculated
    /// score. There is no rollback/replay or second scoring pass on acceptance.
    /// None means a cooperative limit prevented the trial and callback.
    ///
    /// Every attempted trial consumes one score/move evaluation and its actual
    /// constraint work. A rejected trial adds no committed step, best notification
    /// or retained-best update. Acceptance uses the same best retention and step
    /// accounting as execute, including accepted worsening/partial states. The
    /// accepted StepStarted event occurs after the decision, before commit; rejected
    /// trials emit no step events. Cancellation raised during scoring/decision is
    /// observed at the next cooperative boundary, as for execute.
    ///
    /// A decision panic rolls back; catching it leaves the command usable and
    /// retains known work, as for evaluate. Scoring/commit/lifecycle failures
    /// invalidate the phase even if caught. Failures before commit roll back;
    /// failures in best/step notifications occur after the move has committed.
    /// Captured model facts must not be mutated through shared interior state.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)])?;
    /// let mut n = ConstraintNetwork::new(1);
    /// n.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap()))?;
    /// let r = custom_phase(&p, &n, p.initial_assignment(), CustomPhaseConfig::default(), |c| {
    ///     let current = c.score();
    ///     let trial = c.execute_if(&Move::Change { variable: 0, value: Some(1) },
    ///         |_, candidate| candidate > &current)?.unwrap();
    ///     assert!(trial.committed);
    ///     assert_eq!(trial.score, SimpleScore::new(1));
    ///     Ok(())
    /// })?;
    /// assert_eq!(r.work.evaluations, 2); // Initialization plus one candidate.
    /// assert_eq!(r.work.committed_moves, 1);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn execute_if(
        &mut self,
        movement: &M,
        decide: impl FnOnce(&A, &S) -> bool,
    ) -> Result<Option<CustomTrialOutcome<S>>, CustomPhaseError<E>> {
        if !self.begin() {
            return Ok(None);
        }
        self.failed = true;
        let failed = &mut self.failed;
        let work = &mut self.work;
        let shared = &mut self.shared;
        let mut decide = Some(decide);
        let mut outcome = None;
        let result = self.director.trial(movement, &mut |a, score, count| {
            work.constraint_evaluations += count;
            *failed = false;
            let committed = decide.take().expect("one candidate")(a, &score);
            *failed = true;
            if committed && let Some(shared) = shared {
                shared.step_started();
            }
            outcome = Some(CustomTrialOutcome { score, committed });
            committed
        });
        self.operation(result)?;
        self.failed = false;
        let outcome = outcome.expect("successful trial visits candidate");
        if outcome.committed {
            self.work.committed_moves += 1;
            self.finish_committed(&outcome.score);
        }
        Ok(Some(outcome))
    }
    /// Run a visitor while a scored move is applied, then undo it. The visitor
    /// may inspect partial assignments. Its return value cannot borrow the trial.
    pub fn evaluate<R>(
        &mut self,
        movement: &M,
        visit: impl FnOnce(&A, S) -> R,
    ) -> Result<Option<R>, CustomPhaseError<E>> {
        if !self.begin() {
            return Ok(None);
        }
        self.evaluate_started(movement, visit)
    }
    /// Preview list dependency consistency before temporary scoring. A flawed
    /// candidate goes only to `flawed`, with its proposed assignment and affected
    /// value indices; it is never installed or scored. Sound candidates go to
    /// `valid` through the ordinary scored transaction. Basic models have no
    /// dependency flaws and always use that path, including partial assignments.
    ///
    /// Every attempted candidate consumes one evaluation and move slot. An
    /// unscored flawed preview adds zero constraint work. A stopped phase invokes
    /// neither handler. Domain/pin/index errors invalidate the phase as usual;
    /// caught handler panics leave it usable after rollback. Handler results
    /// cannot borrow the preview/trial. Use ordinary evaluate when no preview is
    /// needed; inspected sound candidates require an additional snapshot.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = ListProblem::new(vec![0, 1], vec![ListEntity::new(vec![0, 1])])?
    ///     .with_dependencies(vec![vec![], vec![0]])?;
    /// let n = ListConstraintNetwork::<_, SimpleScore>::new(1, 2);
    /// let result = list_custom_phase(&p, &n, p.initial_assignment(),
    ///     CustomPhaseConfig::default(), |context| {
    ///         let swap = ListMove::Swap {
    ///             left: ListPosition { entity: 0, index: 0 },
    ///             right: ListPosition { entity: 0, index: 1 },
    ///         };
    ///         let affected = context.evaluate_inspected(&swap,
    ///             |_, _| Vec::new(), |preview| preview.inconsistent_values().to_vec())?;
    ///         assert_eq!(affected, Some(vec![0, 1]));
    ///         Ok(())
    ///     })?;
    /// assert_eq!(result.assignment, p.initial_assignment());
    /// assert_eq!(result.work.committed_moves, 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn evaluate_inspected<R>(
        &mut self,
        movement: &M,
        valid: impl FnOnce(&A, S) -> R,
        flawed: impl FnOnce(&MoveInspection<A>) -> R,
    ) -> Result<Option<R>, CustomPhaseError<E>> {
        if !self.begin() {
            return Ok(None);
        }
        self.failed = true;
        let preview = self
            .director
            .preview(movement)
            .map_err(CustomPhaseError::Scoring)?;
        self.failed = false;
        if let Some(preview) = preview
            && preview.is_flawed()
        {
            Ok(Some(flawed(&preview)))
        } else {
            self.evaluate_started(movement, valid)
        }
    }
    fn evaluate_started<R>(
        &mut self,
        movement: &M,
        visit: impl FnOnce(&A, S) -> R,
    ) -> Result<Option<R>, CustomPhaseError<E>> {
        self.failed = true;
        let failed = &mut self.failed;
        let work = &mut self.work;
        let mut visit = Some(visit);
        let mut value = None;
        let result = self.director.trial(movement, &mut |a, s, count| {
            work.constraint_evaluations += count;
            // Scoring succeeded and its work is known. A caught visitor panic
            // can therefore resume after the transaction guard rolls back.
            *failed = false;
            value = Some(visit.take().expect("one candidate")(a, s));
            *failed = true;
            false
        });
        self.operation(result)?;
        self.failed = false;
        Ok(value)
    }
    /// Diagnostic full-cache verification; excluded from work budgets/counters.
    pub fn assert_consistent(&mut self) -> Result<(), CustomPhaseError<E>> {
        let already_failed = self.failed;
        self.failed = true;
        let result = self.director.assert_consistent();
        self.operation(result)?;
        self.failed = already_failed;
        Ok(())
    }
}
fn run<A: Clone, M, S: Score, D: CommandDirector<A, M, S>>(
    initial: A,
    config: CustomPhaseConfig<'_>,
    complete: impl Fn(&A) -> bool,
    create: impl FnOnce(A) -> Result<D, D::Error>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, A, M, S, D::Error>,
    ) -> Result<(), CustomPhaseError<D::Error>>,
    observer: impl FnMut(SolutionUpdate<'_, A, S>),
) -> Result<CustomPhaseResult<A, S>, CustomPhaseError<D::Error>> {
    let clock = crate::time_limit::monotonic_clock(config.max_time.is_some());
    run_with_clock(initial, config, complete, create, command, observer, clock)
}
fn run_with_clock<A: Clone, M, S: Score, D: CommandDirector<A, M, S>>(
    initial: A,
    config: CustomPhaseConfig<'_>,
    complete: impl Fn(&A) -> bool,
    create: impl FnOnce(A) -> Result<D, D::Error>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, A, M, S, D::Error>,
    ) -> Result<(), CustomPhaseError<D::Error>>,
    observer: impl FnMut(SolutionUpdate<'_, A, S>),
    clock: impl FnMut() -> Duration,
) -> Result<CustomPhaseResult<A, S>, CustomPhaseError<D::Error>> {
    run_monitored_with_clock(
        initial, config, complete, create, command, observer, clock, None, None, false,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_monitored<A: Clone, M, S: Score, D: CommandDirector<A, M, S>>(
    initial: A,
    config: CustomPhaseConfig<'_>,
    complete: impl Fn(&A) -> bool,
    create: impl FnOnce(A) -> Result<D, D::Error>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, A, M, S, D::Error>,
    ) -> Result<(), CustomPhaseError<D::Error>>,
    observer: impl FnMut(SolutionUpdate<'_, A, S>),
    shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    target: Option<S>,
    stop_when_feasible: bool,
) -> Result<CustomPhaseResult<A, S>, CustomPhaseError<D::Error>> {
    let clock = crate::solver_clock::monitored_clock(config.max_time.is_some(), &shared);
    run_monitored_with_clock(
        initial,
        config,
        complete,
        create,
        command,
        observer,
        clock,
        shared,
        target,
        stop_when_feasible,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_monitored_with_clock<A: Clone, M, S: Score, D: CommandDirector<A, M, S>>(
    initial: A,
    config: CustomPhaseConfig<'_>,
    complete: impl Fn(&A) -> bool,
    create: impl FnOnce(A) -> Result<D, D::Error>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, A, M, S, D::Error>,
    ) -> Result<(), CustomPhaseError<D::Error>>,
    mut observer: impl FnMut(SolutionUpdate<'_, A, S>),
    mut clock: impl FnMut() -> Duration,
    mut shared: Option<&mut dyn crate::time_limit::SearchControl<S>>,
    target: Option<S>,
    stop_when_feasible: bool,
) -> Result<CustomPhaseResult<A, S>, CustomPhaseError<D::Error>> {
    let early = if config.max_evaluations == 0 {
        Some(CustomPhaseTermination::EvaluationLimit)
    } else if config
        .cancellation
        .is_some_and(CancellationToken::is_cancelled)
    {
        Some(CustomPhaseTermination::Cancelled)
    } else {
        None
    };
    if let Some(termination) = early {
        return Ok(CustomPhaseResult {
            complete: complete(&initial),
            assignment: initial,
            score: None,
            work: CustomPhaseWork::default(),
            termination,
        });
    }
    let mut director = create(initial).map_err(CustomPhaseError::Scoring)?;
    let score_counter = crate::score_evaluations::from_control(&shared);
    crate::score_evaluations::record(&score_counter);
    let work = CustomPhaseWork {
        evaluations: 1,
        constraint_evaluations: director.initial_evaluations(),
        ..Default::default()
    };
    let best_assignment = director.assignment().clone();
    let best_score = director.score();
    let best_complete = complete(&best_assignment);
    director.notify_best(best_complete);
    observer(SolutionUpdate {
        assignment: &best_assignment,
        score: best_score.clone(),
        complete: best_complete,
    });
    if let Some(shared) = &mut shared {
        shared.start_search();
        if best_complete {
            shared.observe_complete_best_score(&best_score);
        } else {
            shared.observe_best_score(&best_score);
        }
    }
    let move_counter = crate::move_metrics::from_control(&shared);
    let mut shared = SharedControl(shared);
    let mut context = CustomPhaseContext {
        director: &mut director,
        config,
        clock: &mut clock,
        elapsed: Duration::ZERO,
        complete: &complete,
        observer: &mut observer,
        best_assignment,
        best_score,
        best_complete,
        work,
        stop: None,
        failed: false,
        shared: Some(&mut shared),
        move_counter,
        score_counter,
        target,
        stop_when_feasible,
    };
    if !context.is_terminated() {
        command(&mut context)?;
    }
    if context.failed {
        return Err(CustomPhaseError::IgnoredOperationError);
    }
    let termination = context
        .termination()
        .unwrap_or(CustomPhaseTermination::Completed);
    Ok(CustomPhaseResult {
        assignment: context.best_assignment,
        score: Some(context.best_score),
        complete: context.best_complete,
        work: context.work,
        termination,
    })
}

/// Run one bounded custom command on an explicit initial assignment. See
/// [CustomPhaseContext]. Input validation precedes zero budgets and cancellation.
pub fn custom_phase<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, ScoringError>,
    ) -> Result<(), CustomPhaseError<ScoringError>>,
) -> Result<CustomPhaseResult<Assignment<V>, S>, CustomPhaseError<ScoringError>> {
    custom_phase_with_observer(problem, network, initial, config, command, |_| {})
}
/// As [custom_phase], with synchronous initial-best and strict-best notifications.
/// Callback time belongs to the phase deadline. Panics propagate.
#[allow(clippy::too_many_arguments)]
pub fn custom_phase_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &FiniteProblem<V>,
    network: &ConstraintNetwork<'_, V, S>,
    initial: Assignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, ScoringError>,
    ) -> Result<(), CustomPhaseError<ScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<CustomPhaseResult<Assignment<V>, S>, CustomPhaseError<ScoringError>> {
    network
        .validate(problem, &initial)
        .map_err(CustomPhaseError::Scoring)?;
    run(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| ScoreDirector::new(problem, network, a),
        command,
        observer,
    )
}

/// Run one bounded custom command on an explicit initial assignment. See
/// [CustomPhaseContext]. Input validation precedes zero budgets and cancellation.
pub fn list_custom_phase<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListScoringError>,
    ) -> Result<(), CustomPhaseError<ListScoringError>>,
) -> Result<CustomPhaseResult<ListAssignment<V>, S>, CustomPhaseError<ListScoringError>> {
    list_custom_phase_with_observer(problem, network, initial, config, command, |_| {})
}
/// As [list_custom_phase], with synchronous initial-best and strict-best notifications.
/// Callback time belongs to the phase deadline. Panics propagate.
#[allow(clippy::too_many_arguments)]
pub fn list_custom_phase_with_observer<V: Eq + Clone, S: AdditiveScore>(
    problem: &ListProblem<V>,
    network: &ListConstraintNetwork<'_, V, S>,
    initial: ListAssignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListScoringError>,
    ) -> Result<(), CustomPhaseError<ListScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<CustomPhaseResult<ListAssignment<V>, S>, CustomPhaseError<ListScoringError>> {
    network
        .validate(problem, &initial)
        .map_err(CustomPhaseError::Scoring)?;
    run(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| ListScoreDirector::new(problem, network, a),
        command,
        observer,
    )
}

/// Run one bounded custom command on an explicit initial assignment. See
/// [CustomPhaseContext]. Input validation precedes zero budgets and cancellation.
pub fn mixed_custom_phase<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, MixedAssignment<B, L>, MixedMove<B, L>, S, MixedScoringError>,
    ) -> Result<(), CustomPhaseError<MixedScoringError>>,
) -> Result<CustomPhaseResult<MixedAssignment<B, L>, S>, CustomPhaseError<MixedScoringError>> {
    mixed_custom_phase_with_observer(problem, network, initial, config, command, |_| {})
}
/// As [mixed_custom_phase], with synchronous initial-best and strict-best notifications.
/// Callback time belongs to the phase deadline. Panics propagate.
#[allow(clippy::too_many_arguments)]
pub fn mixed_custom_phase_with_observer<B: Eq + Clone, L: Eq + Clone, S: AdditiveScore>(
    problem: &MixedProblem<B, L>,
    network: &MixedConstraintNetwork<'_, B, L, S>,
    initial: MixedAssignment<B, L>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, MixedAssignment<B, L>, MixedMove<B, L>, S, MixedScoringError>,
    ) -> Result<(), CustomPhaseError<MixedScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<CustomPhaseResult<MixedAssignment<B, L>, S>, CustomPhaseError<MixedScoringError>> {
    network
        .validate(problem, &initial)
        .map_err(CustomPhaseError::Scoring)?;
    run(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| MixedScoreDirector::new(problem, network, a),
        command,
        observer,
    )
}

/// Run one bounded custom command on an explicit initial assignment. See
/// [CustomPhaseContext]. Input validation precedes zero budgets and cancellation.
pub fn stream_custom_phase<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
    ) -> Result<(), CustomPhaseError<StreamScoringError>>,
) -> Result<CustomPhaseResult<Assignment<V>, S>, CustomPhaseError<StreamScoringError>> {
    stream_custom_phase_with_observer(problem, graph, sources, initial, config, command, |_| {})
}
/// As [stream_custom_phase], with synchronous initial-best and strict-best notifications.
/// Callback time belongs to the phase deadline. Panics propagate.
#[allow(clippy::too_many_arguments)]
pub fn stream_custom_phase_with_observer<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &FiniteProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: PlanningStreamSource<V>,
    initial: Assignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, Assignment<V>, Move<V>, S, StreamScoringError>,
    ) -> Result<(), CustomPhaseError<StreamScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, Assignment<V>, S>),
) -> Result<CustomPhaseResult<Assignment<V>, S>, CustomPhaseError<StreamScoringError>> {
    crate::stream_scoring::validate_source(&graph, &sources).map_err(CustomPhaseError::Scoring)?;
    problem
        .validate(&initial)
        .map_err(StreamScoringError::from)
        .map_err(CustomPhaseError::Scoring)?;
    run(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| StreamScoreDirector::new(problem, graph, sources, a),
        command,
        observer,
    )
}

/// Run one bounded custom command on an explicit initial assignment. See
/// [CustomPhaseContext]. Input validation precedes zero budgets and cancellation.
pub fn list_stream_custom_phase<V: Eq + Clone + 'static, S: AdditiveScore + 'static>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListStreamScoringError>,
    ) -> Result<(), CustomPhaseError<ListStreamScoringError>>,
) -> Result<CustomPhaseResult<ListAssignment<V>, S>, CustomPhaseError<ListStreamScoringError>> {
    list_stream_custom_phase_with_observer(
        problem,
        graph,
        sources,
        initial,
        config,
        command,
        |_| {},
    )
}
/// As [list_stream_custom_phase], with synchronous initial-best and strict-best notifications.
/// Callback time belongs to the phase deadline. Panics propagate.
#[allow(clippy::too_many_arguments)]
pub fn list_stream_custom_phase_with_observer<
    V: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &ListProblem<V>,
    graph: StreamGraph<'_, S>,
    sources: ListStreamSources<V>,
    initial: ListAssignment<V>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<'_, ListAssignment<V>, ListMove<V>, S, ListStreamScoringError>,
    ) -> Result<(), CustomPhaseError<ListStreamScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, ListAssignment<V>, S>),
) -> Result<CustomPhaseResult<ListAssignment<V>, S>, CustomPhaseError<ListStreamScoringError>> {
    sources
        .validate(&graph)
        .map_err(CustomPhaseError::Scoring)?;
    problem
        .validate(&initial)
        .map_err(ListStreamScoringError::from)
        .map_err(CustomPhaseError::Scoring)?;
    run(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| ListStreamScoreDirector::new(problem, graph, sources, a),
        command,
        observer,
    )
}

/// Run one bounded custom command on an explicit initial assignment. See
/// [CustomPhaseContext]. Input validation precedes zero budgets and cancellation.
pub fn mixed_stream_custom_phase<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<
            '_,
            MixedAssignment<B, L>,
            MixedMove<B, L>,
            S,
            MixedStreamScoringError,
        >,
    ) -> Result<(), CustomPhaseError<MixedStreamScoringError>>,
) -> Result<CustomPhaseResult<MixedAssignment<B, L>, S>, CustomPhaseError<MixedStreamScoringError>>
{
    mixed_stream_custom_phase_with_observer(
        problem,
        graph,
        sources,
        initial,
        config,
        command,
        |_| {},
    )
}
/// As [mixed_stream_custom_phase], with synchronous initial-best and strict-best notifications.
/// Callback time belongs to the phase deadline. Panics propagate.
#[allow(clippy::too_many_arguments)]
pub fn mixed_stream_custom_phase_with_observer<
    B: Eq + Clone + 'static,
    L: Eq + Clone + 'static,
    S: AdditiveScore + 'static,
>(
    problem: &MixedProblem<B, L>,
    graph: StreamGraph<'_, S>,
    sources: MixedStreamSources<B, L>,
    initial: MixedAssignment<B, L>,
    config: CustomPhaseConfig<'_>,
    command: impl FnOnce(
        &mut CustomPhaseContext<
            '_,
            MixedAssignment<B, L>,
            MixedMove<B, L>,
            S,
            MixedStreamScoringError,
        >,
    ) -> Result<(), CustomPhaseError<MixedStreamScoringError>>,
    observer: impl FnMut(SolutionUpdate<'_, MixedAssignment<B, L>, S>),
) -> Result<CustomPhaseResult<MixedAssignment<B, L>, S>, CustomPhaseError<MixedStreamScoringError>>
{
    sources
        .validate(&graph)
        .map_err(CustomPhaseError::Scoring)?;
    problem
        .validate(&initial)
        .map_err(MixedStreamScoringError::from)
        .map_err(CustomPhaseError::Scoring)?;
    run(
        initial,
        config,
        |a| problem.is_complete(a),
        |a| MixedStreamScoreDirector::new(problem, graph, sources, a),
        command,
        observer,
    )
}

// Reborrow the optional outer monitor without tying the command context to the
// lifetime of the solve's mutable trait-object reference.
struct SharedControl<'a, S>(Option<&'a mut dyn crate::time_limit::SearchControl<S>>);
impl<S> crate::time_limit::SearchControl<S> for SharedControl<'_, S> {
    fn clock_session(&self) -> Option<&crate::solver_clock::ClockSession> {
        self.0.as_ref().and_then(|s| s.clock_session())
    }
    fn monitors_step_scores(&self) -> bool {
        self.0.as_ref().is_some_and(|m| m.monitors_step_scores())
    }
    fn record_step_score(&mut self, score: S, complete: bool) {
        if let Some(m) = &mut self.0 {
            m.record_step_score(score, complete);
        }
    }
    fn step_started(&mut self) {
        if let Some(m) = &mut self.0 {
            m.step_started();
        }
    }
    fn step_ended(&mut self, completed: bool) {
        if let Some(m) = &mut self.0 {
            m.step_ended(completed);
        }
    }
    fn score_counter(&self) -> Option<crate::score_evaluations::ScoreCounter> {
        crate::score_evaluations::from_control(&self.0)
    }
    fn move_counter(&self) -> Option<crate::move_metrics::MoveCounter> {
        crate::move_metrics::from_control(&self.0)
    }
    fn observe_complete_best_score(&mut self, score: &S) {
        if let Some(m) = &mut self.0 {
            m.observe_complete_best_score(score);
        }
    }
    fn observe_best_score(&mut self, score: &S) {
        if let Some(m) = &mut self.0 {
            m.observe_best_score(score);
        }
    }
    fn start_search(&mut self) {
        if let Some(m) = &mut self.0 {
            m.start_search();
        }
    }
    fn record_improvement(&mut self) {
        if let Some(m) = &mut self.0 {
            m.record_improvement();
        }
    }
    fn complete_step(&mut self, improved: bool) {
        if let Some(m) = &mut self.0 {
            m.complete_step(improved);
        }
    }
    fn stop(&mut self) -> Option<LocalSearchTermination> {
        self.0.as_mut().and_then(|m| m.stop())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    #[test]
    fn custom_deadlines_include_initial_scoring_observers_commands_and_candidate_work() {
        for stage in 0..4 {
            let now = Cell::new(0);
            let calls = Cell::new(0);
            let invoked = Cell::new(false);
            let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("value", vec![0], |v| {
                calls.set(calls.get() + 1);
                if (stage == 0 && calls.get() == 1) || (stage == 3 && calls.get() == 2) {
                    now.set(11);
                }
                SimpleScore::new(v[0].copied().unwrap())
            })
            .unwrap();
            let r = run_with_clock(
                p.initial_assignment(),
                CustomPhaseConfig {
                    max_time: Some(Duration::from_millis(10)),
                    ..Default::default()
                },
                |a| p.is_complete(a),
                |a| ScoreDirector::new(&p, &n, a),
                |c| {
                    invoked.set(true);
                    if stage == 2 {
                        now.set(11);
                    } else {
                        c.execute(&Move::Change {
                            variable: 0,
                            value: Some(1),
                        })?;
                    }
                    Ok(())
                },
                |_| {
                    if stage == 1 {
                        now.set(11);
                    }
                },
                || Duration::from_millis(now.get()),
            )
            .unwrap();
            assert_eq!(invoked.get(), stage >= 2);
            assert_eq!(r.termination, CustomPhaseTermination::TimeLimit);
            assert_eq!(r.work.evaluations, if stage == 3 { 2 } else { 1 });
            assert_eq!(r.score, Some(SimpleScore::new(i64::from(stage == 3))));
        }
    }
    #[test]
    fn custom_termination_latches_and_rejects_later_work_after_time_or_counts_expire() {
        let now = Cell::new(0);
        let p = FiniteProblem::new(vec![Variable::new(vec![0i64, 1]).with_initial(0)]).unwrap();
        let n = ConstraintNetwork::<_, SimpleScore>::new(1);
        let r = run_with_clock(
            p.initial_assignment(),
            CustomPhaseConfig {
                max_time: Some(Duration::from_millis(1)),
                ..Default::default()
            },
            |a| p.is_complete(a),
            |a| ScoreDirector::new(&p, &n, a),
            |c| {
                now.set(1);
                assert!(c.is_terminated());
                now.set(0);
                assert_eq!(c.termination(), Some(CustomPhaseTermination::TimeLimit));
                assert_eq!(
                    c.execute(&Move::Change {
                        variable: 0,
                        value: Some(1)
                    })?,
                    None
                );
                Ok(())
            },
            |_| {},
            || Duration::from_millis(now.get()),
        )
        .unwrap();
        assert_eq!(r.termination, CustomPhaseTermination::TimeLimit);
        assert_eq!(r.work.evaluations, 1);
        for (scores, moves, reason) in [
            (1, 0, CustomPhaseTermination::EvaluationLimit),
            (2, 0, CustomPhaseTermination::MoveEvaluationLimit),
        ] {
            let r = custom_phase(
                &p,
                &n,
                p.initial_assignment(),
                CustomPhaseConfig {
                    max_evaluations: scores,
                    max_move_evaluations: Some(moves),
                    ..Default::default()
                },
                |_| panic!("exhausted before command"),
            )
            .unwrap();
            assert_eq!(r.termination, reason);
            assert_eq!(r.work.evaluations, 1);
        }
    }
}
