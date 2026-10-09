//! Atomic final-state rounds over shared equality partitions.
use super::*;
use crate::incremental_stream::join_batch::last_updates;

type Prepared<I, E, K, T> = Vec<(I, Option<(E, K, T)>)>;
fn prepare<I: Ord, E, K, T>(
    changes: impl IntoIterator<Item = TupleChange<I, T>>,
    project: &dyn Fn(&T) -> (E, K),
) -> Prepared<I, E, K, T> {
    last_updates(changes)
        .into_iter()
        .map(|(id, value)| {
            (
                id,
                value.map(|row| {
                    let (equal, key) = project(&row);
                    (equal, key, row)
                }),
            )
        })
        .collect()
}
fn install<
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
    changes: &mut Prepared<I, E, K, L>,
    access: JoinPartitionAccess<I, J, L, R, K, B>,
) {
    for (id, value) in changes {
        let old = membership.get(id).cloned();
        let same_group = old
            .as_ref()
            .zip(value.as_ref())
            .is_some_and(|(old, (new, _, _))| old == new);
        if !same_group && let Some(old) = old {
            let mut bucket = groups.get(&old).expect("membership group").clone();
            (access.split)(&mut bucket)
                .0
                .as_mut()
                .expect("initialized side")
                .install_projected(id.clone(), None, false);
            if (access.empty)(&bucket) {
                groups.remove(&old);
            } else {
                groups.insert(old, bucket);
            }
            membership.remove(id);
        }
        if let Some((equal, key, row)) = value.take() {
            let mut bucket = groups.get(&equal).cloned().unwrap_or_else(access.create);
            let side = (access.split)(&mut bucket)
                .0
                .get_or_insert_with(JoinSide::new);
            let same_key = side.rows.get(id).is_some_and(|(old, _)| *old == key);
            side.install_projected(id.clone(), Some((key, row)), same_key);
            if !same_group {
                membership.insert(id.clone(), equal.clone());
            }
            groups.insert(equal, bucket);
        }
    }
}
impl<I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, E: Ord + Clone, K: Ord + Clone>
    SharedJoinPartitions<I, J, L, R, E, K>
{
    fn affected(
        &self,
        left: &Prepared<I, E, K, L>,
        right: &Prepared<J, E, K, R>,
        comparison: JoinComparison,
        pairs: &mut Vec<(I, J)>,
    ) {
        for (id, _) in left {
            if let Some(equal) = self.left_membership.get(id) {
                let b = self.groups.get(equal).expect("membership group");
                let (key, _) = b
                    .left
                    .as_ref()
                    .expect("left side")
                    .rows
                    .get(id)
                    .expect("left row");
                if let Some(other) = &b.right {
                    pairs.extend(
                        other
                            .matching_ids(key, comparison)
                            .map(|j| (id.clone(), j.clone())),
                    );
                }
            }
        }
        for (id, _) in right {
            if let Some(equal) = self.right_membership.get(id) {
                let b = self.groups.get(equal).expect("membership group");
                let (key, _) = b
                    .right
                    .as_ref()
                    .expect("right side")
                    .rows
                    .get(id)
                    .expect("right row");
                if let Some(other) = &b.left {
                    pairs.extend(
                        other
                            .matching_ids(key, comparison.reversed())
                            .map(|i| (i.clone(), id.clone())),
                    );
                }
            }
        }
    }
    fn pair(&self, i: &I, j: &J, comparison: JoinComparison) -> Option<(L, R)> {
        let equal = self.left_membership.get(i)?;
        if self.right_membership.get(j)? != equal {
            return None;
        }
        let b = self.groups.get(equal).expect("membership group");
        let (lk, l) = b.left.as_ref()?.rows.get(i)?;
        let (rk, r) = b.right.as_ref()?.rows.get(j)?;
        comparison.matches(lk, rk).then(|| (l.clone(), r.clone()))
    }
}
impl<I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, E: Ord + Clone, K: Ord + Clone>
    PartitionedJoinNode<'_, I, J, L, R, E, K>
{
    /// Apply both sides as one final-state round. Only the last update per input
    /// ID is projected. Visit old/new matching ranges within affected equality
    /// partitions, then emit at most one change per affected pair, sorted by ID.
    /// Surviving pairs refresh their payloads; intermediate matches are not emitted.
    ///
    /// Key projection, comparison, iterator or payload-clone panics leave the
    /// node unchanged. Candidate roots and outputs are prepared before installation.
    /// Pure callbacks/stable keys are required; destructor panics and callback side
    /// effects are excluded. Persistent state holds input rows/indexes, not pairs;
    /// dense batches can require quadratic temporary output space.
    pub fn apply_batch(
        &mut self,
        left: impl IntoIterator<Item = TupleChange<I, L>>,
        right: impl IntoIterator<Item = TupleChange<J, R>>,
    ) -> JoinChanges<I, J, L, R> {
        let mut left = prepare(left, &self.left_key);
        let mut right = prepare(right, &self.right_key);
        if left.is_empty() && right.is_empty() {
            return Vec::new();
        }
        let mut pairs = Vec::new();
        self.state
            .affected(&left, &right, self.comparison, &mut pairs);
        let mut next = self.state.clone();
        install(
            &mut next.groups,
            &mut next.left_membership,
            &mut left,
            JoinPartitionAccess {
                create: SharedJoinBucket::new,
                split: |b| (&mut b.left, &b.right),
                empty: SharedJoinBucket::is_empty,
            },
        );
        install(
            &mut next.groups,
            &mut next.right_membership,
            &mut right,
            JoinPartitionAccess {
                create: SharedJoinBucket::new,
                split: |b| (&mut b.right, &b.left),
                empty: SharedJoinBucket::is_empty,
            },
        );
        next.affected(&left, &right, self.comparison, &mut pairs);
        pairs.sort();
        pairs.dedup();
        let output = pairs
            .into_iter()
            .map(|(i, j)| {
                let value = next.pair(&i, &j, self.comparison);
                TupleChange { id: (i, j), value }
            })
            .collect();
        self.state = next;
        output
    }
}
