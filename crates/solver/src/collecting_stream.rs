//! Composable reversible collectors and persistent keyed/global aggregation.
use crate::{IntCollector, IntSummary, SnapshotMap, TupleChange};
use std::any::Any;

/// A captured-value collector. `Default` must be empty; `Clone` must isolate
/// subsequent mutation. Outputs must be immutable snapshots. Methods must be
/// deterministic, and `finish` must not change state. The owning CollectNode
/// isolates changes and rolls back ordinary panics, including in custom collectors.
/// A false removal indicates a missing occurrence and is an invariant violation
/// when called by a node that captured that occurrence.
pub trait ReversibleCollector<V>: Clone + Default {
    type Output: Clone;
    fn insert(&mut self, value: &V);
    fn remove(&mut self, value: &V) -> bool;
    /// Replace one captured occurrence. The default isolates removal/insertion;
    /// collectors may override this when replacement has distinct semantics.
    /// False means the old occurrence was absent, without changing this collector.
    fn replace(&mut self, old: &V, new: &V) -> bool {
        let mut next = self.clone();
        if !next.remove(old) {
            return false;
        }
        next.insert(new);
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output;
}
impl ReversibleCollector<i64> for IntCollector {
    type Output = IntSummary;
    fn insert(&mut self, value: &i64) {
        self.insert(*value);
    }
    fn remove(&mut self, value: &i64) -> bool {
        self.remove(*value)
    }
    fn finish(&self) -> IntSummary {
        self.summary()
    }
}
/// Counts occurrences without retaining their values. The owning node captures
/// them for updates and removals. Empty finish is zero.
#[derive(Clone, Debug, Default)]
pub struct CountCollector(usize);
impl<V> ReversibleCollector<V> for CountCollector {
    type Output = usize;
    fn insert(&mut self, _: &V) {
        self.0 = self.0.checked_add(1).expect("collector count overflow");
    }
    fn remove(&mut self, _: &V) -> bool {
        if self.0 == 0 {
            return false;
        }
        self.0 -= 1;
        true
    }
    fn finish(&self) -> usize {
        self.0
    }
}
/// An ordered set with captured occurrence multiplicities. The output map's keys
/// are the distinct value classes; its unit values carry no additional data.
/// As with BTreeSet, equality/ordering define membership; use stable fact IDs
/// instead of mutable fact objects. Finish returns an inexpensive snapshot.
#[derive(Clone)]
pub struct SetCollector<V> {
    counts: SnapshotMap<V, usize>,
    keys: SnapshotMap<V, ()>,
}
impl<V> Default for SetCollector<V> {
    fn default() -> Self {
        Self {
            counts: SnapshotMap::new(),
            keys: SnapshotMap::new(),
        }
    }
}
impl<V: Ord + Clone> ReversibleCollector<V> for SetCollector<V> {
    type Output = SnapshotMap<V, ()>;
    fn insert(&mut self, value: &V) {
        let mut next = self.clone();
        let count = next
            .counts
            .get(value)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .expect("collector count overflow");
        next.counts.insert(value.clone(), count);
        if count == 1 {
            next.keys.insert(value.clone(), ());
        }
        *self = next;
    }
    fn remove(&mut self, value: &V) -> bool {
        let Some(&count) = self.counts.get(value) else {
            return false;
        };
        let mut next = self.clone();
        if count == 1 {
            next.counts.remove(value);
            next.keys.remove(value);
        } else {
            next.counts.insert(value.clone(), count - 1);
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        self.keys.clone()
    }
}
/// Compose two collectors on independently mapped values. Nest pairs to collect
/// more fields. Each operation installs both components together.
#[derive(Clone, Default)]
pub struct PairCollector<A, B>(pub A, pub B);
impl<U, V, A: ReversibleCollector<U>, B: ReversibleCollector<V>> ReversibleCollector<(U, V)>
    for PairCollector<A, B>
{
    type Output = (A::Output, B::Output);
    fn insert(&mut self, value: &(U, V)) {
        let mut next = self.clone();
        next.0.insert(&value.0);
        next.1.insert(&value.1);
        *self = next;
    }
    fn remove(&mut self, value: &(U, V)) -> bool {
        let mut next = self.clone();
        if !next.0.remove(&value.0) || !next.1.remove(&value.1) {
            return false;
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        (self.0.finish(), self.1.finish())
    }
    fn replace(&mut self, old: &(U, V), new: &(U, V)) -> bool {
        let mut next = self.clone();
        if !next.0.replace(&old.0, &new.0) || !next.1.replace(&old.1, &new.1) {
            return false;
        }
        *self = next;
        true
    }
}
#[derive(Clone)]
struct State<I, K, V, C> {
    inputs: SnapshotMap<I, (K, V)>,
    groups: SnapshotMap<K, (usize, C)>,
}
impl<I, K, V, C> Default for State<I, K, V, C> {
    fn default() -> Self {
        Self {
            inputs: SnapshotMap::new(),
            groups: SnapshotMap::new(),
        }
    }
}
/// Reversible aggregation by key, capturing the mapped value of every occurrence.
/// Empty groups retract, including a global group represented by the unit key.
/// A collector's empty `finish()` does not imply an empty stream emits a row.
/// Changes prepare persistent state and finished outputs before installation,
/// preserving the prior node on ordinary clone/key/collector panics.
/// Built-in collectors share persistent roots; custom collector cloning may cost more.
///
/// ```
/// use timetuck::{CollectNode, CountCollector, TupleChange};
/// let mut node=CollectNode::<usize,(),(),CountCollector>::new();
/// assert_eq!(node.apply(TupleChange::put(0,((),()))),vec![TupleChange::put((),1)]);
/// assert_eq!(node.apply(TupleChange::retract(0)),vec![TupleChange::retract(())]);
/// assert_eq!(node.groups().count(),0);
/// ```
pub struct CollectNode<I, K, V, C> {
    state: State<I, K, V, C>,
}
impl<I: Ord + Clone, K: Ord + Clone, V: Clone, C: ReversibleCollector<V>> Default
    for CollectNode<I, K, V, C>
{
    fn default() -> Self {
        Self::new()
    }
}
impl<I: Ord + Clone, K: Ord + Clone, V: Clone, C: ReversibleCollector<V>> CollectNode<I, K, V, C> {
    pub fn new() -> Self {
        Self {
            state: State::default(),
        }
    }
    pub fn groups(&self) -> impl Iterator<Item = (&K, C::Output)> {
        self.state
            .groups
            .iter()
            .map(|(key, (_, c))| (key, c.finish()))
    }
    pub(crate) fn clear(&mut self) {
        self.state = State::default();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        K: 'static,
        V: 'static,
        C: 'static,
    {
        Box::new(self.state.clone())
    }
    pub(crate) fn restore(&mut self, c: Box<dyn Any>)
    where
        I: 'static,
        K: 'static,
        V: 'static,
        C: 'static,
    {
        self.state = *c.downcast().expect("collect checkpoint type");
    }
    pub fn apply(&mut self, change: TupleChange<I, (K, V)>) -> Vec<TupleChange<K, C::Output>> {
        let old = self.state.inputs.get(&change.id);
        if let (Some((old_key, old_value)), Some((key, value))) = (old, &change.value)
            && old_key == key
        {
            let mut next = self.state.clone();
            let (count, mut collector) = next.groups[key].clone();
            assert!(
                collector.replace(old_value, value),
                "captured collector occurrence"
            );
            let output = collector.finish();
            next.groups.insert(key.clone(), (count, collector));
            next.inputs.insert(change.id, (key.clone(), value.clone()));
            let output = vec![TupleChange::put(key.clone(), output)];
            self.state = next;
            return output;
        }
        let mut affected = Vec::new();
        if let Some((key, _)) = old {
            affected.push(key.clone());
        }
        if let Some((key, _)) = &change.value
            && old.map(|(k, _)| k) != Some(key)
        {
            affected.push(key.clone());
        }
        let mut next = self.state.clone();
        if let Some((key, value)) = old {
            let (count, mut c) = next.groups[key].clone();
            assert!(c.remove(value), "captured collector occurrence");
            if count == 1 {
                next.groups.remove(key);
            } else {
                next.groups.insert(key.clone(), (count - 1, c));
            }
        }
        next.inputs.remove(&change.id);
        if let Some((key, value)) = change.value {
            let (count, mut c) = next.groups.get(&key).cloned().unwrap_or_default();
            let count = count.checked_add(1).expect("group count overflow");
            c.insert(&value);
            next.groups.insert(key.clone(), (count, c));
            next.inputs.insert(change.id, (key, value));
        }
        let output = affected
            .into_iter()
            .map(|key| {
                let value = next.groups.get(&key).map(|(_, c)| c.finish());
                TupleChange { id: key, value }
            })
            .collect();
        self.state = next;
        output
    }
}
