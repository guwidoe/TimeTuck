//! Pair-free existence checks over equality partitions and ordered keys.
use crate::{ExistenceMode, JoinComparison, SnapshotMap, TupleChange};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    ops::Bound::{self, *},
};

type KeyProjection<'a, T, E, K> = dyn Fn(&T) -> (E, K) + 'a;
type Identity<I, J> = (fn(&I) -> J, fn(&J) -> I);
type RightMembers<J, E, K> = SnapshotMap<(E, K), SnapshotMap<J, ()>>;

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
    members: Option<RightMembers<J, E, K>>,
}
impl<I, J, L, E, K> State<I, J, L, E, K> {
    fn new() -> Self {
        Self {
            left: SnapshotMap::new(),
            right: SnapshotMap::new(),
            groups: SnapshotMap::new(),
            members: None,
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
    fn matched(&self, e: &E, k: &K, c: JoinComparison, excluded: Option<&J>) -> bool {
        let Some(p) = self.groups.get(e) else {
            return false;
        };
        let excluded = excluded
            .and_then(|id| self.right.get(id))
            .filter(|(partition, _)| partition == e)
            .map(|(_, key)| key);
        if c == JoinComparison::Equal {
            p.right.get(k).copied().unwrap_or(0) > usize::from(excluded == Some(k))
        } else {
            let Some(best) = boundary(Some(p), c) else {
                return false;
            };
            if !c.matches(k, best) {
                return false;
            }
            if excluded != Some(best) || p.right[best] > 1 {
                return true;
            }
            let second = match c {
                JoinComparison::LessThan | JoinComparison::LessThanOrEqual => {
                    p.right.len().checked_sub(2)
                }
                _ => Some(1),
            };
            second
                .and_then(|index| p.right.entry_at(index))
                .is_some_and(|(next, _)| c.matches(k, next))
        }
    }
    fn member(&self, e: &E, k: &K) -> Option<&J> {
        if self.groups.get(e)?.right.get(k)? != &1 {
            return None;
        }
        self.members
            .as_ref()?
            .get(&(e.clone(), k.clone()))?
            .first_key_value()
            .map(|(id, _)| id)
    }
    fn update_member(&mut self, id: &J, e: &E, k: &K, insert: bool) {
        let Some(members) = &mut self.members else {
            return;
        };
        let key = (e.clone(), k.clone());
        let mut ids = members.get(&key).cloned().unwrap_or_default();
        if insert {
            ids.insert(id.clone(), ());
        } else {
            ids.remove(id);
        }
        if ids.is_empty() {
            members.remove(&key);
        } else {
            members.insert(key, ids);
        }
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
            self.update_member(&id, &e, &k, false);
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
            self.update_member(&id, e, k, true);
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
/// key determines ordinary visibility. Right updates visit crossed left keys;
/// equality updates visit buckets only on zero crossings. Identity exclusion also
/// checks sole-witness IDs and the next extremum, without scanning matching pairs.
/// Both sides support atomic, coalesced final-state rounds and immediate updates.
///
/// Keys/callbacks must be stable and pure. Ordinary iterator, projection, comparison
/// and clone panics leave state unchanged; destructor panics and external effects
/// are excluded. Use ExistsNode for arbitrary residual predicates. The ordinary
/// constructor includes matching self occurrences; new_other excludes equal IDs.
pub struct ExistsCompareNode<'a, I, J, L, R, E, K> {
    state: State<I, J, L, E, K>,
    comparison: JoinComparison,
    mode: ExistenceMode,
    identity: Option<Identity<I, J>>,
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
            identity: None,
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
        }
    }
    fn visible(&self, state: &State<I, J, L, E, K>, id: &I) -> bool {
        let excluded = self.identity.map(|(to_right, _)| to_right(id));
        state.left.get(id).is_some_and(|(e, k, _)| {
            state.matched(e, k, self.comparison, excluded.as_ref())
                == (self.mode == ExistenceMode::Exists)
        })
    }
    pub fn rows(&self) -> impl Iterator<Item = (&I, &L)> {
        self.state.left.iter().filter_map(|(id, (e, k, l))| {
            let excluded = self.identity.map(|(to_right, _)| to_right(id));
            (self.state.matched(e, k, self.comparison, excluded.as_ref())
                == (self.mode == ExistenceMode::Exists))
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
                if let Some((_, to_left)) = self.identity {
                    affected.extend(self.state.member(e, k).map(to_left));
                    affected.extend(next.member(e, k).map(to_left));
                }
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
                if let Some((_, to_left)) = self.identity {
                    affected.extend(a.and_then(|k| self.state.member(e, k)).map(to_left));
                    affected.extend(b.and_then(|k| next.member(e, k)).map(to_left));
                }
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
                let before = self.visible(&self.state, &id);
                let after = self.visible(&next, &id);
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
        if self.identity.is_some() {
            self.state.members = Some(SnapshotMap::new());
        }
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

impl<'a, I: Ord + Clone, L: Clone, R, E: Ord + Clone, K: Ord + Clone>
    ExistsCompareNode<'a, I, I, L, R, E, K>
{
    /// Exclude the right occurrence with the same stable ID as each left row.
    /// Both inputs must use the same identity namespace. Projections may differ.
    /// Retains an additional linear right-key membership index to find the sole
    /// witness at a key; no pair links are stored. Empty and singleton partitions,
    /// duplicate keys and changing second-best witnesses retain final-batch semantics.
    pub fn new_other(
        mode: ExistenceMode,
        comparison: JoinComparison,
        left_key: impl Fn(&L) -> (E, K) + 'a,
        right_key: impl Fn(&R) -> (E, K) + 'a,
    ) -> Self {
        let mut node = Self::new(mode, comparison, left_key, right_key);
        node.identity = Some((I::clone, I::clone));
        node.state.members = Some(SnapshotMap::new());
        node
    }
}
