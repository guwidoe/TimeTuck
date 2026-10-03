//! Owned job callbacks and contained callback failures.
use crate::SolverJobError;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolverCallbackStage {
    Started,
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
type Final<R> = Box<dyn FnOnce(u64, &R) + Send>;
type Handler<E> = Box<dyn for<'a> FnMut(u64, SolverJobFailure<'a, E>) + Send>;

/// Per-job callbacks. Started runs once on the solver thread before the solve
/// function; a panic is recorded and solving continues. Final (success only) and
/// error delivery run on the manager's consumer pool after solving. Errors are
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
    final_result: Option<Final<R>>,
    error: Option<Handler<E>>,
}
impl<R, E> Default for SolverJobCallbacks<R, E> {
    fn default() -> Self {
        Self {
            started: None,
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
        self.started.is_none() && self.final_result.is_none() && self.error.is_none()
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
    pub(crate) fn finish(
        &mut self,
        id: u64,
        result: &Result<R, SolverJobError<E>>,
        failures: &mut Vec<SolverCallbackFailure>,
    ) {
        for failure in failures.clone() {
            self.notify(id, SolverJobFailure::Callback(&failure), failures);
        }
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
