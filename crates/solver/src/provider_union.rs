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
    unweighted: bool,
    primed_polling: bool,
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
        Ok(Self {
            children,
            order,
            unweighted: false,
            primed_polling: false,
        })
    }
    /// Compose children with unit selection weights and retain their explicit
    /// unweighted structure for neighborhood counting. Candidate selection still
    /// chooses immediate children equally; counting alone does not flatten them.
    /// Use MoveProviderGroup to flatten leaf probabilities across nested groups.
    pub fn unweighted(
        children: impl IntoIterator<Item = &'p mut dyn MoveProvider<P, A, M>>,
        order: UnionOrder,
    ) -> Result<Self, EmptyMoveProviders> {
        let mut result = Self::new(children.into_iter().map(|p| (p, 1)), order)?;
        result.unweighted = true;
        Ok(result)
    }
    /// Opt in to availability polling: opening a scan primes every enabled child,
    /// then each draw refills only its selected child before returning the move.
    /// Iterator lookahead buffers candidates without scoring them. Child factories
    /// therefore run before the first draw, and callback panics fuse that scan.
    /// Existing lazy factory behavior is retained unless this method is called.
    pub fn with_primed_polling(mut self) -> Self {
        self.primed_polling = true;
        self
    }

    /// Read-only provider identity and weights, in construction order.
    pub fn providers(&self) -> impl Iterator<Item = (&dyn MoveProvider<P, A, M>, u32)> {
        self.children
            .iter()
            .map(|(provider, weight)| (&**provider, *weight))
    }
}
impl<P, A, M> MoveProvider<P, A, M> for ProviderUnion<'_, P, A, M> {
    fn neighborhood_count(&self) -> Result<usize, crate::NeighborhoodCountError> {
        if !self.unweighted {
            return Err(crate::NeighborhoodCountError::WeightedUnion);
        }
        self.children.iter().try_fold(0usize, |n, (child, _)| {
            n.checked_add(child.neighborhood_count()?)
                .ok_or(crate::NeighborhoodCountError::Overflow)
        })
    }
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
        if self.primed_polling {
            Box::new(crate::PrimedMoveUnion::from_iterators(children, order))
        } else {
            Box::new(MoveUnion::new(children, order))
        }
    }
}
