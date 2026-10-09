//! Group providers without changing the selection probability of their leaves.
use crate::{EmptyMoveProviders, MoveProvider, ProviderUnion, UnionOrder};

/// A composition tree whose leaves each represent one move generator. Random
/// selection gives every leaf equal weight, so a group's share is proportional
/// to its leaf count. Nesting and empty groups do not alter that distribution.
/// Sequential selection preserves depth-first leaf order.
///
/// Compile the tree with `into_union` before passing it to the solver. Compilation
/// flattens the tree once; it invokes no provider factories or lifecycle callbacks.
/// The resulting ProviderUnion opens selected leaves lazily for each snapshot,
/// forwards lifecycle events, and removes exhausted leaves from that scan.
///
/// An opaque provider is one leaf, even if its implementation internally combines
/// generators. Keep groups explicit until compilation to retain their leaf counts.
/// This API does not inspect arbitrary wrappers or infer weights from move counts.
/// Custom weighted composition remains available through ProviderUnion::new.
/// Counting and compilation use iterative traversal, in O(tree size) time.
pub enum MoveProviderGroup<'p, P, A, M> {
    Leaf(&'p mut dyn MoveProvider<P, A, M>),
    Group(Vec<Self>),
}
impl<'p, P, A, M> MoveProviderGroup<'p, P, A, M> {
    pub fn leaf(provider: &'p mut dyn MoveProvider<P, A, M>) -> Self {
        Self::Leaf(provider)
    }
    pub fn group(children: impl IntoIterator<Item = Self>) -> Self {
        Self::Group(children.into_iter().collect())
    }
    pub fn leaf_count(&self) -> usize {
        let mut pending = vec![self];
        let mut count = 0;
        while let Some(node) = pending.pop() {
            match node {
                Self::Leaf(_) => count += 1,
                Self::Group(children) => pending.extend(children.iter()),
            }
        }
        count
    }
    /// Consume the grouping structure and assign unit weight to each leaf.
    /// Trees containing no leaves return EmptyMoveProviders. Exhaustion is handled
    /// per leaf, so finite children need not remain selectable with their group's
    /// original weight after they run out of moves.
    pub fn into_union(
        self,
        order: UnionOrder,
    ) -> Result<ProviderUnion<'p, P, A, M>, EmptyMoveProviders> {
        let mut pending = vec![self];
        let mut leaves = Vec::new();
        while let Some(node) = pending.pop() {
            match node {
                Self::Leaf(provider) => leaves.push((provider, 1)),
                Self::Group(children) => pending.extend(children.into_iter().rev()),
            }
        }
        ProviderUnion::new(leaves, order)
    }
}
