//! Snapshot-local reverse receiver memberships for sparse list swap scans.
use crate::{ListAssignment, ListPosition, ListProblem};
use std::sync::Arc;

struct Group {
    entity: usize,
    start: usize,
    end: usize,
    offset: usize,
}
pub(super) struct SwapReceivers {
    groups: Vec<Group>,
    restricted: Vec<Vec<usize>>,
    unrestricted: Vec<usize>,
}
impl SwapReceivers {
    pub(super) fn new<V: Eq + Clone>(
        p: &ListProblem<V>,
        a: &ListAssignment<V>,
        order: impl Iterator<Item = usize>,
    ) -> Option<Self> {
        let mut groups = vec![];
        let mut offset = 0;
        for entity in order {
            let meta = &p.entities()[entity];
            let start = meta.pinned_prefix();
            let end = a.lists()[entity].len();
            if meta.is_pinned() || start == end {
                continue;
            }
            groups.push(Group {
                entity,
                start,
                end,
                offset,
            });
            offset += end - start;
        }
        // A large registry alone does not justify indexing a tiny selected or
        // mostly pinned/empty layout. Decide before allocating per-value rows.
        if offset < 64 || p.values().len() as u128 > 8 * offset as u128 {
            return None;
        }
        let mut result = Self {
            groups,
            restricted: vec![vec![]; p.values().len()],
            unrestricted: vec![],
        };
        for (group, row) in result.groups.iter().enumerate() {
            match p.indexed_entity_range(row.entity) {
                Some(ids) => {
                    for &id in ids {
                        result.restricted[id].push(group);
                    }
                }
                None => result.unrestricted.push(group),
            }
        }
        Some(result)
    }
    pub(super) fn positions(
        self: &Arc<Self>,
        value: usize,
        after: Option<usize>,
    ) -> ReceiverPositions {
        ReceiverPositions {
            index: self.clone(),
            value,
            after,
            restricted: 0,
            unrestricted: 0,
            active: None,
        }
    }
}
pub(super) enum SwapTraversal<D, I> {
    Direct(D),
    Indexed(I),
}
impl<D: Iterator, I: Iterator<Item = D::Item>> Iterator for SwapTraversal<D, I> {
    type Item = D::Item;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Direct(i) => i.next(),
            Self::Indexed(i) => i.next(),
        }
    }
}
pub(super) enum SwapPositions<I> {
    Direct(I),
    Indexed(ReceiverPositions),
}
impl<I: Iterator<Item = ListPosition>> Iterator for SwapPositions<I> {
    type Item = ListPosition;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Direct(i) => i.next(),
            Self::Indexed(i) => i.next(),
        }
    }
}
pub(super) struct ReceiverPositions {
    index: Arc<SwapReceivers>,
    value: usize,
    after: Option<usize>,
    restricted: usize,
    unrestricted: usize,
    active: Option<(usize, usize)>,
}
impl Iterator for ReceiverPositions {
    type Item = ListPosition;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((group, index)) = &mut self.active {
                let row = &self.index.groups[*group];
                if *index < row.end {
                    let at = ListPosition {
                        entity: row.entity,
                        index: *index,
                    };
                    *index += 1;
                    return Some(at);
                }
                self.active = None;
            }
            // Both streams are sorted by supplied encounter order; merging
            // avoids sorting per source and never multiplies unrestricted groups.
            let restricted = self.index.restricted[self.value]
                .get(self.restricted)
                .copied();
            let unrestricted = self.index.unrestricted.get(self.unrestricted).copied();
            let group = match (restricted, unrestricted) {
                (None, None) => return None,
                (Some(r), Some(u)) if r < u => {
                    self.restricted += 1;
                    r
                }
                (Some(r), None) => {
                    self.restricted += 1;
                    r
                }
                (_, Some(u)) => {
                    self.unrestricted += 1;
                    u
                }
            };
            let row = &self.index.groups[group];
            let skip = self
                .after
                .map_or(0, |offset| (offset + 1).saturating_sub(row.offset));
            let start = row.start + skip.min(row.end - row.start);
            self.active = Some((group, start));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ListEntity, ListMove, Neighborhood};
    #[test]
    fn arbitrary_entity_order_preserves_occurrences_and_unique_pair_ranks() {
        let n = 40;
        let entities: Vec<_> = (0..n)
            .map(|e| {
                let row = ListEntity::new(vec![2 * e + 1, 2 * e]);
                if e % 9 == 0 {
                    row.with_pinned(true)
                } else {
                    row.with_pinned_prefix(usize::from(e % 5 == 0))
                        .with_value_range(
                            [e, (e + 1) % n, (e + n - 1) % n]
                                .into_iter()
                                .flat_map(|g| [2 * g, 2 * g + 1])
                                .collect(),
                        )
                }
            })
            .collect();
        let p = ListProblem::new_indexed((0..2 * n).rev().collect(), entities).unwrap();
        let a = p.initial_assignment();
        // Includes duplicate entity occurrences to guard generic ordered callers.
        let order: Vec<_> = (0..n).rev().chain([1, 7, 1]).collect();
        let positions: Vec<_> = order
            .iter()
            .flat_map(|&entity| {
                let meta = &p.entities()[entity];
                let start = if meta.is_pinned() {
                    a.lists()[entity].len()
                } else {
                    meta.pinned_prefix()
                };
                (start..a.lists()[entity].len()).map(move |index| ListPosition { entity, index })
            })
            .collect();
        for unique in [false, true] {
            let mut expected = vec![];
            for (l, &left) in positions.iter().enumerate() {
                for (r, &right) in positions.iter().enumerate() {
                    if (!unique || r > l)
                        && left != right
                        && p.accepts(left.entity, &a.lists()[right.entity][right.index])
                        && p.accepts(right.entity, &a.lists()[left.entity][left.index])
                    {
                        expected.push(ListMove::Swap { left, right });
                    }
                }
            }
            let family = if unique {
                Neighborhood::UniqueSwap
            } else {
                Neighborhood::Swap
            };
            let actual =
                super::super::ordered(&p, &a, family, order.iter().copied()).collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
    }
    #[test]
    fn abandoned_first_source_does_not_construct_receiver_index() {
        let p = ListProblem::new_indexed(
            (0..80).collect(),
            (0..40)
                .map(|e| {
                    ListEntity::new(vec![2 * e, 2 * e + 1]).with_value_range(vec![2 * e, 2 * e + 1])
                })
                .collect(),
        )
        .unwrap();
        assert!(p.has_sparse_entity_ranges());
        let a = p.initial_assignment();
        let order = std::iter::once(0).chain(std::iter::from_fn(|| -> Option<usize> {
            panic!("an early proposal must not scan later owners")
        }));
        let mut moves = super::super::ordered(&p, &a, Neighborhood::Swap, order);
        assert_eq!(
            moves.next(),
            Some(ListMove::Swap {
                left: ListPosition {
                    entity: 0,
                    index: 0
                },
                right: ListPosition {
                    entity: 0,
                    index: 1
                }
            })
        );
        drop(moves);
    }
    #[test]
    fn tiny_selected_or_mostly_unassigned_layout_does_not_allocate_memberships() {
        let p = ListProblem::new_indexed(
            (0..10_000).collect(),
            (0..40)
                .map(|e| {
                    ListEntity::new(vec![2 * e, 2 * e + 1]).with_value_range(vec![2 * e, 2 * e + 1])
                })
                .collect(),
        )
        .unwrap();
        let a = p.initial_assignment();
        assert!(p.has_sparse_entity_ranges());
        assert!(SwapReceivers::new(&p, &a, 0..40).is_none());
        let dense = ListProblem::new_indexed((0..80).collect(), p.entities().to_vec()).unwrap();
        assert!(SwapReceivers::new(&dense, &a, 0..2).is_none());
        assert!(SwapReceivers::new(&dense, &a, 0..40).is_some());
        assert_eq!(
            super::super::ordered(&p, &a, Neighborhood::Swap, 0..40).count(),
            80
        );
    }
}
