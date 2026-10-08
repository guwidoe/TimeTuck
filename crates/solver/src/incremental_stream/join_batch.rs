//! Atomic rounds over join indexes, without retaining expanded matched pairs.
use super::{JoinChanges, JoinComparison, JoinNode, JoinSide, TupleChange, healthy};
use std::collections::{BTreeMap, BTreeSet};

fn last_updates<I: Ord, T>(
    changes: impl IntoIterator<Item = TupleChange<I, T>>,
) -> BTreeMap<I, Option<T>> {
    let mut out = BTreeMap::new();
    for change in changes {
        out.insert(change.id, change.value);
    }
    out
}
fn update<I: Ord + Clone, T: Clone, K: Ord + Clone>(
    side: &mut JoinSide<I, T, K>,
    changes: &BTreeMap<I, Option<T>>,
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
    changes: &BTreeMap<I, Option<L>>,
    comparison: JoinComparison,
    mut accept: impl FnMut(I, J),
) {
    for id in changes.keys() {
        if let Some((key, _)) = side.rows.get(id) {
            for other_id in other.matching_ids(key, comparison) {
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
        let mut candidates = BTreeSet::new();
        for (l, r) in [(&self.left, &self.right), (&next_left, &next_right)] {
            affected(l, r, &left, self.comparison, |i, j| {
                candidates.insert((i, j));
            });
            affected(r, l, &right, self.comparison.reversed(), |j, i| {
                candidates.insert((i, j));
            });
        }
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
