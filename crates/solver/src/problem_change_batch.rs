//! Ordered problem edits submitted as one live-solver revision.
use crate::solver_manager_callbacks::{capture, dispose};
use crate::{LiveSolverJob, LiveSolverManager, ProblemChangeReceipt, ProblemChangeSubmitError};
use std::{collections::VecDeque, fmt};

type Edit<P> = Box<dyn FnOnce(&mut P) -> Result<(), String> + Send>;

/// A group of owned edits applied in order without another queued change or solve
/// between them. Submission produces one revision and one delivery receipt.
///
/// This is not a rollback transaction: a failed or panicking edit stops the batch,
/// but earlier mutations remain. Cancellation is checked around the entire batch,
/// not between its edits. Keep callbacks short and nonblocking. Unexecuted callbacks
/// are disposed individually with destructor panics contained, outside queue locks.
/// An empty submitted batch requests a fresh solve and receipt without changing P.
#[must_use]
pub struct ProblemChangeBatch<P> {
    edits: VecDeque<Edit<P>>,
}
impl<P> Default for ProblemChangeBatch<P> {
    fn default() -> Self {
        Self {
            edits: VecDeque::new(),
        }
    }
}
impl<P> ProblemChangeBatch<P> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, edit: impl FnOnce(&mut P) -> Result<(), String> + Send + 'static) {
        self.edits.push_back(Box::new(edit));
    }
    pub fn len(&self) -> usize {
        self.edits.len()
    }
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }
    /// Apply synchronously. Errors identify the one-based edit position; panics
    /// become errors. Unexecuted edits are dropped without being called.
    pub fn apply(mut self, problem: &mut P) -> Result<(), String> {
        let mut position = 0;
        while let Some(edit) = self.edits.pop_front() {
            position += 1;
            match capture(|| edit(problem)) {
                Ok(Ok(())) => (),
                Ok(Err(message)) => return Err(format!("batch edit {position} failed: {message}")),
                Err(message) => return Err(format!("batch edit {position} panicked: {message}")),
            }
        }
        Ok(())
    }
}
impl<P: 'static> ProblemChangeBatch<P> {
    /// Queue the whole batch on a job. Completion follows normal live delivery
    /// semantics: application alone does not acknowledge the receipt.
    pub fn submit<R, E>(
        self,
        job: &LiveSolverJob<P, R, E>,
    ) -> Result<ProblemChangeReceipt, ProblemChangeSubmitError> {
        job.add_problem_change(move |problem| self.apply(problem))
    }
    /// Submit by problem identity through its manager.
    pub fn submit_to<R, E>(
        self,
        manager: &LiveSolverManager<P, R, E>,
        problem_id: u64,
    ) -> Result<ProblemChangeReceipt, ProblemChangeSubmitError>
    where
        P: Send,
        R: Send + Sync + 'static,
        E: fmt::Display + Send + Sync + 'static,
    {
        manager.add_problem_change(problem_id, move |problem| self.apply(problem))
    }
}
impl<P> Drop for ProblemChangeBatch<P> {
    fn drop(&mut self) {
        while let Some(edit) = self.edits.pop_front() {
            dispose(edit);
        }
    }
}
