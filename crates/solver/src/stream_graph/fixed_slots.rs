//! Dense rows for an immutable identity universe; move-based nested undo.
use std::rc::Rc;
pub(super) type SlotSnapshot<T> = Vec<Option<Rc<T>>>;
pub(super) struct Slots<T> {
    values: SlotSnapshot<T>,
    undo: Vec<(usize, Option<Rc<T>>)>,
    frames: Vec<usize>,
}
impl<T> Slots<T> {
    pub(super) fn new(len: usize) -> Self {
        Self {
            values: (0..len).map(|_| None).collect(),
            undo: Vec::new(),
            frames: Vec::new(),
        }
    }
    pub(super) fn get(&self, index: usize) -> Option<&T> {
        self.values[index].as_deref()
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = Option<&T>> {
        self.values.iter().map(Option::as_deref)
    }
    pub(super) fn set(&mut self, index: usize, value: Option<T>) {
        self.replace(index, value.map(Rc::new));
    }
    // Explicit shared-input adapters retain the producer's immutable row owner.
    // Ordinary set above keeps its allocation and ownership behavior unchanged.
    pub(super) fn set_shared(&mut self, index: usize, value: Option<Rc<T>>) {
        self.replace(index, value);
    }
    fn replace(&mut self, index: usize, value: Option<Rc<T>>) {
        let old = std::mem::replace(&mut self.values[index], value);
        if !self.frames.is_empty() {
            self.undo.push((index, old));
        }
    }
    pub(super) fn clear(&mut self) {
        for index in 0..self.values.len() {
            if self.values[index].is_some() {
                self.replace(index, None);
            }
        }
    }
    pub(super) fn snapshot(&self) -> SlotSnapshot<T> {
        self.values.clone()
    }
    pub(super) fn install(&mut self, snapshot: SlotSnapshot<T>) {
        assert_eq!(snapshot.len(), self.values.len(), "fixed slot universe");
        if self.frames.is_empty() {
            self.values = snapshot;
        } else {
            for (index, value) in snapshot.into_iter().enumerate() {
                let unchanged = match (&self.values[index], &value) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                    _ => false,
                };
                if !unchanged {
                    self.replace(index, value);
                }
            }
        }
    }
    pub(super) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.undo.len());
        token
    }
    pub(super) fn commit(&mut self, token: usize) {
        assert_eq!(token + 1, self.frames.len(), "innermost slot journal");
        self.frames.pop();
        if self.frames.is_empty() {
            self.undo.clear();
        }
    }
    pub(super) fn rollback(&mut self, token: usize) {
        assert_eq!(token + 1, self.frames.len(), "innermost slot journal");
        let start = self.frames.pop().expect("slot journal");
        while self.undo.len() > start {
            let (index, value) = self.undo.pop().expect("slot undo");
            self.values[index] = value;
        }
    }
    pub(super) fn entry_snapshot(&self, token: usize) -> SlotSnapshot<T> {
        let mut values = self.snapshot();
        for (index, value) in self.undo[self.frames[token]..].iter().rev() {
            values[*index] = value.clone();
        }
        values
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_commit_restore_clear_and_outer_undo_need_no_payload_clone() {
        struct Value(i32); // Deliberately not Clone.
        let mut slots = Slots::new(3);
        slots.set(0, Some(Value(1)));
        let original = slots.snapshot();
        let outer = slots.begin();
        slots.set(0, Some(Value(2)));
        let inner = slots.begin();
        slots.set(1, Some(Value(3)));
        slots.commit(inner);
        let held = slots.snapshot();
        slots.clear();
        slots.install(original);
        assert_eq!(slots.get(0).unwrap().0, 1);
        assert!(slots.get(1).is_none());
        let entry = slots.entry_snapshot(outer);
        assert_eq!(entry[0].as_ref().unwrap().0, 1);
        slots.rollback(outer);
        assert_eq!(slots.get(0).unwrap().0, 1);
        assert!(slots.get(1).is_none());
        slots.install(held);
        assert_eq!(slots.get(0).unwrap().0, 2);
        assert_eq!(slots.get(1).unwrap().0, 3);
        let outer = slots.begin();
        let inner = slots.begin();
        slots.set(0, None);
        slots.rollback(inner);
        slots.commit(outer);
        assert!(slots.undo.is_empty());
        assert!(slots.frames.is_empty());
        assert_eq!(slots.get(0).unwrap().0, 2);
    }
}
