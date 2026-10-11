//! Stable, independently ordered views over one immutable reachability index.
use crate::ReachableValues;
use std::{any::Any, cmp::Ordering, collections::BTreeMap, sync::Arc};

/// Identifies one comparator policy. Reusing a key reuses the original comparator;
/// callers assign a new key when the comparator's meaning changes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ReachabilityViewKey(pub u64);
type ValueComparator<V> = dyn Fn(&V, &V) -> Ordering + Send + Sync;
pub struct ReachabilityView<E, V> {
    index: ReachableValues<E, V>,
    compare: Option<Box<ValueComparator<V>>>,
}
impl<E: Ord + Clone, V: Ord + Clone + 'static> ReachabilityView<E, V> {
    pub fn reachable_index(&self) -> &ReachableValues<E, V> {
        &self.index
    }
    pub fn ordered_co_values(&self, origin: &V) -> Vec<&V> {
        match &self.compare {
            Some(compare) => self.index.co_values_by(origin, compare),
            None => self.index.co_values(origin),
        }
    }
    /// Exact native value type check for erased payloads, independent of domain
    /// membership. Entity and value types should be distinct if roles matter.
    /// Rust has no Java subclass/assignability or reflection equivalence here.
    pub fn matches_value_payload(&self, payload: &dyn Any) -> bool {
        payload.is::<V>()
    }
}
/// Cache ordered view identity by explicit policy key. Unsorted and sorted views
/// share an immutable index but remain distinct handles. Repeated key requests
/// retain the original handle/comparator. unsorted_view always requests the unsorted view;
/// creating another policy never mutates a previous view or the stored index.
/// Old Arc handles survive clear_views; the next request creates a fresh handle.
/// Comparators must maintain a stable total order; external state is not frozen.
///
/// ```
/// use timetuck::{ReachableValues, ReachabilityViews, ReachabilityViewKey};
/// use std::sync::Arc;
/// let index = ReachableValues::new(vec![(0, vec![1, 2, 3], false)]).unwrap();
/// let mut views = ReachabilityViews::new(index);
/// let descending = views.sorted_view(ReachabilityViewKey(1), |a, b| b.cmp(a));
/// assert_eq!(descending.ordered_co_values(&2), vec![&3, &1]);
/// assert!(Arc::ptr_eq(&descending,
///     &views.sorted_view(ReachabilityViewKey(1), |a, b| b.cmp(a))));
/// assert_eq!(views.unsorted_view().ordered_co_values(&2), vec![&1, &3]);
/// ```
pub struct ReachabilityViews<E, V> {
    index: ReachableValues<E, V>,
    unsorted: Option<Arc<ReachabilityView<E, V>>>,
    sorted: BTreeMap<ReachabilityViewKey, Arc<ReachabilityView<E, V>>>,
}
impl<E: Ord + Clone, V: Ord + Clone + 'static> ReachabilityViews<E, V> {
    pub fn new(index: ReachableValues<E, V>) -> Self {
        Self {
            index,
            unsorted: None,
            sorted: BTreeMap::new(),
        }
    }
    pub fn unsorted_view(&mut self) -> Arc<ReachabilityView<E, V>> {
        self.unsorted
            .get_or_insert_with(|| {
                Arc::new(ReachabilityView {
                    index: self.index.clone(),
                    compare: None,
                })
            })
            .clone()
    }
    pub fn sorted_view(
        &mut self,
        key: ReachabilityViewKey,
        compare: impl Fn(&V, &V) -> Ordering + Send + Sync + 'static,
    ) -> Arc<ReachabilityView<E, V>> {
        self.sorted
            .entry(key)
            .or_insert_with(|| {
                Arc::new(ReachabilityView {
                    index: self.index.clone(),
                    compare: Some(Box::new(compare)),
                })
            })
            .clone()
    }
    pub fn clear_views(&mut self) {
        self.unsorted = None;
        self.sorted.clear();
    }
}
