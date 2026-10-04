//! Lazy mixed neighborhoods, optionally including simultaneous component moves.
use crate::neighborhood;
use crate::{ListMove, MixedAssignment, MixedError, MixedMove, MixedProblem, Move, Neighborhood};

fn basics<'a, B: Eq + Clone, L: Eq + Clone, I: Iterator<Item = usize> + Clone + 'a>(
    problem: &'a MixedProblem<B, L>,
    assignment: &'a MixedAssignment<B, L>,
    neighborhood: Neighborhood,
    order: I,
) -> impl Iterator<Item = Move<B>> + 'a {
    neighborhood::ordered(problem.basic(), assignment.basic(), neighborhood, order)
}

/// Basic moves (changes/swaps or whole pillars), then list moves, then (when
/// requested) every pair of one basic and one list move. Each component retains its ordering, range,
/// optionality and pin rules. Joint moves use the same immutable starting state;
/// component independence makes their combined final state valid. Complete input
/// stays complete. Ordered swaps can produce duplicate successor states, as in
/// the component neighborhoods. Enumeration is lazy, including the product.
pub fn mixed_moves<'a, B: Eq + Clone, L: Eq + Clone>(
    problem: &'a MixedProblem<B, L>,
    assignment: &'a MixedAssignment<B, L>,
    neighborhood: Neighborhood,
    joint_moves: bool,
) -> Result<impl Iterator<Item = MixedMove<B, L>> + 'a, MixedError> {
    problem.validate(assignment)?;
    neighborhood
        .validate_basic(problem.basic())
        .map_err(MixedError::Basic)?;
    neighborhood
        .validate_list(problem.lists())
        .map_err(MixedError::Lists)?;
    Ok(ordered(
        problem,
        assignment,
        neighborhood,
        joint_moves,
        0..problem.basic().variables().len(),
        0..problem.lists().entities().len(),
    ))
}

pub(crate) fn ordered<
    'a,
    B: Eq + Clone,
    L: Eq + Clone,
    I: Iterator<Item = usize> + Clone + 'a,
    J: Iterator<Item = usize> + Clone + 'a,
>(
    problem: &'a MixedProblem<B, L>,
    assignment: &'a MixedAssignment<B, L>,
    neighborhood: Neighborhood,
    joint_moves: bool,
    basic_order: I,
    list_order: J,
) -> impl Iterator<Item = MixedMove<B, L>> + 'a {
    let basic =
        basics(problem, assignment, neighborhood, basic_order.clone()).map(|basic| MixedMove {
            basic,
            lists: ListMove::Composite(vec![]),
        });
    let lists = crate::list_neighborhood::ordered(
        problem.lists(),
        assignment.lists(),
        neighborhood,
        list_order.clone(),
    )
    .map(|lists| MixedMove {
        basic: Move::Composite(vec![]),
        lists,
    });
    let joint = joint_moves
        .then(move || {
            basics(problem, assignment, neighborhood, basic_order).flat_map(move |basic| {
                crate::list_neighborhood::ordered(
                    problem.lists(),
                    assignment.lists(),
                    neighborhood,
                    list_order.clone(),
                )
                .map(move |lists| MixedMove {
                    basic: basic.clone(),
                    lists,
                })
            })
        })
        .into_iter()
        .flatten();
    basic.chain(lists).chain(joint)
}

// Basic variable i and list entity i must never collide in variable tabu memory.
pub(crate) fn tabu_keys<B, L>(movement: &MixedMove<B, L>, basic_count: usize) -> Vec<usize> {
    let mut keys = movement.basic.touched_variables();
    keys.extend(movement.lists.touched_entities().into_iter().map(|entity| {
        basic_count
            .checked_add(entity)
            .expect("mixed tabu key fits allocated model")
    }));
    keys
}
