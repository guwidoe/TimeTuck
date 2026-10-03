//! Equality partitions containing independent ordered join indexes.
use super::{JoinChanges, JoinComparison, JoinSide, TupleChange};
use crate::SnapshotMap;
use std::any::Any;

type Projection<'a, T, E, K> = dyn Fn(&T) -> (E, K) + 'a;
#[derive(Clone)]
struct Side<I, T, E, K> {
    membership: SnapshotMap<I, E>,
    groups: SnapshotMap<E, JoinSide<I, T, K>>,
}
impl<I, T, E, K> Side<I, T, E, K> {
    fn new() -> Self {
        Self {
            membership: SnapshotMap::new(),
            groups: SnapshotMap::new(),
        }
    }
}
impl<I: Ord + Clone, T: Clone, E: Ord + Clone, K: Ord + Clone> Side<I, T, E, K> {
    fn apply<J: Ord + Clone, U: Clone>(
        &mut self,
        other: &Side<J, U, E, K>,
        change: TupleChange<I, T>,
        projection: &dyn Fn(&T) -> (E, K),
        comparison: JoinComparison,
    ) -> JoinChanges<I, J, T, U> {
        let new = change.value.map(|row| {
            let (equal, key) = projection(&row);
            (equal, key, row)
        });
        let old = self.membership.get(&change.id);
        let same_group = old
            .zip(new.as_ref())
            .is_some_and(|(old, (new, _, _))| old == new);
        let mut candidate = self.clone();
        let mut output = Vec::new();
        let empty = JoinSide::new();
        if !same_group && let Some(equal) = old {
            let mut group = candidate
                .groups
                .get(equal)
                .expect("membership group")
                .clone();
            output.extend(group.apply(
                other.groups.get(equal).unwrap_or(&empty),
                TupleChange::retract(change.id.clone()),
                &|_| unreachable!("retraction has no projection"),
                comparison,
            ));
            if group.rows.is_empty() {
                candidate.groups.remove(equal);
            } else {
                candidate.groups.insert(equal.clone(), group);
            }
            candidate.membership.remove(&change.id);
        }
        if let Some((equal, key, row)) = new {
            let mut group = candidate
                .groups
                .get(&equal)
                .cloned()
                .unwrap_or_else(JoinSide::new);
            output.extend(group.apply(
                other.groups.get(&equal).unwrap_or(&empty),
                TupleChange::put(change.id.clone(), row),
                &|_| key.clone(),
                comparison,
            ));
            if !same_group {
                candidate.membership.insert(change.id, equal.clone());
            }
            candidate.groups.insert(equal, group);
        }
        // Only persistent candidates were changed. Ordinary projection, key/ID
        // ordering and clone panics above leave the installed roots intact.
        *self = candidate;
        output
    }
}

/// Join equal partitions and compare ordered keys within each partition.
/// Projections return `(equality_key, ordered_key)`. Tuple equality keys express
/// multiple equalities; the ordered key uses one native `Ord` relation.
/// Unmatched rows remain indexed. Pair IDs are `(left_id, right_id)`.
///
/// Updates visit only the old/new equality partitions and matching ordered-key
/// ranges. Same-key replacements retain membership; retractions use captured keys
/// without callbacks. Empty side partitions are removed. Persistent candidate
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
    left: Side<I, L, E, K>,
    right: Side<J, R, E, K>,
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
            left: Side::new(),
            right: Side::new(),
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
        }
    }
    pub fn apply_left(&mut self, change: TupleChange<I, L>) -> JoinChanges<I, J, L, R> {
        self.left
            .apply(&self.right, change, &self.left_key, self.comparison)
    }
    pub fn apply_right(&mut self, change: TupleChange<J, R>) -> JoinChanges<I, J, L, R> {
        self.right
            .apply(
                &self.left,
                change,
                &self.right_key,
                self.comparison.reversed(),
            )
            .into_iter()
            .map(|change| TupleChange {
                id: (change.id.1, change.id.0),
                value: change.value.map(|(r, l)| (l, r)),
            })
            .collect()
    }
    /// Current pairs ordered by equality key, left ID, right ordered key and
    /// right ID. Inspection is independent of insertion order.
    pub fn matches(&self) -> impl Iterator<Item = ((&I, &J), (&L, &R))> {
        self.left.groups.iter().flat_map(move |(equal, left)| {
            self.right
                .groups
                .get(equal)
                .into_iter()
                .flat_map(move |right| {
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
        self.left = Side::new();
        self.right = Side::new();
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
        Box::new((self.left.clone(), self.right.clone()))
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
        (self.left, self.right) = *c.downcast().expect("partitioned join checkpoint type");
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
            assert_eq!(j.left.groups.len(), 1);
            assert_eq!(j.right.groups.len(), 1);
            assert_eq!(j.matches().count(), 1);
        }
        j.apply_left(TupleChange::retract(0));
        assert!(j.left.groups.is_empty());
        assert!(j.left.membership.is_empty());
        assert_eq!(j.right.groups.len(), 1);
        j.apply_right(TupleChange::retract(0));
        assert!(j.right.groups.is_empty());
        assert!(j.right.membership.is_empty());
    }
}
