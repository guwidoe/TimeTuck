//! One-to-many expansion with stable keyed occurrence identities.
use crate::{SnapshotMap, TupleChange};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet, VecDeque},
};

type Expansion<'a, T, U> = dyn Fn(&T) -> Vec<U> + 'a;
struct Occurrences<K> {
    rows: Vec<(K, usize)>,
    next: usize,
}
/// Expands inputs while matching each new occurrence to the earliest unmatched
/// old occurrence with the same key. Duplicates stay separate. Surviving IDs
/// remain stable across reordering; payloads are refreshed on every replacement.
/// New IDs increase within an input's lifetime and are not reused until that
/// input is retracted. Output order is removals first, then the new expansion.
/// IDs encode identity, not current position. Keys must have stable ordering.
///
/// `apply` returns changes immediately. Alternatively, `enqueue` defers both
/// mapping and delivery until `propagate`; queued changes run in insertion order
/// without coalescing. Mixing immediate updates with a pending queue is rejected.
/// Mapping, iteration, key, comparison and clone panics leave state and pending
/// work unchanged. External callback effects and panicking destructors are excluded.
/// Checkpoints share persistent roots. An update costs O((old+new) log(old+new))
/// plus the input-map update; it retains keys and IDs, not output payloads.
pub struct StableFlatMapNode<'a, I, T, U, K> {
    rows: SnapshotMap<I, Occurrences<K>>,
    mapping: Box<Expansion<'a, T, U>>,
    key: Box<dyn Fn(&U) -> K + 'a>,
    pending: Vec<TupleChange<I, T>>,
}
impl<'a, I: Ord + Clone, T, U, K: Ord + Clone> StableFlatMapNode<'a, I, T, U, K> {
    pub fn new<R: IntoIterator<Item = U>>(
        mapping: impl Fn(&T) -> R + 'a,
        key: impl Fn(&U) -> K + 'a,
    ) -> Self {
        Self {
            rows: SnapshotMap::new(),
            mapping: Box::new(move |v| mapping(v).into_iter().collect()),
            key: Box::new(key),
            pending: Vec::new(),
        }
    }
    fn prepare(
        &self,
        rows: &mut SnapshotMap<I, Occurrences<K>>,
        change: &TupleChange<I, T>,
    ) -> Vec<TupleChange<(I, usize), U>> {
        let old = rows.get(&change.id);
        let mut output = Vec::new();
        let Some(value) = &change.value else {
            if let Some(old) = old {
                output.extend(
                    old.rows
                        .iter()
                        .map(|(_, id)| TupleChange::retract((change.id.clone(), *id))),
                );
            }
            rows.remove(&change.id);
            return output;
        };
        let values = (self.mapping)(value);
        let mut available: BTreeMap<K, VecDeque<usize>> = BTreeMap::new();
        let mut next = old.map_or(0, |old| old.next);
        if let Some(old) = old {
            for (key, id) in &old.rows {
                available.entry(key.clone()).or_default().push_back(*id);
            }
        }
        let mut retained = BTreeSet::new();
        let mut occurrences = Vec::with_capacity(values.len());
        let mut puts = Vec::with_capacity(values.len());
        for value in values {
            let key = (self.key)(&value);
            let id = match available.get_mut(&key).and_then(VecDeque::pop_front) {
                Some(id) => {
                    retained.insert(id);
                    id
                }
                None => {
                    let id = next;
                    next = next
                        .checked_add(1)
                        .expect("stable expansion occurrence IDs exhausted");
                    id
                }
            };
            occurrences.push((key, id));
            puts.push(TupleChange::put((change.id.clone(), id), value));
        }
        if let Some(old) = old {
            output.extend(
                old.rows
                    .iter()
                    .filter(|(_, id)| !retained.contains(id))
                    .map(|(_, id)| TupleChange::retract((change.id.clone(), *id))),
            );
        }
        output.extend(puts);
        rows.insert(
            change.id.clone(),
            Occurrences {
                rows: occurrences,
                next,
            },
        );
        output
    }
    pub fn apply(&mut self, change: TupleChange<I, T>) -> Vec<TupleChange<(I, usize), U>> {
        assert!(
            self.pending.is_empty(),
            "propagate pending expansion changes before applying immediately"
        );
        let mut rows = self.rows.clone();
        let output = self.prepare(&mut rows, &change);
        self.rows = rows;
        output
    }
    pub fn enqueue(&mut self, change: TupleChange<I, T>) {
        self.pending.push(change);
    }
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
    /// Prepare the entire queue before publishing any change. A callback panic
    /// preserves the queue for retry and the last successfully propagated state.
    pub fn propagate(&mut self) -> Vec<TupleChange<(I, usize), U>> {
        let mut rows = self.rows.clone();
        let mut output = Vec::new();
        for change in &self.pending {
            output.extend(self.prepare(&mut rows, change));
        }
        self.rows = rows;
        self.pending.clear();
        output
    }
    pub(crate) fn clear(&mut self) {
        self.rows.clear();
        self.pending.clear();
    }
    pub(crate) fn checkpoint(&self) -> Box<dyn Any>
    where
        I: 'static,
        K: 'static,
    {
        assert!(
            self.pending.is_empty(),
            "graph expansions do not queue changes"
        );
        Box::new(self.rows.clone())
    }
    pub(crate) fn restore(&mut self, checkpoint: Box<dyn Any>)
    where
        I: 'static,
        K: 'static,
    {
        self.rows = *checkpoint
            .downcast()
            .expect("stable expansion checkpoint type");
        self.pending.clear();
    }
}
