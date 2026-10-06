//! Inspect structural dependency flaws without scoring or installing a candidate.
use crate::{
    ListAssignment, ListError, ListMove, ListProblem, MixedAssignment, MixedError, MixedMove,
    MixedProblem,
};

/// A real candidate snapshot whose layout, domains and pins are valid. Dependency
/// cycles and their assigned downstream dependents are reported separately. No
/// candidate score is implied. In mixed models the indices refer to list values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoveInspection<A> {
    assignment: A,
    inconsistent_values: Vec<usize>,
}
impl<A> MoveInspection<A> {
    pub fn assignment(&self) -> &A {
        &self.assignment
    }
    pub fn inconsistent_values(&self) -> &[usize] {
        &self.inconsistent_values
    }
    pub fn is_flawed(&self) -> bool {
        !self.inconsistent_values.is_empty()
    }
}
/// Preview a move from a valid starting assignment. Only dependency consistency is
/// deferred; malformed coordinates, duplicates, domains and pins remain errors.
/// The original model/assignment are unchanged, even for errors or ordinary panic.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![0,1],vec![ListEntity::new(vec![0,1])])?
///     .with_dependencies(vec![vec![],vec![0]])?;
/// let m=ListMove::Swap {left:ListPosition {entity:0,index:0},right:ListPosition {entity:0,index:1}};
/// let preview=inspect_list_move(&p,&p.initial_assignment(),&m)?;
/// assert!(preview.is_flawed());
/// assert_eq!(preview.assignment().lists(),&[vec![1,0]]);
/// assert_eq!(preview.inconsistent_values(),&[0,1]);
/// # Ok::<(),ListError>(())
/// ```
pub fn inspect_list_move<V: Eq + Clone>(
    problem: &ListProblem<V>,
    initial: &ListAssignment<V>,
    movement: &ListMove<V>,
) -> Result<MoveInspection<ListAssignment<V>>, ListError> {
    problem.validate(initial)?;
    let mut layout_model = problem.clone();
    layout_model.dependencies = None;
    let mut assignment = initial.clone();
    layout_model.apply(&mut assignment, movement)?.commit();
    let inconsistent_values = problem.inconsistent_values(&assignment)?;
    Ok(MoveInspection {
        assignment,
        inconsistent_values,
    })
}
/// Inspect both components atomically. Basic-variable errors remain errors;
/// structural list flaws return the complete proposed mixed snapshot.
pub fn inspect_mixed_move<B: Eq + Clone, L: Eq + Clone>(
    problem: &MixedProblem<B, L>,
    initial: &MixedAssignment<B, L>,
    movement: &MixedMove<B, L>,
) -> Result<MoveInspection<MixedAssignment<B, L>>, MixedError> {
    problem.validate(initial)?;
    let mut basic = initial.basic().clone();
    problem
        .basic()
        .apply(&mut basic, &movement.basic)
        .map_err(MixedError::Basic)?
        .commit();
    let lists = inspect_list_move(problem.lists(), initial.lists(), &movement.lists)
        .map_err(MixedError::Lists)?;
    Ok(MoveInspection {
        assignment: MixedAssignment::new(basic, lists.assignment),
        inconsistent_values: lists.inconsistent_values,
    })
}
