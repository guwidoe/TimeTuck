//! Indexed unique pairs over one occurrence stream.
use crate::{SnapshotMap, TupleChange};
use std::any::Any;

type PairChanges<I, T> = Vec<TupleChange<(I, I), (T, T)>>;
#[derive(Clone)]
struct State<I, T, K> {
    rows: SnapshotMap<I, (K, T)>,
    buckets: SnapshotMap<K, SnapshotMap<I, ()>>,
}
impl<I, T, K> State<I, T, K> {
    fn new() -> Self {
        Self {
            rows: SnapshotMap::new(),
            buckets: SnapshotMap::new(),
        }
    }
}

/// Incremental unordered pairs, oriented by stable occurrence ID: `(smaller, larger)`.
/// Equal projected keys qualify; a constant key gives every unique pair. Duplicate
/// payloads remain distinct occurrences. No self pair or reversed duplicate is
/// emitted. Source arrival order does not affect orientation.
///
/// Updates visit only the old/new key buckets. Same-key updates refresh every
/// affected pair once; key changes retract old pairs before inserting new ones.
/// Retractions use captured keys without invoking the projection. Persistent
/// state stores one row and index membership per occurrence, not all pairs.
/// Ordinary callback/clone/order panics preserve the prior standalone state.
/// Keys and IDs must have stable ordering and payloads must be immutable snapshots.
/// Dense output is still quadratic; downstream nodes may retain all pairs.
pub struct UniquePairNode<'a, I, T, K> {
    state: State<I, T, K>,
    key: Box<dyn Fn(&T) -> K + 'a>,
}
impl<'a, I: Ord + Clone, T: Clone, K: Ord + Clone> UniquePairNode<'a, I, T, K> {
    pub fn new(key: impl Fn(&T) -> K + 'a) -> Self {
        Self {
            state: State::new(),
            key: Box::new(key),
        }
    }
    pub fn input_len(&self) -> usize {
        self.state.rows.len()
    }
    pub fn key_count(&self) -> usize {
        self.state.buckets.len()
    }
    pub fn rows(&self) -> impl Iterator<Item = ((&I, &I), (&T, &T))> {
        self.state.rows.iter().flat_map(move |(id, (key, value))| {
            self.state
                .buckets
                .get(key)
                .into_iter()
                .flat_map(move |bucket| {
                    bucket
                        .keys()
                        .filter(move |other| id < *other)
                        .map(move |other| ((id, other), (value, &self.state.rows[other].1)))
                })
        })
    }
    pub fn apply(&mut self, change: TupleChange<I, T>) -> PairChanges<I, T> {
        let new = change.value.map(|value| ((self.key)(&value), value));
        let old = self.state.rows.get(&change.id);
        let same = old.zip(new.as_ref()).is_some_and(|(a, b)| a.0 == b.0);
        let mut out = Vec::new();
        if !same && let Some((key, _)) = old {
            for other in self.state.buckets[key].keys() {
                if other != &change.id {
                    let pair = if change.id < *other {
                        (change.id.clone(), other.clone())
                    } else {
                        (other.clone(), change.id.clone())
                    };
                    out.push(TupleChange::retract(pair));
                }
            }
        }
        if let Some((key, value)) = &new
            && let Some(bucket) = self.state.buckets.get(key)
        {
            for other in bucket.keys() {
                if other != &change.id {
                    let other_value = &self.state.rows[other].1;
                    let (pair, values) = if change.id < *other {
                        (
                            (change.id.clone(), other.clone()),
                            (value.clone(), other_value.clone()),
                        )
                    } else {
                        (
                            (other.clone(), change.id.clone()),
                            (other_value.clone(), value.clone()),
                        )
                    };
                    out.push(TupleChange::put(pair, values));
                }
            }
        }
        let mut next = self.state.clone();
        if !same {
            if let Some((key, _)) = old {
                let mut bucket = next.buckets[key].clone();
                bucket.remove(&change.id);
                if bucket.is_empty() {
                    next.buckets.remove(key);
                } else {
                    next.buckets.insert(key.clone(), bucket);
                }
            }
            if let Some((key, _)) = &new {
                let mut bucket = next.buckets.get(key).cloned().unwrap_or_default();
                bucket.insert(change.id.clone(), ());
                next.buckets.insert(key.clone(), bucket);
            }
        }
        if let Some(row) = new {
            next.rows.insert(change.id, row);
        } else {
            next.rows.remove(&change.id);
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
        T: 'static,
        K: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        T: 'static,
        K: 'static,
    {
        self.state = *checkpoint.downcast().expect("unique pair checkpoint type");
    }
}
