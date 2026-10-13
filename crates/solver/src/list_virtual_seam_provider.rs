//! Automatic discovery of the supported virtual-row three-opt family.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, MoveProvider,
    VirtualSeam3OptPlan,
};

/// Discover plans from live ownership and positions, in ordered owner-pair then
/// internal-cut order. The first owner supplies its movable wrap and a strictly
/// interior adjacent cut; the second supplies any movable adjacent cut.
/// `max_attempts` bounds topology checks, including rejected receiving ranges and
/// dependency cycles. Zero still validates the assignment. No endpoint payloads,
/// target lengths or expected states are supplied by callers. Each plan retains
/// current endpoints; reopen after edits rather than replay stale coordinates.
///
/// This supported virtual-row family differs from physical combined-cycle k-opt:
/// its three declared cuts need not be three physical edges of the concatenation.
/// Larger physical repartitions can also express some resulting permutations.
/// This is finite ordered discovery, not uniform sampling or a Java trace claim.
pub fn discover_virtual_seam_three_opt<'a, V: Eq + Clone>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    max_attempts: usize,
) -> Result<impl Iterator<Item = VirtualSeam3OptPlan<V>> + 'a, ListError> {
    problem.validate(assignment)?;
    let entities = problem.entities();
    let coordinates = (0..entities.len())
        .filter(|&a| !entities[a].is_pinned())
        .flat_map(move |a| {
            let start = entities[a].pinned_prefix();
            let end = assignment.lists()[a].len();
            (0..entities.len())
                .filter(move |&b| b != a && !entities[b].is_pinned())
                .flat_map(move |b| {
                    let b_start = entities[b].pinned_prefix();
                    let b_end = assignment.lists()[b].len();
                    (start.saturating_add(1)..end.saturating_sub(2)).flat_map(move |left| {
                        (b_start..b_end.saturating_sub(1))
                            .map(move |other| (a, b, start, end, left, other))
                    })
                })
        });
    Ok(coordinates
        .take(max_attempts)
        .filter_map(move |(a, b, start, end, left, other)| {
            let at = |entity, index| ListPosition { entity, index };
            let plan = VirtualSeam3OptPlan::prepare(
                problem,
                assignment,
                &[
                    (at(a, end - 1), at(a, start)),
                    (at(a, left), at(a, left + 1)),
                    (at(b, other), at(b, other + 1)),
                ],
                &[(1, 3), (2, 5), (4, 0)],
            )
            .ok()?;
            plan.is_doable(problem, assignment).then_some(plan)
        }))
}

/// Lazy, bounded provider for list *_with_provider search APIs. Every scan derives
/// coordinates anew from its immutable live assignment. Combine with other
/// providers through ProviderUnion; built-in seeded neighborhoods are unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtualSeam3OptProvider {
    max_attempts: usize,
}
impl VirtualSeam3OptProvider {
    pub const fn new(max_attempts: usize) -> Self {
        Self { max_attempts }
    }
    pub const fn max_attempts(self) -> usize {
        self.max_attempts
    }
}
impl<V: Eq + Clone> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for VirtualSeam3OptProvider
{
    fn moves<'a>(
        &'a mut self,
        problem: &'a ListProblem<V>,
        assignment: &'a ListAssignment<V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            discover_virtual_seam_three_opt(problem, assignment, self.max_attempts)
                .expect("provider requires a valid list assignment")
                .filter_map(move |plan| plan.movement(problem, assignment).ok()),
        )
    }
}
