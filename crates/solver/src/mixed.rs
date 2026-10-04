//! Joint basic-variable and ordered-list planning with atomic edits.

use std::{error::Error, fmt, ops::Deref};

use crate::{
    Assignment, FiniteProblem, ListAssignment, ListError, ListMove, ListProblem, ModelError, Move,
};

/// One basic assignment and one ordered-list assignment. Their value types may
/// differ; objectives can relate them through stable application identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedAssignment<B, L> {
    basic: Assignment<B>,
    lists: ListAssignment<L>,
}
impl<B, L> MixedAssignment<B, L> {
    pub fn new(basic: Assignment<B>, lists: ListAssignment<L>) -> Self {
        Self { basic, lists }
    }
    pub fn basic(&self) -> &Assignment<B> {
        &self.basic
    }
    pub fn lists(&self) -> &ListAssignment<L> {
        &self.lists
    }
}

/// Both component moves form one transaction. Use an empty Composite in either
/// component for a move affecting only the other component. Each component may
/// contain nested composites; final domains and pins are checked before exposure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixedMove<B, L> {
    pub basic: Move<B>,
    pub lists: ListMove<L>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MixedError {
    Basic(ModelError),
    Lists(ListError),
}
impl fmt::Display for MixedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic(error) => write!(f, "mixed basic component: {error}"),
            Self::Lists(error) => write!(f, "mixed list component: {error}"),
        }
    }
}
impl Error for MixedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Basic(error) => error,
            Self::Lists(error) => error,
        })
    }
}

/// Product of independently validated domains. Cross-component hard constraints
/// belong in the objective, not in domain validation. Partial states are allowed.
#[derive(Clone, Debug)]
pub struct MixedProblem<B, L> {
    basic: FiniteProblem<B>,
    lists: ListProblem<L>,
}
impl<B: Eq + Clone, L: Eq + Clone> MixedProblem<B, L> {
    pub fn new(basic: FiniteProblem<B>, lists: ListProblem<L>) -> Self {
        Self { basic, lists }
    }
    pub fn basic(&self) -> &FiniteProblem<B> {
        &self.basic
    }
    pub fn lists(&self) -> &ListProblem<L> {
        &self.lists
    }
    pub fn initial_assignment(&self) -> MixedAssignment<B, L> {
        MixedAssignment::new(
            self.basic.initial_assignment(),
            self.lists.initial_assignment(),
        )
    }
    pub fn validate(&self, assignment: &MixedAssignment<B, L>) -> Result<(), MixedError> {
        self.basic
            .validate(&assignment.basic)
            .map_err(MixedError::Basic)?;
        self.lists
            .validate(&assignment.lists)
            .map_err(MixedError::Lists)
    }
    pub fn is_complete(&self, assignment: &MixedAssignment<B, L>) -> bool {
        self.basic.is_complete(&assignment.basic) && self.lists.is_complete(&assignment.lists)
    }

    /// Prepare both edits in an isolated clone before installing either. Errors
    /// and ordinary panic unwinding leave the original untouched. Dropping the
    /// returned guard restores both components; commit keeps both. Clone/Eq must
    /// preserve value identity without mutating shared state. Normal drop-guard
    /// limitations (abort, forget, panicking destructors) apply.
    pub fn apply<'a>(
        &self,
        assignment: &'a mut MixedAssignment<B, L>,
        movement: &MixedMove<B, L>,
    ) -> Result<AppliedMixedMove<'a, B, L>, MixedError> {
        self.validate(assignment)?;
        let mut candidate = assignment.clone();
        self.basic
            .apply(&mut candidate.basic, &movement.basic)
            .map_err(MixedError::Basic)?
            .commit();
        self.lists
            .apply(&mut candidate.lists, &movement.lists)
            .map_err(MixedError::Lists)?
            .commit();
        let original = Some(std::mem::replace(assignment, candidate));
        Ok(AppliedMixedMove {
            assignment,
            original,
        })
    }
}

#[must_use = "dropping restores both components; call commit to keep changes"]
pub struct AppliedMixedMove<'a, B, L> {
    assignment: &'a mut MixedAssignment<B, L>,
    original: Option<MixedAssignment<B, L>>,
}
impl<B, L> Deref for AppliedMixedMove<'_, B, L> {
    type Target = MixedAssignment<B, L>;
    fn deref(&self) -> &Self::Target {
        self.assignment
    }
}
impl<B, L> AppliedMixedMove<'_, B, L> {
    pub fn original(&self) -> &MixedAssignment<B, L> {
        self.original.as_ref().expect("live mixed transaction")
    }
    pub fn commit(mut self) {
        self.original = None;
    }
}
impl<B, L> Drop for AppliedMixedMove<'_, B, L> {
    fn drop(&mut self) {
        if let Some(mut original) = self.original.take() {
            std::mem::swap(self.assignment, &mut original);
        }
    }
}

/// Distinct planning-value namespaces in a mixed model, even when B and L have
/// the same Rust type. Basic None is a real unassigned key; lists have no null key.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum MixedPlanningValue<B, L> {
    Basic(Option<B>),
    List(L),
}
impl<B: Eq + Clone, L: Eq + Clone> MixedProblem<B, L> {
    /// Capture both components with full move validation. No partial model state
    /// escapes, and equal basic/list values never share a tabu identity.
    pub fn planning_values(
        &self,
        before: &MixedAssignment<B, L>,
        movement: &MixedMove<B, L>,
    ) -> Result<Vec<MixedPlanningValue<B, L>>, MixedError> {
        {
            let mut copy = before.clone();
            let _transaction = self.apply(&mut copy, movement)?;
        }
        let mut values: Vec<_> = before
            .basic
            .planning_values(&movement.basic)
            .map_err(|error| MixedError::Basic(error.into()))?
            .into_iter()
            .map(MixedPlanningValue::Basic)
            .collect();
        values.extend(
            self.lists
                .planning_values(&before.lists, &movement.lists)
                .map_err(MixedError::Lists)?
                .into_iter()
                .map(MixedPlanningValue::List),
        );
        Ok(values)
    }
}
