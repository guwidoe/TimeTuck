//! Reversible collection of logical items into consecutive indexed sequences.
use crate::{ReversibleCollector, SnapshotMap};
use std::sync::Arc;

/// Distance rules for ordered indices. Implementations must be deterministic and
/// use immutable state. `maximum_gap()` must be greater than `zero()`. Adjacent
/// indices connect when their distance lies in `[zero, maximum_gap]`. A sequence's
/// length is its endpoint distance plus maximum_gap; a break uses distance alone.
/// Arithmetic and its overflow policy belong to the metric implementation.
pub trait SequenceMetric<P>: Clone + Default {
    type Distance: Clone + Ord;
    fn distance(&self, from: &P, to: &P) -> Self::Distance;
    fn add(&self, left: Self::Distance, right: Self::Distance) -> Self::Distance;
    fn zero(&self) -> Self::Distance;
    fn maximum_gap(&self) -> Self::Distance;
}
/// Integer indices with exact widened distances, including i32 and i64 endpoints.
/// STEP is both the largest connecting gap and the length of a singleton.
/// This does not reproduce Java int distance overflow.
#[derive(Clone, Copy, Debug, Default)]
pub struct IntegerSequenceMetric<const STEP: u64 = 1>;
impl<const STEP: u64> SequenceMetric<i64> for IntegerSequenceMetric<STEP> {
    type Distance = i128;
    fn distance(&self, from: &i64, to: &i64) -> i128 {
        i128::from(*to) - i128::from(*from)
    }
    fn add(&self, left: i128, right: i128) -> i128 {
        left.checked_add(right).expect("sequence distance overflow")
    }
    fn zero(&self) -> i128 {
        0
    }
    fn maximum_gap(&self) -> i128 {
        i128::from(STEP)
    }
}
impl<const STEP: u64> SequenceMetric<i32> for IntegerSequenceMetric<STEP> {
    type Distance = i128;
    fn distance(&self, from: &i32, to: &i32) -> i128 {
        i128::from(*to) - i128::from(*from)
    }
    fn add(&self, left: i128, right: i128) -> i128 {
        left.checked_add(right).expect("sequence distance overflow")
    }
    fn zero(&self) -> i128 {
        0
    }
    fn maximum_gap(&self) -> i128 {
        i128::from(STEP)
    }
}
/// Explicit wrapping signed-32-bit sequence distances and lengths.
///
/// Distance uses `to.wrapping_sub(from)` and length addition uses `wrapping_add`.
/// A wrapped negative distance becomes a break under the ordinary nonnegative
/// connectivity rule. Break and sequence lengths may therefore be negative.
/// STEP is the largest connecting gap and singleton length; constructing a
/// collector rejects zero or negative STEP. Exact integer metrics remain default.
#[derive(Clone, Copy, Debug, Default)]
pub struct WrappingIntegerSequenceMetric<const STEP: i32 = 1>;
impl<const STEP: i32> SequenceMetric<i32> for WrappingIntegerSequenceMetric<STEP> {
    type Distance = i32;
    fn distance(&self, from: &i32, to: &i32) -> i32 {
        to.wrapping_sub(*from)
    }
    fn add(&self, left: i32, right: i32) -> i32 {
        left.wrapping_add(right)
    }
    fn zero(&self) -> i32 {
        0
    }
    fn maximum_gap(&self) -> i32 {
        STEP
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceItem<I, P, V> {
    pub id: I,
    pub index: P,
    pub value: V,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsecutiveSequence<I, P, V, D> {
    items: Vec<SequenceItem<I, P, V>>,
    length: D,
    first: bool,
    last: bool,
    previous_break: Option<usize>,
    next_break: Option<usize>,
}
impl<I, P, V, D> ConsecutiveSequence<I, P, V, D> {
    pub fn items(&self) -> &[SequenceItem<I, P, V>] {
        &self.items
    }
    pub fn count(&self) -> usize {
        self.items.len()
    }
    pub fn first_item(&self) -> &SequenceItem<I, P, V> {
        &self.items[0]
    }
    pub fn last_item(&self) -> &SequenceItem<I, P, V> {
        self.items.last().expect("nonempty sequence")
    }
    pub fn length(&self) -> &D {
        &self.length
    }
    pub fn is_first(&self) -> bool {
        self.first
    }
    pub fn is_last(&self) -> bool {
        self.last
    }
    pub fn previous_break_index(&self) -> Option<usize> {
        self.previous_break
    }
    pub fn next_break_index(&self) -> Option<usize> {
        self.next_break
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceBreak<I, P, V, D> {
    previous: SequenceItem<I, P, V>,
    next: SequenceItem<I, P, V>,
    length: D,
    first: bool,
    last: bool,
}
impl<I, P, V, D> SequenceBreak<I, P, V, D> {
    pub fn previous_sequence_end(&self) -> &SequenceItem<I, P, V> {
        &self.previous
    }
    pub fn next_sequence_start(&self) -> &SequenceItem<I, P, V> {
        &self.next
    }
    pub fn length(&self) -> &D {
        &self.length
    }
    pub fn is_first(&self) -> bool {
        self.first
    }
    pub fn is_last(&self) -> bool {
        self.last
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct ChainData<I, P, V, D> {
    sequences: Vec<ConsecutiveSequence<I, P, V, D>>,
    breaks: Vec<SequenceBreak<I, P, V, D>>,
}
/// Immutable sequence/break snapshot. Equal-index items use ascending logical ID
/// order; Java's identity-hash tie order is not reproduced. Unlike Java live views,
/// a saved snapshot retains its contents after collector mutation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceChain<I, P, V, D>(Arc<ChainData<I, P, V, D>>);
impl<I, P, V, D> SequenceChain<I, P, V, D> {
    pub fn sequences(&self) -> &[ConsecutiveSequence<I, P, V, D>] {
        &self.0.sequences
    }
    pub fn breaks(&self) -> &[SequenceBreak<I, P, V, D>] {
        &self.0.breaks
    }
    pub fn first_sequence(&self) -> Option<&ConsecutiveSequence<I, P, V, D>> {
        self.sequences().first()
    }
    pub fn last_sequence(&self) -> Option<&ConsecutiveSequence<I, P, V, D>> {
        self.sequences().last()
    }
    pub fn first_break(&self) -> Option<&SequenceBreak<I, P, V, D>> {
        self.breaks().first()
    }
    pub fn last_break(&self) -> Option<&SequenceBreak<I, P, V, D>> {
        self.breaks().last()
    }
}
/// Collect `(logical_id, index, value)` occurrences into consecutive sequences.
/// Repeated IDs retain multiplicity but appear once in the result. Distinct IDs
/// at the same index remain distinct items. Repeated IDs must have equal captured
/// indices and values; remove all occurrences before changing that identity.
/// Removal uses the captured logical ID, never a newly computed index/value.
///
/// Clone shares persistent roots. Updates stage both indexes before installation;
/// ordinary clone/comparison panics leave state unchanged. Finish visits all
/// distinct items and returns an immutable snapshot. Metric callbacks run only in
/// construction/finish and must not mutate external state. This collector works
/// with `collect`, `group_collect_with`, conditional and composed collectors.
///
/// ```
/// use timetuck::{SequenceCollector, ReversibleCollector};
/// let mut c = SequenceCollector::<usize, i64, &str>::default();
/// c.insert(&(1, 3, "third"));
/// c.insert(&(2, 1, "first"));
/// let saved = c.finish();
/// assert_eq!(saved.sequences().len(), 2);
/// assert_eq!(*saved.first_break().unwrap().length(), 2);
/// c.insert(&(3, 2, "bridge"));
/// let joined = c.finish();
/// assert_eq!(joined.first_sequence().unwrap().count(), 3);
/// assert_eq!(*joined.first_sequence().unwrap().length(), 3);
/// assert!(joined.breaks().is_empty());
/// assert_eq!(saved.sequences().len(), 2);
/// ```
#[derive(Clone)]
pub struct SequenceCollector<I, P, V, M = IntegerSequenceMetric> {
    entries: SnapshotMap<I, (P, V, usize)>,
    order: SnapshotMap<(P, I), ()>,
    metric: M,
}
impl<I, P, V, M: SequenceMetric<P>> Default for SequenceCollector<I, P, V, M> {
    fn default() -> Self {
        Self::with_metric(M::default())
    }
}
impl<I, P, V, M: SequenceMetric<P>> SequenceCollector<I, P, V, M> {
    pub fn with_metric(metric: M) -> Self {
        assert!(
            metric.maximum_gap() > metric.zero(),
            "sequence gap must be positive"
        );
        Self {
            entries: SnapshotMap::new(),
            order: SnapshotMap::new(),
            metric,
        }
    }
}
impl<I: Ord + Clone, P: Ord + Clone, V: Eq + Clone, M: SequenceMetric<P>>
    ReversibleCollector<(I, P, V)> for SequenceCollector<I, P, V, M>
{
    type Output = SequenceChain<I, P, V, M::Distance>;
    fn insert(&mut self, (id, index, value): &(I, P, V)) {
        let mut next = self.clone();
        let count = if let Some((p, v, count)) = self.entries.get(id) {
            assert!(
                p == index && v == value,
                "inconsistent duplicate sequence identity"
            );
            count
                .checked_add(1)
                .expect("sequence multiplicity overflow")
        } else {
            1
        };
        next.entries
            .insert(id.clone(), (index.clone(), value.clone(), count));
        if count == 1 {
            next.order.insert((index.clone(), id.clone()), ());
        }
        *self = next;
    }
    fn remove(&mut self, (id, _, _): &(I, P, V)) -> bool {
        let Some((index, value, count)) = self.entries.get(id) else {
            return false;
        };
        let mut next = self.clone();
        if *count == 1 {
            next.order.remove(&(index.clone(), id.clone()));
            next.entries.remove(id);
        } else {
            next.entries
                .insert(id.clone(), (index.clone(), value.clone(), count - 1));
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        let mut runs: Vec<Vec<SequenceItem<I, P, V>>> = vec![];
        let gap = self.metric.maximum_gap();
        let zero = self.metric.zero();
        for ((index, id), ()) in self.order.iter() {
            let connected = runs.last().is_some_and(|r| {
                let d = self.metric.distance(&r.last().unwrap().index, index);
                d >= zero && d <= gap
            });
            if !connected {
                runs.push(vec![]);
            }
            runs.last_mut().unwrap().push(SequenceItem {
                id: id.clone(),
                index: index.clone(),
                value: self.entries[id].1.clone(),
            });
        }
        let n = runs.len();
        let sequences: Vec<_> = runs
            .into_iter()
            .enumerate()
            .map(|(i, items)| {
                let length = self.metric.add(
                    self.metric
                        .distance(&items[0].index, &items.last().unwrap().index),
                    gap.clone(),
                );
                ConsecutiveSequence {
                    items,
                    length,
                    first: i == 0,
                    last: i + 1 == n,
                    previous_break: i.checked_sub(1),
                    next_break: (i + 1 < n).then_some(i),
                }
            })
            .collect();
        let breaks = sequences
            .windows(2)
            .enumerate()
            .map(|(i, pair)| {
                let previous = pair[0].last_item().clone();
                let next = pair[1].first_item().clone();
                let length = self.metric.distance(&previous.index, &next.index);
                SequenceBreak {
                    previous,
                    next,
                    length,
                    first: i == 0,
                    last: i + 2 == n,
                }
            })
            .collect();
        SequenceChain(Arc::new(ChainData { sequences, breaks }))
    }
}
