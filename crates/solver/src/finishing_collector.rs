//! Pure output transformations of reversible collectors.
use crate::ReversibleCollector;
use std::marker::PhantomData;

/// A deterministic transformation of an owned collector result.
///
/// Implementations must not mutate shared state or depend on call history. Empty
/// results are passed through too. Input and output must obey the collector's
/// immutable-snapshot contract. Use a graph map for closures with runtime captures.
pub trait CollectorFinisher<Input> {
    type Output: Clone;
    fn finish(input: Input) -> Self::Output;
}

/// Transform a collector's result without changing its captured input semantics.
/// Nest PairCollector to combine any number of independently mapped inputs, then
/// use this adapter to flatten or derive a custom result. The finisher is called
/// on each finish, including empty standalone results; an empty stream group still
/// emits no row. It is never called by standalone insert/remove/replace.
///
/// Mutations stage the inner collector before installation. Replacement delegates
/// to its specialized replace method, preserving checked intermediate arithmetic
/// and conditional-update behavior. A missing removal/replacement leaves the
/// adapter unchanged. A finisher panic leaves the collector unchanged; inside a
/// graph transaction, normal graph failure/rollback rules also apply.
///
/// ```
/// use timetuck::{CollectorFinisher,CountCollector,FinishingCollector,ReversibleCollector};
/// struct Tens;
/// impl CollectorFinisher<usize> for Tens {
///     type Output=usize;
///     fn finish(count:usize)->usize {count*10}
/// }
/// let mut c=FinishingCollector::<CountCollector,Tens>::default();
/// c.insert(&"a"); c.insert(&"b");
/// assert_eq!(<FinishingCollector<CountCollector,Tens> as ReversibleCollector<&str>>::finish(&c),20);
/// ```
pub struct FinishingCollector<C, F> {
    inner: C,
    finisher: PhantomData<fn() -> F>,
}
impl<C, F> FinishingCollector<C, F> {
    pub fn from_collector(inner: C) -> Self {
        Self {
            inner,
            finisher: PhantomData,
        }
    }
    pub fn into_inner(self) -> C {
        self.inner
    }
}
impl<C: Default, F> Default for FinishingCollector<C, F> {
    fn default() -> Self {
        Self::from_collector(C::default())
    }
}
impl<C: Clone, F> Clone for FinishingCollector<C, F> {
    fn clone(&self) -> Self {
        Self::from_collector(self.inner.clone())
    }
}
impl<V, C: ReversibleCollector<V>, F: CollectorFinisher<C::Output>> ReversibleCollector<V>
    for FinishingCollector<C, F>
{
    type Output = F::Output;
    fn insert(&mut self, value: &V) {
        let mut next = self.inner.clone();
        next.insert(value);
        self.inner = next;
    }
    fn remove(&mut self, value: &V) -> bool {
        let mut next = self.inner.clone();
        if !next.remove(value) {
            return false;
        }
        self.inner = next;
        true
    }
    fn replace(&mut self, old: &V, new: &V) -> bool {
        let mut next = self.inner.clone();
        if !next.replace(old, new) {
            return false;
        }
        self.inner = next;
        true
    }
    fn finish(&self) -> Self::Output {
        F::finish(self.inner.finish())
    }
}
