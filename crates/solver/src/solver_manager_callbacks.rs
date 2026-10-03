//! Owned job callbacks and contained callback failures.
use crate::SolverJobError;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverCallbackStage {
    Started,
    Best,
    Final,
    ErrorHandler,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SolverCallbackFailure {
    pub stage: SolverCallbackStage,
    pub message: String,
}
#[derive(Debug)]
pub enum SolverJobFailure<'a, E> {
    Solving(&'a SolverJobError<E>),
    Callback(&'a SolverCallbackFailure),
}
type Started = Box<dyn FnOnce(u64) + Send>;
type Best<R> = Box<dyn FnMut(u64, &R) + Send>;
type Final<R> = Box<dyn FnOnce(u64, &R) + Send>;
type Handler<E> = Box<dyn for<'a> FnMut(u64, SolverJobFailure<'a, E>) + Send>;

/// Per-job callbacks. Started runs once on the solver thread before the solve
/// function; a panic is recorded and solving continues. Best, final (success only)
/// and error delivery run on the manager's consumer pool, serially per job. Errors are
/// delivered once per cause; a panicking error handler is recorded and disabled.
/// Callback failures never replace the solve result. Queued cancellation invokes
/// none of these callbacks. Waits include final/error delivery; callbacks must not
/// wait on their own job or a saturated consumer pool.
///
/// ```
/// use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
/// use timetuck::{SolverManager, SolverJobCallbacks};
/// let delivered = Arc::new(AtomicUsize::new(0));
/// let seen = delivered.clone();
/// let mut manager = SolverManager::<usize, String>::new(1)?;
/// let job = manager.submit_with_callbacks(1, |_| Ok(42),
///     SolverJobCallbacks::default().on_final(move |id, value| {
///         assert_eq!(id, 1); seen.store(*value, Ordering::SeqCst);
///     }))?;
/// assert!(job.wait().is_ok());
/// assert_eq!(delivered.load(Ordering::SeqCst), 42);
/// manager.shutdown()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SolverJobCallbacks<R, E> {
    started: Option<Started>,
    started_reported: bool,
    best: Option<Best<R>>,
    final_result: Option<Final<R>>,
    error: Option<Handler<E>>,
}
impl<R, E> Default for SolverJobCallbacks<R, E> {
    fn default() -> Self {
        Self {
            started: None,
            started_reported: false,
            best: None,
            final_result: None,
            error: None,
        }
    }
}
impl<R, E> SolverJobCallbacks<R, E> {
    pub fn on_started(mut self, callback: impl FnOnce(u64) + Send + 'static) -> Self {
        self.started = Some(Box::new(callback));
        self
    }
    /// Receive explicitly published best snapshots on a consumer thread. Older
    /// waiting snapshots may be skipped. A panic disables this callback and is
    /// reported once; final/error delivery and solving continue.
    pub fn on_best(mut self, callback: impl FnMut(u64, &R) + Send + 'static) -> Self {
        self.best = Some(Box::new(callback));
        self
    }
    pub(crate) fn has_best(&self) -> bool {
        self.best.is_some()
    }
    pub fn on_final(mut self, callback: impl FnOnce(u64, &R) + Send + 'static) -> Self {
        self.final_result = Some(Box::new(callback));
        self
    }
    pub fn on_error(
        mut self,
        callback: impl for<'a> FnMut(u64, SolverJobFailure<'a, E>) + Send + 'static,
    ) -> Self {
        self.error = Some(Box::new(callback));
        self
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.started.is_none()
            && self.best.is_none()
            && self.final_result.is_none()
            && self.error.is_none()
    }
    pub(crate) fn start(&mut self, id: u64) -> Vec<SolverCallbackFailure> {
        let mut failures = Vec::new();
        if let Some(callback) = self.started.take()
            && let Err(message) = capture(|| callback(id))
        {
            failures.push(SolverCallbackFailure {
                stage: SolverCallbackStage::Started,
                message,
            });
        }
        failures
    }
    fn notify(
        &mut self,
        id: u64,
        cause: SolverJobFailure<'_, E>,
        failures: &mut Vec<SolverCallbackFailure>,
    ) {
        if let Some(callback) = self.error.as_mut()
            && let Err(message) = capture(|| callback(id, cause))
        {
            failures.push(SolverCallbackFailure {
                stage: SolverCallbackStage::ErrorHandler,
                message,
            });
            // Never recurse into a failing handler or use its mutated state again.
            dispose(self.error.take());
        }
    }
    fn report_started(&mut self, id: u64, failures: &mut Vec<SolverCallbackFailure>) {
        if !self.started_reported {
            self.started_reported = true;
            for failure in failures.clone() {
                self.notify(id, SolverJobFailure::Callback(&failure), failures);
            }
        }
    }
    pub(crate) fn best(
        &mut self,
        id: u64,
        snapshot: &R,
        failures: &mut Vec<SolverCallbackFailure>,
    ) {
        self.report_started(id, failures);
        if let Some(callback) = self.best.as_mut()
            && let Err(message) = capture(|| callback(id, snapshot))
        {
            let failure = SolverCallbackFailure {
                stage: SolverCallbackStage::Best,
                message,
            };
            failures.push(failure.clone());
            dispose(self.best.take());
            self.notify(id, SolverJobFailure::Callback(&failure), failures);
        }
    }
    pub(crate) fn finish(
        &mut self,
        id: u64,
        result: &Result<R, SolverJobError<E>>,
        failures: &mut Vec<SolverCallbackFailure>,
    ) {
        self.report_started(id, failures);
        match result {
            Err(error) => self.notify(id, SolverJobFailure::Solving(error), failures),
            Ok(value) => {
                if let Some(callback) = self.final_result.take()
                    && let Err(message) = capture(|| callback(id, value))
                {
                    let failure = SolverCallbackFailure {
                        stage: SolverCallbackStage::Final,
                        message,
                    };
                    failures.push(failure.clone());
                    self.notify(id, SolverJobFailure::Callback(&failure), failures);
                }
            }
        }
    }
}
pub(crate) fn capture<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "non-string panic payload".to_owned());
        // Contain destructor panics too. Forget only a secondary panic payload,
        // whose destructor could recursively panic if we tried to destroy it.
        if let Err(secondary) = catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            std::mem::forget(secondary);
        }
        message
    })
}
pub(crate) fn dispose<T>(value: T) {
    let _ = capture(|| drop(value));
}

/// A cloneable publisher tied to one job execution. Pass owned snapshots when the
/// solve finds a new best solution; the caller is responsible for selecting bests
/// and taking independent snapshots of mutable working state. At most one waiting
/// snapshot is retained, in addition to one currently being consumed. Publishing
/// returns false without a listener, after it is disabled, or after the solve returns.
/// A true return means accepted, not necessarily consumed: newer snapshots can
/// replace this one. Cancellation alone does not close an active publisher.
///
/// Clones do not keep the job alive and cannot publish into a later run of its ID.
/// No Clone bound is required on the solution itself. Publishing never waits for
/// a callback; disposal of replaced snapshots runs on the publishing thread.
/// The manager does not automatically publish the function's final return value.
///
/// ```
/// use timetuck::{SolverManager, SolverJobCallbacks};
/// let mut manager = SolverManager::<u64, String>::new(1)?;
/// let job = manager.submit_with_progress(1, |_, progress| {
///     assert!(progress.publish(10));
///     assert!(progress.publish(20));
///     Ok(20)
/// }, SolverJobCallbacks::default().on_best(|id, score| {
///     assert_eq!(id, 1);
///     assert!([10, 20].contains(score)); // 10 may be skipped by coalescing.
/// }))?;
/// assert_eq!(*job.wait().as_ref().as_ref().unwrap(), 20);
/// manager.shutdown()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct SolverProgress<R> {
    send: std::sync::Arc<dyn Fn(R) -> bool + Send + Sync>,
}
impl<R> SolverProgress<R> {
    pub(crate) fn new(send: impl Fn(R) -> bool + Send + Sync + 'static) -> Self {
        Self {
            send: std::sync::Arc::new(send),
        }
    }
    pub fn publish(&self, snapshot: R) -> bool {
        (self.send)(snapshot)
    }
}
impl<R> Clone for SolverProgress<R> {
    fn clone(&self) -> Self {
        Self {
            send: self.send.clone(),
        }
    }
}
