//! Borrowed candidate validation for joint basic/list planning.
use crate::{
    BasicMoveChecker, ListMoveChecker, MixedAssignment, MixedError, MixedMove, MixedProblem,
};

impl<B: Eq + Clone, L: Eq + Clone> MixedProblem<B, L> {
    /// Validate both source components once for repeated joint candidate checks.
    /// The returned checker borrows this model and assignment. Basic source errors
    /// precede list source errors, and both sources are checked before any move.
    pub fn move_checker<'a>(
        &'a self,
        assignment: &'a MixedAssignment<B, L>,
    ) -> Result<MixedMoveChecker<'a, B, L>, MixedError> {
        let basic = self
            .basic()
            .move_checker(assignment.basic())
            .map_err(MixedError::Basic)?;
        let lists = self
            .lists()
            .move_checker(assignment.lists())
            .map_err(MixedError::Lists)?;
        Ok(MixedMoveChecker { basic, lists })
    }

    /// Whether a valid joint move changes either component under value equality.
    /// Both component moves must be valid even when the first already changes a
    /// value. Errors have the same source/basic-move/list-move order as `apply`.
    /// Restoring composites return false; partial assignments are supported.
    /// This does not clone values, mutate state, calculate scores or call listeners.
    /// For many proposals against the same snapshot, use [`Self::move_checker`].
    pub fn is_move_doable(
        &self,
        assignment: &MixedAssignment<B, L>,
        movement: &MixedMove<B, L>,
    ) -> Result<bool, MixedError> {
        self.move_checker(assignment)?.is_doable(movement)
    }
}

/// Repeated joint candidate checks without cloning planning values.
///
/// Basic checks borrow written values; list checks copy references for touched
/// rows. Pins, ranges and list dependencies follow ordinary mixed move validation.
/// A change in either component does not bypass validation of the other component.
/// Equality must remain stable while the source is borrowed.
///
/// ```
/// use timetuck::*;
/// let problem = MixedProblem::new(
///     FiniteProblem::new(vec![Variable::new(vec![1, 2]).with_initial(1)])?,
///     ListProblem::new(vec![7], vec![ListEntity::new(vec![7])])?,
/// );
/// let assignment = problem.initial_assignment();
/// let checker = problem.move_checker(&assignment)?;
/// assert!(checker.is_doable(&MixedMove {
///     basic: Move::Change { variable: 0, value: Some(2) },
///     lists: ListMove::Composite(vec![]),
/// })?);
/// assert!(!checker.is_doable(&MixedMove {
///     basic: Move::Composite(vec![]),
///     lists: ListMove::Composite(vec![]),
/// })?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct MixedMoveChecker<'a, B, L> {
    basic: BasicMoveChecker<'a, B>,
    lists: ListMoveChecker<'a, L>,
}

impl<B: Eq, L: Eq + Clone> MixedMoveChecker<'_, B, L> {
    /// Check the joint proposal without revalidating the borrowed source.
    /// Invalid basic proposals precede invalid list proposals, as with `apply`.
    pub fn is_doable(&self, movement: &MixedMove<B, L>) -> Result<bool, MixedError> {
        let basic_changed = self
            .basic
            .is_doable(&movement.basic)
            .map_err(MixedError::Basic)?;
        let lists_changed = self
            .lists
            .is_doable(&movement.lists)
            .map_err(MixedError::Lists)?;
        Ok(basic_changed || lists_changed)
    }
}
