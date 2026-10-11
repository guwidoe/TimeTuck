//! Post-removal slot ranks without rescanning every receiver per proposal.
use crate::{ListAssignment, ListProblem, random::SplitMix64};

pub(super) struct DestinationSlots {
    through: Vec<u128>,
}
impl DestinationSlots {
    pub(super) fn new<V: Eq + Clone>(
        problem: &ListProblem<V>,
        assignment: &ListAssignment<V>,
    ) -> Self {
        let mut total = 0;
        let through = problem
            .entities()
            .iter()
            .enumerate()
            .map(|(entity, metadata)| {
                if !metadata.is_pinned() {
                    total +=
                        (assignment.lists()[entity].len() - metadata.pinned_prefix()) as u128 + 1;
                }
                total
            })
            .collect();
        Self { through }
    }

    fn at(&self, source: usize, removed: usize, rank: u128) -> (usize, usize) {
        // Removing a span shortens only its source's interval. Expand every rank
        // after that shortened interval back into the unmodified prefix table.
        // The gap is at the interval end, not the span's original start: native
        // destination indices already describe the list after source removal.
        let boundary = self.through[source] - removed as u128;
        let expanded = rank + if rank >= boundary { removed as u128 } else { 0 };
        let entity = self.through.partition_point(|&end| end <= expanded);
        let before = entity.checked_sub(1).map_or(0, |i| self.through[i]);
        (entity, (expanded - before) as usize)
    }

    pub(super) fn draw(
        &self,
        source: usize,
        removed: usize,
        unassignment: bool,
        random: &mut SplitMix64,
    ) -> Option<(usize, usize)> {
        let count = self.through.last().expect("movable source") - removed as u128;
        let rank = random.below_wide(count + u128::from(unassignment));
        (rank != count).then(|| self.at(source, removed, rank))
    }
}

pub(super) fn direct<V: Eq + Clone>(
    problem: &ListProblem<V>,
    assignment: &ListAssignment<V>,
    source: usize,
    removed: usize,
    unassignment: bool,
    random: &mut SplitMix64,
) -> Option<(usize, usize)> {
    let width = |entity: usize| {
        let metadata = &problem.entities()[entity];
        if metadata.is_pinned() {
            0
        } else {
            let available = assignment.lists()[entity].len() - metadata.pinned_prefix();
            (available - if entity == source { removed } else { 0 }) as u128 + 1
        }
    };
    let slots: u128 = (0..problem.entities().len()).map(width).sum();
    let mut rank = random.below_wide(slots + u128::from(unassignment));
    if rank == slots {
        return None;
    }
    let entity = (0..problem.entities().len())
        .find(|&entity| {
            let count = width(entity);
            if rank < count {
                true
            } else {
                rank -= count;
                false
            }
        })
        .expect("the movable source has an insertion slot");
    Some((entity, rank as usize))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ListEntity;

    #[test]
    fn every_adjusted_rank_matches_independent_slot_enumeration() {
        // Repeated prefix sums from whole pins, empty lists, append-only prefixes,
        // and sources on either side of each cumulative boundary.
        let mut next = 0;
        let entities = (0..67)
            .map(|entity| {
                let count = entity % 7;
                let row = (next..next + count).collect();
                next += count;
                ListEntity::new(row)
                    .with_pinned(entity % 5 == 0)
                    .with_pinned_prefix(if entity % 5 == 0 { 0 } else { count / 2 })
            })
            .collect();
        let p = ListProblem::new_indexed((0..next).collect(), entities).unwrap();
        let a = p.initial_assignment();
        let index = DestinationSlots::new(&p, &a);
        for source in 0..p.entities().len() {
            let meta = &p.entities()[source];
            if meta.is_pinned() {
                continue;
            }
            for removed in 1..=a.lists()[source].len() - meta.pinned_prefix() {
                let expected: Vec<_> = p
                    .entities()
                    .iter()
                    .enumerate()
                    .filter(|(_, m)| !m.is_pinned())
                    .flat_map(|(entity, m)| {
                        let length = a.lists()[entity].len()
                            - m.pinned_prefix()
                            - if entity == source { removed } else { 0 };
                        (0..=length).map(move |slot| (entity, slot))
                    })
                    .collect();
                assert_eq!(
                    index.through.last().unwrap() - removed as u128,
                    expected.len() as u128
                );
                for (rank, expected) in expected.into_iter().enumerate() {
                    assert_eq!(index.at(source, removed, rank as u128), expected);
                }
            }
        }
    }

    #[test]
    fn indexed_and_direct_draws_preserve_random_state_and_unassignment() {
        for entities in [1, 4, 32, 65, 512] {
            let rows = (0..entities)
                .map(|entity| {
                    ListEntity::new((entity * 5..entity * 5 + 5).collect())
                        .with_pinned(entity % 7 == 1)
                        .with_pinned_prefix(if entity % 7 == 1 { 0 } else { entity % 3 })
                })
                .collect();
            let p = ListProblem::new_indexed((0..entities * 5).collect(), rows).unwrap();
            let a = p.initial_assignment();
            let index = DestinationSlots::new(&p, &a);
            for optional in [false, true] {
                for seed in [0, 19, u64::MAX] {
                    let mut indexed = SplitMix64::new(seed);
                    let mut scanned = SplitMix64::new(seed);
                    for attempt in 0..1000 {
                        let mut source = attempt % entities;
                        if p.entities()[source].is_pinned() {
                            source = 0;
                        }
                        let available = 5 - p.entities()[source].pinned_prefix();
                        let removed = 1 + attempt % available;
                        assert_eq!(
                            index.draw(source, removed, optional, &mut indexed),
                            direct(&p, &a, source, removed, optional, &mut scanned)
                        );
                        // Interleaved span and reversal draws must also remain identical.
                        assert_eq!(
                            indexed.below_wide(1 + attempt as u128),
                            scanned.below_wide(1 + attempt as u128)
                        );
                        assert_eq!(indexed.below(2), scanned.below(2));
                    }
                }
            }
        }
    }
}
