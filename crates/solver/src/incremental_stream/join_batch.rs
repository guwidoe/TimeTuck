//! Atomic rounds over join indexes, without retaining expanded matched pairs.
use super::{JoinChanges, JoinComparison, JoinNode, JoinSide, TupleChange, healthy};

type PreparedChange<I, K, T> = (I, Option<(K, T)>);
type PreparedChanges<I, K, T> = Vec<PreparedChange<I, K, T>>;

pub(super) fn last_updates<I: Ord, T>(
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

fn project<I, T, K>(
    changes: Vec<(I, Option<T>)>,
    key: &dyn Fn(&T) -> K,
) -> PreparedChanges<I, K, T> {
    changes
        .into_iter()
        .map(|(id, value)| (id, value.map(|row| (key(&row), row))))
        .collect()
}
fn unchanged<I: Ord + Clone, T: Clone, K: Ord + Clone>(
    side: &JoinSide<I, T, K>,
    changes: &[PreparedChange<I, K, T>],
) -> bool {
    changes.iter().all(|(id, new)| {
        side.rows
            .get(id)
            .zip(new.as_ref())
            .is_some_and(|((old, _), (new, _))| old == new)
    })
}
fn final_payload<'a, I: Ord + Clone, T: Clone, K: Ord + Clone>(
    side: &'a JoinSide<I, T, K>,
    changes: &'a [PreparedChange<I, K, T>],
    id: &I,
) -> &'a T {
    match changes.binary_search_by(|(changed, _)| changed.cmp(id)) {
        Ok(i) => &changes[i].1.as_ref().expect("replacement row").1,
        Err(_) => &side.rows.get(id).expect("matched row").1,
    }
}
fn update<I: Ord + Clone, T: Clone, K: Ord + Clone>(
    side: &mut JoinSide<I, T, K>,
    changes: &mut [PreparedChange<I, K, T>],
) {
    for (id, value) in changes {
        let same_key = side
            .rows
            .get(id)
            .zip(value.as_ref())
            .is_some_and(|((old, _), (new, _))| old == new);
        side.install_projected(id.clone(), value.take(), same_key);
    }
}
fn affected<I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone, C>(
    side: &JoinSide<I, L, K>,
    other: &JoinSide<J, R, K>,
    next_side: &JoinSide<I, L, K>,
    next_other: &JoinSide<J, R, K>,
    changes: &[(I, C)],
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
        self.apply_batch_projected(left, right, |l, r| Some((l.clone(), r.clone())))
    }
    // Projection happens only on final matching candidates, before installation.
    // Rejected candidates carry a retraction; graph callers suppress IDs that
    // were never published. The public unfiltered operation keeps its contract.
    pub(crate) fn apply_batch_projected<U>(
        &mut self,
        left: impl IntoIterator<Item = TupleChange<I, L>>,
        right: impl IntoIterator<Item = TupleChange<J, R>>,
        mut pair: impl FnMut(&L, &R) -> Option<U>,
    ) -> Vec<TupleChange<(I, J), U>> {
        self.apply_batch_with_previous::<false, U>(left, right, &mut pair, |_, _| false)
    }
    // Re-evaluate prior acceptance only when the final pair emits no payload.
    // All callbacks finish before installation, preserving standalone atomicity.
    pub(crate) fn apply_batch_recomputed(
        &mut self,
        left: impl IntoIterator<Item = TupleChange<I, L>>,
        right: impl IntoIterator<Item = TupleChange<J, R>>,
        predicate: &dyn Fn(&L, &R) -> bool,
    ) -> JoinChanges<I, J, L, R> {
        self.apply_batch_with_previous::<true, _>(
            left,
            right,
            |l, r| predicate(l, r).then(|| (l.clone(), r.clone())),
            predicate,
        )
    }
    fn prior_matches(&self, i: &I, j: &J, predicate: &mut impl FnMut(&L, &R) -> bool) -> bool {
        self.left
            .rows
            .get(i)
            .zip(self.right.rows.get(j))
            .filter(|((lk, _), (rk, _))| self.comparison.matches(lk, rk))
            .is_some_and(|((_, l), (_, r))| predicate(l, r))
    }
    fn apply_batch_with_previous<const RECOMPUTE: bool, U>(
        &mut self,
        left: impl IntoIterator<Item = TupleChange<I, L>>,
        right: impl IntoIterator<Item = TupleChange<J, R>>,
        mut pair: impl FnMut(&L, &R) -> Option<U>,
        mut previous: impl FnMut(&L, &R) -> bool,
    ) -> Vec<TupleChange<(I, J), U>> {
        healthy(self.is_poisoned());
        let mut left = project(last_updates(left), &self.left_key);
        let mut right = project(last_updates(right), &self.right_key);
        if unchanged(&self.left, &left) && unchanged(&self.right, &right) {
            return self.replace_payloads::<RECOMPUTE, U>(left, right, &mut pair, &mut previous);
        }
        let mut next_left = self.left.clone();
        let mut next_right = self.right.clone();
        update(&mut next_left, &mut left);
        update(&mut next_right, &mut right);
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
                .and_then(|((_, l), (_, r))| pair(l, r));
            if value.is_some() || !RECOMPUTE || self.prior_matches(&i, &j, &mut previous) {
                out.push(TupleChange { id: (i, j), value });
            }
        }
        self.left.rows.install_snapshot(next_left.rows);
        self.left.index = next_left.index;
        self.left.poisoned = next_left.poisoned;
        self.right.rows.install_snapshot(next_right.rows);
        self.right.index = next_right.index;
        self.right.poisoned = next_right.poisoned;
        out
    }
    // Every changed row exists and retains its key, so old membership is final
    // membership. Prepare all output/callback work before mutating row payloads.
    fn replace_payloads<const RECOMPUTE: bool, U>(
        &mut self,
        left: PreparedChanges<I, K, L>,
        right: PreparedChanges<J, K, R>,
        pair: &mut impl FnMut(&L, &R) -> Option<U>,
        previous: &mut impl FnMut(&L, &R) -> bool,
    ) -> Vec<TupleChange<(I, J), U>> {
        let mut candidates = Vec::new();
        affected(
            &self.left,
            &self.right,
            &self.left,
            &self.right,
            &left,
            self.comparison,
            |i, j| candidates.push((i, j)),
        );
        affected(
            &self.right,
            &self.left,
            &self.right,
            &self.left,
            &right,
            self.comparison.reversed(),
            |j, i| candidates.push((i, j)),
        );
        candidates.sort();
        candidates.dedup();
        let mut output = Vec::with_capacity(candidates.len());
        for (i, j) in candidates {
            let value = pair(
                final_payload(&self.left, &left, &i),
                final_payload(&self.right, &right, &j),
            );
            if value.is_some() || !RECOMPUTE || self.prior_matches(&i, &j, previous) {
                output.push(TupleChange { id: (i, j), value });
            }
        }
        // These nested guards also protect standalone joins against comparison
        // panics during final installation. Graph trials retain an outer journal.
        let mut left_rows = self.left.rows.journal_transaction();
        let mut right_rows = self.right.rows.journal_transaction();
        for (id, value) in left {
            left_rows.insert(id, value.expect("replacement row"));
        }
        for (id, value) in right {
            right_rows.insert(id, value.expect("replacement row"));
        }
        left_rows.commit();
        right_rows.commit();
        output
    }
}
