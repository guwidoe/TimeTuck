//! Equality partitions containing independent ordered join indexes.
use super::{JoinChanges, JoinComparison, JoinSide, TupleChange};
use crate::SnapshotMap;
use std::any::Any;
mod batch;

type Projection<'a, T, E, K> = dyn Fn(&T) -> (E, K) + 'a;
#[derive(Clone)]
struct SharedJoinBucket<I, J, L, R, K> {
    left: Option<JoinSide<I, L, K>>,
    right: Option<JoinSide<J, R, K>>,
}
impl<I, J, L, R, K> SharedJoinBucket<I, J, L, R, K> {
    fn new() -> Self {
        Self {
            left: None,
            right: None,
        }
    }
    fn is_empty(&self) -> bool {
        self.left.as_ref().is_none_or(|s| s.rows.is_empty())
            && self.right.as_ref().is_none_or(|s| s.rows.is_empty())
    }
}
#[derive(Clone)]
struct SharedJoinPartitions<I, J, L, R, E, K> {
    groups: SnapshotMap<E, SharedJoinBucket<I, J, L, R, K>>,
    left_membership: SnapshotMap<I, E>,
    right_membership: SnapshotMap<J, E>,
}
impl<I, J, L, R, E, K> SharedJoinPartitions<I, J, L, R, E, K> {
    fn new() -> Self {
        Self {
            groups: SnapshotMap::new(),
            left_membership: SnapshotMap::new(),
            right_membership: SnapshotMap::new(),
        }
    }
}
type JoinPartitionSplit<I, J, L, R, K, B> =
    fn(&mut B) -> (&mut Option<JoinSide<I, L, K>>, &Option<JoinSide<J, R, K>>);
struct JoinPartitionAccess<I, J, L, R, K, B> {
    create: fn() -> B,
    split: JoinPartitionSplit<I, J, L, R, K, B>,
    empty: fn(&B) -> bool,
}
// Work only on candidate roots. The caller installs them after every projection,
// comparison, key/row clone and output event has completed successfully.
fn update_partition<
    I: Ord + Clone,
    J: Ord + Clone,
    L: Clone,
    R: Clone,
    E: Ord + Clone,
    K: Ord + Clone,
    B: Clone,
>(
    groups: &mut SnapshotMap<E, B>,
    membership: &mut SnapshotMap<I, E>,
    change: TupleChange<I, L>,
    projection: &dyn Fn(&L) -> (E, K),
    comparison: JoinComparison,
    access: JoinPartitionAccess<I, J, L, R, K, B>,
) -> JoinChanges<I, J, L, R> {
    let new = change.value.map(|row| {
        let (equal, key) = projection(&row);
        (equal, key, row)
    });
    let old = membership.get(&change.id).cloned();
    let same_group = old
        .as_ref()
        .zip(new.as_ref())
        .is_some_and(|(old, (new, _, _))| old == new);
    let mut output = Vec::new();
    let empty = JoinSide::new();
    if !same_group && let Some(equal) = old {
        let mut bucket = groups.get(&equal).expect("membership group").clone();
        let (side, other) = (access.split)(&mut bucket);
        output.extend(side.as_mut().expect("initialized membership side").apply(
            other.as_ref().unwrap_or(&empty),
            TupleChange::retract(change.id.clone()),
            &|_| unreachable!("retraction has no projection"),
            comparison,
        ));
        if (access.empty)(&bucket) {
            groups.remove(&equal);
        } else {
            groups.insert(equal, bucket);
        }
        membership.remove(&change.id);
    }
    if let Some((equal, key, row)) = new {
        let mut bucket = groups.get(&equal).cloned().unwrap_or_else(access.create);
        let (side, other) = (access.split)(&mut bucket);
        output.extend(side.get_or_insert_with(JoinSide::new).apply(
            other.as_ref().unwrap_or(&empty),
            TupleChange::put(change.id.clone(), row),
            &|_| key.clone(),
            comparison,
        ));
        if !same_group {
            membership.insert(change.id, equal.clone());
        }
        groups.insert(equal, bucket);
    }
    output
}

/// Input counts and lazy-side state for one shared equality partition. Empty
/// sides may remain initialized until the last row on both sides is removed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JoinPartitionState {
    pub left_rows: usize,
    pub right_rows: usize,
    pub left_initialized: bool,
    pub right_initialized: bool,
}

/// Join equal partitions and compare ordered keys within each partition.
/// Projections return `(equality_key, ordered_key)`. Tuple equality keys express
/// multiple equalities; the ordered key uses one native `Ord` relation.
/// Unmatched rows remain indexed. Pair IDs are `(left_id, right_id)`.
///
/// Updates visit only the old/new equality partitions and matching ordered-key
/// ranges. Same-key replacements retain membership; retractions use captured keys
/// without callbacks. Each equality key owns one shared partition with lazily initialized sides.
/// A partition is removed only when both sides are empty. Persistent candidate
/// roots and events are prepared before installation; ordinary projection,
/// comparison or clone panics leave this standalone node unchanged. External
/// callback side effects are not rolled back. Inputs and callbacks must be stable.
/// Graph transactions provide rollback across downstream nodes.
///
/// ```
/// use timetuck::{JoinComparison, PartitionedJoinNode, TupleChange};
/// let mut join = PartitionedJoinNode::new(JoinComparison::LessThan,
///     |r: &(char, i32)| *r, |r: &(char, i32)| *r);
/// join.apply_left(TupleChange::put(0, ('A', 5)));
/// assert!(join.apply_right(TupleChange::put(0, ('B', 8))).is_empty());
/// assert_eq!(join.apply_right(TupleChange::put(1, ('A', 8))),
///     vec![TupleChange::put((0, 1), (('A', 5), ('A', 8)))]);
/// ```
pub struct PartitionedJoinNode<'a, I, J, L, R, E, K> {
    comparison: JoinComparison,
    state: SharedJoinPartitions<I, J, L, R, E, K>,
    left_key: Box<Projection<'a, L, E, K>>,
    right_key: Box<Projection<'a, R, E, K>>,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, E: Ord + Clone, K: Ord + Clone>
    PartitionedJoinNode<'a, I, J, L, R, E, K>
{
    pub fn new(
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> (E, K) + 'a,
        right_key: impl Fn(&R) -> (E, K) + 'a,
    ) -> Self {
        Self {
            comparison,
            state: SharedJoinPartitions::new(),
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
        }
    }
    /// Number of equality partitions, counting keys present on either side once.
    pub fn partition_count(&self) -> usize {
        self.state.groups.len()
    }
    /// Inspect a partition without creating a missing side or partition.
    pub fn partition_state(&self, equal: &E) -> Option<JoinPartitionState> {
        self.state.groups.get(equal).map(|b| JoinPartitionState {
            left_rows: b.left.as_ref().map_or(0, |s| s.rows.len()),
            right_rows: b.right.as_ref().map_or(0, |s| s.rows.len()),
            left_initialized: b.left.is_some(),
            right_initialized: b.right.is_some(),
        })
    }
    /// Right-side candidates for a prospective left equality/ordered key. This
    /// borrows indexed rows without requiring an inserted left row or allocating
    /// the missing side. Rows follow ordered key, then ID order.
    pub fn matching_right<'b>(
        &'b self,
        equal: &E,
        left_key: &'b K,
    ) -> impl Iterator<Item = (&'b J, &'b R)> {
        self.state
            .groups
            .get(equal)
            .and_then(|b| b.right.as_ref())
            .into_iter()
            .flat_map(move |side| {
                side.matching_ids(left_key, self.comparison)
                    .map(|id| (id, &side.rows.get(id).expect("indexed row").1))
            })
    }
    /// Left-side candidates for a prospective right key, reversing the ordered
    /// relation. Querying an absent partition or side is empty and allocates no index.
    pub fn matching_left<'b>(
        &'b self,
        equal: &E,
        right_key: &'b K,
    ) -> impl Iterator<Item = (&'b I, &'b L)> {
        self.state
            .groups
            .get(equal)
            .and_then(|b| b.left.as_ref())
            .into_iter()
            .flat_map(move |side| {
                side.matching_ids(right_key, self.comparison.reversed())
                    .map(|id| (id, &side.rows.get(id).expect("indexed row").1))
            })
    }
    pub fn apply_left(&mut self, change: TupleChange<I, L>) -> JoinChanges<I, J, L, R> {
        let mut candidate = self.state.clone();
        let output = update_partition(
            &mut candidate.groups,
            &mut candidate.left_membership,
            change,
            &self.left_key,
            self.comparison,
            JoinPartitionAccess {
                create: SharedJoinBucket::new,
                split: |b| (&mut b.left, &b.right),
                empty: SharedJoinBucket::is_empty,
            },
        );
        self.state = candidate;
        output
    }
    pub fn apply_right(&mut self, change: TupleChange<J, R>) -> JoinChanges<I, J, L, R> {
        let mut candidate = self.state.clone();
        let output = update_partition(
            &mut candidate.groups,
            &mut candidate.right_membership,
            change,
            &self.right_key,
            self.comparison.reversed(),
            JoinPartitionAccess {
                create: SharedJoinBucket::new,
                split: |b| (&mut b.right, &b.left),
                empty: SharedJoinBucket::is_empty,
            },
        )
        .into_iter()
        .map(|change| TupleChange {
            id: (change.id.1, change.id.0),
            value: change.value.map(|(r, l)| (l, r)),
        })
        .collect();
        self.state = candidate;
        output
    }
    /// Current pairs ordered by equality key, left ID, right ordered key and
    /// right ID. Inspection is independent of insertion order.
    pub fn matches(&self) -> impl Iterator<Item = ((&I, &J), (&L, &R))> {
        self.state.groups.iter().flat_map(move |(_, bucket)| {
            bucket
                .left
                .iter()
                .zip(bucket.right.iter())
                .flat_map(move |(left, right)| {
                    left.rows.iter().flat_map(move |(id, (key, row))| {
                        right.matching_ids(key, self.comparison).map(move |other| {
                            (
                                (id, other),
                                (row, &right.rows.get(other).expect("indexed row").1),
                            )
                        })
                    })
                })
        })
    }
    pub(crate) fn clear(&mut self) {
        self.state = SharedJoinPartitions::new();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        E: 'static,
        K: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, c: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        E: 'static,
        K: 'static,
    {
        self.state = *c.downcast().expect("partitioned join checkpoint type");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_partitions_and_memberships_are_removed_after_churn() {
        let mut j = PartitionedJoinNode::new(
            JoinComparison::LessThanOrEqual,
            |r: &(i32, i32)| *r,
            |r: &(i32, i32)| *r,
        );
        for i in 0..2000 {
            j.apply_left(TupleChange::put(0, (i, 0)));
            j.apply_right(TupleChange::put(0, (i, 1)));
            assert_eq!(j.partition_count(), 1);
            assert_eq!(j.partition_state(&i).unwrap().left_rows, 1);
            assert_eq!(j.partition_state(&i).unwrap().right_rows, 1);
            assert_eq!(j.matches().count(), 1);
        }
        j.apply_left(TupleChange::retract(0));
        assert!(j.state.left_membership.is_empty());
        assert_eq!(j.partition_count(), 1);
        assert_eq!(j.partition_state(&1999).unwrap().left_rows, 0);
        j.apply_right(TupleChange::retract(0));
        assert!(j.state.groups.is_empty());
        assert!(j.state.right_membership.is_empty());
    }
}
