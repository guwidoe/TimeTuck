//! Optional live component views over the existing connected-range collector.
use crate::{
    ConnectedRangeChain, ConnectedRangeCollector, IntegerRangeMetric, RangeMetric,
    ReversibleCollector,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    hash::{Hash, Hasher},
    rc::Rc,
};

struct PublishedRanges<I, P, V, D> {
    chain: ConnectedRangeChain<I, P, V, D>,
    components: BTreeMap<I, usize>,
}

type SharedRanges<I, P, V, D> = Rc<RefCell<PublishedRanges<I, P, V, D>>>;

/// A held view follows the component containing its captured logical occurrence.
/// Merges and splits are observed through the same view; removing the anchor
/// makes it absent, and reinserting that ID reattaches it. Immutable snapshots
/// retain their prior data. Hash reads current contents and does not cache them;
/// no Eq implementation implies a mutable view is a stable collection key.
pub struct LiveConnectedRangeView<I, P, V, D> {
    anchor: I,
    published: SharedRanges<I, P, V, D>,
}
impl<I: Ord, P, V, D> LiveConnectedRangeView<I, P, V, D> {
    pub fn with_current<R>(
        &self,
        read: impl FnOnce(Option<&crate::ConnectedRange<I, P, V, D>>) -> R,
    ) -> R {
        let published = self.published.borrow();
        read(
            published
                .components
                .get(&self.anchor)
                .map(|&index| &published.chain.ranges()[index]),
        )
    }
}
impl<I: Ord + Hash, P: Hash, V: Hash, D: Hash> Hash for LiveConnectedRangeView<I, P, V, D> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.with_current(|range| range.hash(state));
    }
}

/// One owner stages actual collector updates and endpoint sweeps before
/// publishing a new immutable chain to held views. It intentionally has no Clone
/// implementation: independent collector forks must not share mutable publication.
/// Updates use the existing collector's rebuild algorithm and add an ID index;
/// this optional facade makes no incremental-maintenance or speed claim. Hashing
/// performs one index lookup and hashes borrowed component contents without cloning
/// payloads or rebuilding. Existing ordinary collectors/snapshots remain unchanged.
pub struct LiveConnectedRangeCollector<I, P, V, M: RangeMetric<P> = IntegerRangeMetric> {
    collector: ConnectedRangeCollector<I, P, V, M>,
    published: SharedRanges<I, P, V, M::Distance>,
}
impl<I: Ord + Clone, P: Ord + Clone, V: Eq + Clone, M: RangeMetric<P>> Default
    for LiveConnectedRangeCollector<I, P, V, M>
{
    fn default() -> Self {
        Self::with_metric(M::default())
    }
}
impl<I: Ord + Clone, P: Ord + Clone, V: Eq + Clone, M: RangeMetric<P>>
    LiveConnectedRangeCollector<I, P, V, M>
{
    pub fn with_metric(metric: M) -> Self {
        let collector = ConnectedRangeCollector::with_metric(metric);
        let chain = collector.finish();
        Self {
            collector,
            published: Rc::new(RefCell::new(PublishedRanges {
                chain,
                components: BTreeMap::new(),
            })),
        }
    }
    fn publish(&mut self, next: ConnectedRangeCollector<I, P, V, M>) {
        let chain = next.finish();
        let components = chain
            .ranges()
            .iter()
            .enumerate()
            .flat_map(|(index, range)| {
                range
                    .items()
                    .iter()
                    .map(move |item| (item.id.clone(), index))
            })
            .collect();
        let published = PublishedRanges { chain, components };
        // Acquire before changing either owner state, so a conflicting read also
        // cannot partially publish. All fallible cloning/metric work is complete.
        let mut target = self.published.borrow_mut();
        self.collector = next;
        *target = published;
    }
    pub fn insert(&mut self, item: &(I, P, P, V)) {
        let mut next = self.collector.clone();
        next.insert(item);
        self.publish(next);
    }
    pub fn remove(&mut self, item: &(I, P, P, V)) -> bool {
        let mut next = self.collector.clone();
        if !next.remove(item) {
            return false;
        }
        self.publish(next);
        true
    }
    pub fn snapshot(&self) -> ConnectedRangeChain<I, P, V, M::Distance> {
        self.published.borrow().chain.clone()
    }
    pub fn view(&self, anchor: I) -> Option<LiveConnectedRangeView<I, P, V, M::Distance>> {
        self.published
            .borrow()
            .components
            .contains_key(&anchor)
            .then(|| LiveConnectedRangeView {
                anchor,
                published: self.published.clone(),
            })
    }
}
