//! Atomic rounds over join indexes, without retaining expanded matched pairs.
use super::{JoinChanges, JoinComparison, JoinNode, JoinSide, TupleChange, healthy};

fn last_updates<I: Ord, T>(
    changes: impl IntoIterator<Item = TupleChange<I, T>>,
) -> Vec<(I, Option<T>)> {
    let mut out: Vec<_> = changes
        .into_iter()
        .map(|change| (change.id, change.value))
        .collect();
    // Stable sorting retains the first ID representative, just like map insert.
    // Move the final value into that first slot; no input payload clones.
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out.dedup_by(|later, earlier| {
        if later.0 == earlier.0 {
            std::mem::swap(&mut later.1, &mut earlier.1);
            true
        } else {
            false
        }
    });
    out
}

fn update<I: Ord + Clone, T: Clone, K: Ord + Clone>(
    side: &mut JoinSide<I, T, K>,
    changes: &[(I, Option<T>)],
    key: &dyn Fn(&T) -> K,
) {
    let empty = JoinSide::<(), (), K>::new();
    for (id, value) in changes {
        side.apply(
            &empty,
            TupleChange {
                id: id.clone(),
                value: value.clone(),
            },
            key,
            JoinComparison::Equal,
        );
    }
}
fn affected<I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>(
    side: &JoinSide<I, L, K>,
    other: &JoinSide<J, R, K>,
    next_side: &JoinSide<I, L, K>,
    next_other: &JoinSide<J, R, K>,
    changes: &[(I, Option<L>)],
    comparison: JoinComparison,
    mut accept: impl FnMut(I, J),
) {
    for (id, _) in changes {
        let old_key = side.rows.get(id).map(|(key, _)| key);
        let new_key = next_side.rows.get(id).map(|(key, _)| key);
        if let Some(key) = old_key {
            for other_id in other.matching_ids(key, comparison) {
                accept(id.clone(), other_id.clone());
            }
        }
        // Payload replacement often leaves join membership untouched. Shared
        // roots prove this without comparing or traversing all opposite IDs.
        // Equality can check only its bucket even when unrelated buckets change.
        let unchanged = old_key.zip(new_key).is_some_and(|(old, new)| {
            old == new
                && if other.index.shares_root(&next_other.index) {
                    true
                } else if comparison == JoinComparison::Equal {
                    match (other.index.get(old), next_other.index.get(new)) {
                        (None, None) => true,
                        (Some(a), Some(b)) => a.shares_root(b),
                        _ => false,
                    }
                } else {
                    false
                }
        });
        if !unchanged && let Some(key) = new_key {
            for other_id in next_other.matching_ids(key, comparison) {
                accept(id.clone(), other_id.clone());
            }
        }
    }
}
impl<I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    JoinNode<'_, I, J, L, R, K>
{
    /// Apply an atomic indexed join round. Each side retains only its final update
    /// per ID; projections run only on final inserted/replaced rows. The output
    /// contains one final change per affected pair, sorted by `(left_id,right_id)`.
    /// Surviving pairs refresh payloads even without a membership change.
    ///
    /// Equality and ordered comparisons use the old/new matching index ranges.
    /// Only input rows and indexes persist; candidate pairs and output rows are
    /// temporary. Dense batches may still require quadratic temporary space.
    /// Iterator, projection, comparison or clone panics preserve the committed
    /// indexes and their health. Callbacks/keys must be pure and stable; panicking
    /// destructors and external side effects are excluded. An existing poisoned
    /// node must be rebuilt, as with single-input updates.
    pub fn apply_batch(
        &mut self,
        left: impl IntoIterator<Item = TupleChange<I, L>>,
        right: impl IntoIterator<Item = TupleChange<J, R>>,
    ) -> JoinChanges<I, J, L, R> {
        healthy(self.is_poisoned());
        let left = last_updates(left);
        let right = last_updates(right);
        let mut next_left = self.left.clone();
        let mut next_right = self.right.clone();
        update(&mut next_left, &left, &self.left_key);
        update(&mut next_right, &right, &self.right_key);
        let mut candidates = Vec::new();
        affected(
            &self.left,
            &self.right,
            &next_left,
            &next_right,
            &left,
            self.comparison,
            |i, j| {
                candidates.push((i, j));
            },
        );
        affected(
            &self.right,
            &self.left,
            &next_right,
            &next_left,
            &right,
            self.comparison.reversed(),
            |j, i| {
                candidates.push((i, j));
            },
        );
        candidates.sort();
        candidates.dedup();
        let mut out = Vec::with_capacity(candidates.len());
        for (i, j) in candidates {
            let value = next_left
                .rows
                .get(&i)
                .zip(next_right.rows.get(&j))
                .filter(|((lk, _), (rk, _))| self.comparison.matches(lk, rk))
                .map(|((_, l), (_, r))| (l.clone(), r.clone()));
            // Each candidate matched before or after the round. A missing final
            // match therefore retracts an actual old pair, never a transient one.
            out.push(TupleChange { id: (i, j), value });
        }
        self.left = next_left;
        self.right = next_right;
        out
    }
}
