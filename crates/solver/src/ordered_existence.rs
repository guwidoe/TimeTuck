//! Pair-free existence checks over equality partitions and ordered keys.
use crate::{ExistenceMode, JoinComparison, SnapshotMap, TupleChange};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    ops::Bound::{self, *},
};

type KeyProjection<'a, T, E, K> = dyn Fn(&T) -> (E, K) + 'a;

fn last_updates<I: Ord, T>(
    changes: impl IntoIterator<Item = TupleChange<I, T>>,
) -> BTreeMap<I, Option<T>> {
    changes.into_iter().map(|c| (c.id, c.value)).collect()
}

#[derive(Clone)]
struct Partition<I, K> {
    left: SnapshotMap<K, SnapshotMap<I, ()>>,
    right: SnapshotMap<K, usize>,
}
impl<I, K> Partition<I, K> {
    fn new() -> Self {
        Self {
            left: SnapshotMap::new(),
            right: SnapshotMap::new(),
        }
    }
    fn empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }
}
#[derive(Clone)]
struct State<I, J, L, E, K> {
    left: SnapshotMap<I, (E, K, L)>,
    right: SnapshotMap<J, (E, K)>,
    groups: SnapshotMap<E, Partition<I, K>>,
}
impl<I, J, L, E, K> State<I, J, L, E, K> {
    fn new() -> Self {
        Self {
            left: SnapshotMap::new(),
            right: SnapshotMap::new(),
            groups: SnapshotMap::new(),
        }
    }
}
fn boundary<I, K>(p: Option<&Partition<I, K>>, c: JoinComparison) -> Option<&K> {
    let p = p?;
    match c {
        JoinComparison::LessThan | JoinComparison::LessThanOrEqual => p.right.last_key_value(),
        _ => p.right.first_key_value(),
    }
    .map(|(k, _)| k)
}
impl<I: Ord + Clone, J: Ord + Clone, L: Clone, E: Ord + Clone, K: Ord + Clone>
    State<I, J, L, E, K>
{
    fn save(&mut self, e: E, p: Partition<I, K>) {
        if p.empty() {
            self.groups.remove(&e);
        } else {
            self.groups.insert(e, p);
        }
    }
    fn matched(&self, e: &E, k: &K, c: JoinComparison) -> bool {
        let p = self.groups.get(e);
        if c == JoinComparison::Equal {
            p.is_some_and(|p| p.right.contains_key(k))
        } else {
            boundary(p, c).is_some_and(|r| c.matches(k, r))
        }
    }
    fn visible(&self, id: &I, c: JoinComparison, mode: ExistenceMode) -> bool {
        self.left
            .get(id)
            .is_some_and(|(e, k, _)| self.matched(e, k, c) == (mode == ExistenceMode::Exists))
    }
    fn install_left(&mut self, id: I, row: Option<(E, K, L)>) {
        let old = self.left.get(&id);
        let same = old
            .zip(row.as_ref())
            .is_some_and(|(a, b)| a.0 == b.0 && a.1 == b.1);
        if !same {
            if let Some((e, k, _)) = old {
                let (e, k) = (e.clone(), k.clone());
                let mut p = self.groups.get(&e).unwrap().clone();
                let mut ids = p.left.get(&k).unwrap().clone();
                ids.remove(&id);
                if ids.is_empty() {
                    p.left.remove(&k);
                } else {
                    p.left.insert(k, ids);
                }
                self.save(e, p);
            }
            if let Some((e, k, _)) = &row {
                let mut p = self.groups.get(e).cloned().unwrap_or_else(Partition::new);
                let mut ids = p.left.get(k).cloned().unwrap_or_default();
                ids.insert(id.clone(), ());
                p.left.insert(k.clone(), ids);
                self.save(e.clone(), p);
            }
        }
        if let Some(row) = row {
            self.left.insert(id, row);
        } else {
            self.left.remove(&id);
        }
    }
    fn install_right(&mut self, id: J, row: Option<(E, K)>) {
        let old = self.right.get(&id);
        if old == row.as_ref() {
            return;
        }
        if let Some((e, k)) = old {
            let (e, k) = (e.clone(), k.clone());
            let mut p = self.groups.get(&e).unwrap().clone();
            let n = p.right[&k];
            if n == 1 {
                p.right.remove(&k);
            } else {
                p.right.insert(k, n - 1);
            }
            self.save(e, p);
        }
        if let Some((e, k)) = &row {
            let mut p = self.groups.get(e).cloned().unwrap_or_else(Partition::new);
            p.right
                .insert(k.clone(), p.right.get(k).copied().unwrap_or(0) + 1);
            self.save(e.clone(), p);
        }
        if let Some(row) = row {
            self.right.insert(id, row);
        } else {
            self.right.remove(&id);
        }
    }
}

/// Semi/anti join for `(equality partition, ordered key)` projections. A unit
/// partition gives an unpartitioned comparison; tuples express multiple equalities.
/// Every accepted left ID appears once, regardless of the number of witnesses.
///
/// Stores left rows/indexes and right ID-to-key mappings/key counts, never matching
/// pairs or right payloads. For inequalities, only the minimum or maximum right
/// key determines visibility. Right updates visit left keys only where that
/// boundary changes; equal-key updates visit a bucket only on zero crossings.
/// Both sides support atomic, coalesced final-state rounds and immediate updates.
///
/// Keys/callbacks must be stable and pure. Ordinary iterator, projection, comparison
/// and clone panics leave state unchanged; destructor panics and external effects
/// are excluded. No arbitrary residual predicate or self-exclusion is supported;
/// use ExistsNode for those cases. Self joins include matching self occurrences.
pub struct ExistsCompareNode<'a, I, J, L, R, E, K> {
    state: State<I, J, L, E, K>,
    comparison: JoinComparison,
    mode: ExistenceMode,
    left_key: Box<KeyProjection<'a, L, E, K>>,
    right_key: Box<KeyProjection<'a, R, E, K>>,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R, E: Ord + Clone, K: Ord + Clone>
    ExistsCompareNode<'a, I, J, L, R, E, K>
{
    pub fn new(
        mode: ExistenceMode,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> (E, K) + 'a,
        right_key: impl Fn(&R) -> (E, K) + 'a,
    ) -> Self {
        Self {
            state: State::new(),
            mode,
            comparison,
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
        }
    }
    pub fn rows(&self) -> impl Iterator<Item = (&I, &L)> {
        self.state.left.iter().filter_map(|(id, (e, k, l))| {
            (self.state.matched(e, k, self.comparison) == (self.mode == ExistenceMode::Exists))
                .then_some((id, l))
        })
    }
    pub fn partition_count(&self) -> usize {
        self.state.groups.len()
    }
    pub fn apply_left(&mut self, change: TupleChange<I, L>) -> Option<TupleChange<I, L>> {
        self.apply_batch([change], []).pop()
    }
    pub fn apply_right(&mut self, change: TupleChange<J, R>) -> Vec<TupleChange<I, L>> {
        self.apply_batch([], [change])
    }
    /// Last update per input ID wins. Emit final visibility transitions, plus
    /// payload refreshes for visible changed left IDs, sorted by left ID.
    pub fn apply_batch(
        &mut self,
        left: impl IntoIterator<Item = TupleChange<I, L>>,
        right: impl IntoIterator<Item = TupleChange<J, R>>,
    ) -> Vec<TupleChange<I, L>> {
        let left: Vec<_> = last_updates(left)
            .into_iter()
            .map(|(i, v)| {
                (
                    i,
                    v.map(|l| {
                        let (e, k) = (self.left_key)(&l);
                        (e, k, l)
                    }),
                )
            })
            .collect();
        let right: Vec<_> = last_updates(right)
            .into_iter()
            .map(|(j, v)| (j, v.map(|r| (self.right_key)(&r))))
            .collect();
        if left.is_empty() && right.is_empty() {
            return vec![];
        }
        let changed_left: BTreeSet<_> = left.iter().map(|(i, _)| i.clone()).collect();
        let mut keys = BTreeSet::new();
        for (id, row) in &right {
            keys.extend(self.state.right.get(id).cloned());
            keys.extend(row.clone());
        }
        let mut next = self.state.clone();
        for (id, row) in left {
            next.install_left(id, row);
        }
        for (id, row) in right {
            next.install_right(id, row);
        }
        let mut affected = changed_left.clone();
        let mut visited = BTreeSet::new();
        for (e, k) in &keys {
            let old = self.state.groups.get(e);
            let new = next.groups.get(e);
            let Some(p) = new else {
                continue;
            };
            if self.comparison == JoinComparison::Equal {
                if old.is_some_and(|p| p.right.contains_key(k)) != p.right.contains_key(k)
                    && let Some(ids) = p.left.get(k)
                {
                    affected.extend(ids.keys().cloned());
                }
            } else if visited.insert(e) {
                let (a, b) = (
                    boundary(old, self.comparison),
                    boundary(new, self.comparison),
                );
                if let Some((lo, hi)) = changed_range(a, b, self.comparison) {
                    for (_, ids) in p.left.range(lo, hi) {
                        affected.extend(ids.keys().cloned());
                    }
                }
            }
        }
        let output = affected
            .into_iter()
            .filter_map(|id| {
                let before = self.state.visible(&id, self.comparison, self.mode);
                let after = next.visible(&id, self.comparison, self.mode);
                if after && (!before || changed_left.contains(&id)) {
                    Some(TupleChange::put(
                        id.clone(),
                        next.left.get(&id).unwrap().2.clone(),
                    ))
                } else if before && !after {
                    Some(TupleChange::retract(id))
                } else {
                    None
                }
            })
            .collect();
        self.state = next;
        output
    }
    pub(crate) fn clear(&mut self) {
        self.state = State::new();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        J: 'static,
        L: 'static,
        E: 'static,
        K: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, s: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        E: 'static,
        K: 'static,
    {
        self.state = *s.downcast().expect("ordered existence checkpoint");
    }
}
fn changed_range<'a, K: Ord>(
    a: Option<&'a K>,
    b: Option<&'a K>,
    c: JoinComparison,
) -> Option<(Bound<&'a K>, Bound<&'a K>)> {
    if a == b {
        return None;
    }
    let closed_low = matches!(
        c,
        JoinComparison::LessThan | JoinComparison::GreaterThanOrEqual
    );
    Some(match (a, b) {
        (Some(a), Some(b)) => {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            if closed_low {
                (Included(lo), Excluded(hi))
            } else {
                (Excluded(lo), Included(hi))
            }
        }
        (Some(k), None) | (None, Some(k)) => match c {
            JoinComparison::LessThan => (Unbounded, Excluded(k)),
            JoinComparison::LessThanOrEqual => (Unbounded, Included(k)),
            JoinComparison::GreaterThan => (Excluded(k), Unbounded),
            JoinComparison::GreaterThanOrEqual => (Included(k), Unbounded),
            JoinComparison::Equal => unreachable!(),
        },
        _ => unreachable!(),
    })
}
