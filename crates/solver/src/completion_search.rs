//! Exact completion visitors that preserve existing planning decisions.
use crate::{
    Assignment, FiniteProblem, ListAssignment, ListError, ListProblem, MixedAssignment, MixedError,
    MixedProblem, ModelError,
};

/// Visit each complete extension of a basic assignment. Assigned values and pins
/// are retained; missing movable variables follow domain order with optional None
/// last. Return true on exhaustion, false when the callback requests stopping.
/// Validation precedes callbacks. No candidate catalog or recursive stack is used.
/// An exhausted extension space does not establish global model optimality.
pub fn visit_basic_completions<V: Eq + Clone>(
    p: &FiniteProblem<V>,
    a: &Assignment<V>,
    visit: impl FnMut(&Assignment<V>) -> bool,
) -> Result<bool, ModelError> {
    p.validate(a)?;
    Ok(crate::search::visit_basic_from(p, a.clone(), true, visit))
}
/// Visit ordered-list extensions without moving/removing existing values. New
/// values may be inserted between existing ones, preserving their relative order
/// and owners, subject to full/prefix pins, entity ranges and final dependencies.
/// Optional missing values may remain absent. Each completion occurs once; return
/// false to stop immediately. Validation precedes callbacks and inputs are unchanged.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![1,2,3],vec![ListEntity::new(vec![1,3])])?;
/// let mut completions=vec![];
/// assert!(visit_list_completions(&p,&p.initial_assignment(),|a| {
///     completions.push(a.lists()[0].clone()); true
/// })?);
/// assert_eq!(completions, [vec![2,1,3],vec![1,2,3],vec![1,3,2]]);
/// # Ok::<(),ListError>(())
/// ```
pub fn visit_list_completions<V: Eq + Clone>(
    p: &ListProblem<V>,
    a: &ListAssignment<V>,
    visit: impl FnMut(&ListAssignment<V>) -> bool,
) -> Result<bool, ListError> {
    p.validate(a)?;
    Ok(crate::list_search::visit_list_from(p, a.clone(), visit))
}
/// Cartesian product of list and basic completions, list completion outermost.
/// A stop propagates through both enumerations without additional callbacks.
pub fn visit_mixed_completions<B: Eq + Clone, L: Eq + Clone>(
    p: &MixedProblem<B, L>,
    a: &MixedAssignment<B, L>,
    mut visit: impl FnMut(&MixedAssignment<B, L>) -> bool,
) -> Result<bool, MixedError> {
    p.validate(a)?;
    if p.basic().search_space_size() == Some(0) {
        return Ok(true);
    }
    Ok(crate::list_search::visit_list_from(
        p.lists(),
        a.lists().clone(),
        |lists| {
            crate::search::visit_basic_from(p.basic(), a.basic().clone(), true, |basic| {
                visit(&MixedAssignment::new(basic.clone(), lists.clone()))
            })
        },
    ))
}
