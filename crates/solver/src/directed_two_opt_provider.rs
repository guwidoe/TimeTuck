//! Lazy directed two-opt search through the standard provider extension.
use crate::{
    ListAssignment, ListError, ListMove, ListPosition, ListProblem, MixedAssignment, MixedMove,
    MixedProblem, Move, MoveProvider, list_directed_two_opt_plan,
};

/// Enumerate forward and wrapped reversals by entity, first cut, second cut,
/// followed by suffix exchanges for each unordered pair of entities. Include empty
/// receiving tails. Full pins are skipped; prefix pins bound both cuts. Final
/// range/dependency violations and unchanged results are omitted. Duplicate
/// successors from distinct cuts are retained; no all-candidate storage is built.
///
/// Opening validates the current state but does not prepare moves. Each next call
/// prepares candidates until it finds a valid changed result; candidate preparation
/// owns temporary snapshots. This is a finite provider, not constant-time sampling.
/// It never assigns an unassigned value or removes assigned membership.
pub fn list_directed_two_opt_moves<'a, V: Clone + Eq>(
    problem: &'a ListProblem<V>,
    assignment: &'a ListAssignment<V>,
    cross_entity: bool,
) -> Result<impl Iterator<Item = ListMove<V>> + 'a, ListError> {
    problem.validate(assignment)?;
    let local = (0..problem.entities().len())
        .filter(|&e| !problem.entities()[e].is_pinned())
        .flat_map(move |e| {
            let prefix = problem.entities()[e].pinned_prefix();
            let len = assignment.lists()[e].len();
            (prefix..=len).flat_map(move |first| {
                (prefix..=len).filter_map(move |second| {
                    let span = if first <= second {
                        second - first
                    } else {
                        len - first + second - prefix
                    };
                    (span >= 2).then_some((
                        ListPosition {
                            entity: e,
                            index: first,
                        },
                        ListPosition {
                            entity: e,
                            index: second,
                        },
                    ))
                })
            })
        });
    let cross = (0..problem.entities().len())
        .filter(move |&e| cross_entity && !problem.entities()[e].is_pinned())
        .flat_map(move |left| {
            ((left + 1)..problem.entities().len())
                .filter(|&e| !problem.entities()[e].is_pinned())
                .flat_map(move |right| {
                    let left_len = assignment.lists()[left].len();
                    let right_len = assignment.lists()[right].len();
                    (problem.entities()[left].pinned_prefix()..=left_len).flat_map(move |i| {
                        (problem.entities()[right].pinned_prefix()..=right_len).filter_map(
                            move |j| {
                                (i != left_len || j != right_len).then_some((
                                    ListPosition {
                                        entity: left,
                                        index: i,
                                    },
                                    ListPosition {
                                        entity: right,
                                        index: j,
                                    },
                                ))
                            },
                        )
                    })
                })
        });
    Ok(local.chain(cross).filter_map(move |(first, second)| {
        let plan = list_directed_two_opt_plan(problem, assignment, first, second).ok()?;
        (plan.assignment() != assignment).then(|| plan.movement().clone())
    }))
}
/// Supply directed list two-opt candidates at every local-search step. Use with
/// list/mixed *_with_provider APIs or a phase's custom provider. Mixed candidates
/// leave basic variables unchanged; combine providers explicitly for other moves.
/// The engine retains its normal acceptance, scoring, limits and undo behavior.
/// Inputs must be valid model assignments, as guaranteed by the search engines.
///
/// ```
/// use timetuck::*;
/// let p=ListProblem::new(vec![1,2,3],vec![ListEntity::new(vec![1,2,3])])?;
/// let a=p.initial_assignment();
/// let mut provider=DirectedTwoOptProvider::new(false);
/// assert!(provider.moves(&p,&a,0).next().is_some());
/// # Ok::<(), ListError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DirectedTwoOptProvider {
    cross_entity: bool,
}
impl DirectedTwoOptProvider {
    pub fn new(cross_entity: bool) -> Self {
        Self { cross_entity }
    }
    pub fn cross_entity(self) -> bool {
        self.cross_entity
    }
}
impl<V: Clone + Eq> MoveProvider<ListProblem<V>, ListAssignment<V>, ListMove<V>>
    for DirectedTwoOptProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a ListProblem<V>,
        a: &'a ListAssignment<V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = ListMove<V>> + 'a>
    where
        ListMove<V>: 'a,
    {
        Box::new(
            list_directed_two_opt_moves(p, a, self.cross_entity)
                .expect("provider requires a valid list assignment"),
        )
    }
}
impl<B: Clone + Eq, V: Clone + Eq>
    MoveProvider<MixedProblem<B, V>, MixedAssignment<B, V>, MixedMove<B, V>>
    for DirectedTwoOptProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, V>,
        a: &'a MixedAssignment<B, V>,
        _: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, V>> + 'a>
    where
        MixedMove<B, V>: 'a,
    {
        p.validate(a)
            .expect("provider requires a valid mixed assignment");
        Box::new(
            list_directed_two_opt_moves(p.lists(), a.lists(), self.cross_entity)
                .expect("validated list assignment")
                .map(|lists| MixedMove {
                    basic: Move::Composite(vec![]),
                    lists,
                }),
        )
    }
}
