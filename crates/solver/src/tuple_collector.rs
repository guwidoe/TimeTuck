//! Flat, atomic collector composition and distinct occurrence counting.
use crate::{ReversibleCollector, SnapshotMap};

// Inputs and outputs have the same flat tuple shape. Each component sees only its
// captured field; replacing delegates to specialized collector replacement.
macro_rules! tuple_collector {
    ($($collector:ident $value:ident $index:tt),+) => {
        impl<$($value, $collector: ReversibleCollector<$value>),+>
            ReversibleCollector<($($value,)+)> for ($($collector,)+)
        {
            type Output = ($($collector::Output,)+);
            fn insert(&mut self, value: &($($value,)+)) {
                let mut next = self.clone();
                $(next.$index.insert(&value.$index);)+
                *self = next;
            }
            fn remove(&mut self, value: &($($value,)+)) -> bool {
                let mut next = self.clone();
                $(if !next.$index.remove(&value.$index) { return false; })+
                *self = next;
                true
            }
            fn replace(&mut self, old: &($($value,)+), new: &($($value,)+)) -> bool {
                let mut next = self.clone();
                $(if !next.$index.replace(&old.$index, &new.$index) { return false; })+
                *self = next;
                true
            }
            fn finish(&self) -> Self::Output { ($(self.$index.finish(),)+) }
        }
    };
}
tuple_collector!(A U 0, B V 1);
tuple_collector!(A U 0, B V 1, C W 2);
tuple_collector!(A U 0, B V 1, C W 2, D X 3);

/// Count distinct captured values while retaining occurrence multiplicities.
/// Removing one duplicate keeps the value active until its final occurrence is
/// removed. Empty finish is zero. Clones share persistent roots but subsequent
/// changes are isolated. Keys must have stable, consistent native Ord semantics.
///
/// Native tuples of two, three or four collectors implement ReversibleCollector
/// over corresponding input tuples, returning flat output tuples. All component
/// changes install together; false removal/replacement or ordinary panics preserve
/// the tuple's previous state, provided delegates honor the collector contract.
/// No input Clone bound is added by tuple composition. finish is pure and called
/// independently on each component; graph rollback handles downstream failure.
///
/// ```
/// use timetuck::*;
/// type C = (CountCollector, DistinctCountCollector<i32>, IntCollector);
/// type Input = ((), i32, i64);
/// let mut c = C::default();
/// c.insert(&((), 7, 10)); c.insert(&((), 7, 20));
/// let (count, distinct, summary) = <C as ReversibleCollector<Input>>::finish(&c);
/// assert_eq!((count, distinct, summary.sum), (2, 1, 30));
/// assert!(c.remove(&((), 7, 10)));
/// assert_eq!(<C as ReversibleCollector<Input>>::finish(&c).1, 1);
/// ```
#[derive(Clone)]
pub struct DistinctCountCollector<V> {
    counts: SnapshotMap<V, usize>,
}
impl<V> Default for DistinctCountCollector<V> {
    fn default() -> Self {
        Self {
            counts: SnapshotMap::new(),
        }
    }
}
impl<V: Ord + Clone> ReversibleCollector<V> for DistinctCountCollector<V> {
    type Output = usize;
    fn insert(&mut self, value: &V) {
        let count = self
            .counts
            .get(value)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .expect("distinct occurrence count overflow");
        let mut next = self.counts.clone();
        next.insert(value.clone(), count);
        self.counts = next;
    }
    fn remove(&mut self, value: &V) -> bool {
        let Some(&count) = self.counts.get(value) else {
            return false;
        };
        let mut next = self.counts.clone();
        if count == 1 {
            next.remove(value);
        } else {
            next.insert(value.clone(), count - 1);
        }
        self.counts = next;
        true
    }
    fn finish(&self) -> usize {
        self.counts.len()
    }
}
