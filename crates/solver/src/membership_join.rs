//! Indexed collection intersection with stable pair identities.
use crate::{SnapshotMap, TupleChange};
use std::{any::Any, collections::BTreeSet, sync::Arc};

type Projection<'a, T, E, K> = dyn Fn(&T) -> (E, Vec<K>) + 'a;
type Changes<I, J, L, R> = Vec<TupleChange<(I, J), (L, R)>>;
type Buckets<I, K> = SnapshotMap<K, SnapshotMap<I, ()>>;
#[derive(Clone)]
struct Row<T, E, K> {
    partition: E,
    keys: Arc<BTreeSet<K>>,
    value: T,
}
#[derive(Clone)]
struct Side<I, T, E, K> {
    rows: SnapshotMap<I, Row<T, E, K>>,
    index: SnapshotMap<E, Buckets<I, K>>,
}
impl<I, T, E, K> Side<I, T, E, K> {
    fn new() -> Self {
        Self {
            rows: SnapshotMap::new(),
            index: SnapshotMap::new(),
        }
    }
}
impl<I: Ord + Clone, T: Clone, E: Ord + Clone, K: Ord + Clone> Side<I, T, E, K> {
    fn matching(&self, partition: &E, keys: &BTreeSet<K>) -> BTreeSet<&I> {
        let mut ids = BTreeSet::new();
        if let Some(index) = self.index.get(partition) {
            for key in keys {
                if let Some(bucket) = index.get(key) {
                    ids.extend(bucket.keys());
                }
            }
        }
        ids
    }
    fn unindex(&mut self, id: &I, row: &Row<T, E, K>) {
        if row.keys.is_empty() {
            return;
        }
        let mut index = self
            .index
            .get(&row.partition)
            .expect("indexed partition")
            .clone();
        for key in row.keys.iter() {
            let mut bucket = index.get(key).expect("indexed key").clone();
            bucket.remove(id);
            if bucket.is_empty() {
                index.remove(key);
            } else {
                index.insert(key.clone(), bucket);
            }
        }
        if index.is_empty() {
            self.index.remove(&row.partition);
        } else {
            self.index.insert(row.partition.clone(), index);
        }
    }
    fn index(&mut self, id: &I, row: &Row<T, E, K>) {
        if row.keys.is_empty() {
            return;
        }
        let mut index = self.index.get(&row.partition).cloned().unwrap_or_default();
        for key in row.keys.iter() {
            let mut bucket = index.get(key).cloned().unwrap_or_default();
            bucket.insert(id.clone(), ());
            index.insert(key.clone(), bucket);
        }
        self.index.insert(row.partition.clone(), index);
    }
    fn apply<J: Ord + Clone, U: Clone>(
        &mut self,
        other: &Side<J, U, E, K>,
        change: TupleChange<I, T>,
        project: &dyn Fn(&T) -> (E, Vec<K>),
    ) -> Changes<I, J, T, U> {
        let new = change.value.map(|value| {
            let (partition, keys) = project(&value);
            Row {
                partition,
                keys: Arc::new(keys.into_iter().collect()),
                value,
            }
        });
        let old = self.rows.get(&change.id);
        let same = old
            .zip(new.as_ref())
            .is_some_and(|(a, b)| a.partition == b.partition && a.keys == b.keys);
        let new_matches = new
            .as_ref()
            .map(|r| other.matching(&r.partition, &r.keys))
            .unwrap_or_default();
        let mut out = Vec::new();
        if !same && let Some(row) = old {
            for id in other.matching(&row.partition, &row.keys) {
                if !new_matches.contains(id) {
                    out.push(TupleChange::retract((change.id.clone(), id.clone())));
                }
            }
        }
        if let Some(row) = &new {
            for id in new_matches {
                out.push(TupleChange::put(
                    (change.id.clone(), id.clone()),
                    (
                        row.value.clone(),
                        other.rows.get(id).expect("indexed row").value.clone(),
                    ),
                ));
            }
        }
        let mut candidate = self.clone();
        if !same {
            if let Some(row) = old {
                candidate.unindex(&change.id, row);
            }
            if let Some(row) = &new {
                candidate.index(&change.id, row);
            }
        }
        if let Some(row) = new {
            candidate.rows.insert(change.id, row);
        } else {
            candidate.rows.remove(&change.id);
        }
        // Captured keys, persistent roots and output clones are all ready before
        // installation. User projection/comparison/clone panics preserve self.
        *self = candidate;
        out
    }
}

/// Join collections sharing at least one key, optionally inside equal partitions.
/// Duplicate elements and multiple shared keys never multiply a pair. Empty
/// collections match nothing; their source rows are still retained. A scalar
/// projected as a singleton expresses contains/contained-in, including an explicit
/// Option::None key when null membership is desired. Collection-null policy is
/// the caller's responsibility. Keys use stable native Ord/Eq semantics.
///
/// This native set-membership contract intentionally differs from observed
/// Timefold Community 2.7.0 public-join edge cases: duplicate collection members
/// can multiply Java matches, and unpartitioned scalar joins can skip null matches.
/// See `docs/constraint-stream-contract.md` for the fixed divergence probes.
///
/// Each side indexes distinct keys to occurrence IDs. An update unions only
/// matching opposite buckets and emits retractions for lost pairs, then puts for
/// new/surviving pairs, ordered by the opposite ID. Pair identity is (left,right).
/// Retraction uses captured keys without projections. Same-key-set replacements
/// retain index membership but refresh payloads. State is linear in source rows
/// and distinct per-row keys; transient match sets and dense output can be large.
/// Snapshots share roots and captured key sets. Ordinary callback/clone/comparison
/// panics preserve this standalone node. Downstream rollback requires a graph
/// transaction; callbacks must be deterministic and have no external side effects.
///
/// ```
/// use timetuck::{MembershipJoinNode,TupleChange};
/// let mut j=MembershipJoinNode::new(|v:&Vec<i32>|v.clone(),|v:&Vec<i32>|v.clone());
/// j.apply_left(TupleChange::put(0,vec![1,1,2]));
/// let out=j.apply_right(TupleChange::put(0,vec![1,2,2]));
/// assert_eq!(out.len(),1);
/// assert_eq!(j.matches().count(),1);
/// assert_eq!(j.apply_right(TupleChange::put(0,vec![])),vec![TupleChange::retract((0,0))]);
/// ```
pub struct MembershipJoinNode<'a, I, J, L, R, K, E = ()> {
    left: Side<I, L, E, K>,
    right: Side<J, R, E, K>,
    left_key: Box<Projection<'a, L, E, K>>,
    right_key: Box<Projection<'a, R, E, K>>,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    MembershipJoinNode<'a, I, J, L, R, K>
{
    pub fn new(left: impl Fn(&L) -> Vec<K> + 'a, right: impl Fn(&R) -> Vec<K> + 'a) -> Self {
        Self::partitioned(move |r| ((), left(r)), move |r| ((), right(r)))
    }
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone, E: Ord + Clone>
    MembershipJoinNode<'a, I, J, L, R, K, E>
{
    /// Each projection returns an equality key and collection. Tuple equality
    /// keys can express several simultaneous equalities.
    pub fn partitioned(
        left: impl Fn(&L) -> (E, Vec<K>) + 'a,
        right: impl Fn(&R) -> (E, Vec<K>) + 'a,
    ) -> Self {
        Self {
            left: Side::new(),
            right: Side::new(),
            left_key: Box::new(left),
            right_key: Box::new(right),
        }
    }
    pub fn apply_left(&mut self, change: TupleChange<I, L>) -> Changes<I, J, L, R> {
        self.left.apply(&self.right, change, &self.left_key)
    }
    pub fn apply_right(&mut self, change: TupleChange<J, R>) -> Changes<I, J, L, R> {
        self.right
            .apply(&self.left, change, &self.right_key)
            .into_iter()
            .map(|c| TupleChange {
                id: (c.id.1, c.id.0),
                value: c.value.map(|(r, l)| (l, r)),
            })
            .collect()
    }
    /// Retained input occurrences, including unmatched and empty-key rows.
    pub fn left_len(&self) -> usize {
        self.left.rows.len()
    }
    /// Retained input occurrences, including unmatched and empty-key rows.
    pub fn right_len(&self) -> usize {
        self.right.rows.len()
    }
    /// Current matches in left-ID/right-ID order, deduplicating shared keys.
    pub fn matches(&self) -> impl Iterator<Item = ((&I, &J), (&L, &R))> {
        self.left.rows.iter().flat_map(move |(id, row)| {
            self.right
                .matching(&row.partition, &row.keys)
                .into_iter()
                .map(move |other| {
                    (
                        (id, other),
                        (
                            &row.value,
                            &self.right.rows.get(other).expect("indexed row").value,
                        ),
                    )
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
        K: 'static,
        E: 'static,
    {
        Box::new((self.left.clone(), self.right.clone()))
    }
    pub(crate) fn restore(&mut self, c: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        K: 'static,
        E: 'static,
    {
        (self.left, self.right) = *c.downcast().expect("membership checkpoint type");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn churn_releases_empty_buckets_but_retains_empty_collection_rows() {
        let mut j = MembershipJoinNode::partitioned(
            |r: &(i32, Vec<i32>)| r.clone(),
            |r: &(i32, Vec<i32>)| r.clone(),
        );
        for i in 0..2000 {
            j.apply_left(TupleChange::put(0, (i, vec![i, i, i + 1])));
            j.apply_right(TupleChange::put(0, (i, vec![i + 1, i])));
            assert_eq!(j.matches().count(), 1);
            for index in [&j.left.index, &j.right.index] {
                assert_eq!(index.len(), 1);
                assert_eq!(index.get(&i).unwrap().len(), 2);
            }
        }
        j.apply_left(TupleChange::put(0, (0, vec![])));
        assert!(j.left.index.is_empty());
        assert_eq!(j.left_len(), 1);
        j.apply_left(TupleChange::retract(0));
        j.apply_right(TupleChange::retract(0));
        assert_eq!((j.left_len(), j.right_len()), (0, 0));
        assert!(j.left.index.is_empty() && j.right.index.is_empty());
    }
}
