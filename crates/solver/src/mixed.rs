//! Joint basic-variable and ordered-list planning with atomic edits.

use std::{error::Error, fmt, ops::Deref, sync::OnceLock};

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
    pub(crate) fn install_snapshot(
        &mut self,
        snapshot: crate::candidate_snapshot::MixedSnapshot<B, L>,
    ) {
        snapshot.basic.install(&mut self.basic);
        snapshot.lists.install(&mut self.lists);
    }
    pub(crate) fn into_parts(self) -> (Assignment<B>, ListAssignment<L>) {
        (self.basic, self.lists)
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
impl MixedError {
    pub fn range_report(&self) -> Option<&crate::ValueRangeReport> {
        match self {
            Self::Basic(e) => e.range_report(),
            Self::Lists(e) => e.range_report(),
        }
    }
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
    /// Start a new solve from a validated joint assignment without losing domains,
    /// ranges, optionality or pins. Neither original component is modified on error.
    pub fn with_initial_assignment(
        &self,
        assignment: &MixedAssignment<B, L>,
    ) -> Result<Self, MixedError> {
        self.validate(assignment)?;
        Ok(Self::new(
            self.basic
                .with_initial_assignment(assignment.basic())
                .map_err(MixedError::Basic)?,
            self.lists
                .with_initial_assignment(assignment.lists())
                .map_err(MixedError::Lists)?,
        ))
    }

    pub fn initial_assignment(&self) -> MixedAssignment<B, L> {
        MixedAssignment::new(
            self.basic.initial_assignment(),
            self.lists.initial_assignment(),
        )
    }
    /// Prepare list dependencies while preserving the validated basic component.
    pub fn prepare_assignment(
        &self,
        assignment: &MixedAssignment<B, L>,
    ) -> Result<MixedAssignment<B, L>, MixedError> {
        self.basic
            .validate(assignment.basic())
            .map_err(MixedError::Basic)?;
        Ok(MixedAssignment::new(
            assignment.basic().clone(),
            self.lists
                .prepare_assignment(assignment.lists())
                .map_err(MixedError::Lists)?,
        ))
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

    /// Journal basic writes and save only touched list rows. Both component
    /// guards remain armed until validation succeeds, so errors and ordinary
    /// panic unwinding restore both components. Dropping the returned guard
    /// restores both components; commit keeps both. Clone/Eq must
    /// preserve value identity without mutating shared state. Normal drop-guard
    /// limitations (abort, forget, panicking destructors) apply.
    pub fn apply<'a>(
        &self,
        assignment: &'a mut MixedAssignment<B, L>,
        movement: &MixedMove<B, L>,
    ) -> Result<AppliedMixedMove<'a, B, L>, MixedError> {
        self.validate(assignment)?;
        self.apply_validated(assignment, movement)
    }
    // Owning score directors validate on entry and only install validated moves.
    pub(crate) fn apply_validated<'a>(
        &self,
        assignment: &'a mut MixedAssignment<B, L>,
        movement: &MixedMove<B, L>,
    ) -> Result<AppliedMixedMove<'a, B, L>, MixedError> {
        let basic = self
            .basic
            .apply_validated(&mut assignment.basic, &movement.basic)
            .map_err(MixedError::Basic)?;
        let lists = self
            .lists
            .apply_validated(&mut assignment.lists, &movement.lists)
            .map_err(MixedError::Lists)?;
        // No fallible work or user code between disarming these guards and
        // installing their undo data in the joint guard.
        let basic_prior = basic.into_undo_journal();
        let saved_lists = Some(lists.into_undo_lists());
        Ok(AppliedMixedMove {
            assignment,
            basic_prior,
            saved_lists,
            original: OnceLock::new(),
        })
    }
}

#[must_use = "dropping restores both components; call commit to keep changes"]
pub struct AppliedMixedMove<'a, B, L> {
    assignment: &'a mut MixedAssignment<B, L>,
    basic_prior: Vec<crate::PriorValue<B>>,
    saved_lists: Option<crate::list::undo::ListUndo<L>>,
    original: OnceLock<MixedAssignment<B, L>>,
}
impl<B, L> Deref for AppliedMixedMove<'_, B, L> {
    type Target = MixedAssignment<B, L>;
    fn deref(&self) -> &Self::Target {
        self.assignment
    }
}
impl<B, L> AppliedMixedMove<'_, B, L> {
    pub(crate) fn snapshot(&self) -> crate::candidate_snapshot::MixedSnapshot<B, L>
    where
        B: Clone,
        L: Clone,
    {
        crate::candidate_snapshot::MixedSnapshot {
            basic: crate::candidate_snapshot::BasicSnapshot::capture(
                &self.assignment.basic,
                &self.basic_prior,
            ),
            lists: crate::candidate_snapshot::ListSnapshot::capture(
                &self.assignment.lists,
                self.saved_lists
                    .as_ref()
                    .expect("live mixed transaction")
                    .entity_count(),
                self.saved_lists
                    .as_ref()
                    .expect("live mixed transaction")
                    .entities(),
            ),
        }
    }

    pub(crate) fn basic_prior_values(&self) -> &[crate::PriorValue<B>] {
        &self.basic_prior
    }

    pub(crate) fn assigned_list_count_delta(&self) -> i64 {
        self.saved_lists
            .as_ref()
            .expect("live mixed transaction")
            .assigned_delta(&self.assignment.lists)
    }

    /// Borrow a basic slot before the entire move, including repeated writes.
    /// Unknown variables return None; a known unassigned slot returns Some(None).
    pub fn original_basic(&self, variable: usize) -> Option<&Option<B>> {
        self.basic_prior
            .iter()
            .find(|prior| prior.variable == variable)
            .map(|prior| &prior.value)
            .or_else(|| self.assignment.basic.get(variable))
    }
    /// Borrow a pre-move list. Compact list swaps clone their touched rows lazily
    /// on the first request; later requests reuse the same cache. Rollback remains
    /// armed if a value clone panics while materializing that cache.
    pub fn original_list(&self, entity: usize) -> Option<&[L]> {
        self.saved_lists
            .as_ref()
            .expect("live mixed transaction")
            .original_list(&self.assignment.lists, entity)
    }
    /// Materialize the complete pre-move snapshot once, on demand. Prefer the
    /// borrowed component accessors when only particular slots or rows are needed.
    /// Rollback does not depend on constructing this snapshot.
    pub fn original(&self) -> &MixedAssignment<B, L>
    where
        B: Clone,
        L: Clone,
    {
        self.original.get_or_init(|| {
            // Repeated component writes keep their first pre-move value. Build
            // the reference view in O(slots + writes), then clone each slot once.
            let mut basic: Vec<_> = self.assignment.basic.values().iter().collect();
            for prior in self.basic_prior.iter().rev() {
                basic[prior.variable] = &prior.value;
            }
            MixedAssignment::new(
                Assignment::new(basic.into_iter().cloned().collect()),
                ListAssignment::new(
                    (0..self.assignment.lists.lists().len())
                        .map(|i| self.original_list(i).unwrap().to_vec())
                        .collect(),
                ),
            )
        })
    }
    pub fn commit(mut self) {
        self.saved_lists = None;
    }
}
impl<B, L> Drop for AppliedMixedMove<'_, B, L> {
    fn drop(&mut self) {
        if let Some(mut saved) = self.saved_lists.take() {
            for prior in self.basic_prior.iter_mut().rev() {
                std::mem::swap(
                    &mut self.assignment.basic.values[prior.variable],
                    &mut prior.value,
                );
            }
            saved.rollback(&mut self.assignment.lists);
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
