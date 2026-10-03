//! Asynchronous, independent solves on a fixed-size native worker pool.
use crate::solver_manager_callbacks::{ProgressMessage, capture, dispose};
use crate::{CancellationToken, SolverCallbackFailure, SolverJobCallbacks, SolverProgress};
use std::{
    cell::Cell,
    collections::{BTreeMap, VecDeque},
    error::Error,
    fmt, io,
    sync::{Arc, Condvar, Mutex, Weak},
    thread::{self, JoinHandle},
    time::Duration,
};

thread_local! {
    static CURRENT_POOL: Cell<Option<usize>> = const { Cell::new(None) };
}
struct PoolThreadGuard(Option<usize>);
impl PoolThreadGuard {
    fn enter<R, E>(shared: &Shared<R, E>) -> Self {
        Self(CURRENT_POOL.replace(Some(std::ptr::from_ref(shared) as usize)))
    }
}
impl Drop for PoolThreadGuard {
    fn drop(&mut self) {
        CURRENT_POOL.set(self.0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverStatus {
    Scheduled,
    Active,
    NotSolving,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitError {
    Closed,
    AlreadySolving,
}
impl fmt::Display for SubmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Closed => "solver manager is closed",
            Self::AlreadySolving => "problem ID is already solving or scheduled",
        })
    }
}
impl Error for SubmitError {}
#[derive(Debug)]
pub enum SolverJobError<E> {
    /// The job never ran; its captured input is dropped rather than solved.
    CancelledBeforeStart,
    Solve(E),
    /// Unwinding panics are captured. Abort/OOM cannot be recovered.
    Panicked(String),
}
impl<E: fmt::Display> fmt::Display for SolverJobError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CancelledBeforeStart => f.write_str("job cancelled before solving started"),
            Self::Solve(e) => write!(f, "solver failed: {e}"),
            Self::Panicked(s) => write!(f, "solver panicked: {s}"),
        }
    }
}
impl<E: Error + 'static> Error for SolverJobError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Solve(e) => Some(e),
            _ => None,
        }
    }
}
pub type SolverJobResult<R, E> = Arc<Result<R, SolverJobError<E>>>;
struct JobData<R, E> {
    status: SolverStatus,
    result: Option<SolverJobResult<R, E>>,
    failures: Vec<SolverCallbackFailure>,
    statistics: crate::statistics::StatisticsState,
}
struct JobCell<R, E> {
    data: Mutex<JobData<R, E>>,
    ready: Condvar,
    cancellation: CancellationToken,
}
type SolveTask<R, E> =
    Box<dyn FnOnce(Option<R>, CancellationToken, SolverProgress<R>) -> Result<R, E> + Send>;
struct Task<R, E> {
    id: u64,
    cell: Arc<JobCell<R, E>>,
    input: Option<R>,
    run: SolveTask<R, E>,
    callbacks: SolverJobCallbacks<R, E>,
}
struct Delivery<R, E> {
    id: u64,
    cell: Arc<JobCell<R, E>>,
    state: Mutex<DeliveryState<R, E>>,
}
struct DeliveryState<R, E> {
    // Exactly one queue entry or consumer owns delivery while scheduled is true.
    scheduled: bool,
    closed: bool,
    listening: bool,
    latest: Option<(R, Option<crate::SolverEventProducer>)>,
    initialized: Option<(R, crate::SolverInitialization)>,
    initialized_available: bool,
    initialized_before_latest: bool,
    result: Option<Result<R, SolverJobError<E>>>,
    callbacks: Option<SolverJobCallbacks<R, E>>,
    failures: Vec<SolverCallbackFailure>,
}
struct Queue<R, E> {
    closed: bool,
    running: usize,
    completions: VecDeque<Arc<Delivery<R, E>>>,
    tasks: VecDeque<Task<R, E>>,
    jobs: BTreeMap<u64, Arc<JobCell<R, E>>>,
}
struct Shared<R, E> {
    queue: Mutex<Queue<R, E>>,
    available: Condvar,
    completed: Condvar,
}
impl<R, E> Shared<R, E> {
    // Lock order is queue -> delivery or queue -> job. User code and captured input destruction
    // run outside both locks. Handles never acquire queue while holding job.
    fn finish_locked(
        queue: &mut Queue<R, E>,
        id: u64,
        cell: &JobCell<R, E>,
        result: Result<R, SolverJobError<E>>,
        failures: Vec<SolverCallbackFailure>,
    ) {
        let mut data = cell.data.lock().unwrap();
        queue.jobs.remove(&id);
        data.statistics.finish();
        data.status = SolverStatus::NotSolving;
        data.result = Some(Arc::new(result));
        data.failures = failures;
        cell.ready.notify_all();
    }
    fn cancel(&self, id: u64, expected: Option<&Arc<JobCell<R, E>>>) -> bool {
        let mut queue = self.queue.lock().unwrap();
        let Some(cell) = queue.jobs.get(&id).cloned() else {
            return false;
        };
        if expected.is_some_and(|expected| !Arc::ptr_eq(expected, &cell)) {
            return false;
        }
        let first = cell.cancellation.cancel();
        let mut removed = queue
            .tasks
            .iter()
            .position(|task| task.id == id)
            .and_then(|index| queue.tasks.remove(index));
        if let Some(task) = &mut removed {
            Self::finish_locked(
                &mut queue,
                id,
                &cell,
                task.input
                    .take()
                    .ok_or(SolverJobError::CancelledBeforeStart),
                Vec::new(),
            );
        }
        drop(queue);
        drop(removed);
        first
    }
    fn close(&self) {
        let mut queue = self.queue.lock().unwrap();
        queue.closed = true;
        for cell in queue.jobs.values() {
            cell.cancellation.cancel();
        }
        let mut tasks = std::mem::take(&mut queue.tasks);
        for task in &mut tasks {
            Self::finish_locked(
                &mut queue,
                task.id,
                &task.cell,
                task.input
                    .take()
                    .ok_or(SolverJobError::CancelledBeforeStart),
                Vec::new(),
            );
        }
        self.available.notify_all();
        self.completed.notify_all();
        drop(queue);
        drop(tasks);
    }
}

/// A cloneable result handle. Results are shared immutably without requiring the
/// solution or error to implement Clone. Dropping a handle does not cancel its job.
/// A completed handle remains attached to its original run if an ID is reused.
pub struct SolverJob<R, E> {
    id: u64,
    cell: Arc<JobCell<R, E>>,
    manager: Weak<Shared<R, E>>,
}
impl<R, E> Clone for SolverJob<R, E> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            cell: self.cell.clone(),
            manager: self.manager.clone(),
        }
    }
}
impl<R, E> SolverJob<R, E> {
    pub fn problem_id(&self) -> u64 {
        self.id
    }
    /// Snapshot actual reported work and model size, with active solve time.
    /// Queue/started-callback/consumer delays are excluded; unknown work is None.
    pub fn statistics(&self) -> crate::SolverJobStatistics {
        self.cell.data.lock().unwrap().statistics.read()
    }
    pub fn status(&self) -> SolverStatus {
        self.cell.data.lock().unwrap().status
    }
    /// Final callback diagnostics are published with the result. Empty while the
    /// solve/callback pipeline is still active; use a result wait before inspecting.
    pub fn callback_failures(&self) -> Vec<SolverCallbackFailure> {
        self.cell.data.lock().unwrap().failures.clone()
    }
    pub fn try_result(&self) -> Option<SolverJobResult<R, E>> {
        self.cell.data.lock().unwrap().result.clone()
    }
    pub fn wait(&self) -> SolverJobResult<R, E> {
        let data = self.cell.data.lock().unwrap();
        self.cell
            .ready
            .wait_while(data, |d| d.result.is_none())
            .unwrap()
            .result
            .as_ref()
            .unwrap()
            .clone()
    }
    /// A timeout leaves the job running and does not consume its result.
    pub fn wait_timeout(&self, timeout: Duration) -> Option<SolverJobResult<R, E>> {
        let data = self.cell.data.lock().unwrap();
        self.cell
            .ready
            .wait_timeout_while(data, timeout, |d| d.result.is_none())
            .unwrap()
            .0
            .result
            .clone()
    }
    /// Whether cancellation was requested while this run was still registered.
    /// Retained after completion; cancelling a stale handle does not change it.
    pub fn is_termination_requested(&self) -> bool {
        self.cell.cancellation.is_cancelled()
    }
    /// Request cancellation and wait for the result, including callback completion.
    /// Queued owned-input jobs return that input without invoking user callbacks.
    /// From this pool's solver/consumer threads, an unfinished result returns
    /// WouldBlock after requesting cancellation, avoiding a pool-thread deadlock.
    /// Arbitrary uncooperative user code can still delay an external caller forever.
    pub fn cancel_and_wait(&self) -> io::Result<SolverJobResult<R, E>> {
        self.cancel();
        if let Some(result) = self.try_result() {
            return Ok(result);
        }
        if self
            .manager
            .upgrade()
            .is_some_and(|m| CURRENT_POOL.get() == Some(Arc::as_ptr(&m) as usize))
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "cannot wait for an unfinished job from its own solver manager pool",
            ));
        }
        Ok(self.wait())
    }
    /// Scheduled jobs are removed immediately, returning the original input for
    /// submit_owned and CancelledBeforeStart otherwise. Active jobs receive a cooperative
    /// request and remain Active through solve and callback completion. Returns true only
    /// for the first request on a live run; stale handles cannot cancel a new run.
    pub fn cancel(&self) -> bool {
        self.manager
            .upgrade()
            .is_some_and(|m| m.cancel(self.id, Some(&self.cell)))
    }
}

/// The pool function executed by a managed thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SolverThreadRole {
    Solver,
    Consumer,
}
/// Factory calls occur synchronously on the constructor's thread, in solver-then-
/// consumer order. Indices start at zero independently for each role.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SolverThreadSpec {
    pub role: SolverThreadRole,
    pub index: usize,
}
impl SolverThreadSpec {
    pub fn default_name(self) -> String {
        let role = match self.role {
            SolverThreadRole::Solver => "solver",
            SolverThreadRole::Consumer => "consumer",
        };
        format!("timetuck-{role}-{}", self.index)
    }
}
/// Run exactly once on the newly spawned thread. A factory must return its handle
/// promptly, and must not execute this long-lived task inline in the constructor.
pub type SolverThreadTask = Box<dyn FnOnce() + Send + 'static>;

/// A fixed number of solver threads execute owned solve functions. Separate
/// consumer threads deliver best/final/error callbacks (one by default). Each job gets
/// an independent cancellation token; pass it into the existing solver config.
/// Build thread-local score networks inside the function, so closures in those
/// networks do not need Send. Only the captured input and returned result cross
/// threads. IDs are native u64 values and are unique among queued/active jobs.
///
/// Submission is nonblocking (the FIFO queue is not size-limited). Completion
/// releases the ID and wakes all result waiters atomically, after final/error
/// callback delivery. Active includes jobs awaiting or executing that delivery. Failed/panicking jobs
/// also release their slot. No user solver runs under the manager's locks.
///
/// Drop requests cancellation, removes queued jobs and detaches workers, without
/// waiting for potentially uncooperative code. Call shutdown to join workers.
/// Cancellation does not preempt user code; active jobs may still return success.
/// Do not block a worker waiting for work queued to the same saturated pool,
/// or a callback waiting for its own final result. Explicit shutdown drains
/// callbacks from started jobs even after cancellation is requested.
///
/// ```
/// use timetuck::SolverManager;
/// let mut manager = SolverManager::<u64, String>::new(2)?;
/// let job = manager.submit(7, |_cancel| Ok(42))?;
/// assert_eq!(job.wait().as_ref().as_ref().unwrap(), &42);
/// manager.shutdown()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SolverManager<R, E> {
    shared: Arc<Shared<R, E>>,
    workers: Vec<JoinHandle<()>>,
}
impl<R: Send + Sync + 'static, E: Send + Sync + 'static> SolverManager<R, E> {
    /// Create a solver pool with one separate callback consumer thread.
    pub fn new(parallel_solver_count: usize) -> io::Result<Self> {
        Self::with_consumer_count(parallel_solver_count, 1)
    }
    pub fn with_consumer_count(
        parallel_solver_count: usize,
        consumer_count: usize,
    ) -> io::Result<Self> {
        Self::with_thread_factory(parallel_solver_count, consumer_count, |spec, task| {
            thread::Builder::new().name(spec.default_name()).spawn(task)
        })
    }
    /// Supply native thread creation, including names, stack sizes and wrappers.
    /// Every successful call must spawn the supplied task once and return that
    /// thread's JoinHandle. The factory runs without manager locks and may borrow
    /// caller-local state. Returned errors preserve their kind/message; unwinding
    /// panics during a factory call become io::ErrorKind::Other. Both paths close and join all threads
    /// whose handles were already returned. The factory owns cleanup for any
    /// thread it starts without returning its handle.
    ///
    /// ```
    /// use timetuck::SolverManager;
    /// let mut manager = SolverManager::<String, String>::with_thread_factory(1, 1,
    ///     |spec, task| std::thread::Builder::new()
    ///         .name(format!("app-{}", spec.default_name()))
    ///         .stack_size(2 * 1024 * 1024)
    ///         .spawn(task))?;
    /// let job = manager.submit(1, |_| Ok(std::thread::current().name().unwrap().to_owned()))?;
    /// assert_eq!(job.wait().as_ref().as_ref().unwrap(), "app-timetuck-solver-0");
    /// manager.shutdown()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn with_thread_factory(
        parallel_solver_count: usize,
        consumer_count: usize,
        mut factory: impl FnMut(SolverThreadSpec, SolverThreadTask) -> io::Result<JoinHandle<()>>,
    ) -> io::Result<Self> {
        if parallel_solver_count == 0 || consumer_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "solver and consumer counts must be positive",
            ));
        }
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue {
                closed: false,
                running: 0,
                completions: VecDeque::new(),
                tasks: VecDeque::new(),
                jobs: BTreeMap::new(),
            }),
            available: Condvar::new(),
            completed: Condvar::new(),
        });
        let mut manager = Self {
            shared,
            workers: Vec::new(),
        };
        manager
            .workers
            .try_reserve_exact(
                parallel_solver_count
                    .checked_add(consumer_count)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidInput, "worker count overflow")
                    })?,
            )
            .map_err(io::Error::other)?;
        for (role, count) in [
            (SolverThreadRole::Solver, parallel_solver_count),
            (SolverThreadRole::Consumer, consumer_count),
        ] {
            for index in 0..count {
                let shared = manager.shared.clone();
                let task: SolverThreadTask = match role {
                    SolverThreadRole::Solver => Box::new(move || worker(shared)),
                    SolverThreadRole::Consumer => Box::new(move || consumer(shared)),
                };
                let result = capture(|| factory(SolverThreadSpec { role, index }, task))
                    .unwrap_or_else(|message| {
                        Err(io::Error::other(format!(
                            "solver thread factory panicked: {message}"
                        )))
                    });
                match result {
                    Ok(worker) => manager.workers.push(worker),
                    Err(error) => {
                        let _ = manager.shutdown();
                        return Err(error);
                    }
                }
            }
        }
        Ok(manager)
    }
    pub fn submit(
        &self,
        problem_id: u64,
        run: impl FnOnce(CancellationToken) -> Result<R, E> + Send + 'static,
    ) -> Result<SolverJob<R, E>, SubmitError> {
        self.submit_with_callbacks(problem_id, run, SolverJobCallbacks::default())
    }
    pub fn submit_with_callbacks(
        &self,
        problem_id: u64,
        run: impl FnOnce(CancellationToken) -> Result<R, E> + Send + 'static,
        callbacks: SolverJobCallbacks<R, E>,
    ) -> Result<SolverJob<R, E>, SubmitError> {
        self.submit_with_progress(problem_id, move |token, _| run(token), callbacks)
    }
    /// Run a job with an explicit publisher for owned best-solution snapshots.
    /// The producer selects improvements; the manager does not compare scores.
    /// Updates coalesce while a consumer is busy. Delivery is serial per job,
    /// before final/error completion, and never blocks solving on the callback.
    pub fn submit_with_progress(
        &self,
        problem_id: u64,
        run: impl FnOnce(CancellationToken, SolverProgress<R>) -> Result<R, E> + Send + 'static,
        callbacks: SolverJobCallbacks<R, E>,
    ) -> Result<SolverJob<R, E>, SubmitError> {
        self.enqueue(
            problem_id,
            None,
            None,
            move |_, token, progress| run(token, progress),
            callbacks,
        )
    }
    /// Make computed model size available immediately, including while queued.
    /// The solve function reports its actual work through SolverProgress.
    ///
    /// ```
    /// use timetuck::*;
    /// let p = FiniteProblem::new(vec![Variable::new(vec![7i64, 9])])?;
    /// let mut manager = SolverManager::<Vec<Option<i64>>, String>::new(1)?;
    /// let job = manager.submit_with_statistics(1, p.problem_size_statistics(),
    ///     move |token, progress| {
    ///         let mut score = ConstraintNetwork::new(1);
    ///         score.add_constraint("value", vec![0], |v| SimpleScore::new(v[0].copied().unwrap_or(0))).unwrap();
    ///         let result = solve(&p, &score, SolverConfig {
    ///             limits: SolverLimits { cancellation: Some(&token), ..Default::default() },
    ///             local_search: vec![], ..Default::default()
    ///         }).map_err(|e| e.to_string())?;
    ///         progress.record_work(result.work);
    ///         Ok(result.assignment.values().to_vec())
    ///     }, SolverJobCallbacks::default())?;
    /// assert!(job.wait().is_ok());
    /// let statistics = job.statistics();
    /// assert_eq!(statistics.work.unwrap().evaluations, 3);
    /// assert_eq!(statistics.problem_size.unwrap().size.assignment_space.decimal(), "2");
    /// manager.shutdown()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn submit_with_statistics(
        &self,
        problem_id: u64,
        problem_size: crate::ProblemSizeStatistics,
        run: impl FnOnce(CancellationToken, SolverProgress<R>) -> Result<R, E> + Send + 'static,
        callbacks: SolverJobCallbacks<R, E>,
    ) -> Result<SolverJob<R, E>, SubmitError> {
        self.enqueue(
            problem_id,
            None,
            Some(problem_size),
            move |_, token, progress| run(token, progress),
            callbacks,
        )
    }
    /// Submit an owned input whose type is also the result type. If cancelled
    /// while queued (including shutdown), return this exact input as success,
    /// without cloning, solving or invoking callbacks. Once a worker starts,
    /// cancellation is cooperative and the solve function determines the result.
    /// Rejected submissions still drop their input, like the other submit APIs.
    ///
    /// ```
    /// use std::sync::{Arc, mpsc};
    /// use timetuck::{SolverManager, SolverJobCallbacks};
    /// let mut manager = SolverManager::<Arc<Vec<i64>>, String>::new(1)?;
    /// let (started, ready) = mpsc::channel();
    /// let (release, resume) = mpsc::channel();
    /// let first = manager.submit(1, move |_| {
    ///     started.send(()).unwrap();
    ///     resume.recv().unwrap();
    ///     Ok(Arc::new(vec![]))
    /// })?;
    /// ready.recv()?;
    /// let input = Arc::new(vec![1, 2]);
    /// let queued = manager.submit_owned(2, input.clone(),
    ///     |problem, _, _| Ok(problem), SolverJobCallbacks::default())?;
    /// let result = queued.cancel_and_wait()?;
    /// assert!(Arc::ptr_eq(result.as_ref().as_ref().unwrap(), &input));
    /// assert!(queued.is_termination_requested());
    /// release.send(())?;
    /// assert!(first.wait().is_ok());
    /// manager.shutdown()?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn submit_owned(
        &self,
        problem_id: u64,
        input: R,
        run: impl FnOnce(R, CancellationToken, SolverProgress<R>) -> Result<R, E> + Send + 'static,
        callbacks: SolverJobCallbacks<R, E>,
    ) -> Result<SolverJob<R, E>, SubmitError> {
        self.enqueue(
            problem_id,
            Some(input),
            None,
            move |input, token, progress| {
                run(input.expect("owned submission has input"), token, progress)
            },
            callbacks,
        )
    }
    fn enqueue(
        &self,
        problem_id: u64,
        input: Option<R>,
        problem_size: Option<crate::ProblemSizeStatistics>,
        run: impl FnOnce(Option<R>, CancellationToken, SolverProgress<R>) -> Result<R, E>
        + Send
        + 'static,
        callbacks: SolverJobCallbacks<R, E>,
    ) -> Result<SolverJob<R, E>, SubmitError> {
        let mut queue = self.shared.queue.lock().unwrap();
        if queue.closed {
            return Err(SubmitError::Closed);
        }
        if queue.jobs.contains_key(&problem_id) {
            return Err(SubmitError::AlreadySolving);
        }
        let cell = Arc::new(JobCell {
            data: Mutex::new(JobData {
                status: SolverStatus::Scheduled,
                result: None,
                failures: Vec::new(),
                statistics: crate::statistics::StatisticsState::new(problem_size),
            }),
            ready: Condvar::new(),
            cancellation: CancellationToken::new(),
        });
        queue.jobs.insert(problem_id, cell.clone());
        queue.tasks.push_back(Task {
            id: problem_id,
            cell: cell.clone(),
            input,
            run: Box::new(run),
            callbacks,
        });
        self.shared.available.notify_one();
        Ok(SolverJob {
            id: problem_id,
            cell,
            manager: Arc::downgrade(&self.shared),
        })
    }
    /// Unknown and completed IDs are NotSolving; completed handles keep results.
    pub fn status(&self, problem_id: u64) -> SolverStatus {
        self.shared
            .queue
            .lock()
            .unwrap()
            .jobs
            .get(&problem_id)
            .map_or(SolverStatus::NotSolving, |cell| {
                cell.data.lock().unwrap().status
            })
    }
    pub fn terminate_early(&self, problem_id: u64) -> bool {
        self.shared.cancel(problem_id, None)
    }
    /// Cancel the currently registered run for this ID and wait through delivery.
    /// Returns None for an unknown/completed ID. ID reuse cannot redirect the wait.
    /// As with SolverJob::cancel_and_wait, pool-thread waits may return WouldBlock.
    pub fn terminate_early_and_wait(
        &self,
        problem_id: u64,
    ) -> io::Result<Option<SolverJobResult<R, E>>> {
        let job = self
            .shared
            .queue
            .lock()
            .unwrap()
            .jobs
            .get(&problem_id)
            .map(|cell| SolverJob {
                id: problem_id,
                cell: cell.clone(),
                manager: Arc::downgrade(&self.shared),
            });
        job.map(|job| job.cancel_and_wait()).transpose()
    }
    /// Reject new submissions, cancel queued jobs, and request active cancellation.
    /// This method does not wait; use shutdown or individual job result handles.
    pub fn request_shutdown(&self) {
        self.shared.close();
    }
    /// Request shutdown and wait for every worker to exit. Calling from a worker
    /// of this same pool returns WouldBlock after requesting shutdown, avoiding a
    /// self-join. Uncooperative jobs can delay shutdown indefinitely.
    pub fn shutdown(&mut self) -> io::Result<()> {
        self.request_shutdown();
        if self
            .workers
            .iter()
            .any(|w| w.thread().id() == thread::current().id())
        {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "cannot join solver manager from its own worker",
            ));
        }
        let mut panicked = false;
        for worker in self.workers.drain(..) {
            panicked |= worker.join().is_err();
        }
        if panicked {
            Err(io::Error::other("solver manager worker panicked"))
        } else {
            Ok(())
        }
    }
}
impl<R, E> Drop for SolverManager<R, E> {
    fn drop(&mut self) {
        self.shared.close();
    }
}
fn worker<R: Send + Sync + 'static, E: Send + Sync + 'static>(shared: Arc<Shared<R, E>>) {
    let _pool = PoolThreadGuard::enter(shared.as_ref());
    loop {
        let task = {
            let queue = shared.queue.lock().unwrap();
            let mut queue = shared
                .available
                .wait_while(queue, |q| q.tasks.is_empty() && !q.closed)
                .unwrap();
            let Some(task) = queue.tasks.pop_front() else {
                return;
            };
            queue.running += 1;
            task.cell.data.lock().unwrap().status = SolverStatus::Active;
            task
        };
        let Task {
            id,
            cell,
            input,
            run,
            mut callbacks,
        } = task;
        let dispatch = !callbacks.is_empty();
        let failures = callbacks.start(id);
        let delivery = Arc::new(Delivery {
            id,
            cell: cell.clone(),
            state: Mutex::new(DeliveryState {
                scheduled: false,
                closed: false,
                listening: callbacks.has_best(),
                latest: None,
                initialized: None,
                initialized_available: callbacks.has_initialized(),
                initialized_before_latest: true,
                result: None,
                callbacks: Some(callbacks),
                failures,
            }),
        });
        let manager = Arc::downgrade(&shared);
        let target = Arc::downgrade(&delivery);
        let progress = SolverProgress::new(move |message| {
            let (Some(shared), Some(delivery)) = (manager.upgrade(), target.upgrade()) else {
                dispose(message);
                return false;
            };
            if let ProgressMessage::Statistics(update) = message {
                return delivery.cell.data.lock().unwrap().statistics.update(update);
            }
            let mut queue = shared.queue.lock().unwrap();
            let mut state = delivery.state.lock().unwrap();
            let listening = match &message {
                ProgressMessage::Best(..) => state.listening,
                ProgressMessage::Initialized(..) => state.initialized_available,
                ProgressMessage::Statistics(_) => unreachable!("handled before delivery"),
            };
            if state.closed || !listening {
                drop(state);
                drop(queue);
                dispose(message);
                return false;
            }
            let old = match message {
                ProgressMessage::Statistics(_) => unreachable!("handled before delivery"),
                ProgressMessage::Best(snapshot, producer) => {
                    state.initialized_before_latest = true;
                    state.latest.replace((snapshot, producer))
                }
                ProgressMessage::Initialized(snapshot, initialization) => {
                    state.initialized_available = false;
                    state.initialized_before_latest = state.latest.is_none();
                    state.initialized = Some((snapshot, initialization));
                    None
                }
            };
            if !state.scheduled {
                state.scheduled = true;
                queue.completions.push_back(delivery.clone());
                shared.completed.notify_one();
            }
            drop(state);
            drop(queue);
            dispose(old);
            true
        });
        cell.data.lock().unwrap().statistics.start();
        let result = match capture(|| run(input, cell.cancellation.clone(), progress)) {
            Ok(result) => result.map_err(SolverJobError::Solve),
            Err(message) => Err(SolverJobError::Panicked(message)),
        };
        cell.data.lock().unwrap().statistics.finish();
        let mut queue = shared.queue.lock().unwrap();
        queue.running -= 1;
        let mut state = delivery.state.lock().unwrap();
        state.closed = true;
        if dispatch {
            state.result = Some(result);
            if !state.scheduled {
                state.scheduled = true;
                queue.completions.push_back(delivery.clone());
            }
        } else {
            Shared::finish_locked(&mut queue, id, &cell, result, Vec::new());
        }
        shared.completed.notify_all();
        drop(state);
        drop(queue);
        dispose(delivery);
        dispose(cell);
    }
}
fn consumer<R: Send + Sync + 'static, E: Send + Sync + 'static>(shared: Arc<Shared<R, E>>) {
    let _pool = PoolThreadGuard::enter(shared.as_ref());
    loop {
        let delivery = {
            let queue = shared.queue.lock().unwrap();
            let mut queue = shared
                .completed
                .wait_while(queue, |q| {
                    q.completions.is_empty() && !(q.closed && q.running == 0)
                })
                .unwrap();
            let Some(delivery) = queue.completions.pop_front() else {
                return;
            };
            delivery
        };
        let (mut callbacks, mut failures, latest, mut initialized, initialized_first, result) = {
            let mut state = delivery.state.lock().unwrap();
            (
                state.callbacks.take().unwrap(),
                std::mem::take(&mut state.failures),
                state.latest.take(),
                state.initialized.take(),
                state.initialized_before_latest,
                state.result.take(),
            )
        };
        if initialized_first && let Some((snapshot, initialization)) = initialized.take() {
            callbacks.initialized(delivery.id, &snapshot, initialization, &mut failures);
            dispose(snapshot);
        }
        if let Some((snapshot, producer)) = latest {
            callbacks.best(delivery.id, &snapshot, producer, &mut failures);
            dispose(snapshot);
        }
        if let Some((snapshot, initialization)) = initialized {
            callbacks.initialized(delivery.id, &snapshot, initialization, &mut failures);
            dispose(snapshot);
        }
        if let Some(result) = result {
            callbacks.finish(delivery.id, &result, &mut failures);
            dispose(callbacks);
            Shared::finish_locked(
                &mut shared.queue.lock().unwrap(),
                delivery.id,
                &delivery.cell,
                result,
                failures,
            );
        } else {
            let mut queue = shared.queue.lock().unwrap();
            let mut state = delivery.state.lock().unwrap();
            state.listening = callbacks.has_best();
            state.callbacks = Some(callbacks);
            state.failures = failures;
            // A producer may have published or finished while the callback ran.
            // Requeue at the back for fairness, without concurrent job callbacks.
            if state.latest.is_some() || state.initialized.is_some() || state.result.is_some() {
                queue.completions.push_back(delivery.clone());
                shared.completed.notify_all();
            } else {
                state.scheduled = false;
            }
        }
        dispose(delivery);
    }
}
