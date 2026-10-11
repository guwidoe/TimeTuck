//! Encounter-ordered grouping with optional, bounded collision lookup.
use std::{
    collections::{HashMap, hash_map::RandomState},
    sync::Arc,
};

const INDEX_GROUPS: usize = 64;
pub(crate) const INDEX_ROWS: usize = 256;
const MAX_COLLISIONS: usize = 8;

struct Index {
    state: RandomState,
    heads: HashMap<u64, (usize, usize)>,
    next: Vec<usize>,
}
impl Index {
    fn new(groups: &[Vec<usize>], hash: &impl Fn(&RandomState, usize) -> u64) -> Option<Self> {
        let mut index = Self {
            state: RandomState::new(),
            heads: HashMap::new(),
            next: Vec::new(),
        };
        for group in groups {
            if !index.insert(hash(&index.state, group[0])) {
                return None;
            }
        }
        Some(index)
    }
    // Group IDs are append-only, and identical tuple hashes retain distinct
    // equality groups. Disable indexing rather than lengthening collision chains.
    fn insert(&mut self, hash: u64) -> bool {
        let (head, count) = self.heads.get(&hash).copied().unwrap_or((usize::MAX, 0));
        if count >= MAX_COLLISIONS {
            return false;
        }
        self.heads.insert(hash, (self.next.len(), count + 1));
        self.next.push(head);
        true
    }
    fn find(&self, hash: u64, mut equal: impl FnMut(usize) -> bool) -> Option<usize> {
        let mut group = self.heads.get(&hash)?.0;
        while group != usize::MAX {
            if equal(group) {
                return Some(group);
            }
            group = self.next[group];
        }
        None
    }
}

#[inline]
pub(crate) fn group_indices(
    order: impl Iterator<Item = usize>,
    hash: Option<impl Fn(&RandomState, usize) -> u64>,
    equal: impl Fn(usize, usize) -> bool,
) -> Arc<Vec<Vec<usize>>> {
    let mut groups: Vec<Vec<usize>> = vec![];
    let Some(hash) = hash else {
        // Default/Eq-only models keep the original loop with no per-row index
        // checks, random state creation or hash-table allocation.
        for row in order {
            if let Some(group) = groups.iter_mut().find(|g| equal(g[0], row)) {
                group.push(row);
            } else {
                groups.push(vec![row]);
            }
        }
        return Arc::new(groups);
    };
    let mut index = None;
    let mut attempted = false;
    for (processed, row) in order.enumerate() {
        if !attempted && processed >= INDEX_ROWS && groups.len() >= INDEX_GROUPS {
            attempted = true;
            index = Index::new(&groups, &hash);
        }
        if let Some(lookup) = &mut index {
            let h = hash(&lookup.state, row);
            if let Some(group) = lookup.find(h, |g| equal(groups[g][0], row)) {
                groups[group].push(row);
            } else {
                groups.push(vec![row]);
                if !lookup.insert(h) {
                    index = None;
                }
            }
        } else if let Some(group) = groups.iter_mut().find(|g| equal(g[0], row)) {
            group.push(row);
        } else {
            groups.push(vec![row]);
        }
    }
    Arc::new(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FiniteProblem, PillarSchema, Variable};
    #[test]
    fn indexed_groups_preserve_duplicate_encounters_and_schema_deduplication() {
        let variables: Vec<_> = (0..800)
            .map(|i| {
                let mut v = Variable::new(vec![i % 101]).allowing_unassigned();
                if i % 23 != 0 {
                    v = v.with_initial(i % 101);
                }
                v.with_pinned(i % 37 == 0)
            })
            .collect();
        let plain = FiniteProblem::new(variables.clone()).unwrap();
        let indexed = FiniteProblem::new_indexed(variables).unwrap();
        let a = plain.initial_assignment();
        let orders = [
            (0..800).collect::<Vec<_>>(),
            (0..800).rev().chain(50..250).collect(),
        ];
        let schema = PillarSchema::new((0..400).map(|i| vec![2 * i + 1, 2 * i]).collect()).unwrap();
        for order in orders {
            assert_eq!(
                crate::pillar::groups(&plain, &a, order.iter().copied()),
                crate::pillar::groups(&indexed, &a, order.iter().copied())
            );
            // Exercise the private selected-slot order through actual moves;
            // repeated slots must not duplicate a selected entity row.
            let l = plain.clone().with_pillar_schema(schema.clone()).unwrap();
            let r = indexed.clone().with_pillar_schema(schema.clone()).unwrap();
            assert_eq!(
                crate::pillar::ordered(&l, &a, false, order.iter().copied()).collect::<Vec<_>>(),
                crate::pillar::ordered(&r, &a, false, order.iter().copied()).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn collision_limit_disables_lookup_without_changing_group_order() {
        let order = (0..256).chain((0..256).rev());
        let expected = group_indices(
            order.clone(),
            None::<fn(&RandomState, usize) -> u64>,
            |l, r| l == r,
        );
        let actual = group_indices(order, Some(|_: &RandomState, _| 0), |l, r| l == r);
        assert_eq!(actual, expected);
        // A collision cluster arriving after successful activation also falls
        // back, while retaining all previously grouped members.
        let hash = |_: &RandomState, i| if i < 300 { i as u64 } else { 999 };
        let actual = group_indices((0..512).chain(0..512), Some(hash), |l, r| l == r);
        assert_eq!(*actual, (0..512).map(|i| vec![i, i]).collect::<Vec<_>>());
    }
}
