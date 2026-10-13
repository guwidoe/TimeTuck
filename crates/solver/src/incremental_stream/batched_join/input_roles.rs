//! Optional reconciliation retaining independent input-role walks.
use super::*;
impl<I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    BatchedJoinNode<'_, I, J, L, R, K>
{
    pub(super) fn prepare_by_input_role(
        &self,
        state: &State<I, J, L, R, K>,
        left: &SnapshotMap<I, Option<L>>,
        right: &SnapshotMap<J, Option<R>>,
    ) -> Prepared<I, J, L, R, K> {
        let mut next = state.clone();
        update_side(&mut next.left, left, &self.left_key);
        update_side(&mut next.right, right, &self.right_key);
        let mut dirty = SnapshotMap::new();
        // Each dirty role reconciles its own old/new neighborhood. A pair dirty
        // on both roles therefore receives both real predicate evaluations and
        // state writes; publication below still coalesces by actual pair identity.
        for role in [true, false] {
            let mut candidates = BTreeSet::new();
            for s in [state, &next] {
                if role {
                    collect_affected(&s.left, &s.right, left, |i, j| {
                        candidates.insert((i, j));
                    });
                } else {
                    collect_affected(&s.right, &s.left, right, |j, i| {
                        candidates.insert((i, j));
                    });
                }
            }
            for id in candidates {
                let pair = next
                    .left
                    .rows
                    .get(&id.0)
                    .zip(next.right.rows.get(&id.1))
                    .filter(|((lk, l), (rk, r))| lk == rk && (self.predicate)(l, r))
                    .map(|((_, l), (_, r))| (l.clone(), r.clone()));
                if pair.is_some() || state.pairs.contains_key(&id) {
                    dirty.insert(id.clone(), ());
                }
                if let Some(pair) = pair {
                    next.pairs.insert(id, pair);
                } else {
                    next.pairs.remove(&id);
                }
            }
        }
        Prepared { state: next, dirty }
    }
}
