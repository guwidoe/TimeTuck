//! Bounded allocation reuse after an owner has logically retired. Shared owners
//! and all weak observers retain ordinary Arc destruction. No user Clone runs.
use super::{Entry, Undo};
use std::{alloc::Layout, mem, sync::Arc};

const MAX_ITEMS: usize = 64;
const MAX_BYTES: usize = 32 * 1024;
pub(crate) struct Pool<T> {
    slots: Vec<Arc<T>>,
    reset: Option<fn(&mut T)>,
}
impl<T> Pool<T> {
    pub(crate) fn new() -> Self {
        Self {
            slots: Vec::new(),
            reset: None,
        }
    }
    pub(crate) fn empty_clone(&self) -> Self {
        Self {
            slots: Vec::new(),
            reset: self.reset,
        }
    }
    fn limit() -> usize {
        // Account for the strong/weak counters, data alignment, and stored Arc
        // handle. Overaligned or large values cannot grow beyond this budget.
        let Ok((layout, _)) = Layout::new::<[usize; 2]>().extend(Layout::new::<T>()) else {
            return 0;
        };
        let Some(bytes) = layout
            .pad_to_align()
            .size()
            .checked_add(mem::size_of::<Arc<T>>())
        else {
            return 0;
        };
        MAX_ITEMS.min(MAX_BYTES / bytes.max(1))
    }
    pub(crate) fn take(&mut self, value: T) -> Arc<T> {
        match self.slots.pop() {
            Some(mut owner) => {
                *Arc::get_mut(&mut owner).expect("private retired Arc owner") = value;
                owner
            }
            None => Arc::new(value),
        }
    }
    pub(crate) fn retire(&mut self, mut owner: Arc<T>) {
        if (mem::needs_drop::<T>() && self.reset.is_none())
            || self.slots.len() >= Self::limit()
            || Arc::strong_count(&owner) != 1
            || Arc::weak_count(&owner) != 0
        {
            return;
        }
        if let Some(reset) = self.reset {
            // Only the typed library-collector opt-in supplies this reset. It
            // releases private tree resources at the original drop point, so
            // the pool retains the blank outer allocation only.
            reset(Arc::get_mut(&mut owner).expect("unique retired Arc owner"));
        }
        if self.slots.capacity() == 0 {
            self.slots.reserve_exact(Self::limit());
        }
        self.slots.push(owner);
    }
}

pub(super) struct Pools<K, V> {
    keys: Pool<K>,
    values: Pool<V>,
}
impl<K, V> Pools<K, V> {
    pub(super) fn new() -> Self {
        Self {
            keys: Pool::new(),
            values: Pool::new(),
        }
    }
    pub(super) fn empty_clone(&self) -> Self {
        Self {
            keys: self.keys.empty_clone(),
            values: self.values.empty_clone(),
        }
    }
    fn enabled(&self) -> bool {
        !mem::needs_drop::<V>() || self.values.reset.is_some()
    }
    pub(super) fn enable_value_reset(&mut self, reset: fn(&mut V)) {
        self.values.reset = Some(reset);
    }
    pub(super) fn key(&mut self, key: K) -> Arc<K> {
        if self.enabled() {
            self.keys.take(key)
        } else {
            Arc::new(key)
        }
    }
    pub(super) fn value(&mut self, value: V) -> Arc<V> {
        if self.enabled() {
            self.values.take(value)
        } else {
            Arc::new(value)
        }
    }
    pub(super) fn retire_value(&mut self, value: Arc<V>) {
        if self.enabled() {
            self.values.retire(value);
        }
    }
    pub(super) fn retire_entry(&mut self, entry: Option<Entry<K, V>>) {
        if let Some(Entry { key, value }) = entry {
            // Preserve the original field release order. Weak observers reject
            // pooling independently for each actual key/payload allocation.
            if self.enabled() {
                self.keys.retire(key);
            } else {
                drop(key);
            }
            self.retire_value(value);
        }
    }
    pub(super) fn retire_undo(&mut self, undo: Undo<K, V>) {
        match undo {
            Undo::Value(_, value) => self.retire_value(value),
            Undo::Entry(_, entry) => self.retire_entry(entry),
        }
    }
}

#[cfg(test)]
mod tests;
