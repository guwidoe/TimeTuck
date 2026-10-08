//! Lazily combine custom providers using the existing solver extension API.
use crate::{MoveProvider, MoveUnion, UnionOrder};
use std::{error::Error, fmt};

/// A borrowed provider and its selection weight. Zero disables it entirely.
pub type WeightedMoveProvider<'p, P, A, M> = (&'p mut dyn MoveProvider<P, A, M>, u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmptyMoveProviders;
impl fmt::Display for EmptyMoveProviders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("at least one move provider is required")
    }
}
impl Error for EmptyMoveProviders {}

/// Compose providers for any model/scoring backend accepting MoveProvider.
/// Construction retains providers without invoking them. Each moves call opens a
/// union for the current immutable assignment and completed-step index; a child
/// factory runs only when first selected. Disabled children are never called.
/// Dropping the union iterator drops its child cursors. Lifecycle events reach
/// every enabled child, even one whose lazy cursor was never opened.
///
/// Positive weights affect random child choice, not ordering within a child.
/// Random seeds advance by completed_steps with wrapping addition. Sequential
/// selection drains children in order and may starve later children when a prior
/// child is infinite. Duplicates remain; exhausted children are not reopened within
/// a step. Consumers/search limits must bound infinite productive providers.
pub struct ProviderUnion<'p, P, A, M> {
    children: Vec<WeightedMoveProvider<'p, P, A, M>>,
    order: UnionOrder,
}
impl<'p, P, A, M> ProviderUnion<'p, P, A, M> {
    /// Empty lists are rejected. A nonempty list with all weights zero is valid
    /// and produces no moves, preserving the disabled-provider contract.
    pub fn new(
        children: impl IntoIterator<Item = WeightedMoveProvider<'p, P, A, M>>,
        order: UnionOrder,
    ) -> Result<Self, EmptyMoveProviders> {
        let children: Vec<_> = children.into_iter().collect();
        if children.is_empty() {
            return Err(EmptyMoveProviders);
        }
        Ok(Self { children, order })
    }
    /// Read-only provider identity and weights, in construction order.
    pub fn providers(&self) -> impl Iterator<Item = (&dyn MoveProvider<P, A, M>, u32)> {
        self.children
            .iter()
            .map(|(provider, weight)| (&**provider, *weight))
    }
}
impl<P, A, M> MoveProvider<P, A, M> for ProviderUnion<'_, P, A, M> {
    fn event(&mut self, event: crate::MoveProviderEvent) {
        for (provider, weight) in &mut self.children {
            if *weight != 0 {
                provider.event(event);
            }
        }
    }

    fn moves<'a>(
        &'a mut self,
        problem: &'a P,
        assignment: &'a A,
        completed_steps: u64,
    ) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        let order = match self.order {
            UnionOrder::Sequential => UnionOrder::Sequential,
            UnionOrder::Random { seed } => UnionOrder::Random {
                seed: seed.wrapping_add(completed_steps),
            },
        };
        let children = self
            .children
            .iter_mut()
            .filter(|(_, weight)| *weight != 0)
            .map(|(provider, weight)| {
                let lazy = std::iter::once_with(move || {
                    provider.moves(problem, assignment, completed_steps)
                })
                .flatten();
                (lazy, *weight)
            });
        Box::new(MoveUnion::new(children, order))
    }
}
