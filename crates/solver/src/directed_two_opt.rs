//! Directed two-opt cuts, including reversal across a list's end.
use crate::{
    ListAssignment, ListError, ListLifecyclePlan, ListMove, ListNotificationScope, ListPosition,
    ListProblem, ListRange,
};

/// Prepare a directed two-opt edit at two insertion boundaries (0..=list length).
/// Within one entity, ascending cuts reverse [first, second); descending cuts
/// reverse the cyclic span [first, end) followed by [pinned_prefix, second).
/// For an unpinned wrapped reversal, rotate the result to retain the original
/// first value as the cycle anchor. A pinned prefix remains in place and the
/// movable suffix is not rotated. Equal cuts and one-value reversals are no-ops.
///
/// Across entities, exchange the two suffixes without reversing them. Empty tails
/// are allowed, including two empty tails as a no-op. Pins, entity ranges and
/// dependency validation apply to the final state before any callback is invoked.
/// Wrapped reversals notify the entire movable suffix; other edits notify their
/// changed intervals. The returned plan supports atomic execution, undo and
/// snapshot-staleness checks; movement() can be used by native scoring directors.
/// This does not change the existing non-wrapping two-opt neighborhood provider.
///
/// ```
/// use timetuck::*;
/// let p = ListProblem::new((1..=5).collect(),vec![ListEntity::new(vec![1,2,3,4,5])])?;
/// let a = p.initial_assignment();
/// let plan = list_directed_two_opt_plan(&p,&a,
///     ListPosition{entity:0,index:4},ListPosition{entity:0,index:2})?;
/// assert_eq!(plan.assignment().lists(), &[vec![1,5,3,4,2]]);
/// # Ok::<(), ListError>(())
/// ```
pub fn list_directed_two_opt_plan<'p, V: Clone + Eq>(
    problem: &'p ListProblem<V>,
    before: &ListAssignment<V>,
    first: ListPosition,
    second: ListPosition,
) -> Result<ListLifecyclePlan<'p, V>, ListError> {
    problem.validate(before)?;
    let movement = directed_two_opt_movement(problem, before, first, second)?;
    let wrapped = first.entity == second.entity && first.index > second.index;
    ListLifecyclePlan::prepare_with_scope(
        problem,
        before,
        movement,
        if wrapped {
            ListNotificationScope::MovableSuffix
        } else {
            ListNotificationScope::ChangedRange
        },
    )
}

/// Build only the positional edit. Callers validate the source snapshot once;
/// final pins, ranges and dependency checks belong to their plan or checker.
pub(crate) fn directed_two_opt_movement<V: Clone + Eq>(
    problem: &ListProblem<V>,
    before: &ListAssignment<V>,
    first: ListPosition,
    second: ListPosition,
) -> Result<ListMove<V>, ListError> {
    for cut in [first, second] {
        let row = before.list(cut.entity).ok_or(ListError::EntityIndex {
            entity: cut.entity,
            count: problem.entities().len(),
        })?;
        if cut.index > row.len() {
            return Err(ListError::InsertionIndex(cut));
        }
        if cut.index < problem.entities()[cut.entity].pinned_prefix() {
            return Err(ListError::PinChanged { entity: cut.entity });
        }
    }
    let left = before.lists()[first.entity].len();
    let right = before.lists()[second.entity].len();
    let wrapped = first.entity == second.entity && first.index > second.index;
    let movement = if first.entity != second.entity {
        match (first.index == left, second.index == right) {
            (true, true) => ListMove::Composite(vec![]),
            (true, false) => ListMove::SubListChange {
                entity: second.entity,
                start: second.index,
                end: right,
                destination: first,
                reverse: false,
            },
            (false, true) => ListMove::SubListChange {
                entity: first.entity,
                start: first.index,
                end: left,
                destination: second,
                reverse: false,
            },
            (false, false) => ListMove::SubListSwap {
                left: ListRange {
                    entity: first.entity,
                    start: first.index,
                    end: left,
                },
                right: ListRange {
                    entity: second.entity,
                    start: second.index,
                    end: right,
                },
                reverse: false,
            },
        }
    } else if !wrapped {
        ListMove::SubListChange {
            entity: first.entity,
            start: first.index,
            end: second.index,
            destination: first,
            reverse: true,
        }
    } else {
        let prefix = problem.entities()[first.entity].pinned_prefix();
        let mut target: Vec<_> = (0..left).collect();
        crate::reverse_cyclic_span(
            &mut target[prefix..],
            first.index - prefix,
            second.index - prefix,
        )
        .expect("validated insertion boundaries");
        if prefix == 0 && !target.is_empty() {
            let anchor = target
                .iter()
                .position(|&i| i == 0)
                .expect("permutation anchor");
            target.rotate_left(anchor);
        }
        let mut current: Vec<_> = (0..left).collect();
        let mut edits = vec![];
        for (index, &wanted) in target.iter().enumerate() {
            if current[index] != wanted {
                let source = current
                    .iter()
                    .position(|&i| i == wanted)
                    .expect("permutation member");
                edits.push(ListMove::Swap {
                    left: ListPosition {
                        entity: first.entity,
                        index,
                    },
                    right: ListPosition {
                        entity: first.entity,
                        index: source,
                    },
                });
                current.swap(index, source);
            }
        }
        ListMove::Composite(edits)
    };
    Ok(movement)
}
