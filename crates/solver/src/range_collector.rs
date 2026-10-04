//! Reversible connected ranges, implemented from public interval semantics.
use crate::{ReversibleCollector, SnapshotMap};
use std::{collections::BTreeMap, sync::Arc};

/// Pure distance between ordered range endpoints. The metric must be immutable
/// and deterministic. Ordering, connectivity and overlap use points, not distance.
pub trait RangeMetric<P>: Clone + Default {
    type Distance: Clone;
    fn distance(&self, start: &P, end: &P) -> Self::Distance;
}
/// Exact widened i64 endpoint subtraction; does not reproduce Java int overflow.
#[derive(Clone, Copy, Debug, Default)]
pub struct IntegerRangeMetric;
impl RangeMetric<i64> for IntegerRangeMetric {
    type Distance = i128;
    fn distance(&self, a: &i64, b: &i64) -> i128 {
        i128::from(*b) - i128::from(*a)
    }
}
/// Captured logical range. Results repeat the item once per occurrence.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RangeItem<I, P, V> {
    pub id: I,
    pub start: P,
    pub end: P,
    pub value: V,
}
/// One connected component, including touching endpoints and empty ranges.
/// Overlap counts apply on positive-length segments only; an empty component has
/// no such segment and returns None for both bounds. Empty ranges contribute no
/// overlap. This differs from Java's integer sentinels and empty-range flags.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ConnectedRange<I, P, V, D> {
    items: Vec<RangeItem<I, P, V>>,
    start: P,
    end: P,
    length: D,
    minimum_overlap: Option<usize>,
    maximum_overlap: Option<usize>,
}
impl<I, P, V, D> ConnectedRange<I, P, V, D> {
    pub fn items(&self) -> &[RangeItem<I, P, V>] {
        &self.items
    }
    pub fn count(&self) -> usize {
        self.items.len()
    }
    pub fn start(&self) -> &P {
        &self.start
    }
    pub fn end(&self) -> &P {
        &self.end
    }
    pub fn length(&self) -> &D {
        &self.length
    }
    pub fn minimum_overlap(&self) -> Option<usize> {
        self.minimum_overlap
    }
    pub fn maximum_overlap(&self) -> Option<usize> {
        self.maximum_overlap
    }
    pub fn has_overlap(&self) -> bool {
        self.maximum_overlap.is_some_and(|n| n > 1)
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct RangeGap<P, D> {
    previous: P,
    next: P,
    length: D,
}
impl<P, D> RangeGap<P, D> {
    pub fn previous_range_end(&self) -> &P {
        &self.previous
    }
    pub fn next_range_start(&self) -> &P {
        &self.next
    }
    pub fn length(&self) -> &D {
        &self.length
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct RangeChainData<I, P, V, D> {
    ranges: Vec<ConnectedRange<I, P, V, D>>,
    gaps: Vec<RangeGap<P, D>>,
}
/// Owned immutable snapshot. Items sort by captured start, end, then logical ID.
/// Java identity-hash ordering and mutable live views are not reproduced. Equality
/// and hashing compare complete native values, including logical IDs.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct ConnectedRangeChain<I, P, V, D>(Arc<RangeChainData<I, P, V, D>>);
impl<I, P, V, D> ConnectedRangeChain<I, P, V, D> {
    pub fn ranges(&self) -> &[ConnectedRange<I, P, V, D>] {
        &self.0.ranges
    }
    pub fn gaps(&self) -> &[RangeGap<P, D>] {
        &self.0.gaps
    }
}
struct RangeGroup<I, P, V> {
    start: P,
    end: P,
    items: Vec<RangeItem<I, P, V>>,
}

/// Collect `(id,start,end,value)` occurrences into touching/overlapping ranges.
/// Start must not exceed end. Repeated IDs must have equal captured fields;
/// remove all occurrences before changing that identity. Removal uses only ID,
/// including after a caller's external endpoints have changed. Distinct IDs may
/// have equal endpoints. Unlike SequenceCollector, repeated IDs remain repeated
/// in the result and contribute their multiplicity to nonempty overlap counts.
///
/// Clone shares persistent captured state. Updates install staged state only
/// after validation and cloning succeed; finish uses an endpoint sweep and owns
/// its output. It rebuilds components on finish, with no incremental boundary
/// maintenance or performance claim. Metric and value callbacks must be pure.
/// Empty standalone collection finishes to an empty chain; generic stream
/// aggregation emits no row for an empty group.
///
/// ```
/// use timetuck::{ConnectedRangeCollector, ReversibleCollector};
/// let mut c = ConnectedRangeCollector::<usize,i64,&str>::default();
/// c.insert(&(0,0,2,"a")); c.insert(&(1,2,4,"b"));
/// let saved=c.finish();
/// assert_eq!(saved.ranges().len(),1);
/// assert!(!saved.ranges()[0].has_overlap());
/// c.insert(&(2,1,3,"overlap"));
/// assert_eq!(c.finish().ranges()[0].maximum_overlap(),Some(2));
/// assert!(c.remove(&(1,99,100,"changed externally")));
/// assert_eq!(*c.finish().ranges()[0].end(),3);
/// assert_eq!(*saved.ranges()[0].end(),4);
/// ```
#[derive(Clone)]
pub struct ConnectedRangeCollector<I, P, V, M = IntegerRangeMetric> {
    entries: SnapshotMap<I, (P, P, V, usize)>,
    metric: M,
}
impl<I, P, V, M: RangeMetric<P>> Default for ConnectedRangeCollector<I, P, V, M> {
    fn default() -> Self {
        Self::with_metric(M::default())
    }
}
impl<I, P, V, M: RangeMetric<P>> ConnectedRangeCollector<I, P, V, M> {
    pub fn with_metric(metric: M) -> Self {
        Self {
            entries: SnapshotMap::new(),
            metric,
        }
    }
}
impl<I: Ord + Clone, P: Ord + Clone, V: Eq + Clone, M: RangeMetric<P>>
    ReversibleCollector<(I, P, P, V)> for ConnectedRangeCollector<I, P, V, M>
{
    type Output = ConnectedRangeChain<I, P, V, M::Distance>;
    fn insert(&mut self, (id, start, end, value): &(I, P, P, V)) {
        assert!(start <= end, "range start must not exceed end");
        let count = if let Some((a, b, v, n)) = self.entries.get(id) {
            assert!(
                a == start && b == end && v == value,
                "inconsistent duplicate range identity"
            );
            n.checked_add(1).expect("range multiplicity overflow")
        } else {
            1
        };
        let mut next = self.entries.clone();
        next.insert(
            id.clone(),
            (start.clone(), end.clone(), value.clone(), count),
        );
        self.entries = next;
    }
    fn remove(&mut self, (id, _, _, _): &(I, P, P, V)) -> bool {
        let Some((a, b, v, n)) = self.entries.get(id) else {
            return false;
        };
        let mut next = self.entries.clone();
        if *n == 1 {
            next.remove(id);
        } else {
            next.insert(id.clone(), (a.clone(), b.clone(), v.clone(), n - 1));
        }
        self.entries = next;
        true
    }
    fn finish(&self) -> Self::Output {
        let mut rows = self.entries.iter().collect::<Vec<_>>();
        rows.sort_by(|(ia, (a, b, _, _)), (ib, (c, d, _, _))| {
            a.cmp(c).then(b.cmp(d)).then(ia.cmp(ib))
        });
        let mut groups: Vec<RangeGroup<I, P, V>> = vec![];
        for (id, (a, b, v, n)) in rows {
            if groups.last().is_none_or(|g| a > &g.end) {
                groups.push(RangeGroup {
                    start: a.clone(),
                    end: b.clone(),
                    items: vec![],
                });
            }
            let RangeGroup { end, items, .. } = groups.last_mut().unwrap();
            if b > &*end {
                *end = b.clone();
            }
            for _ in 0..*n {
                items.push(RangeItem {
                    id: id.clone(),
                    start: a.clone(),
                    end: b.clone(),
                    value: v.clone(),
                });
            }
        }
        let ranges = groups
            .into_iter()
            .map(|RangeGroup { start, end, items }| {
                let mut events = BTreeMap::<P, (usize, usize)>::new();
                for item in &items {
                    if item.start < item.end {
                        events.entry(item.start.clone()).or_default().0 += 1;
                        events.entry(item.end.clone()).or_default().1 += 1;
                    }
                }
                let mut active = 0usize;
                let mut low = None;
                let mut high = None;
                let mut it = events.into_iter().peekable();
                while let Some((_, (added, removed))) = it.next() {
                    active = active
                        .checked_sub(removed)
                        .expect("range event balance")
                        .checked_add(added)
                        .expect("range overlap overflow");
                    if it.peek().is_some() {
                        low = Some(low.map_or(active, |v: usize| v.min(active)));
                        high = Some(high.map_or(active, |v: usize| v.max(active)));
                    }
                }
                assert_eq!(active, 0, "range event balance");
                let length = self.metric.distance(&start, &end);
                ConnectedRange {
                    items,
                    start,
                    end,
                    length,
                    minimum_overlap: low,
                    maximum_overlap: high,
                }
            })
            .collect::<Vec<_>>();
        let gaps = ranges
            .windows(2)
            .map(|p| RangeGap {
                previous: p[0].end.clone(),
                next: p[1].start.clone(),
                length: self.metric.distance(&p[0].end, &p[1].start),
            })
            .collect();
        ConnectedRangeChain(Arc::new(RangeChainData { ranges, gaps }))
    }
}
