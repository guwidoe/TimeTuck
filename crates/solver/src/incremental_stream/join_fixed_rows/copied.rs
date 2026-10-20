//! Private consumed-graph payloads. ID representatives and the membership index
//! remain shared in FixedRows; only certified builtin captured values copy.
use std::{any::TypeId, sync::Arc};
pub(in crate::incremental_stream) type Copier<V> = fn(&V) -> V;
type Undo<V> = Vec<(usize, Option<V>)>;
pub(super) struct Capture<V> {
    values: Undo<V>,
}
pub(super) struct Rows<V> {
    values: Vec<Option<V>>,
    copy: Copier<V>,
    frames: Vec<Undo<V>>,
    spare: Vec<Undo<V>>,
}
impl<V> Clone for Rows<V> {
    fn clone(&self) -> Self {
        Self {
            values: self
                .values
                .iter()
                .map(|v| v.as_ref().map(self.copy))
                .collect(),
            copy: self.copy,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
}
impl<V> Rows<V> {
    pub(super) fn new(values: Vec<Option<Arc<V>>>, copy: Copier<V>) -> Self {
        Self {
            values: values.into_iter().map(|v| v.as_deref().map(copy)).collect(),
            copy,
            frames: Vec::new(),
            spare: Vec::new(),
        }
    }
    pub(super) fn len(&self) -> usize {
        self.values.len()
    }
    pub(super) fn get(&self, id: usize) -> Option<&V> {
        self.values.get(id).and_then(Option::as_ref)
    }
    pub(super) fn replace(&mut self, id: usize, value: Option<V>) {
        let old = std::mem::replace(&mut self.values[id], value);
        if let Some(frame) = self.frames.last_mut()
            && !frame.iter().any(|(old, _)| *old == id)
        {
            frame.push((id, old));
        }
    }
    pub(super) fn clear(&mut self) {
        for i in 0..self.values.len() {
            self.replace(i, None);
        }
    }
    pub(super) fn begin(&mut self) -> usize {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        token
    }
    pub(super) fn commit(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "bound join journal order");
        let mut frame = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            for (id, value) in frame.drain(..) {
                if !parent.iter().any(|(old, _)| *old == id) {
                    parent.push((id, value));
                }
            }
        }
        frame.clear();
        self.spare.push(frame);
    }
    pub(super) fn rollback(&mut self, token: usize) {
        assert_eq!(self.frames.len(), token + 1, "bound join journal order");
        let mut frame = self.frames.pop().unwrap();
        for (id, value) in frame.drain(..).rev() {
            self.values[id] = value;
        }
        self.spare.push(frame);
    }
    pub(super) fn capture(&self, token: usize) -> Capture<V> {
        assert!(token < self.frames.len(), "active fixed row capture token");
        let mut ids: Vec<_> = self.frames[token..]
            .iter()
            .flatten()
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Capture {
            values: ids
                .into_iter()
                .map(|id| (id, self.values[id].as_ref().map(self.copy)))
                .collect(),
        }
    }
    pub(super) fn install_capture(&mut self, capture: Capture<V>) {
        assert!(
            capture.values.iter().all(|(id, _)| *id < self.values.len()),
            "fixed row capture index"
        );
        for (id, value) in capture.values {
            self.replace(id, value);
        }
    }
    pub(super) fn install(&mut self, saved: Self) {
        assert_eq!(self.values.len(), saved.values.len(), "bound join universe");
        for (id, value) in saved.values.into_iter().enumerate() {
            self.replace(id, value);
        }
    }
}
fn primitive<T: 'static>() -> bool {
    let id = TypeId::of::<T>();
    [
        TypeId::of::<usize>(),
        TypeId::of::<(usize, usize)>(),
        TypeId::of::<(usize, usize, usize)>(),
        TypeId::of::<((usize, usize), usize)>(),
        TypeId::of::<(usize, (usize, usize))>(),
        TypeId::of::<((usize, usize), (usize, usize))>(),
        TypeId::of::<(usize, usize, usize, usize)>(),
    ]
    .contains(&id)
}
fn payload<T: 'static>() -> bool {
    type U2 = (usize, usize);
    type U3 = (usize, usize, usize);
    type U4 = (usize, usize, usize, usize);
    let id = TypeId::of::<T>();
    [
        TypeId::of::<()>(),
        TypeId::of::<bool>(),
        TypeId::of::<usize>(),
        TypeId::of::<i64>(),
        TypeId::of::<Option<usize>>(),
        TypeId::of::<(usize, Option<usize>)>(),
        TypeId::of::<(usize, i64)>(),
        TypeId::of::<U2>(),
        TypeId::of::<U3>(),
        TypeId::of::<U4>(),
        TypeId::of::<(U2, U2)>(),
        TypeId::of::<(U3, U3)>(),
        TypeId::of::<(U4, U4)>(),
    ]
    .contains(&id)
}
pub(in crate::incremental_stream) fn certify<K: Clone + 'static, T: Clone + 'static>()
-> Option<Copier<(K, T)>> {
    // Only exact builtin identities have unobservable Clone/Drop behavior.
    // NoDrop user structs, including manually implemented Copy, do not qualify.
    (primitive::<K>() && payload::<T>()).then_some(Clone::clone)
}
