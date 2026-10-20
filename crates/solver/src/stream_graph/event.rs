//! Exact Copy-only hot events; arbitrary types keep their shared erased owner.
use super::{Any, Rc, TupleChange};

type Triple = (usize, usize, usize);
type Planning = (usize, Option<usize>);

// Keep the largest inline row to a pair of triples. Wider group summaries and
// nested/flat-map rows stay shared instead of widening every pending event.
pub(super) enum Event {
    Pair(TupleChange<(usize, usize), (Triple, Triple)>),
    Triple(TupleChange<usize, Triple>),
    Planning(TupleChange<usize, Planning>),
    Shared(Rc<dyn Any>),
}
impl Event {
    #[inline]
    pub(super) fn new<T: Any>(value: T) -> Self {
        #[cfg(test)]
        if FORCE_SHARED.with(std::cell::Cell::get) {
            return Self::Shared(Rc::new(value));
        }
        let erased = &value as &dyn Any;
        if let Some(row) = erased.downcast_ref::<TupleChange<(usize, usize), (Triple, Triple)>>() {
            return Self::Pair(TupleChange {
                id: row.id,
                value: row.value,
            });
        }
        if let Some(row) = erased.downcast_ref::<TupleChange<usize, Triple>>() {
            return Self::Triple(TupleChange {
                id: row.id,
                value: row.value,
            });
        }
        if let Some(row) = erased.downcast_ref::<TupleChange<usize, Planning>>() {
            return Self::Planning(TupleChange {
                id: row.id,
                value: row.value,
            });
        }
        Self::Shared(Rc::new(value))
    }
    // Promotion belongs to the producer, before cloning any fanout owners.
    pub(super) fn promote(self) -> Self {
        let owner: Rc<dyn Any> = match self {
            Self::Pair(row) => Rc::new(row),
            Self::Triple(row) => Rc::new(row),
            Self::Planning(row) => Rc::new(row),
            Self::Shared(row) => return Self::Shared(row),
        };
        Self::Shared(owner)
    }
    // Whole-event retaining adapters require a producer-promoted shared owner.
    // Never allocate a separate row owner for an individual endpoint here.
    pub(super) fn downcast<T: Any>(self) -> Result<Rc<T>, Self> {
        match self {
            Self::Shared(owner) => owner.downcast().map_err(Self::Shared),
            inline => Err(inline),
        }
    }
    #[inline]
    pub(super) fn downcast_ref<T: Any>(&self) -> Option<&T> {
        // Keep each concrete inline vtable at its own downcast. Merging them
        // first prevents the known type test from folding to the enum tag.
        match self {
            Self::Pair(row) => (row as &dyn Any).downcast_ref::<T>(),
            Self::Triple(row) => (row as &dyn Any).downcast_ref::<T>(),
            Self::Planning(row) => (row as &dyn Any).downcast_ref::<T>(),
            Self::Shared(row) => row.as_ref().downcast_ref::<T>(),
        }
    }
    #[cfg(test)]
    pub(super) fn shared_ref(&self) -> &Rc<dyn Any> {
        match self {
            Self::Shared(row) => row,
            _ => panic!("expected shared custom event"),
        }
    }
}
impl Clone for Event {
    #[inline]
    fn clone(&self) -> Self {
        // These exact built-in fields are copied, never user-cloned. The shared
        // branch retains Rc clone/drop behavior for custom IDs and payloads.
        match self {
            Self::Pair(row) => Self::Pair(TupleChange {
                id: row.id,
                value: row.value,
            }),
            Self::Triple(row) => Self::Triple(TupleChange {
                id: row.id,
                value: row.value,
            }),
            Self::Planning(row) => Self::Planning(TupleChange {
                id: row.id,
                value: row.value,
            }),
            Self::Shared(row) => Self::Shared(row.clone()),
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
thread_local! {
    static FORCE_SHARED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
fn with_forced_shared<T>(shared: bool, operation: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            FORCE_SHARED.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(FORCE_SHARED.with(|flag| flag.replace(shared)));
    operation()
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("graph Event")
    }
}
