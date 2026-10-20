//! Private bounded row storage for directly bound mixed planning inputs only.
//! Default keys and payloads retain their original Arc identity. Whole-graph
//! consuming searches may explicitly copy certified builtin payloads by value;
//! canonical ID keys and the index remain shared. Each full snapshot copies at
//! most4096 slots; trial frames retain only the first original per slot.
//! Nested commits merge originals, so repeated writes do not retain intermediates.
//! Generic/unbounded inputs remain maps. Mutable bound join membership uses bitsets.
//! Undo lookup/merge is linear in first-touched slots (at most4096 per frame),
//! avoiding an additional marks array or unbounded retention of intermediates.
use std::sync::Arc;
pub(super) mod copied;
#[cfg(test)]
mod copied_tests;
enum Payloads<V> {
    Shared,
    Copied(Box<copied::Rows<V>>),
}
impl<V> Clone for Payloads<V> {
    fn clone(&self) -> Self {
        match self {
            Self::Shared => Self::Shared,
            Self::Copied(rows) => Self::Copied(rows.clone()),
        }
    }
}

type Undo<V> = Vec<(usize, Option<Arc<V>>)>;
pub(super) struct Capture<V> {
    count: usize,
    values: Undo<V>,
    copied: Option<copied::Capture<V>>,
}
pub(super) struct FixedRows<I, V> {
    keys: Vec<Arc<I>>,
    values: Vec<Option<Arc<V>>>,
    payloads: Payloads<V>,
    index: fn(&I) -> usize,
    frames: Vec<Undo<V>>,
    spare: Vec<Undo<V>>,
}
impl<I, V> Clone for FixedRows<I, V> {
    fn clone(&self) -> Self {
        Self {
            keys: self.keys.clone(),
            values: self.values.clone(),
            payloads: self.payloads.clone(),
            index: self.index,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
}
impl<I, V> FixedRows<I, V> {
    pub(super) fn new(rows: Vec<(Arc<I>, Arc<V>)>, index: fn(&I) -> usize) -> Self {
        let (keys, values) = rows.into_iter().map(|(k, v)| (k, Some(v))).unzip();
        Self {
            keys,
            values,
            payloads: Payloads::Shared,
            index,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
    pub(super) fn enable_copied(&mut self, copy: copied::Copier<V>) {
        assert!(self.frames.is_empty(), "copied rows before journals");
        assert!(
            matches!(self.payloads, Payloads::Shared),
            "immutable copied row binding"
        );
        self.payloads = Payloads::Copied(Box::new(copied::Rows::new(
            std::mem::take(&mut self.values),
            copy,
        )));
    }
    fn count(&self) -> usize {
        match &self.payloads {
            Payloads::Shared => self.values.len(),
            Payloads::Copied(rows) => rows.len(),
        }
    }
    fn at(&self, index: usize) -> Option<&V> {
        match &self.payloads {
            Payloads::Shared => self.values.get(index).and_then(Option::as_deref),
            Payloads::Copied(rows) => rows.get(index),
        }
    }
    pub(super) fn get(&self, id: &I) -> Option<&V> {
        self.at((self.index)(id))
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&I, &V)> {
        self.keys
            .iter()
            .enumerate()
            .filter_map(|(index, k)| self.at(index).map(|v| (k.as_ref(), v)))
    }
    fn replace(&mut self, index: usize, value: Option<Arc<V>>) {
        let old = std::mem::replace(&mut self.values[index], value);
        if let Some(frame) = self.frames.last_mut()
            && !frame.iter().any(|(id, _)| *id == index)
        {
            frame.push((index, old));
        }
    }
    pub(super) fn validate_insert(&self, id: &I) -> usize {
        let index = (self.index)(id);
        assert!(index < self.count(), "validated bound join row ID");
        index
    }
    pub(super) fn insert(&mut self, id: I, value: V) {
        let index = self.validate_insert(&id);
        // Match map replacement: discard the incoming representative before
        // replacing the stored payload; the original key owner stays retained.
        drop(id);
        if let Payloads::Copied(rows) = &mut self.payloads {
            rows.replace(index, Some(value));
        } else {
            self.replace(index, Some(Arc::new(value)));
        }
    }
    pub(super) fn remove(&mut self, id: &I) {
        let index = (self.index)(id);
        if index < self.count() {
            if let Payloads::Copied(rows) = &mut self.payloads {
                rows.replace(index, None);
            } else {
                self.replace(index, None);
            }
        }
    }
    pub(super) fn clear(&mut self) {
        if let Payloads::Copied(rows) = &mut self.payloads {
            rows.clear();
            return;
        }
        for index in 0..self.values.len() {
            self.replace(index, None);
        }
    }
    pub(super) fn begin(&mut self) -> usize {
        if let Payloads::Copied(rows) = &mut self.payloads {
            return rows.begin();
        }
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(super) fn capture_current(&self, token: usize) -> Capture<V> {
        if let Payloads::Copied(rows) = &self.payloads {
            return Capture {
                count: self.count(),
                values: Vec::new(),
                copied: Some(rows.capture(token)),
            };
        }
        assert!(token < self.frames.len(), "active fixed row capture token");
        let mut slots: Vec<_> = self.frames[token..]
            .iter()
            .flatten()
            .map(|(slot, _)| *slot)
            .collect();
        slots.sort_unstable();
        slots.dedup();
        Capture {
            count: self.values.len(),
            copied: None,
            values: slots
                .into_iter()
                .map(|slot| (slot, self.values[slot].clone()))
                .collect(),
        }
    }
    // The capture is relative to its journal's original base. Keys remain the
    // fixed universe's exact owners, while ordinary replacements log rollback.
    pub(super) fn install_capture(&mut self, saved: Capture<V>) {
        assert_eq!(self.count(), saved.count, "fixed row capture universe");
        if let Some(captured) = saved.copied {
            let Payloads::Copied(rows) = &mut self.payloads else {
                panic!("fixed row capture binding");
            };
            rows.install_capture(captured);
            return;
        }
        assert!(
            matches!(self.payloads, Payloads::Shared),
            "fixed row capture binding"
        );
        for (slot, value) in saved.values {
            self.replace(slot, value);
        }
    }
    pub(super) fn rollback(&mut self, token: usize) {
        if let Payloads::Copied(rows) = &mut self.payloads {
            rows.rollback(token);
            return;
        }
        assert_eq!(self.frames.len(), token + 1, "bound join journal order");
        let mut frame = self.frames.pop().unwrap();
        for (index, value) in frame.drain(..).rev() {
            self.values[index] = value;
        }
        self.spare.push(frame);
    }
    pub(super) fn commit(&mut self, token: usize) {
        if let Payloads::Copied(rows) = &mut self.payloads {
            rows.commit(token);
            return;
        }
        assert_eq!(self.frames.len(), token + 1, "bound join journal order");
        let mut frame = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            for (index, value) in frame.drain(..) {
                if !parent.iter().any(|(id, _)| *id == index) {
                    parent.push((index, value));
                }
            }
        }
        frame.clear();
        self.spare.push(frame);
    }
    pub(super) fn install(&mut self, saved: Self) {
        assert_eq!(self.count(), saved.count(), "bound join universe");
        match (&mut self.payloads, saved.payloads) {
            (Payloads::Copied(rows), Payloads::Copied(old)) => {
                rows.install(*old);
                return;
            }
            (Payloads::Shared, Payloads::Shared) => {}
            _ => panic!("immutable fixed row payload binding"),
        }
        for (index, value) in saved.values.into_iter().enumerate() {
            self.replace(index, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_originals_merge_nested_frames_without_retaining_intermediates() {
        let key = Arc::new(0usize);
        let original = Arc::new(Arc::new(0));
        let mut rows = FixedRows::new(vec![(key.clone(), original.clone())], |id| *id);
        let outer = rows.begin();
        rows.insert(0, Arc::new(1));
        let middle = Arc::downgrade(rows.get(&0).unwrap());
        let inner = rows.begin();
        rows.insert(0, Arc::new(2));
        rows.commit(inner);
        assert!(middle.upgrade().is_none());
        assert_eq!(rows.frames[0].len(), 1);
        let saved = rows.clone();
        rows.clear();
        rows.insert(0, Arc::new(3));
        rows.install(saved.clone());
        assert_eq!(**rows.get(&0).unwrap(), 2);
        rows.rollback(outer);
        assert!(std::ptr::eq(rows.get(&0).unwrap(), original.as_ref()));
        assert!(std::ptr::eq(rows.iter().next().unwrap().0, key.as_ref()));
        assert_eq!(**saved.get(&0).unwrap(), 2);
        // A child first-touch must become the parent's original on commit.
        let outer = rows.begin();
        let inner = rows.begin();
        rows.remove(&0);
        rows.commit(inner);
        rows.rollback(outer);
        assert!(std::ptr::eq(rows.get(&0).unwrap(), original.as_ref()));
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    #[test]
    fn fixed_capture_saves_current_unique_touches_without_cloning_values_or_the_universe() {
        struct NoClone(i64);
        let keys: Vec<_> = (0..4).map(Arc::new).collect();
        let original: Vec<_> = (0..4).map(|n| Arc::new(NoClone(n))).collect();
        let mut rows = FixedRows::new(
            keys.iter().cloned().zip(original.iter().cloned()).collect(),
            |id| *id,
        );
        let outer = rows.begin();
        rows.insert(0, NoClone(10));
        rows.insert(0, NoClone(11));
        rows.remove(&2);
        let inner = rows.begin();
        rows.insert(3, NoClone(13));
        let saved = rows.capture_current(outer);
        let nested = rows.capture_current(outer);
        assert_eq!(saved.values.len(), 3);
        assert_eq!(
            Arc::strong_count(&keys[1]),
            2,
            "untouched key universe is not copied"
        );
        assert_eq!(
            Arc::strong_count(&original[1]),
            2,
            "untouched payload owner is not copied"
        );
        let new = rows.get(&0).unwrap() as *const _;
        rows.rollback(inner);
        rows.rollback(outer);
        let other = rows.begin();
        rows.clear();
        rows.rollback(other);
        let outer = rows.begin();
        rows.install_capture(nested);
        assert_eq!(rows.get(&0).unwrap().0, 11);
        assert!(rows.get(&2).is_none());
        assert_eq!(rows.get(&3).unwrap().0, 13);
        assert_eq!(rows.get(&0).unwrap() as *const _, new);
        rows.rollback(outer);
        assert!(std::ptr::eq(rows.get(&0).unwrap(), original[0].as_ref()));
        assert!(std::ptr::eq(
            rows.iter().next().unwrap().0,
            keys[0].as_ref()
        ));
        rows.install_capture(saved);
        assert_eq!(rows.get(&0).unwrap().0, 11);
        assert!(rows.get(&2).is_none());
        assert_eq!(rows.get(&3).unwrap().0, 13);
    }
}
