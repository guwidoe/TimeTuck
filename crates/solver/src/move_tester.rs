//! Isolated custom-move drafts and commit/temporary execution across native models.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListError, ListMove, ListProblem, MixedAssignment,
    MixedError, MixedMove, MixedProblem, ModelError, Move,
};
use std::{error::Error, fmt, sync::Arc};

/// Model adapter for isolated editing. Assignments must have immutable Clone
/// semantics. Applying to a copy must not mutate the supplied assignment/model.
/// Composition must reproduce the final assignment of the ordered steps.
/// Native finite, list and mixed models retain ordinary domain/pin validation.
pub trait MoveDraftModel {
    type Assignment: Clone;
    type Movement: Clone;
    type Error;
    fn validate_draft(&self, assignment: &Self::Assignment) -> Result<(), Self::Error>;
    fn apply_to_copy(
        &self,
        assignment: &Self::Assignment,
        movement: &Self::Movement,
    ) -> Result<Self::Assignment, Self::Error>;
    fn compose(steps: Vec<Self::Movement>) -> Self::Movement;
}
impl<V: Clone + Eq> MoveDraftModel for FiniteProblem<V> {
    type Assignment = Assignment<V>;
    type Movement = Move<V>;
    type Error = ModelError;
    fn validate_draft(&self, a: &Assignment<V>) -> Result<(), ModelError> {
        self.validate(a)
    }
    fn apply_to_copy(&self, a: &Assignment<V>, m: &Move<V>) -> Result<Assignment<V>, ModelError> {
        let mut next = a.clone();
        self.apply(&mut next, m)?.commit();
        Ok(next)
    }
    fn compose(mut steps: Vec<Move<V>>) -> Move<V> {
        if steps.len() == 1 {
            steps.pop().unwrap()
        } else {
            Move::Composite(steps)
        }
    }
}
impl<V: Clone + Eq> MoveDraftModel for ListProblem<V> {
    type Assignment = ListAssignment<V>;
    type Movement = ListMove<V>;
    type Error = ListError;
    fn validate_draft(&self, a: &ListAssignment<V>) -> Result<(), ListError> {
        self.validate(a)
    }
    fn apply_to_copy(
        &self,
        a: &ListAssignment<V>,
        m: &ListMove<V>,
    ) -> Result<ListAssignment<V>, ListError> {
        let mut next = a.clone();
        self.apply(&mut next, m)?.commit();
        Ok(next)
    }
    fn compose(mut steps: Vec<ListMove<V>>) -> ListMove<V> {
        if steps.len() == 1 {
            steps.pop().unwrap()
        } else {
            ListMove::Composite(steps)
        }
    }
}
impl<B: Clone + Eq, V: Clone + Eq> MoveDraftModel for MixedProblem<B, V> {
    type Assignment = MixedAssignment<B, V>;
    type Movement = MixedMove<B, V>;
    type Error = MixedError;
    fn validate_draft(&self, a: &Self::Assignment) -> Result<(), MixedError> {
        self.validate(a)
    }
    fn apply_to_copy(
        &self,
        a: &Self::Assignment,
        m: &Self::Movement,
    ) -> Result<Self::Assignment, MixedError> {
        let mut next = a.clone();
        self.apply(&mut next, m)?.commit();
        Ok(next)
    }
    fn compose(mut steps: Vec<Self::Movement>) -> Self::Movement {
        if steps.len() == 1 {
            return steps.pop().unwrap();
        }
        let (basic, lists) = steps.into_iter().map(|m| (m.basic, m.lists)).unzip();
        MixedMove {
            basic: Move::Composite(basic),
            lists: ListMove::Composite(lists),
        }
    }
}

/// An isolated assignment and ordered successful move steps. Failed pushes leave
/// earlier steps intact. Cloning forks the draft. Coordinates refer to the current
/// draft; a composite passed to push may have temporarily invalid intermediate
/// states if the model's ordinary composite contract permits them.
///
/// finish produces a normal native move for any scoring backend; apply it only
/// to the original starting assignment. Mixed composition groups basic and list
/// steps separately, preserving each component's order and the final state.
/// This snapshot-based API does not promise constant-time editing.
///
/// ```
/// use timetuck::*;
/// let p=FiniteProblem::new(vec![Variable::new(vec![1,2]).with_initial(1)])?;
/// let a=p.initial_assignment();
/// let mut draft=MoveDraft::new(&p,&a)?;
/// draft.push(Move::Change {variable:0,value:Some(2)})?;
/// assert_eq!(draft.assignment().values(), &[Some(2)]);
/// let movement=draft.finish();
/// let mut next=a.clone(); p.apply(&mut next,&movement)?.commit();
/// assert_eq!(a.values(), &[Some(1)]);
/// # Ok::<(),ModelError>(())
/// ```
pub struct MoveDraft<'p, P: MoveDraftModel> {
    problem: &'p P,
    assignment: P::Assignment,
    steps: Vec<P::Movement>,
    origin: Arc<()>,
}
impl<P: MoveDraftModel> Clone for MoveDraft<'_, P> {
    fn clone(&self) -> Self {
        Self {
            problem: self.problem,
            assignment: self.assignment.clone(),
            steps: self.steps.clone(),
            origin: self.origin.clone(),
        }
    }
}
impl<'p, P: MoveDraftModel> MoveDraft<'p, P> {
    pub fn new(problem: &'p P, assignment: &P::Assignment) -> Result<Self, P::Error> {
        problem.validate_draft(assignment)?;
        Ok(Self {
            problem,
            assignment: assignment.clone(),
            steps: vec![],
            origin: Arc::new(()),
        })
    }
    pub fn assignment(&self) -> &P::Assignment {
        &self.assignment
    }
    pub fn steps(&self) -> &[P::Movement] {
        &self.steps
    }
    pub fn push(&mut self, movement: P::Movement) -> Result<(), P::Error> {
        let next = self.problem.apply_to_copy(&self.assignment, &movement)?;
        self.steps.push(movement);
        self.assignment = next;
        Ok(())
    }
    pub fn finish(self) -> P::Movement {
        P::compose(self.steps)
    }
    pub fn into_parts(self) -> (P::Assignment, P::Movement) {
        (self.assignment, P::compose(self.steps))
    }
}

/// Bind custom-move execution to one immutable model. Non-null references and
/// closures replace runtime null checks. This does not compute scores or shadows;
/// returned moves can be replayed through the ordinary score directors.
pub struct MoveTester<'p, P: MoveDraftModel> {
    problem: &'p P,
}
impl<'p, P: MoveDraftModel> MoveTester<'p, P> {
    pub fn new(problem: &'p P) -> Self {
        Self { problem }
    }
    pub fn problem(&self) -> &'p P {
        self.problem
    }
    pub fn using<'a>(
        &self,
        assignment: &'a mut P::Assignment,
    ) -> Result<MoveTestContext<'p, 'a, P>, P::Error> {
        self.problem.validate_draft(assignment)?;
        Ok(MoveTestContext {
            problem: self.problem,
            assignment,
        })
    }
}
/// The caller's assignment is published only after successful committed execution.
/// Every callback sees an isolated draft. Errors/panics discard that draft; external
/// callback side effects are not rolled back. Temporary execution never publishes
/// the draft, even when inspection mutates it or returns an error value.
pub struct MoveTestContext<'p, 'a, P: MoveDraftModel> {
    problem: &'p P,
    assignment: &'a mut P::Assignment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoveTestError<E> {
    Model(E),
    /// The callback replaced its draft with one from a different opening.
    ReplacedDraft,
}
impl<E: fmt::Debug> fmt::Display for MoveTestError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "move execution: {self:?}")
    }
}
impl<E: fmt::Debug> Error for MoveTestError<E> {}
impl<'p, P: MoveDraftModel> MoveTestContext<'p, '_, P> {
    pub fn assignment(&self) -> &P::Assignment {
        self.assignment
    }
    pub fn execute(
        &mut self,
        edit: impl FnOnce(&mut MoveDraft<'p, P>) -> Result<(), P::Error>,
    ) -> Result<P::Movement, MoveTestError<P::Error>> {
        let mut draft =
            MoveDraft::new(self.problem, self.assignment).map_err(MoveTestError::Model)?;
        let origin = draft.origin.clone();
        edit(&mut draft).map_err(MoveTestError::Model)?;
        if !Arc::ptr_eq(&origin, &draft.origin) {
            return Err(MoveTestError::ReplacedDraft);
        }
        self.problem
            .validate_draft(draft.assignment())
            .map_err(MoveTestError::Model)?;
        let (next, movement) = draft.into_parts();
        *self.assignment = next;
        Ok(movement)
    }
    pub fn execute_temporarily<R>(
        &mut self,
        edit: impl FnOnce(&mut MoveDraft<'p, P>) -> Result<(), P::Error>,
        inspect: impl FnOnce(&mut MoveDraft<'p, P>) -> R,
    ) -> Result<R, MoveTestError<P::Error>> {
        let mut draft =
            MoveDraft::new(self.problem, self.assignment).map_err(MoveTestError::Model)?;
        let origin = draft.origin.clone();
        edit(&mut draft).map_err(MoveTestError::Model)?;
        if !Arc::ptr_eq(&origin, &draft.origin) {
            return Err(MoveTestError::ReplacedDraft);
        }
        let result = inspect(&mut draft);
        if !Arc::ptr_eq(&origin, &draft.origin) {
            return Err(MoveTestError::ReplacedDraft);
        }
        Ok(result)
    }
}
