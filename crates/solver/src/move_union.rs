//! Lazy finite or infinite unions with independently weighted child selection.
use crate::{LocalSearchConfig, Neighborhood, random::SplitMix64};

/// Ordering between children; each child's internal order is preserved.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum UnionOrder {
    #[default]
    Sequential,
    /// Draw a child again for every item, in proportion to its positive weight.
    Random { seed: u64 },
}

/// Replace the single local-search neighborhood with a union. Zero-weight
/// children are disabled, including validation. An empty union yields no moves.
/// Duplicate moves are retained; weights select children, not individual moves.
///
/// ```
/// use timetuck::*;
/// let union = NeighborhoodUnion {
///     children: &[(Neighborhood::Change, 3), (Neighborhood::Swap, 1)],
///     order: UnionOrder::Random { seed: 42 },
/// };
/// let config = LocalSearchConfig::<SimpleScore> {
///     neighborhood_union: Some(&union),
///     ..Default::default()
/// };
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NeighborhoodUnion<'a> {
    pub children: &'a [(Neighborhood, u32)],
    pub order: UnionOrder,
}

/// Lazy union of iterators. No child is polled at construction; only the selected
/// child is polled on each draw. Exhausted children are removed permanently.
/// Zero weights disable children in either order. Sequential ordering ignores
/// positive weight magnitudes. Finite children are exhausted exactly once;
/// infinite children require a consumer budget (and can starve later sequential
/// children). Random selection is reproducible, not Java PRNG compatible.
#[derive(Clone, Debug)]
pub struct MoveUnion<I> {
    children: Children<I>,
    total: u128,
    random: Option<SplitMix64>,
}
// Large random unions retain stable slots; exhausted iterators are dropped at
// once, while zero weights remove their intervals from this prefix-sum tree.
// Small and sequential unions keep the original allocation and selection path.
#[derive(Clone, Debug)]
enum Children<I> {
    Linear(Vec<(I, u32)>),
    Indexed {
        slots: Vec<Option<(I, u32)>>,
        weights: WeightIndex,
    },
}
#[derive(Clone, Debug)]
struct WeightIndex(Vec<u128>);
impl WeightIndex {
    fn new(weights: impl ExactSizeIterator<Item = u32>) -> Self {
        let mut tree = vec![0; weights.len() + 1];
        for (zero, weight) in weights.enumerate() {
            let i = zero + 1;
            tree[i] += u128::from(weight);
            let parent = i + (i & i.wrapping_neg());
            if parent < tree.len() {
                tree[parent] += tree[i];
            }
        }
        Self(tree)
    }
    fn remove(&mut self, slot: usize, weight: u32) {
        let mut i = slot + 1;
        while i < self.0.len() {
            self.0[i] -= u128::from(weight);
            i += i & i.wrapping_neg();
        }
    }
    fn select(&self, mut rank: u128) -> usize {
        let mut slot = 0;
        let mut step = 1usize << (usize::BITS - 1 - self.0.len().leading_zeros());
        while step != 0 {
            let next = slot + step;
            if next < self.0.len() && self.0[next] <= rank {
                rank -= self.0[next];
                slot = next;
            }
            step >>= 1;
        }
        slot
    }
}
impl<I: Iterator> MoveUnion<I> {
    pub fn new(children: impl IntoIterator<Item = (I, u32)>, order: UnionOrder) -> Self {
        let children: Vec<_> = children.into_iter().filter(|(_, w)| *w != 0).collect();
        let total = children.iter().map(|(_, w)| u128::from(*w)).sum();
        let children = if children.len() >= 32 && matches!(order, UnionOrder::Random { .. }) {
            let weights = WeightIndex::new(children.iter().map(|(_, w)| *w));
            Children::Indexed {
                slots: children.into_iter().map(Some).collect(),
                weights,
            }
        } else {
            Children::Linear(children)
        };
        Self {
            children,
            total,
            random: match order {
                UnionOrder::Sequential => None,
                UnionOrder::Random { seed } => Some(SplitMix64::new(seed)),
            },
        }
    }
}
impl<I: Iterator> Iterator for MoveUnion<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        while self.total != 0 {
            match &mut self.children {
                Children::Linear(children) => {
                    let index = if let Some(random) = &mut self.random {
                        let mut draw = random.below_wide(self.total);
                        children
                            .iter()
                            .position(|(_, weight)| {
                                if draw < u128::from(*weight) {
                                    true
                                } else {
                                    draw -= u128::from(*weight);
                                    false
                                }
                            })
                            .expect("positive union weight")
                    } else {
                        0
                    };
                    if let Some(item) = children[index].0.next() {
                        return Some(item);
                    }
                    self.total -= u128::from(children.remove(index).1);
                }
                Children::Indexed { slots, weights } => {
                    let rank = self
                        .random
                        .as_mut()
                        .expect("random indexed union")
                        .below_wide(self.total);
                    let slot = weights.select(rank);
                    let child = slots[slot].as_mut().expect("positive indexed weight");
                    if let Some(item) = child.0.next() {
                        return Some(item);
                    }
                    let weight = child.1;
                    // Release the exhausted iterator immediately, without shifting
                    // other children or changing their draw intervals.
                    weights.remove(slot, weight);
                    self.total -= u128::from(weight);
                    slots[slot] = None;
                }
            }
        }
        None
    }
}
impl<I: Iterator> std::iter::FusedIterator for MoveUnion<I> {}

impl<S> LocalSearchConfig<'_, S> {
    pub(crate) fn neighborhoods(&self) -> impl Iterator<Item = (Neighborhood, u32)> + '_ {
        self.neighborhood_union
            .is_none()
            .then(|| (self.neighborhood.clone(), 1))
            .into_iter()
            .chain(
                self.neighborhood_union
                    .into_iter()
                    .flat_map(|u| u.children.iter().cloned()),
            )
            .filter(|(_, w)| *w != 0)
    }
    pub(crate) fn select_neighborhoods<I: Iterator>(
        &self,
        step: u64,
        mut make: impl FnMut(Neighborhood) -> I,
    ) -> impl Iterator<Item = I::Item> {
        if self.neighborhood_union.is_none() {
            return SelectedNeighborhoods::Single(make(self.neighborhood.at_step(step)));
        }
        let order = match self.neighborhood_union.map(|u| u.order).unwrap_or_default() {
            UnionOrder::Random { seed } => UnionOrder::Random {
                seed: seed.wrapping_add(step),
            },
            order => order,
        };
        SelectedNeighborhoods::Union(MoveUnion::new(
            self.neighborhoods()
                .map(|(n, w)| (make(n.at_step(step)), w)),
            order,
        ))
    }
}

// Preserve the single-family path without allocating a union or selecting a child.
enum SelectedNeighborhoods<I> {
    Single(I),
    Union(MoveUnion<I>),
}
impl<I: Iterator> Iterator for SelectedNeighborhoods<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Single(i) => i.next(),
            Self::Union(i) => i.next(),
        }
    }
}

#[cfg(test)]
mod indexed_tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn weighted_index_matches_expanded_intervals_after_every_removal() {
        for count in 1..130 {
            let mut weights: Vec<_> = (0..count).map(|i| (i % 9) as u32).collect();
            let mut index = WeightIndex::new(weights.iter().copied());
            for offset in 0..count {
                let expected: Vec<_> = weights
                    .iter()
                    .enumerate()
                    .flat_map(|(i, &w)| std::iter::repeat_n(i, w as usize))
                    .collect();
                for (rank, &slot) in expected.iter().enumerate() {
                    assert_eq!(index.select(rank as u128), slot);
                }
                let slot = (offset + count / 2) % count;
                index.remove(slot, weights[slot]);
                weights[slot] = 0;
            }
        }
    }

    #[test]
    fn large_weight_boundaries_do_not_truncate_totals() {
        let weights = [u32::MAX; 65];
        let mut index = WeightIndex::new(weights.into_iter());
        let width = u128::from(u32::MAX);
        for i in 0..65 {
            assert_eq!(index.select(i as u128 * width), i);
            assert_eq!(index.select((i as u128 + 1) * width - 1), i);
        }
        index.remove(32, u32::MAX);
        assert_eq!(index.select(32 * width - 1), 31);
        assert_eq!(index.select(32 * width), 33);
        assert_eq!(index.select(64 * width - 1), 64);
    }

    #[derive(Clone)]
    struct Child {
        id: usize,
        left: usize,
        events: Rc<RefCell<Vec<(usize, bool)>>>,
    }
    impl Iterator for Child {
        type Item = (usize, usize);
        fn next(&mut self) -> Option<Self::Item> {
            self.events.borrow_mut().push((self.id, true));
            if self.left == 0 {
                return None;
            }
            self.left -= 1;
            Some((self.id, self.left))
        }
    }
    impl Drop for Child {
        fn drop(&mut self) {
            self.events.borrow_mut().push((self.id, false));
        }
    }
    // Independent cumulative-interval reference, including eager exhaustion drops.
    fn reference_next(
        children: &mut Vec<(Child, u32)>,
        rng: &mut SplitMix64,
    ) -> Option<(usize, usize)> {
        while !children.is_empty() {
            let total: u128 = children.iter().map(|(_, w)| u128::from(*w)).sum();
            let rank = rng.below_wide(total);
            let mut end = 0;
            let selected = children
                .iter()
                .position(|(_, w)| {
                    end += u128::from(*w);
                    rank < end
                })
                .unwrap();
            if let Some(item) = children[selected].0.next() {
                return Some(item);
            }
            children.remove(selected);
        }
        None
    }

    #[test]
    fn seeded_draws_polls_and_drops_match_reference_through_exhaustion_and_clone() {
        for count in [0, 1, 8, 31, 32, 33, 65, 257] {
            for seed in [0, 1, 42, u64::MAX] {
                let actual_events = Rc::new(RefCell::new(vec![]));
                let reference_events = Rc::new(RefCell::new(vec![]));
                let make = |events: &Rc<RefCell<_>>| {
                    (0..count)
                        .map(|id| {
                            let weight = if id % 11 == 0 {
                                0
                            } else if id % 3 == 0 {
                                u32::MAX
                            } else {
                                1
                            };
                            (
                                Child {
                                    id,
                                    left: id % 7,
                                    events: events.clone(),
                                },
                                weight,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let mut actual = MoveUnion::new(make(&actual_events), UnionOrder::Random { seed });
                let mut expected: Vec<_> = make(&reference_events)
                    .into_iter()
                    .filter(|(_, w)| *w != 0)
                    .collect();
                let mut rng = SplitMix64::new(seed);
                for step in 0.. {
                    let next = reference_next(&mut expected, &mut rng);
                    assert_eq!(actual.next(), next, "count={count} seed={seed} step={step}");
                    assert_eq!(*actual_events.borrow(), *reference_events.borrow());
                    if step == 5 {
                        // Cloning an in-flight tree must retain holes, weights and RNG.
                        let clone = actual.clone().collect::<Vec<_>>();
                        let mut reference_clone = expected.clone();
                        let mut random_clone = rng.clone();
                        let mut rows = vec![];
                        while let Some(row) =
                            reference_next(&mut reference_clone, &mut random_clone)
                        {
                            rows.push(row);
                        }
                        assert_eq!(clone, rows);
                        actual_events.borrow_mut().clear();
                        reference_events.borrow_mut().clear();
                    }
                    if next.is_none() {
                        break;
                    }
                }
                assert_eq!(actual.next(), None);
                assert_eq!(*actual_events.borrow(), *reference_events.borrow());
            }
        }
    }

    #[test]
    fn empty_and_disabled_children_are_not_polled_before_selected() {
        let events = Rc::new(RefCell::new(vec![]));
        let mut union = MoveUnion::new(
            (0..100).map(|id| {
                (
                    Child {
                        id,
                        left: usize::MAX,
                        events: events.clone(),
                    },
                    if id == 0 { 0 } else { 1 },
                )
            }),
            UnionOrder::Random { seed: 4 },
        );
        assert_eq!(&*events.borrow(), &[(0, false)]);
        events.borrow_mut().clear();
        for _ in 0..20 {
            union.next();
        }
        assert_eq!(events.borrow().len(), 20);
        assert!(events.borrow().iter().all(|&(id, poll)| id != 0 && poll));
    }
}
