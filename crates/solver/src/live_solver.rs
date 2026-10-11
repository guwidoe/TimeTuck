//! Owned, revisioned problem changes and cooperative warm restarts.
use crate::solver_manager_callbacks::{capture, dispose};
use crate::*;
use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    fmt, io,
    sync::{
        Arc, Condvar, Mutex, OnceLock, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum LiveDelivery {
    #[default]
    Best,
    Final,
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LiveSolverConfig {
    /// Wait after normal termination; cancellation or finish_after_run exits.
    pub daemon: bool,
    /// Final delivery waits for job completion, including in daemon mode.
    pub delivery: LiveDelivery,
}
/// An independently owned result of solving the indicated problem revision.
#[derive(Debug)]
pub struct LiveSolution<R> {
    pub revision: u64,
    pub solution: Arc<R>,
}
impl<R> Clone for LiveSolution<R> {
    fn clone(&self) -> Self {
        Self {
            revision: self.revision,
            solution: self.solution.clone(),
        }
    }
}
type LiveConsumer<R> = Box<dyn FnMut(u64, SolverBestEvent<'_, LiveSolution<R>>) + Send>;
type LiveInitialized<R> = Box<dyn FnOnce(u64, &LiveSolution<R>, SolverInitialization) + Send>;

/// Revision-aware callbacks for a live job. The solution consumer receives the
/// revision and optional native phase origin together with each snapshot. Best
/// delivery may coalesce updates; Final delivery calls it once with no producer.
/// Successful solution consumption acknowledges changes through that revision.
///
/// Initialization is separate, once per job (not once per restart), and does not
/// acknowledge changes. Both callbacks run serially on the consumer pool. A panic
/// in either callback stops the live loop cooperatively and fails pending receipts;
/// it is recorded in [`LiveSolverJob::callback_failures`]. Never wait on this job
/// from its callbacks. Snapshots must independently own their problem facts.
pub struct LiveSolverCallbacks<R> {
    consumer: LiveConsumer<R>,
    initialized: Option<LiveInitialized<R>>,
}
impl<R> LiveSolverCallbacks<R> {
    pub fn new(
        consumer: impl FnMut(u64, SolverBestEvent<'_, LiveSolution<R>>) + Send + 'static,
    ) -> Self {
        Self {
            consumer: Box::new(consumer),
            initialized: None,
        }
    }
    /// Retain the first explicitly published initialization independently of
    /// best-update coalescing. Delivery also occurs in Final mode or when solving
    /// subsequently fails. No event is synthesized from a returned solve result.
    pub fn on_first_initialized(
        mut self,
        callback: impl FnOnce(u64, &LiveSolution<R>, SolverInitialization) + Send + 'static,
    ) -> Self {
        self.initialized = Some(Box::new(callback));
        self
    }
    fn plain(mut consumer: impl FnMut(u64, &R) + Send + 'static) -> Self {
        Self::new(move |id, event| consumer(id, &event.solution.solution))
    }
}
#[derive(Debug)]
pub enum LiveSolveError<E> {
    Solve(E),
    Change { revision: u64, message: String },
}
impl<E: fmt::Display> fmt::Display for LiveSolveError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Solve(e) => e.fmt(f),
            Self::Change { revision, message } => {
                write!(f, "problem change {revision} failed: {message}")
            }
        }
    }
}
impl<E: Error + 'static> Error for LiveSolveError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solve(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProblemChangeFailure {
    Cancelled,
    Stopped,
    ChangeFailed(String),
    SolveFailed(String),
    ConsumerPanicked(String),
}
impl fmt::Display for ProblemChangeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("problem change cancelled before delivery"),
            Self::Stopped => f.write_str("solver stopped before delivering this problem change"),
            Self::ChangeFailed(s) => write!(f, "problem change failed: {s}"),
            Self::SolveFailed(s) => write!(f, "solving failed before change delivery: {s}"),
            Self::ConsumerPanicked(s) => write!(f, "problem change consumer panicked: {s}"),
        }
    }
}
impl Error for ProblemChangeFailure {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProblemChangeSubmitError {
    Unavailable { problem_id: u64 },
    RevisionExhausted { problem_id: u64 },
}
impl fmt::Display for ProblemChangeSubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { problem_id } => {
                write!(f, "problem {problem_id} is not accepting changes")
            }
            Self::RevisionExhausted { problem_id } => {
                write!(f, "problem {problem_id} exhausted its revision counter")
            }
        }
    }
}
impl Error for ProblemChangeSubmitError {}
#[derive(Default)]
struct Receipt {
    result: Mutex<Option<Result<(), ProblemChangeFailure>>>,
    ready: Condvar,
}
impl Receipt {
    fn complete(&self, result: Result<(), ProblemChangeFailure>) {
        let mut slot = self.result.lock().unwrap();
        if slot.is_none() {
            *slot = Some(result);
            self.ready.notify_all();
        }
    }
}
/// Completion means a consumer returned successfully with this revision or a
/// later one. Application alone is insufficient. Timeout does not cancel work.
#[derive(Clone)]
pub struct ProblemChangeReceipt {
    pub problem_id: u64,
    pub revision: u64,
    cell: Arc<Receipt>,
}
impl ProblemChangeReceipt {
    pub fn try_result(&self) -> Option<Result<(), ProblemChangeFailure>> {
        self.cell.result.lock().unwrap().clone()
    }
    pub fn wait(&self) -> Result<(), ProblemChangeFailure> {
        self.cell
            .ready
            .wait_while(self.cell.result.lock().unwrap(), |r| r.is_none())
            .unwrap()
            .clone()
            .unwrap()
    }
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Result<(), ProblemChangeFailure>> {
        self.cell
            .ready
            .wait_timeout_while(self.cell.result.lock().unwrap(), timeout, |r| r.is_none())
            .unwrap()
            .0
            .clone()
    }
}
type Change<P, R> = Box<dyn FnOnce(&mut P, Option<&LiveSolution<R>>) -> Result<(), String> + Send>;
struct QueuedChange<P, R> {
    revision: u64,
    apply: Change<P, R>,
}
struct Data<P, R> {
    accepting: bool,
    cancelled: bool,
    finishing: bool,
    next: u64,
    applied: u64,
    queue: VecDeque<QueuedChange<P, R>>,
    receipts: VecDeque<(u64, Arc<Receipt>)>,
    running: Option<CancellationToken>,
}
struct State<P, R> {
    cancel_job: OnceLock<Box<dyn Fn() -> bool + Send + Sync>>,
    data: Mutex<Data<P, R>>,
    ready: Condvar,
}
impl<P, R> State<P, R> {
    fn new() -> Self {
        Self {
            cancel_job: OnceLock::new(),
            data: Mutex::new(Data {
                accepting: true,
                cancelled: false,
                finishing: false,
                next: 0,
                applied: 0,
                queue: VecDeque::new(),
                receipts: VecDeque::new(),
                running: None,
            }),
            ready: Condvar::new(),
        }
    }
    fn cancel(&self) -> bool {
        let mut d = self.data.lock().unwrap();
        let first = !d.cancelled;
        let was_accepting = d.accepting;
        d.cancelled = true;
        d.accepting = false;
        if let Some(t) = &d.running {
            t.cancel();
        }
        self.ready.notify_all();
        drop(d);
        // Notification can let the live worker finish and leave the pool before
        // pool cancellation acquires its lock. The accepted live request still
        // counts; only the first caller may report success.
        let pool_cancelled = self.cancel_job.get().is_some_and(|cancel| cancel());
        first && (was_accepting || pool_cancelled)
    }
    fn enqueue(
        &self,
        id: u64,
        apply: Change<P, R>,
    ) -> Result<ProblemChangeReceipt, ProblemChangeSubmitError> {
        let mut d = self.data.lock().unwrap();
        if !d.accepting {
            drop(d);
            dispose(apply);
            return Err(ProblemChangeSubmitError::Unavailable { problem_id: id });
        }
        let Some(revision) = d.next.checked_add(1) else {
            drop(d);
            dispose(apply);
            return Err(ProblemChangeSubmitError::RevisionExhausted { problem_id: id });
        };
        let cell = Arc::new(Receipt::default());
        d.next = revision;
        d.queue.push_back(QueuedChange { revision, apply });
        d.receipts.push_back((revision, cell.clone()));
        if let Some(t) = &d.running {
            t.cancel();
        }
        self.ready.notify_all();
        Ok(ProblemChangeReceipt {
            problem_id: id,
            revision,
            cell,
        })
    }
    fn acknowledge(&self, revision: u64) {
        let mut d = self.data.lock().unwrap();
        while d.receipts.front().is_some_and(|(r, _)| *r <= revision) {
            d.receipts.pop_front().unwrap().1.complete(Ok(()));
        }
    }
    fn fail(&self, error: ProblemChangeFailure) {
        let queue = {
            let mut d = self.data.lock().unwrap();
            d.accepting = false;
            d.cancelled = true;
            if let Some(t) = &d.running {
                t.cancel();
            }
            for (_, r) in d.receipts.drain(..) {
                r.complete(Err(error.clone()));
            }
            self.ready.notify_all();
            std::mem::take(&mut d.queue)
        };
        dispose(queue);
    }
    fn close(&self, started: bool) {
        let queue = {
            let mut d = self.data.lock().unwrap();
            d.accepting = false;
            d.running = None;
            let applied = if started { d.applied } else { 0 };
            let error = if d.cancelled || d.finishing || !started {
                ProblemChangeFailure::Cancelled
            } else {
                ProblemChangeFailure::Stopped
            };
            let mut retained = VecDeque::new();
            for (revision, r) in d.receipts.drain(..) {
                if revision > applied {
                    r.complete(Err(error.clone()));
                } else {
                    retained.push_back((revision, r));
                }
            }
            d.receipts = retained;
            self.ready.notify_all();
            std::mem::take(&mut d.queue)
        };
        dispose(queue);
    }
}
type Registry<P, R> = Arc<Mutex<HashMap<u64, Weak<State<P, R>>>>>;
struct RunGuard<P, R> {
    state: Arc<State<P, R>>,
    registry: Registry<P, R>,
    id: u64,
    started: bool,
}
impl<P, R> Drop for RunGuard<P, R> {
    fn drop(&mut self) {
        self.state.close(self.started);
        let mut registry = self.registry.lock().unwrap();
        if registry
            .get(&self.id)
            .is_some_and(|s| s.ptr_eq(&Arc::downgrade(&self.state)))
        {
            registry.remove(&self.id);
        }
    }
}
/// A completed native run with its actual work counters.
pub struct LiveRun<R> {
    pub solution: R,
    pub work: SolverWork,
}

/// Best-solution publication scoped to one live solve and its problem revision.
/// The publisher cannot outlive the solve callback. Published snapshots are
/// independently owned and may be coalesced before consumption. Acceptance does
/// not acknowledge problem changes: successful consumption does, for this
/// revision and earlier revisions only.
///
/// Intermediate best publication is disabled for Final delivery and once this run's
/// cancellation token is set. Cancellation may race a publication already in
/// progress; that snapshot still carries its original revision. The solve's
/// returned result retains the existing final/warm-restart delivery semantics.
/// Callers must publish actual best snapshots; this API does not compare scores
/// or retain them as warm starts. The returned solve result supplies the warm start.
pub struct LiveSolverProgress<'a, R> {
    revision: u64,
    enabled: bool,
    token: &'a CancellationToken,
    progress: &'a SolverProgress<LiveSolution<R>>,
    finish_requested: AtomicBool,
}
impl<R> LiveSolverProgress<'_, R> {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Finish this job after the current solve callback returns successfully,
    /// including in daemon mode. The returned solution is delivered normally;
    /// queued, unapplied changes are cancelled without starting another run.
    /// Applied receipts still require successful consumer completion.
    ///
    /// Returns true for the first request. This does not cancel the current token
    /// or preempt the callback. Submissions can still be accepted until the callback
    /// returns; those queued changes will be cancelled. A solve error or consumer
    /// failure retains its normal failure semantics. The request cannot be undone.
    pub fn finish_after_run(&self) -> bool {
        !self.finish_requested.swap(true, Ordering::Relaxed)
    }
    /// Queue an owned best snapshot for the existing job consumer. False means
    /// it was not queued; true does not guarantee successful consumer execution.
    pub fn publish(&self, solution: R) -> bool {
        self.publish_best(solution, None)
    }
    /// Publish an owned best snapshot with its actual native phase origin. Origin
    /// and revision are coalesced atomically with the snapshot. Returned solve
    /// results are still published without an origin: no origin is inferred from
    /// an earlier intermediate update.
    pub fn publish_from(&self, solution: R, producer: SolverEventProducer) -> bool {
        self.publish_best(solution, Some(producer))
    }
    fn publish_best(&self, solution: R, producer: Option<SolverEventProducer>) -> bool {
        if !self.enabled || self.token.is_cancelled() {
            dispose(solution);
            return false;
        }
        let snapshot = LiveSolution {
            revision: self.revision,
            solution: Arc::new(solution),
        };
        match producer {
            Some(producer) => self.progress.publish_from(snapshot, producer),
            None => self.progress.publish(snapshot),
        }
    }
    /// Publish the job's first initialization, even in Final delivery mode. This
    /// is explicit: the caller must obtain the initialization metadata from its
    /// solver. Later attempts, including after a revision restart, are ignored.
    /// A cancelled run cannot publish; already accepted events keep their original
    /// revision. Initialization delivery alone never acknowledges problem changes.
    pub fn publish_first_initialized(
        &self,
        solution: R,
        initialization: SolverInitialization,
    ) -> bool {
        if self.token.is_cancelled() {
            dispose(solution);
            return false;
        }
        self.progress.publish_first_initialized(
            LiveSolution {
                revision: self.revision,
                solution: Arc::new(solution),
            },
            initialization,
        )
    }
}
type InspectProblem<P> = Box<dyn Fn(&P) -> ProblemSizeStatistics + Send>;
/// A live job handle. Cancellation remains latched across automatic restarts.
pub struct LiveSolverJob<P, R, E> {
    inner: SolverJob<LiveSolution<R>, LiveSolveError<E>>,
    state: Arc<State<P, R>>,
}
impl<P, R, E> Clone for LiveSolverJob<P, R, E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            state: self.state.clone(),
        }
    }
}
impl<P, R, E> LiveSolverJob<P, R, E> {
    pub fn problem_id(&self) -> u64 {
        self.inner.problem_id()
    }
    pub fn statistics(&self) -> SolverJobStatistics {
        self.inner.statistics()
    }
    pub fn status(&self) -> SolverStatus {
        self.inner.status()
    }
    pub fn callback_failures(&self) -> Vec<SolverCallbackFailure> {
        self.inner.callback_failures()
    }
    pub fn cancel(&self) -> bool {
        self.state.cancel()
    }
    pub fn try_result(&self) -> Option<SolverJobResult<LiveSolution<R>, LiveSolveError<E>>> {
        self.inner.try_result()
    }
    pub fn wait(&self) -> SolverJobResult<LiveSolution<R>, LiveSolveError<E>> {
        self.inner.wait()
    }
    pub fn wait_timeout(
        &self,
        duration: Duration,
    ) -> Option<SolverJobResult<LiveSolution<R>, LiveSolveError<E>>> {
        self.inner.wait_timeout(duration)
    }
    pub fn add_problem_change(
        &self,
        change: impl FnOnce(&mut P) -> Result<(), String> + Send + 'static,
    ) -> Result<ProblemChangeReceipt, ProblemChangeSubmitError> {
        self.add_problem_change_with_previous(move |problem, _| change(problem))
    }
    /// Queue an edit that can inspect the previous completed run's owned result.
    ///
    /// The borrow is selected when the edit is applied on the solver thread,
    /// after the interrupted run returns. It is `None` before the first completed
    /// run. Interim progress publications are not considered, and the result need
    /// not yet have reached the consumer. Its revision identifies the problem
    /// that was solved; it does not include this edit or earlier edits in the same
    /// drained batch. Such edits see each other's changes to `problem` in order,
    /// while all borrow the same previous result. No scoring or result clone is
    /// performed. Published snapshots remain immutable.
    ///
    /// This supports assignment-dependent edits such as pinning a retained value.
    /// Reconcile any retained assignments with the changed model in the solve
    /// closure as usual. Errors, panics, cancellation and receipt completion have
    /// the same semantics as [`Self::add_problem_change`].
    pub fn add_problem_change_with_previous(
        &self,
        change: impl FnOnce(&mut P, Option<&LiveSolution<R>>) -> Result<(), String> + Send + 'static,
    ) -> Result<ProblemChangeReceipt, ProblemChangeSubmitError> {
        self.state.enqueue(self.problem_id(), Box::new(change))
    }
}
/// Live jobs reuse the bounded solver/consumer pools. Problem changes run only
/// on their solver thread. The solve closure builds fresh models/scorers from P,
/// reconciles an optional previous result for a warm start, and passes the supplied
/// per-run token into its solver limits. A queued change cancels that token; explicit
/// job cancellation remains latched across runs. No user code runs under queue locks.
///
/// Results must own independent snapshots: do not share mutable problem facts
/// between P and published R. Each completed solve is eligible for publication,
/// even when a changed problem lowers its score. Best delivery can skip obsolete
/// snapshots; a later consumed revision acknowledges all preceding changes.
/// Consumer panic fails pending receipts and stops the live loop cooperatively.
/// Change/solve failures stop the job. Changes are not rolled back after failure.
///
/// The solve closure controls reconciliation and ordinary termination. Rebuilding
/// models resets phase/solve limits; cancellation cannot preempt arbitrary user code.
/// Progress-enabled submissions also publish intermediate improvements.
///
/// ```
/// use timetuck::*;
/// let mut manager = LiveSolverManager::<Vec<i64>, Vec<Option<i64>>, String>::new(1)?;
/// let job = manager.submit(7, vec![1, 2], LiveSolverConfig {
///     daemon: true, ..Default::default()
/// }, |values, previous, token| {
///     let variables = (0..2).map(|i| {
///         let v = Variable::new(values.clone());
///         previous.and_then(|s| s[i]).filter(|x| values.contains(x))
///             .map_or(v.clone(), |x| v.with_initial(x))
///     }).collect();
///     let problem = FiniteProblem::new(variables).unwrap();
///     let mut score = ConstraintNetwork::new(2);
///     score.add_constraint("sum", vec![0, 1], |v| {
///         SimpleScore::new(v.iter().filter_map(|x| x.copied()).sum())
///     }).unwrap();
///     let result = solve(&problem, &score, SolverConfig {
///         limits: SolverLimits { cancellation: Some(token), ..Default::default() },
///         ..Default::default()
///     }).map_err(|e| e.to_string())?;
///     Ok(result.assignment.values().to_vec())
/// }, |_, solution| { assert_eq!(solution.len(), 2); })?;
/// let receipt = job.add_problem_change(|values| { values.push(3); Ok(()) })?;
/// receipt.wait()?; // The consumer has returned with a solution including the change.
/// job.cancel();
/// let result = job.wait();
/// assert_eq!(result.as_ref().as_ref().unwrap().revision, 1);
/// manager.shutdown()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct LiveSolverManager<P, R, E> {
    inner: SolverManager<LiveSolution<R>, LiveSolveError<E>>,
    registry: Registry<P, R>,
}
impl<P: Send + 'static, R: Send + Sync + 'static, E: fmt::Display + Send + Sync + 'static>
    LiveSolverManager<P, R, E>
{
    pub fn new(workers: usize) -> io::Result<Self> {
        Self::with_consumer_count(workers, 1)
    }
    pub fn with_consumer_count(workers: usize, consumers: usize) -> io::Result<Self> {
        Ok(Self {
            inner: SolverManager::with_consumer_count(workers, consumers)?,
            registry: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    /// Configure both native worker pools; live restarts reuse those same threads.
    /// Factory and cleanup requirements match SolverManager::with_thread_factory.
    pub fn with_thread_factory(
        workers: usize,
        consumers: usize,
        factory: impl FnMut(
            SolverThreadSpec,
            SolverThreadTask,
        ) -> io::Result<std::thread::JoinHandle<()>>,
    ) -> io::Result<Self> {
        Ok(Self {
            inner: SolverManager::with_thread_factory(workers, consumers, factory)?,
            registry: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    pub fn submit(
        &self,
        id: u64,
        problem: P,
        config: LiveSolverConfig,
        mut solve: impl FnMut(&P, Option<&R>, &CancellationToken) -> Result<R, E> + Send + 'static,
        consumer: impl FnMut(u64, &R) + Send + 'static,
    ) -> Result<LiveSolverJob<P, R, E>, SubmitError> {
        self.submit_inner(
            id,
            problem,
            config,
            None,
            move |p, old, token, _| solve(p, old, token).map(|r| (r, None)),
            LiveSolverCallbacks::plain(consumer),
        )
    }
    /// Publish best snapshots while a revision is still solving. Consumers run
    /// asynchronously, serially per job, with the same coalescing and failure
    /// handling as ordinary live results. A consumed intermediate snapshot can
    /// acknowledge applied problem changes before this solve callback returns.
    pub fn submit_with_progress(
        &self,
        id: u64,
        problem: P,
        config: LiveSolverConfig,
        mut solve: impl FnMut(
            &P,
            Option<&R>,
            &CancellationToken,
            &LiveSolverProgress<'_, R>,
        ) -> Result<R, E>
        + Send
        + 'static,
        consumer: impl FnMut(u64, &R) + Send + 'static,
    ) -> Result<LiveSolverJob<P, R, E>, SubmitError> {
        self.submit_inner(
            id,
            problem,
            config,
            None,
            move |p, old, token, progress| solve(p, old, token, progress).map(|r| (r, None)),
            LiveSolverCallbacks::plain(consumer),
        )
    }
    /// Derive model size before queueing and after applying each batch of changes.
    /// Accumulate counters returned by real solves; failed runs add no invented work.
    /// Daemon idle waits are excluded from solving duration.
    pub fn submit_with_statistics(
        &self,
        id: u64,
        problem: P,
        config: LiveSolverConfig,
        inspect: impl Fn(&P) -> ProblemSizeStatistics + Send + 'static,
        mut solve: impl FnMut(&P, Option<&R>, &CancellationToken) -> Result<LiveRun<R>, E>
        + Send
        + 'static,
        consumer: impl FnMut(u64, &R) + Send + 'static,
    ) -> Result<LiveSolverJob<P, R, E>, SubmitError> {
        self.submit_inner(
            id,
            problem,
            config,
            Some(Box::new(inspect)),
            move |p, old, token, _| solve(p, old, token).map(|r| (r.solution, Some(r.work))),
            LiveSolverCallbacks::plain(consumer),
        )
    }
    /// Combine intermediate best publication with model inspection and actual
    /// work returned by each solve. Publishing a snapshot does not add work;
    /// counters are accumulated once from the returned LiveRun.
    pub fn submit_with_statistics_and_progress(
        &self,
        id: u64,
        problem: P,
        config: LiveSolverConfig,
        inspect: impl Fn(&P) -> ProblemSizeStatistics + Send + 'static,
        mut solve: impl FnMut(
            &P,
            Option<&R>,
            &CancellationToken,
            &LiveSolverProgress<'_, R>,
        ) -> Result<LiveRun<R>, E>
        + Send
        + 'static,
        consumer: impl FnMut(u64, &R) + Send + 'static,
    ) -> Result<LiveSolverJob<P, R, E>, SubmitError> {
        self.submit_inner(
            id,
            problem,
            config,
            Some(Box::new(inspect)),
            move |p, old, token, progress| {
                solve(p, old, token, progress).map(|r| (r.solution, Some(r.work)))
            },
            LiveSolverCallbacks::plain(consumer),
        )
    }
    /// Submit a progress-enabled live solve with revision-aware solution and
    /// first-initialized callbacks. Existing simple consumers remain available
    /// through [`Self::submit_with_progress`]. Publication adds no score callbacks
    /// or work counters and clones no solution payloads.
    pub fn submit_with_events(
        &self,
        id: u64,
        problem: P,
        config: LiveSolverConfig,
        mut solve: impl FnMut(
            &P,
            Option<&R>,
            &CancellationToken,
            &LiveSolverProgress<'_, R>,
        ) -> Result<R, E>
        + Send
        + 'static,
        callbacks: LiveSolverCallbacks<R>,
    ) -> Result<LiveSolverJob<P, R, E>, SubmitError> {
        self.submit_inner(
            id,
            problem,
            config,
            None,
            move |p, old, token, progress| solve(p, old, token, progress).map(|r| (r, None)),
            callbacks,
        )
    }
    /// Combine revision-aware events with model inspection and actual work
    /// accumulated once from each returned run, across all revisions.
    pub fn submit_with_statistics_and_events(
        &self,
        id: u64,
        problem: P,
        config: LiveSolverConfig,
        inspect: impl Fn(&P) -> ProblemSizeStatistics + Send + 'static,
        mut solve: impl FnMut(
            &P,
            Option<&R>,
            &CancellationToken,
            &LiveSolverProgress<'_, R>,
        ) -> Result<LiveRun<R>, E>
        + Send
        + 'static,
        callbacks: LiveSolverCallbacks<R>,
    ) -> Result<LiveSolverJob<P, R, E>, SubmitError> {
        self.submit_inner(
            id,
            problem,
            config,
            Some(Box::new(inspect)),
            move |p, old, token, progress| {
                solve(p, old, token, progress).map(|r| (r.solution, Some(r.work)))
            },
            callbacks,
        )
    }
    fn submit_inner(
        &self,
        id: u64,
        problem: P,
        config: LiveSolverConfig,
        inspect: Option<InspectProblem<P>>,
        mut solve: impl FnMut(
            &P,
            Option<&R>,
            &CancellationToken,
            &LiveSolverProgress<'_, R>,
        ) -> Result<(R, Option<SolverWork>), E>
        + Send
        + 'static,
        callbacks: LiveSolverCallbacks<R>,
    ) -> Result<LiveSolverJob<P, R, E>, SubmitError> {
        let initial_size = inspect.as_ref().map(|inspect| inspect(&problem));
        let state = Arc::new(State::new());
        {
            let mut registry = self.registry.lock().unwrap();
            if registry.get(&id).and_then(Weak::upgrade).is_some() {
                return Err(SubmitError::AlreadySolving);
            }
            registry.insert(id, Arc::downgrade(&state));
        }
        let guard = RunGuard {
            state: state.clone(),
            registry: self.registry.clone(),
            id,
            started: false,
        };
        let LiveSolverCallbacks {
            mut consumer,
            initialized,
        } = callbacks;
        let delivered = state.clone();
        let deliver = move |id, event: SolverBestEvent<'_, LiveSolution<R>>| {
            let revision = event.solution.revision;
            match capture(|| consumer(id, event)) {
                Ok(()) => delivered.acknowledge(revision),
                Err(message) => {
                    delivered.fail(ProblemChangeFailure::ConsumerPanicked(message.clone()));
                    std::panic::resume_unwind(Box::new(message));
                }
            }
        };
        let finalized = state.clone();
        let failed = state.clone();
        let mut callbacks = match config.delivery {
            LiveDelivery::Best => SolverJobCallbacks::default()
                .on_best_event(deliver)
                .on_final(move |_, _| finalized.fail(ProblemChangeFailure::Stopped)),
            LiveDelivery::Final => {
                let mut deliver = deliver;
                SolverJobCallbacks::default().on_final(move |id, s| {
                    deliver(
                        id,
                        SolverBestEvent {
                            solution: s,
                            producer: None,
                        },
                    );
                    finalized.fail(ProblemChangeFailure::Stopped);
                })
            }
        }
        .on_error(move |_, cause| {
            if let SolverJobFailure::Solving(error) = cause {
                failed.fail(ProblemChangeFailure::SolveFailed(error.to_string()));
            }
        });
        if let Some(initialized) = initialized {
            let failed = state.clone();
            callbacks = callbacks.on_first_initialized(move |id, snapshot, metadata| {
                if let Err(message) = capture(|| initialized(id, snapshot, metadata)) {
                    failed.fail(ProblemChangeFailure::ConsumerPanicked(message.clone()));
                    std::panic::resume_unwind(Box::new(message));
                }
            });
        }
        let run = move |global: CancellationToken, progress: SolverProgress<LiveSolution<R>>| {
            let mut guard = guard;
            guard.started = true;
            let mut problem = problem;
            let mut best: Option<LiveSolution<R>> = None;
            loop {
                progress.timing(true);
                let (mut batch, token) = {
                    let mut d = guard.state.data.lock().unwrap();
                    let token = CancellationToken::new();
                    if d.cancelled || global.is_cancelled() {
                        token.cancel();
                    }
                    d.running = Some(token.clone());
                    (std::mem::take(&mut d.queue), token)
                };
                while let Some(change) = batch.pop_front() {
                    if guard.state.data.lock().unwrap().cancelled || global.is_cancelled() {
                        dispose(change);
                        dispose(batch);
                        break;
                    }
                    if let Err(message) = (change.apply)(&mut problem, best.as_ref()) {
                        guard
                            .state
                            .fail(ProblemChangeFailure::ChangeFailed(message.clone()));
                        dispose(batch);
                        return Err(LiveSolveError::Change {
                            revision: change.revision,
                            message,
                        });
                    }
                    guard.state.data.lock().unwrap().applied = change.revision;
                }
                let revision = guard.state.data.lock().unwrap().applied;
                if let Some(inspect) = &inspect {
                    progress.record_problem_size(revision, inspect(&problem));
                }
                let publisher = LiveSolverProgress {
                    revision,
                    enabled: config.delivery == LiveDelivery::Best,
                    token: &token,
                    progress: &progress,
                    finish_requested: AtomicBool::new(false),
                };
                let (result, work) = solve(
                    &problem,
                    best.as_ref().map(|b| b.solution.as_ref()),
                    &token,
                    &publisher,
                )
                .map_err(LiveSolveError::Solve)?;
                if publisher.finish_requested.load(Ordering::Relaxed) {
                    let mut d = guard.state.data.lock().unwrap();
                    d.finishing = true;
                    d.accepting = false;
                }
                if let Some(work) = work {
                    progress.add_work(work);
                }
                let snapshot = LiveSolution {
                    revision,
                    solution: Arc::new(result),
                };
                if config.delivery == LiveDelivery::Best {
                    progress.publish(snapshot.clone());
                }
                best = Some(snapshot);
                progress.timing(false);
                let mut d = guard.state.data.lock().unwrap();
                d.running = None;
                while config.daemon
                    && d.queue.is_empty()
                    && !d.cancelled
                    && !d.finishing
                    && !global.is_cancelled()
                {
                    d = guard.state.ready.wait(d).unwrap();
                }
                if d.cancelled || d.finishing || global.is_cancelled() || d.queue.is_empty() {
                    d.accepting = false;
                    break;
                }
            }
            Ok(best.expect("at least one solve"))
        };
        let inner = if let Some(size) = initial_size {
            self.inner.submit_with_statistics(id, size, run, callbacks)
        } else {
            self.inner.submit_with_progress(id, run, callbacks)
        }?;
        let cancellation = inner.clone();
        let _ = state
            .cancel_job
            .set(Box::new(move || cancellation.cancel()));
        // A manager cancellation can race submission before its handle is installed.
        let cancelled = state.data.lock().unwrap().cancelled;
        if cancelled {
            inner.cancel();
        }
        Ok(LiveSolverJob { inner, state })
    }
    pub fn status(&self, id: u64) -> SolverStatus {
        self.inner.status(id)
    }
    pub fn add_problem_change(
        &self,
        id: u64,
        change: impl FnOnce(&mut P) -> Result<(), String> + Send + 'static,
    ) -> Result<ProblemChangeReceipt, ProblemChangeSubmitError> {
        self.add_problem_change_with_previous(id, move |problem, _| change(problem))
    }
    /// Submit an edit with access to the previous completed run's result by job
    /// identity. See [`LiveSolverJob::add_problem_change_with_previous`] for the
    /// distinction between the completed result, interim progress, and pending
    /// revisions. The result is borrowed only while applying the edit.
    pub fn add_problem_change_with_previous(
        &self,
        id: u64,
        change: impl FnOnce(&mut P, Option<&LiveSolution<R>>) -> Result<(), String> + Send + 'static,
    ) -> Result<ProblemChangeReceipt, ProblemChangeSubmitError> {
        let state = self
            .registry
            .lock()
            .unwrap()
            .get(&id)
            .and_then(Weak::upgrade);
        match state {
            Some(s) => s.enqueue(id, Box::new(change)),
            None => {
                dispose(change);
                Err(ProblemChangeSubmitError::Unavailable { problem_id: id })
            }
        }
    }
    pub fn terminate_early(&self, id: u64) -> bool {
        let state = self
            .registry
            .lock()
            .unwrap()
            .get(&id)
            .and_then(Weak::upgrade);
        state.is_some_and(|s| s.cancel())
    }
    pub fn request_shutdown(&self) {
        self.cancel_all();
        self.inner.request_shutdown();
        // Cover live submissions between the first snapshot and pool closure.
        self.cancel_all();
    }
    pub fn shutdown(&mut self) -> io::Result<()> {
        self.request_shutdown();
        self.inner.shutdown()
    }
}
impl<P, R, E> LiveSolverManager<P, R, E> {
    fn cancel_all(&self) {
        let states: Vec<_> = self
            .registry
            .lock()
            .unwrap()
            .values()
            .filter_map(Weak::upgrade)
            .collect();
        for state in states {
            state.cancel();
        }
    }
}
impl<P, R, E> Drop for LiveSolverManager<P, R, E> {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[cfg(test)]
mod cancellation_race_tests {
    use super::*;

    #[test]
    fn live_cancellation_succeeds_when_notified_worker_finishes_before_pool_cancel() {
        let state = Arc::new(State::<(), ()>::new());
        let observed = Arc::downgrade(&state);
        assert!(
            state
                .cancel_job
                .set(Box::new(move || {
                    let state = observed.upgrade().unwrap();
                    let data = state.data.lock().unwrap();
                    assert!(data.cancelled);
                    assert!(!data.accepting);
                    false // Pool no longer has the job after the wakeup.
                }))
                .is_ok()
        );
        assert!(state.cancel());
        assert!(!state.cancel());
        let completed = State::<(), ()>::new();
        completed.data.lock().unwrap().accepting = false;
        assert!(!completed.cancel()); // A naturally completed stale handle.
    }

    #[test]
    fn racing_live_cancel_callers_report_success_only_once() {
        let state = Arc::new(State::<(), ()>::new());
        let barrier = Arc::new(std::sync::Barrier::new(2));
        assert!(
            state
                .cancel_job
                .set(Box::new(move || {
                    barrier.wait();
                    true // Both enter the pool hook after marking the live state.
                }))
                .is_ok()
        );
        let other = state.clone();
        let worker = std::thread::spawn(move || other.cancel());
        let first = state.cancel();
        let second = worker.join().unwrap();
        assert_ne!(first, second);
    }
}
