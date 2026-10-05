//! Deduplicated scheduling against immutable dependency-order snapshots.
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashSet},
    error::Error,
    fmt,
    iter::FusedIterator,
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnknownOrderNode {
    pub node: usize,
    pub node_count: usize,
}
impl fmt::Display for UnknownOrderNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "node {} is outside the order snapshot of {} nodes",
            self.node, self.node_count
        )
    }
}
impl Error for UnknownOrderNode {}

/// Queue only explicitly offered nodes, ordered by (rank, node ID). Offering a
/// queued node does nothing; after polling it may be offered again. Equal ranks
/// are valid for nodes in the same dependency component or unrelated nodes.
///
/// Ranks are captured immutably. Construct a new queue after topology changes;
/// mutation of an external graph cannot silently reorder a populated queue.
/// Rank storage may be shared with Arc. Empty work storage is constant-size;
/// the heap and membership set grow with queued work, not the node universe.
/// Cloning preserves queued work independently while sharing the ranks.
///
/// ```
/// use timetuck::*;
/// let mut q = NodeOrderQueue::new(vec![20, 0, 10]);
/// assert!(q.offer(0)?);
/// assert!(!q.offer(0)?);
/// q.offer(1)?;
/// q.offer(2)?;
/// assert_eq!(q.drain().collect::<Vec<_>>(), [1, 2, 0]);
/// assert!(q.offer(0)?);
/// # Ok::<(), UnknownOrderNode>(())
/// ```
#[derive(Clone, Debug)]
pub struct NodeOrderQueue {
    ranks: Arc<[usize]>,
    ready: BinaryHeap<Reverse<(usize, usize)>>,
    members: HashSet<usize>,
}
impl NodeOrderQueue {
    pub fn new(ranks: impl Into<Arc<[usize]>>) -> Self {
        Self {
            ranks: ranks.into(),
            ready: BinaryHeap::new(),
            members: HashSet::new(),
        }
    }
    pub fn ranks(&self) -> &[usize] {
        &self.ranks
    }
    pub fn len(&self) -> usize {
        self.ready.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ready.is_empty()
    }
    pub fn contains(&self, node: usize) -> bool {
        self.members.contains(&node)
    }
    /// Return true only when new work was scheduled. Unknown IDs are rejected
    /// without changing the queue, including when the universe is empty.
    pub fn offer(&mut self, node: usize) -> Result<bool, UnknownOrderNode> {
        let rank = *self.ranks.get(node).ok_or(UnknownOrderNode {
            node,
            node_count: self.ranks.len(),
        })?;
        if !self.members.insert(node) {
            return Ok(false);
        }
        self.ready.push(Reverse((rank, node)));
        Ok(true)
    }
    pub fn peek(&self) -> Option<usize> {
        self.ready.peek().map(|Reverse((_, node))| *node)
    }
    pub fn pop(&mut self) -> Option<usize> {
        let Reverse((_, node)) = self.ready.pop()?;
        self.members.remove(&node);
        Some(node)
    }
    pub fn clear(&mut self) {
        self.ready.clear();
        self.members.clear();
    }
    /// Drain scheduled work; dropping this iterator early retains unpolled nodes.
    pub fn drain(&mut self) -> impl FusedIterator<Item = usize> + '_ {
        std::iter::from_fn(|| self.pop()).fuse()
    }
}
