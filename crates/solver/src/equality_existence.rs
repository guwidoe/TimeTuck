//! Equality-only existence with one count per right key and no pair links.
use crate::{ExistenceMode, SnapshotMap, TupleChange};
use std::any::Any;

#[derive(Clone)]
struct State<I, J, L, K> {
    left: SnapshotMap<I, (K, L)>,
    left_index: SnapshotMap<K, SnapshotMap<I, ()>>,
    right: SnapshotMap<J, K>,
    counts: SnapshotMap<K, usize>,
}
impl<I, J, L, K> State<I, J, L, K> {
    fn new() -> Self {
        Self {
            left: SnapshotMap::new(),
            left_index: SnapshotMap::new(),
            right: SnapshotMap::new(),
            counts: SnapshotMap::new(),
        }
    }
}
/// Equality-only semi/anti join preserving each accepted left occurrence once.
/// A constant key tests whether any right row exists; tuple keys express several
/// equalities. Right payloads are discarded after projecting their keys.
///
/// Persistent indexes store left rows, left key memberships, right ID-to-key
/// mappings and right counts. Space is linear in input rows even for dense keys.
/// Right updates visit left rows only when a key becomes empty or nonempty;
/// replacing a right row with the same key emits nothing. Left updates refresh
/// visible payloads, and retractions use captured keys without callbacks.
///
/// Candidate roots and outputs precede installation, preserving this standalone
/// node after ordinary callback/clone/comparison panics. Keys and callbacks must
/// remain deterministic and immutable; Eq must agree with Ord. Use ExistsNode
/// when a per-pair predicate or self-exclusion is required.
///
/// ```
/// use timetuck::{ExistsEqualNode, ExistenceMode, TupleChange};
/// let mut n = ExistsEqualNode::new(ExistenceMode::NotExists, |r: &i32| *r, |r: &i32| *r);
/// assert_eq!(n.apply_left(TupleChange::put(0,7)),Some(TupleChange::put(0,7)));
/// assert_eq!(n.apply_right(TupleChange::put(1,7)),vec![TupleChange::retract(0)]);
/// assert!(n.apply_right(TupleChange::put(2,7)).is_empty());
/// assert!(n.apply_right(TupleChange::retract(1)).is_empty());
/// assert_eq!(n.apply_right(TupleChange::retract(2)),vec![TupleChange::put(0,7)]);
/// ```
pub struct ExistsEqualNode<'a, I, J, L, R, K> {
    state: State<I, J, L, K>,
    mode: ExistenceMode,
    left_key: Box<dyn Fn(&L) -> K + 'a>,
    right_key: Box<dyn Fn(&R) -> K + 'a>,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R, K: Ord + Clone>
    ExistsEqualNode<'a, I, J, L, R, K>
{
    pub fn new(
        mode: ExistenceMode,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
    ) -> Self {
        Self {
            state: State::new(),
            mode,
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
        }
    }
    fn visible(&self, state: &State<I, J, L, K>, key: &K) -> bool {
        state.counts.contains_key(key) == (self.mode == ExistenceMode::Exists)
    }
    pub fn rows(&self) -> impl Iterator<Item = (&I, &L)> {
        self.state
            .left
            .iter()
            .filter_map(|(id, (key, row))| self.visible(&self.state, key).then_some((id, row)))
    }
    pub fn apply_left(&mut self, change: TupleChange<I, L>) -> Option<TupleChange<I, L>> {
        let new = change.value.map(|row| ((self.left_key)(&row), row));
        let old = self.state.left.get(&change.id);
        let before = old.is_some_and(|(key, _)| self.visible(&self.state, key));
        let out = match &new {
            Some((key, row)) if self.visible(&self.state, key) => {
                Some(TupleChange::put(change.id.clone(), row.clone()))
            }
            _ if before => Some(TupleChange::retract(change.id.clone())),
            _ => None,
        };
        let mut next = self.state.clone();
        let same = old.zip(new.as_ref()).is_some_and(|(a, b)| a.0 == b.0);
        if !same {
            if let Some((key, _)) = old {
                let mut ids = next.left_index.get(key).expect("left membership").clone();
                ids.remove(&change.id);
                if ids.is_empty() {
                    next.left_index.remove(key);
                } else {
                    next.left_index.insert(key.clone(), ids);
                }
            }
            if let Some((key, _)) = &new {
                let mut ids = next.left_index.get(key).cloned().unwrap_or_default();
                ids.insert(change.id.clone(), ());
                next.left_index.insert(key.clone(), ids);
            }
        }
        if let Some(row) = new {
            next.left.insert(change.id, row);
        } else {
            next.left.remove(&change.id);
        }
        self.state = next;
        out
    }
    pub fn apply_right(&mut self, change: TupleChange<J, R>) -> Vec<TupleChange<I, L>> {
        let new = change.value.map(|row| (self.right_key)(&row));
        let old = self.state.right.get(&change.id);
        if old == new.as_ref() {
            return Vec::new();
        }
        let mut next = self.state.clone();
        if let Some(key) = old {
            let count = next.counts[key];
            if count == 1 {
                next.counts.remove(key);
            } else {
                next.counts.insert(key.clone(), count - 1);
            }
        }
        if let Some(key) = &new {
            let count = next
                .counts
                .get(key)
                .copied()
                .unwrap_or(0)
                .checked_add(1)
                .expect("right count overflow");
            next.counts.insert(key.clone(), count);
        }
        let mut out = Vec::new();
        for key in [old, new.as_ref()].into_iter().flatten() {
            let before = self.visible(&self.state, key);
            let after = self.visible(&next, key);
            if before != after
                && let Some(ids) = next.left_index.get(key)
            {
                for id in ids.keys() {
                    out.push(TupleChange {
                        id: id.clone(),
                        value: after.then(|| next.left[id].1.clone()),
                    });
                }
            }
        }
        // Preserve deterministic left-ID ordering across a two-key transfer.
        out.sort_by(|a, b| a.id.cmp(&b.id));
        if let Some(key) = new {
            next.right.insert(change.id, key);
        } else {
            next.right.remove(&change.id);
        }
        self.state = next;
        out
    }
    pub(crate) fn clear(&mut self) {
        self.state = State::new();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        J: 'static,
        L: 'static,
        K: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, c: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        K: 'static,
    {
        self.state = *c.downcast().expect("equality existence checkpoint type");
    }
}
