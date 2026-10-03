//! Asynchronous, independent solves on a fixed-size native worker pool.
use crate::solver_manager_callbacks::{capture, dispose};
use crate::{CancellationToken, SolverCallbackFailure, SolverJobCallbacks};
use std::{
    collections::{BTreeMap, VecDeque},
    error::Error,
    fmt, io,
    sync::{Arc, Condvar, Mutex, Weak},
    thread::{self, JoinHandle},
    time::Duration,
};

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
}
struct JobCell<R, E> {
    data: Mutex<JobData<R, E>>,
    ready: Condvar,
    cancellation: CancellationToken,
}
struct Task<R, E> {
    id: u64,
    cell: Arc<JobCell<R, E>>,
    run: Box<dyn FnOnce(CancellationToken) -> Result<R, E> + Send>,
    callbacks: SolverJobCallbacks<R, E>,
}
struct Completion<R, E> {
    id: u64,
    cell: Arc<JobCell<R, E>>,
    result: Result<R, SolverJobError<E>>,
    callbacks: SolverJobCallbacks<R, E>,
    failures: Vec<SolverCallbackFailure>,
}
struct Queue<R, E> {
    closed: bool,
    running: usize,
    completions: VecDeque<Completion<R, E>>,
    tasks: VecDeque<Task<R, E>>,
    jobs: BTreeMap<u64, Arc<JobCell<R, E>>>,
}
struct Shared<R, E> {
    queue: Mutex<Queue<R, E>>,
    available: Condvar,
    completed: Condvar,
}
impl<R, E> Shared<R, E> {
    // Lock order is always queue -> job. User code and captured input destruction
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
        let removed = queue
            .tasks
            .iter()
            .position(|task| task.id == id)
            .and_then(|index| queue.tasks.remove(index));
        if removed.is_some() {
            Self::finish_locked(
                &mut queue,
                id,
                &cell,
                Err(SolverJobError::CancelledBeforeStart),
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
        let tasks = std::mem::take(&mut queue.tasks);
        for task in &tasks {
            Self::finish_locked(
                &mut queue,
                task.id,
                &task.cell,
                Err(SolverJobError::CancelledBeforeStart),
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
    /// Scheduled jobs are removed immediately. Active jobs receive a cooperative
    /// request and remain Active until their function returns. Returns true only
    /// for the first request on a live run; stale handles cannot cancel a new run.
    pub fn cancel(&self) -> bool {
        self.manager
            .upgrade()
            .is_some_and(|m| m.cancel(self.id, Some(&self.cell)))
    }
}

/// A fixed number of solver threads execute owned solve functions. Separate
/// consumer threads deliver final/error callbacks (one by default). Each job gets
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
        for index in 0..parallel_solver_count {
            let shared = manager.shared.clone();
            match thread::Builder::new()
                .name(format!("timetuck-solver-{index}"))
                .spawn(move || worker(shared))
            {
                Ok(worker) => manager.workers.push(worker),
                Err(error) => {
                    let _ = manager.shutdown();
                    return Err(error);
                }
            }
        }
        for index in 0..consumer_count {
            let shared = manager.shared.clone();
            match thread::Builder::new()
                .name(format!("timetuck-consumer-{index}"))
                .spawn(move || consumer(shared))
            {
                Ok(worker) => manager.workers.push(worker),
                Err(error) => {
                    let _ = manager.shutdown();
                    return Err(error);
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
            }),
            ready: Condvar::new(),
            cancellation: CancellationToken::new(),
        });
        queue.jobs.insert(problem_id, cell.clone());
        queue.tasks.push_back(Task {
            id: problem_id,
            cell: cell.clone(),
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
            run,
            mut callbacks,
        } = task;
        let dispatch = !callbacks.is_empty();
        let failures = callbacks.start(id);
        let result = match capture(|| run(cell.cancellation.clone())) {
            Ok(result) => result.map_err(SolverJobError::Solve),
            Err(message) => Err(SolverJobError::Panicked(message)),
        };
        let mut queue = shared.queue.lock().unwrap();
        queue.running -= 1;
        if dispatch {
            queue.completions.push_back(Completion {
                id,
                cell,
                result,
                callbacks,
                failures,
            });
            shared.completed.notify_all();
        } else {
            Shared::finish_locked(&mut queue, id, &cell, result, failures);
            shared.completed.notify_all();
            drop(queue);
            dispose(cell);
        }
    }
}
fn consumer<R: Send + Sync + 'static, E: Send + Sync + 'static>(shared: Arc<Shared<R, E>>) {
    loop {
        let completion = {
            let queue = shared.queue.lock().unwrap();
            let mut queue = shared
                .completed
                .wait_while(queue, |q| {
                    q.completions.is_empty() && !(q.closed && q.running == 0)
                })
                .unwrap();
            let Some(completion) = queue.completions.pop_front() else {
                return;
            };
            completion
        };
        let Completion {
            id,
            cell,
            result,
            mut callbacks,
            mut failures,
        } = completion;
        callbacks.finish(id, &result, &mut failures);
        dispose(callbacks);
        Shared::finish_locked(
            &mut shared.queue.lock().unwrap(),
            id,
            &cell,
            result,
            failures,
        );
        dispose(cell);
    }
}
