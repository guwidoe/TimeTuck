//! Planning rows owned only by a consumed basic-search graph. Exact builtin
//! values and their history copy by value, never through Arc promotion. The
//! consuming entrypoint owns the whole graph and prevents its sources/captures
//! from escaping. Public directors and ordinary source nodes never opt in.
use super::*;
use std::{any::TypeId, cell::OnceCell};
type Copier<T> = fn(&T) -> T;
type Undo<T> = Vec<(usize, Option<T>)>;
struct Token(usize);
struct Full<T>(Vec<Option<T>>);
struct Current<T> {
    count: usize,
    values: Undo<T>,
}

fn copier<T: Clone + 'static>() -> Option<Copier<T>> {
    type U2 = (usize, usize);
    type U3 = (usize, usize, usize);
    type U4 = (usize, usize, usize, usize);
    let kind = TypeId::of::<T>();
    let exact = [
        TypeId::of::<usize>(),
        TypeId::of::<bool>(),
        TypeId::of::<i64>(),
        TypeId::of::<Option<usize>>(),
        TypeId::of::<U2>(),
        TypeId::of::<U3>(),
        TypeId::of::<U4>(),
        TypeId::of::<(U2, U2)>(),
        TypeId::of::<(U3, U3)>(),
        TypeId::of::<(U4, U4)>(),
        TypeId::of::<(usize, i64)>(),
        TypeId::of::<(usize, Option<usize>)>(),
    ]
    .contains(&kind);
    // Exact TypeId establishes the builtin Clone; NoDrop alone would permit
    // observable custom Clone implementations and is deliberately insufficient.
    exact.then_some(Clone::clone)
}

pub(super) fn owned_source<
    I: Ord + Clone + 'static,
    T: Clone + 'static,
    S: AdditiveScore + 'static,
>(
    rows: &SnapshotMap<I, T>,
    count: usize,
) -> Option<Box<dyn Node<S>>> {
    if TypeId::of::<I>() != TypeId::of::<usize>() || !rows.is_empty() {
        return None;
    }
    Some(Box::new(Source::new(count, copier::<T>()?)))
}

struct Source<T> {
    values: Vec<Option<T>>,
    copy: Copier<T>,
    len: usize,
    rows: OnceCell<SnapshotMap<usize, T>>,
    frames: Vec<Undo<T>>,
    spare: Vec<Undo<T>>,
    checkpoint_box: CheckpointBox<Token>,
    latest: Vec<(usize, usize)>,
}
impl<T> Source<T> {
    fn new(count: usize, copy: Copier<T>) -> Self {
        Self {
            values: (0..count).map(|_| None).collect(),
            copy,
            len: 0,
            rows: OnceCell::new(),
            frames: Vec::new(),
            spare: Vec::new(),
            checkpoint_box: CheckpointBox::new(),
            latest: Vec::new(),
        }
    }
    fn replace(&mut self, id: usize, value: Option<T>) {
        self.rows.take();
        let old = std::mem::replace(&mut self.values[id], value);
        match (old.is_some(), self.values[id].is_some()) {
            (false, true) => self.len += 1,
            (true, false) => self.len -= 1,
            _ => {}
        }
        if let Some(frame) = self.frames.last_mut() {
            frame.push((id, old));
        }
    }
    fn rows(&self) -> &SnapshotMap<usize, T> {
        // Used only by cold constructor validation/bound discovery in the
        // consumed graph. Neither snapshots nor candidate captures call this.
        self.rows.get_or_init(|| {
            SnapshotMap::from_sorted(
                self.values
                    .iter()
                    .enumerate()
                    .filter_map(|(id, value)| value.as_ref().map(|value| (id, (self.copy)(value))))
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        })
    }
    fn copy_values(&self) -> Vec<Option<T>> {
        self.values
            .iter()
            .map(|v| v.as_ref().map(self.copy))
            .collect()
    }
    fn restore_frame(&mut self, token: usize) {
        assert_eq!(
            self.frames.len(),
            token + 1,
            "consuming source journal order"
        );
        self.rows.take();
        let mut undo = self.frames.pop().unwrap();
        for (id, value) in undo.drain(..).rev() {
            let current = self.values[id].take();
            match (current.is_some(), value.is_some()) {
                (false, true) => self.len += 1,
                (true, false) => self.len -= 1,
                _ => {}
            }
            self.values[id] = value;
        }
        self.spare.push(undo);
    }
}
impl<T: Clone + 'static> Source<T> {
    fn update(&mut self, event: &Event) -> Result<(), GraphError> {
        let change = typed::<usize, T>(event);
        if change.id >= self.values.len() {
            return Err(GraphError::Dependency(
                "bound planning source ID outside its validated universe".into(),
            ));
        }
        // Same delivered-value Clone phase as both ordinary source backends.
        // The strict factory gate makes this copy incapable of user hooks.
        let value = change.value.clone();
        self.replace(change.id, value);
        Ok(())
    }
}
impl<T: Clone + 'static, S: AdditiveScore> Node<S> for Source<T> {
    fn source_count(&self) -> Option<usize> {
        Some(self.len)
    }
    fn source_rows(&self) -> Option<&dyn Any> {
        Some(self.rows())
    }
    fn source_events(&self) -> Vec<Event> {
        erase(self.values.iter().enumerate().filter_map(|(id, value)| {
            value
                .as_ref()
                .map(|value| TupleChange::put(id, value.clone()))
        }))
    }
    fn initialize_sorted_source(&mut self, events: &[(usize, Event)]) -> bool {
        if self.len != 0
            || events.iter().any(|(_, event)| {
                let row = typed::<usize, T>(event);
                row.value.is_none() || row.id >= self.values.len()
            })
            || events
                .windows(2)
                .any(|pair| typed::<usize, T>(&pair[0].1).id >= typed::<usize, T>(&pair[1].1).id)
        {
            return false;
        }
        // Complete all source clones before publishing the initialized rows,
        // matching SourceNode's sorted map preparation and installation phase.
        let values: Vec<_> = events
            .iter()
            .map(|(_, event)| {
                let row = typed::<usize, T>(event);
                (row.id, row.value.as_ref().unwrap().clone())
            })
            .collect();
        for (id, value) in values {
            self.replace(id, Some(value));
        }
        true
    }
    fn apply_source(&mut self, event: &Event) -> Result<(), GraphError> {
        self.update(event)
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        self.update(event)?;
        Ok(Outputs::one(event.clone()))
    }
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        _: NodeArithmetic,
    ) -> Result<BatchOutputs, GraphError> {
        self.latest.clear();
        self.latest.extend(
            inputs
                .iter()
                .enumerate()
                .map(|(position, (_, event))| (typed::<usize, T>(event).id, position)),
        );
        self.latest.sort_unstable();
        self.latest.dedup_by(|later, earlier| {
            if later.0 == earlier.0 {
                std::mem::swap(later, earlier);
                true
            } else {
                false
            }
        });
        let mut out = Vec::with_capacity(self.latest.len());
        for position in 0..self.latest.len() {
            let event = &inputs[self.latest[position].1].1;
            self.update(event)?;
            out.push(event.clone());
        }
        Ok(out)
    }
    fn clear(&mut self) {
        for id in 0..self.values.len() {
            self.replace(id, None);
        }
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(Full(self.copy_values()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        let token = self.frames.len();
        self.frames.push(self.spare.pop().unwrap_or_default());
        self.checkpoint_box.begin(Token(token))
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let (token, empty) =
            CheckpointBox::<Token>::unpack(saved).expect("consuming source journal");
        assert_eq!(
            self.frames.len(),
            token.0 + 1,
            "consuming source journal order"
        );
        let mut undo = self.frames.pop().unwrap();
        if let Some(parent) = self.frames.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut undo)
            } else {
                parent.append(&mut undo)
            }
        }
        undo.clear();
        self.spare.push(undo);
        self.checkpoint_box.recycle(empty);
    }
    fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any> {
        let Some(token) = CheckpointBox::<Token>::get(original) else {
            return <Self as Node<S>>::checkpoint(self);
        };
        assert!(
            token.0 < self.frames.len(),
            "active consuming source journal token"
        );
        let mut ids: Vec<_> = self.frames[token.0..]
            .iter()
            .flat_map(|f| f.iter().map(|(id, _)| *id))
            .collect();
        ids.sort_unstable();
        ids.dedup();
        Box::new(Current {
            count: self.values.len(),
            values: ids
                .into_iter()
                .map(|id| (id, self.values[id].as_ref().map(self.copy)))
                .collect(),
        })
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        let saved = match CheckpointBox::<Token>::unpack(saved) {
            Ok((token, empty)) => {
                self.restore_frame(token.0);
                self.checkpoint_box.recycle(empty);
                return;
            }
            Err(saved) => saved,
        };
        let saved = match saved.downcast::<Current<T>>() {
            Ok(current) => {
                assert_eq!(
                    self.values.len(),
                    current.count,
                    "consuming source capture universe"
                );
                assert!(
                    current.values.iter().all(|(id, _)| *id < self.values.len()),
                    "consuming source capture index"
                );
                for (id, value) in current.values {
                    self.replace(id, value);
                }
                return;
            }
            Err(saved) => saved,
        };
        let Full(values) = *saved
            .downcast::<Full<T>>()
            .expect("consuming source snapshot");
        assert_eq!(
            values.len(),
            self.values.len(),
            "consuming source snapshot universe"
        );
        for (id, value) in values.into_iter().enumerate() {
            self.replace(id, value);
        }
    }
}

#[cfg(test)]
mod tests;
