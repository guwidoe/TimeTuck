//! Bounded direct sampling of comparator-aware pillar moves.
use crate::{
    Assignment, FiniteProblem, ListMove, MixedAssignment, MixedMove, MixedProblem, ModelError,
    Move, MoveProvider, PillarChangeMode, PillarMoveProvider, pillar_groups,
};

/// Search large pillar neighborhoods without enumerating all subsets.
///
/// Each opening regroups the current assignment and prepares eligible left/right
/// row metadata. The seed is `seed.wrapping_add(completed_steps)`; reopening the
/// same snapshot and step replays independently of earlier iterator consumption.
/// Basic and mixed search are supported; mixed moves leave lists unchanged.
///
/// At most `attempts_per_step` proposals are drawn lazily. Enabled change/swap
/// families are chosen uniformly when eligible groups exist. Groups are uniform
/// among those meeting the respective minimum size. Sizes are uniform within
/// clipped policy bounds, then subsets/windows are uniform conditional on size.
/// Changes choose a column and permitted target from its first row uniformly.
/// Swaps choose the left/right groups independently, rejecting equal groups.
/// Range-invalid and unchanged proposals consume attempts without being scored.
/// Duplicates are retained. An empty sample can end search without proving a local
/// optimum. No Java random-trace or uniform-over-all-moves equivalence is claimed.
///
/// Metadata is copied/sorted once per opening. Candidates are generated directly;
/// neither combinations nor all candidate moves are materialized. Invalid direct
/// openings return ModelError; trait openings require valid solver assignments.
///
/// ```
/// use timetuck::*;
/// let p = FiniteProblem::new(vec![Variable::new(vec![0,1]).with_initial(0);80])?;
/// let provider = SampledPillarMoveProvider::new(PillarMoveProvider {
///     left: PillarSelectionPolicy::subsets(40,40)?,
///     changes: PillarChangeMode::AssignedOnly,
///     swaps: false,
///     ..Default::default()
/// }, 42, 100);
/// let a = p.initial_assignment();
/// assert!(provider.select(&p,&a,0)?.count() <= 100);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampledPillarMoveProvider {
    pub neighborhood: PillarMoveProvider,
    pub seed: u64,
    pub attempts_per_step: u64,
}
impl SampledPillarMoveProvider {
    pub fn new(neighborhood: PillarMoveProvider, seed: u64, attempts_per_step: u64) -> Self {
        Self {
            neighborhood,
            seed,
            attempts_per_step,
        }
    }
    pub fn select<'a, V: Eq + Clone>(
        &self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        completed_steps: u64,
    ) -> Result<Box<dyn Iterator<Item = Move<V>> + 'a>, ModelError> {
        p.validate(a)?;
        if self.attempts_per_step == 0
            || (self.neighborhood.changes == PillarChangeMode::Disabled && !self.neighborhood.swaps)
        {
            return Ok(Box::new(std::iter::empty()));
        }
        let groups = pillar_groups(p, a)?;
        let left = groups
            .iter()
            .enumerate()
            .filter_map(|(i, g)| self.neighborhood.left.prepare_samples(g).map(|s| (i, s)))
            .collect::<Vec<_>>();
        let right = if self.neighborhood.swaps {
            groups
                .iter()
                .enumerate()
                .filter_map(|(i, g)| self.neighborhood.right.prepare_samples(g).map(|s| (i, s)))
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let changes = self.neighborhood.changes;
        let can_change = changes != PillarChangeMode::Disabled && !left.is_empty();
        let can_swap = self.neighborhood.swaps && !left.is_empty() && !right.is_empty();
        if !can_change && !can_swap {
            return Ok(Box::new(std::iter::empty()));
        }
        let mut random = crate::random::SplitMix64::new(self.seed.wrapping_add(completed_steps));
        Ok(Box::new((0..self.attempts_per_step).filter_map(
            move |_| {
                let change = can_change && (!can_swap || random.below(2) == 0);
                let (left_id, left_sample) = &left[random.below(left.len() as u64) as usize];
                if change {
                    let pillar = left_sample.draw(&mut random);
                    let col = random.below(pillar.column_count() as u64) as usize;
                    let first = pillar.rows()[0][col];
                    let variable = &p.variables()[first];
                    let assigned = matches!(
                        changes,
                        PillarChangeMode::AssignedOnly | PillarChangeMode::All
                    );
                    let unassign = matches!(
                        changes,
                        PillarChangeMode::UnassignOnly | PillarChangeMode::All
                    ) && variable.allows_unassigned();
                    let values = if assigned { variable.domain().len() } else { 0 };
                    let count = values + usize::from(unassign);
                    if count == 0 {
                        return None;
                    }
                    let target = random.below(count as u64) as usize;
                    let value = if target < values {
                        Some(&variable.domain()[target])
                    } else {
                        None
                    };
                    if value == a.values()[first].as_ref()
                        || !pillar
                            .rows()
                            .iter()
                            .all(|r| crate::pillar_schema::accepts(p, r[col], value))
                    {
                        return None;
                    }
                    Some(Move::Composite(
                        pillar
                            .rows()
                            .iter()
                            .map(|r| Move::Change {
                                variable: r[col],
                                value: value.cloned(),
                            })
                            .collect(),
                    ))
                } else {
                    let (right_id, right_sample) =
                        &right[random.below(right.len() as u64) as usize];
                    if left_id == right_id {
                        return None;
                    }
                    let left = left_sample.draw(&mut random);
                    let right = right_sample.draw(&mut random);
                    let valid = |to: &crate::Pillar, from: &crate::Pillar| {
                        to.rows().iter().all(|r| {
                            r.iter().zip(&from.rows()[0]).all(|(&to, &from)| {
                                crate::pillar_schema::accepts(p, to, a.values()[from].as_ref())
                            })
                        })
                    };
                    if !valid(&left, &right) || !valid(&right, &left) {
                        return None;
                    }
                    Some(left.swap_move(&right, a).expect("disjoint uniform groups"))
                }
            },
        )))
    }
}
impl<V: Eq + Clone> MoveProvider<FiniteProblem<V>, Assignment<V>, Move<V>>
    for SampledPillarMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a FiniteProblem<V>,
        a: &'a Assignment<V>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<V>> + 'a>
    where
        Move<V>: 'a,
    {
        self.select(p, a, step)
            .expect("sampled pillar provider requires a valid assignment")
    }
}
impl<B: Eq + Clone, L: Eq + Clone>
    MoveProvider<MixedProblem<B, L>, MixedAssignment<B, L>, MixedMove<B, L>>
    for SampledPillarMoveProvider
{
    fn moves<'a>(
        &'a mut self,
        p: &'a MixedProblem<B, L>,
        a: &'a MixedAssignment<B, L>,
        step: u64,
    ) -> Box<dyn Iterator<Item = MixedMove<B, L>> + 'a>
    where
        MixedMove<B, L>: 'a,
    {
        p.validate(a)
            .expect("sampled pillar provider requires a valid mixed assignment");
        Box::new(
            self.select(p.basic(), a.basic(), step)
                .expect("validated basic assignment")
                .map(|basic| MixedMove {
                    basic,
                    lists: ListMove::Composite(vec![]),
                }),
        )
    }
}
