//! Extrema of non-ordered payloads, using captured order keys and stable IDs.
use crate::{ReversibleCollector, SnapshotMap};

/// Owned selected item, including the key used for ordering and captured payload.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Extremum<I, K, V> {
    pub id: I,
    pub key: K,
    pub value: V,
}
/// Select the smallest key, breaking ties by smallest active logical ID.
pub type MinByCollector<I, K, V> = ExtremumCollector<I, K, V, false>;
/// Select the largest key, breaking ties by smallest active logical ID.
pub type MaxByCollector<I, K, V> = ExtremumCollector<I, K, V, true>;

/// Collect `(id, order_key, payload)` occurrences. Payloads need no ordering.
/// Capture keys in a mapping callback; use Reverse, tuples or a custom Ord key
/// type to express other orderings. Key ordering/equality must be pure, consistent
/// and immutable. No callback is repeated during retraction or finish.
///
/// Both minima and maxima choose the smallest active ID among equal keys. This
/// stable native rule makes reconstruction and transaction undo deterministic.
/// The public Java API permits any tied element, including different choices after
/// updates. Exact Java tie identity, mutable aliases and live views are not claimed.
///
/// Repeated IDs must retain equal captured key/payload until all occurrences are
/// removed. Retraction uses captured ID only, ignoring supplied key/payload. Use
/// separate occurrence IDs for independently changing rows. Invalid conflicting
/// insertion/replacement panics atomically; missing removal/replacement returns
/// false. Changes stage persistent ID and key indexes before installation.
///
/// Clone shares roots. Finish selects an endpoint key and its first ID, returning
/// an owned value; empty finish is None. Empty stream groups emit no row. No scan
/// of all payloads is required and no chronological ordering is imposed on them.
///
/// ```
/// use timetuck::{MinByCollector,ReversibleCollector,CalendarPeriod};
/// let mut c=MinByCollector::<usize,i32,CalendarPeriod>::default();
/// c.insert(&(4,2,CalendarPeriod::from_days(8)));
/// c.insert(&(1,2,CalendarPeriod::from_months(3)));
/// assert_eq!(c.finish().unwrap().id,1);
/// c.remove(&(1,99,CalendarPeriod::ZERO)); // Uses the captured identity.
/// assert_eq!(c.finish().unwrap().value,CalendarPeriod::from_days(8));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtremumCollector<I, K, V, const MAX: bool> {
    entries: SnapshotMap<I, (K, V, usize)>,
    keys: SnapshotMap<K, SnapshotMap<I, V>>,
}
impl<I, K, V, const MAX: bool> Default for ExtremumCollector<I, K, V, MAX> {
    fn default() -> Self {
        Self {
            entries: SnapshotMap::new(),
            keys: SnapshotMap::new(),
        }
    }
}
impl<I: Clone + Ord, K: Clone + Ord, V: Clone + PartialEq, const MAX: bool>
    ReversibleCollector<(I, K, V)> for ExtremumCollector<I, K, V, MAX>
{
    type Output = Option<Extremum<I, K, V>>;
    fn insert(&mut self, row: &(I, K, V)) {
        let (id, key, value) = row;
        let count = if let Some((old_key, old_value, count)) = self.entries.get(id) {
            assert!(
                old_key == key && old_value == value,
                "conflicting active extremum identity"
            );
            count
                .checked_add(1)
                .expect("extremum occurrence count overflow")
        } else {
            1
        };
        let mut next = self.clone();
        next.entries
            .insert(id.clone(), (key.clone(), value.clone(), count));
        if count == 1 {
            let mut group = next.keys.get(key).cloned().unwrap_or_default();
            group.insert(id.clone(), value.clone());
            next.keys.insert(key.clone(), group);
        }
        *self = next;
    }
    fn remove(&mut self, row: &(I, K, V)) -> bool {
        let id = &row.0;
        let Some((key, value, count)) = self.entries.get(id) else {
            return false;
        };
        let mut next = self.clone();
        if *count > 1 {
            next.entries
                .insert(id.clone(), (key.clone(), value.clone(), count - 1));
        } else {
            next.entries.remove(id);
            let mut group = next.keys[key].clone();
            group.remove(id);
            if group.is_empty() {
                next.keys.remove(key);
            } else {
                next.keys.insert(key.clone(), group);
            }
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        let (key, group) = if MAX {
            self.keys.last_key_value()
        } else {
            self.keys.first_key_value()
        }?;
        let (id, value) = group.first_key_value().expect("nonempty extremum bucket");
        Some(Extremum {
            id: id.clone(),
            key: key.clone(),
            value: value.clone(),
        })
    }
}
