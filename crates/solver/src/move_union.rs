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
    children: Vec<(I, u32)>,
    total: u128,
    random: Option<SplitMix64>,
}
impl<I: Iterator> MoveUnion<I> {
    pub fn new(children: impl IntoIterator<Item = (I, u32)>, order: UnionOrder) -> Self {
        let children: Vec<_> = children.into_iter().filter(|(_, w)| *w != 0).collect();
        let total = children.iter().map(|(_, w)| u128::from(*w)).sum();
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
            let index = if let Some(random) = &mut self.random {
                let mut draw = random.below_wide(self.total);
                self.children
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
            if let Some(item) = self.children[index].0.next() {
                return Some(item);
            }
            self.total -= u128::from(self.children.remove(index).1);
        }
        None
    }
}
impl<I: Iterator> std::iter::FusedIterator for MoveUnion<I> {}

impl<S> LocalSearchConfig<'_, S> {
    pub(crate) fn neighborhoods(&self) -> impl Iterator<Item = (Neighborhood, u32)> + '_ {
        self.neighborhood_union
            .is_none()
            .then_some((self.neighborhood, 1))
            .into_iter()
            .chain(
                self.neighborhood_union
                    .into_iter()
                    .flat_map(|u| u.children.iter().copied()),
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
