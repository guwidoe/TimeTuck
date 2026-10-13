//! Optional-input boundary for actual native custom-move test sessions.
use crate::{MoveDraft, MoveDraftModel, MoveTestContext, MoveTestError, MoveTester};
use std::{error::Error, fmt};

/// Missing Rust optional inputs are rejected before model validation or callbacks.
/// Diagnostic argument names mirror their meaning; no Java exception type or
/// reflection behavior is implied. Model/execution errors retain their originals.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedMoveTestError<E> {
    MissingArgument(&'static str),
    Model(E),
    Execution(MoveTestError<E>),
}
impl<E: fmt::Debug> fmt::Display for CheckedMoveTestError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingArgument(name) => write!(f, "{name} is required"),
            Self::Model(error) => write!(f, "solution validation: {error:?}"),
            Self::Execution(error) => error.fmt(f),
        }
    }
}
impl<E: fmt::Debug> Error for CheckedMoveTestError<E> {}

/// Accept possibly absent input from application boundaries, then delegate to
/// MoveTester. The model remains borrowed; temporary execution retains the native
/// isolated-draft rollback and callback-panic semantics. External callback side
/// effects are not transactional, and scoring/shadows are not computed here.
pub struct CheckedMoveTester<'p, P: MoveDraftModel> {
    tester: MoveTester<'p, P>,
}
impl<'p, P: MoveDraftModel> CheckedMoveTester<'p, P> {
    pub fn new(problem: &'p P) -> Self {
        Self {
            tester: MoveTester::new(problem),
        }
    }
    pub fn problem(&self) -> &'p P {
        self.tester.problem()
    }
    pub fn using<'a>(
        &self,
        solution: Option<&'a mut P::Assignment>,
    ) -> Result<CheckedMoveTestContext<'p, 'a, P>, CheckedMoveTestError<P::Error>> {
        let solution = solution.ok_or(CheckedMoveTestError::MissingArgument("solution"))?;
        let context = self
            .tester
            .using(solution)
            .map_err(CheckedMoveTestError::Model)?;
        Ok(CheckedMoveTestContext { context })
    }
}
pub struct CheckedMoveTestContext<'p, 'a, P: MoveDraftModel> {
    context: MoveTestContext<'p, 'a, P>,
}
impl<'p, P: MoveDraftModel> CheckedMoveTestContext<'p, '_, P> {
    pub fn assignment(&self) -> &P::Assignment {
        self.context.assignment()
    }
    pub fn execute<F>(
        &mut self,
        movement: Option<F>,
    ) -> Result<P::Movement, CheckedMoveTestError<P::Error>>
    where
        F: FnOnce(&mut MoveDraft<'p, P>) -> Result<(), P::Error>,
    {
        let movement = movement.ok_or(CheckedMoveTestError::MissingArgument("move"))?;
        self.context
            .execute(movement)
            .map_err(CheckedMoveTestError::Execution)
    }
    pub fn execute_temporarily<F, C, R>(
        &mut self,
        movement: Option<F>,
        callback: Option<C>,
    ) -> Result<R, CheckedMoveTestError<P::Error>>
    where
        F: FnOnce(&mut MoveDraft<'p, P>) -> Result<(), P::Error>,
        C: FnOnce(&mut MoveDraft<'p, P>) -> R,
    {
        let movement = movement.ok_or(CheckedMoveTestError::MissingArgument("move"))?;
        let callback = callback.ok_or(CheckedMoveTestError::MissingArgument("callback"))?;
        self.context
            .execute_temporarily(movement, callback)
            .map_err(CheckedMoveTestError::Execution)
    }
}
