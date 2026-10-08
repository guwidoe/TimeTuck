//! Indexed existence relations that preserve left occurrence identities.
use crate::{SnapshotMap, TupleChange};
use std::{any::Any, collections::BTreeSet};

/// Whether a left occurrence is retained when at least one matching right exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExistenceMode {
    Exists,
    NotExists,
}
impl ExistenceMode {
    fn accepts(self, matched: bool) -> bool {
        matched == (self == Self::Exists)
    }
}

#[derive(Clone)]
struct Side<I, J, T, K> {
    rows: SnapshotMap<I, (K, T)>,
    index: SnapshotMap<K, SnapshotMap<I, ()>>,
    links: SnapshotMap<I, SnapshotMap<J, ()>>,
}
impl<I, J, T, K> Side<I, J, T, K> {
    fn new() -> Self {
        Self {
            rows: SnapshotMap::new(),
            index: SnapshotMap::new(),
            links: SnapshotMap::new(),
        }
    }
}
impl<I: Ord + Clone, J: Ord + Clone, T: Clone, K: Ord + Clone> Side<I, J, T, K> {
    fn set_row(&mut self, id: I, new: Option<(K, T)>) {
        if let Some((key, _)) = self.rows.get(&id) {
            let mut ids = self.index[key].clone();
            ids.remove(&id);
            if ids.is_empty() {
                self.index.remove(key);
            } else {
                self.index.insert(key.clone(), ids);
            }
        }
        self.rows.remove(&id);
        if let Some((key, row)) = new {
            let mut ids = self.index.get(&key).cloned().unwrap_or_default();
            ids.insert(id.clone(), ());
            self.index.insert(key.clone(), ids);
            self.rows.insert(id, (key, row));
        }
    }
    fn set_link(&mut self, id: &I, other: &J, present: bool) {
        let mut links = self.links.get(id).cloned().unwrap_or_default();
        if present {
            links.insert(other.clone(), ());
        } else {
            links.remove(other);
        }
        if links.is_empty() {
            self.links.remove(id);
        } else {
            self.links.insert(id.clone(), links);
        }
    }
    fn matched(&self, id: &I) -> bool {
        self.links.contains_key(id)
    }
}

#[derive(Clone)]
struct State<I, J, L, R, K> {
    left: Side<I, J, L, K>,
    right: Side<J, I, R, K>,
}
impl<I, J, L, R, K> State<I, J, L, R, K> {
    fn new() -> Self {
        Self {
            left: Side::new(),
            right: Side::new(),
        }
    }
}
type Predicate<'a, L, R> = dyn Fn(&L, &R) -> bool + 'a;

/// Retains each left occurrence once when it has a match (or none in NotExists
/// mode). Equality keys restrict predicate evaluation to the changed row's bucket.
/// A constant key permits an arbitrary predicate; a tuple key permits several
/// equalities. Excluding self matches is an explicit stable-identity predicate.
///
/// Cached bidirectional match IDs make removals callback-free. Storage is linear
/// in facts plus matching pairs; dense buckets can therefore require quadratic
/// space. Right updates emit only visibility transitions. Accepted left updates
/// also emit the new payload, even when membership remains unchanged.
///
/// Each call prepares persistent state and output before installing either, so
/// ordinary callback/key/clone panics leave this node unchanged. Graph guards
/// additionally restore other nodes. Immutable keys/facts and deterministic
/// callbacks are required; panicking destructors retain Rust's usual limitations.
///
/// ```
/// use timetuck::{ExistenceMode, ExistsNode, TupleChange};
/// let mut node = ExistsNode::<u32, u32, &str, &str, ()>::new(
///     ExistenceMode::NotExists, |_| (), |_| (), |left, right| left == right);
/// assert!(node.apply_left(TupleChange::put(1, "skill")).is_some());
/// assert_eq!(node.apply_right(TupleChange::put(2, "skill")),
///     vec![TupleChange::retract(1)]);
/// assert_eq!(node.apply_right(TupleChange::retract(2)),
///     vec![TupleChange::put(1, "skill")]);
/// ```
pub struct ExistsNode<'a, I, J, L, R, K> {
    state: State<I, J, L, R, K>,
    mode: ExistenceMode,
    left_key: Box<dyn Fn(&L) -> K + 'a>,
    right_key: Box<dyn Fn(&R) -> K + 'a>,
    predicate: Box<Predicate<'a, L, R>>,
}
impl<'a, I: Ord + Clone, J: Ord + Clone, L: Clone, R: Clone, K: Ord + Clone>
    ExistsNode<'a, I, J, L, R, K>
{
    pub fn new(
        mode: ExistenceMode,
        left_key: impl Fn(&L) -> K + 'a,
        right_key: impl Fn(&R) -> K + 'a,
        predicate: impl Fn(&L, &R) -> bool + 'a,
    ) -> Self {
        Self {
            state: State::new(),
            mode,
            left_key: Box::new(left_key),
            right_key: Box::new(right_key),
            predicate: Box::new(predicate),
        }
    }
    pub(crate) fn clear(&mut self) {
        self.state = State::new();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        K: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        J: 'static,
        L: 'static,
        R: 'static,
        K: 'static,
    {
        self.state = *checkpoint.downcast().expect("existence checkpoint type");
    }
    pub fn rows(&self) -> impl Iterator<Item = (&I, &L)> {
        self.state.left.rows.iter().filter_map(|(id, (_, row))| {
            self.mode
                .accepts(self.state.left.matched(id))
                .then_some((id, row))
        })
    }
    pub fn apply_left(&mut self, change: TupleChange<I, L>) -> Option<TupleChange<I, L>> {
        let new = change.value.map(|row| ((self.left_key)(&row), row));
        let mut matches = SnapshotMap::new();
        if let Some((key, row)) = &new
            && let Some(ids) = self.state.right.index.get(key)
        {
            for id in ids.keys() {
                if (self.predicate)(row, &self.state.right.rows[id].1) {
                    matches.insert(id.clone(), ());
                }
            }
        }
        let was_visible = self.state.left.rows.contains_key(&change.id)
            && self.mode.accepts(self.state.left.matched(&change.id));
        let output = match &new {
            Some((_, row)) if self.mode.accepts(!matches.is_empty()) => {
                Some(TupleChange::put(change.id.clone(), row.clone()))
            }
            _ if was_visible => Some(TupleChange::retract(change.id.clone())),
            _ => None,
        };
        let mut next = self.state.clone();
        if let Some(old) = self.state.left.links.get(&change.id) {
            for id in old.keys() {
                next.right.set_link(id, &change.id, false);
            }
        }
        for id in matches.keys() {
            next.right.set_link(id, &change.id, true);
        }
        if matches.is_empty() {
            next.left.links.remove(&change.id);
        } else {
            next.left.links.insert(change.id.clone(), matches);
        }
        next.left.set_row(change.id, new);
        self.state = next;
        output
    }
    pub fn apply_right(&mut self, change: TupleChange<J, R>) -> Vec<TupleChange<I, L>> {
        let new = change.value.map(|row| ((self.right_key)(&row), row));
        let mut matches = SnapshotMap::new();
        if let Some((key, row)) = &new
            && let Some(ids) = self.state.left.index.get(key)
        {
            for id in ids.keys() {
                if (self.predicate)(&self.state.left.rows[id].1, row) {
                    matches.insert(id.clone(), ());
                }
            }
        }
        let mut affected = BTreeSet::new();
        let mut next = self.state.clone();
        if let Some(old) = self.state.right.links.get(&change.id) {
            for id in old.keys() {
                affected.insert(id.clone());
                next.left.set_link(id, &change.id, false);
            }
        }
        for id in matches.keys() {
            affected.insert(id.clone());
            next.left.set_link(id, &change.id, true);
        }
        if matches.is_empty() {
            next.right.links.remove(&change.id);
        } else {
            next.right.links.insert(change.id.clone(), matches);
        }
        next.right.set_row(change.id, new);
        let mut output = Vec::new();
        for id in affected {
            let before = self.mode.accepts(self.state.left.matched(&id));
            let after = self.mode.accepts(next.left.matched(&id));
            if before != after {
                let value = after.then(|| next.left.rows[&id].1.clone());
                output.push(TupleChange { id, value });
            }
        }
        self.state = next;
        output
    }
}

pub mod batched;
