//! Conditional aggregation over captured optional values.
use crate::ReversibleCollector;

/// Collect only `Some(value)` occurrences. `None` captures an input rejected by
/// the predicate; it is still a member of its owning CollectNode group. Thus a
/// nonempty group of rejected rows finishes to the delegate's empty result,
/// rather than disappearing as it would after filtering the stream.
///
/// Map the predicate and value once on input, using `bool::then` to avoid mapping
/// rejected inputs. Retraction uses that captured Option without reevaluation.
/// Active-to-active replacement delegates to C::replace, preserving specialized
/// arithmetic semantics. Other transitions insert/remove one captured occurrence.
/// Operations isolate delegate changes before installation, including ordinary
/// panics, provided C honors ReversibleCollector's clone contract.
///
/// ```
/// use timetuck::{ConditionalCollector, IntCollector, ReversibleCollector};
/// let mut c = ConditionalCollector::<IntCollector>::default();
/// c.insert(&None); // Retained group member; no collected integer.
/// assert_eq!(c.finish().min, None);
/// assert!(c.replace(&None, &Some(7)));
/// assert_eq!(c.finish().min, Some(7));
/// assert!(c.replace(&Some(7), &None));
/// assert_eq!(c.finish().count, 0);
/// assert!(c.remove(&None));
/// assert!(!c.remove(&None));
/// ```
#[derive(Clone, Default)]
pub struct ConditionalCollector<C> {
    delegate: C,
    skipped: usize,
}
impl<V, C: ReversibleCollector<V>> ReversibleCollector<Option<V>> for ConditionalCollector<C> {
    type Output = C::Output;
    fn insert(&mut self, value: &Option<V>) {
        let mut next = self.clone();
        match value {
            Some(value) => next.delegate.insert(value),
            None => {
                next.skipped = next
                    .skipped
                    .checked_add(1)
                    .expect("collector count overflow")
            }
        }
        *self = next;
    }
    fn remove(&mut self, value: &Option<V>) -> bool {
        let mut next = self.clone();
        match value {
            Some(value) => {
                if !next.delegate.remove(value) {
                    return false;
                }
            }
            None => {
                let Some(count) = next.skipped.checked_sub(1) else {
                    return false;
                };
                next.skipped = count;
            }
        }
        *self = next;
        true
    }
    fn replace(&mut self, old: &Option<V>, new: &Option<V>) -> bool {
        let mut next = self.clone();
        if let (Some(old), Some(new)) = (old, new) {
            if !next.delegate.replace(old, new) {
                return false;
            }
        } else {
            if !next.remove(old) {
                return false;
            }
            next.insert(new);
        }
        *self = next;
        true
    }
    fn finish(&self) -> Self::Output {
        self.delegate.finish()
    }
}
