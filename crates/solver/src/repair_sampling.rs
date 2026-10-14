//! Native repair sampling; decks are private to one weighted repair pool.
use crate::random::SplitMix64;

#[derive(Clone, Default)]
pub(crate) struct CoverageDeck {
    eligible: Vec<usize>,
    deck: Vec<usize>,
    cursor: usize,
}
impl CoverageDeck {
    pub(crate) fn sample(
        &mut self,
        eligible: &[usize],
        count: usize,
        random: &mut SplitMix64,
    ) -> Vec<usize> {
        assert!(count <= eligible.len());
        if self.eligible != eligible {
            self.eligible = eligible.to_vec();
            self.deck = eligible.to_vec();
            self.cursor = self.deck.len();
        }
        let mut selected = Vec::with_capacity(count);
        while selected.len() < count {
            if self.cursor == self.deck.len() {
                random.shuffle(&mut self.deck);
                self.cursor = 0;
            }
            // At a cycle boundary, defer identities already in this group. They
            // stay in the unconsumed suffix, so no cycle loses an identity.
            let next = (self.cursor..self.deck.len())
                .find(|&i| !selected.contains(&self.deck[i]))
                .expect("a group no larger than its population has a fresh identity");
            self.deck.swap(self.cursor, next);
            selected.push(self.deck[self.cursor]);
            self.cursor += 1;
        }
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn seeded_groups_are_unique_and_each_cycle_covers_the_population() {
        for size in 1..=16 {
            let population: Vec<_> = (0..size).map(|i| i * 7 + 19).collect();
            for count in 1..=size {
                for seed in 0..32 {
                    let mut deck = CoverageDeck::default();
                    let mut replay = CoverageDeck::default();
                    let mut random = SplitMix64::new(seed);
                    let mut replay_random = SplitMix64::new(seed);
                    let mut draws = Vec::new();
                    for _ in 0..size * 3 {
                        let group = deck.sample(&population, count, &mut random);
                        assert_eq!(group, replay.sample(&population, count, &mut replay_random));
                        assert_eq!(group.len(), count);
                        assert_eq!(group.iter().collect::<BTreeSet<_>>().len(), count);
                        draws.extend(group);
                    }
                    let expected: BTreeSet<_> = population.iter().copied().collect();
                    for cycle in draws.chunks_exact(size) {
                        assert_eq!(cycle.iter().copied().collect::<BTreeSet<_>>(), expected);
                    }
                }
            }
        }
    }

    #[test]
    fn zero_draws_are_safe_and_do_not_consume_randomness() {
        let mut deck = CoverageDeck::default();
        let mut random = SplitMix64::new(17);
        assert!(deck.sample(&[], 0, &mut random).is_empty());
        assert!(deck.sample(&[2, 9], 0, &mut random).is_empty());
        let mut untouched = SplitMix64::new(17);
        assert_eq!(random.below(100), untouched.below(100));
    }

    #[test]
    fn eligibility_changes_reset_and_pools_keep_separate_cursors() {
        let mut pools = [CoverageDeck::default(), CoverageDeck::default()];
        let mut random = SplitMix64::new(42);
        pools[0].sample(&[1, 3, 5], 2, &mut random);
        let old_cursor = pools[0].cursor;
        assert_eq!(pools[1].sample(&[7, 9], 2, &mut random).len(), 2);
        assert_eq!(pools[0].cursor, old_cursor);
        let changed = pools[0].sample(&[3, 8], 2, &mut random);
        assert_eq!(
            changed.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([3, 8])
        );
        assert_eq!(pools[0].cursor, 2);
        assert_eq!(pools[1].eligible, [7, 9]);
    }
}
