//! Bounded, private mixed-planning source storage. Ordinary sources and planning
//! sources above 256 rows retain SnapshotMap. Contiguous IDs are checked once
//! after initial binding; later writes check their universe before mutation.
//!
//! Slots avoid AVL path writes during trial/rollback. Undo records move the old
//! Arc rather than cloning user payloads. Full checkpoints copy at most 256 Arc
//! slots; inspection lazily allocates an ordered tree sharing the original keys
//! and payloads. This bounded checkpoint/inspection cost trades against frequent
//! trial writes. Neither checkpoints nor inspection invoke user Clone or Ord.
use super::*;
use std::{cell::OnceCell, sync::Arc};

pub(super) const MAX_ROWS: usize = 256;
type Slots<T> = Vec<Option<Arc<T>>>;
type Undo<T> = Vec<(usize, Option<Arc<T>>)>;
struct Token(usize);
struct Saved<T>(Slots<T>);
pub(super) struct FixedSourceNode<T> {
    keys: Vec<Arc<usize>>,
    values: Slots<T>,
    rows: OnceCell<SnapshotMap<usize, T>>,
    journals: Vec<Undo<T>>,
    spare: Vec<Undo<T>>,
}
impl<T> FixedSourceNode<T> {
    pub(super) fn new(rows: Vec<(Arc<usize>, Arc<T>)>) -> Self {
        let (keys, values) = rows.into_iter().map(|(k, v)| (k, Some(v))).unzip();
        Self {
            keys,
            values,
            rows: OnceCell::new(),
            journals: Vec::new(),
            spare: Vec::new(),
        }
    }
    fn replace(&mut self, index: usize, value: Option<Arc<T>>) {
        self.rows.take();
        let old = std::mem::replace(&mut self.values[index], value);
        if let Some(journal) = self.journals.last_mut() {
            journal.push((index, old));
        }
    }
    fn update(&mut self, event: &Event) -> Result<(), GraphError>
    where
        T: Clone + 'static,
    {
        let change = typed::<usize, T>(event);
        if change.id >= self.values.len() {
            return Err(GraphError::Dependency(
                "bound planning source ID outside its validated universe".into(),
            ));
        }
        // Clone before touching either cache or storage: a user Clone panic is
        // restored by the enclosing transaction, just as for the ordinary map.
        let value = change.value.as_ref().map(|v| Arc::new(v.clone()));
        self.replace(change.id, value);
        Ok(())
    }
    fn rows(&self) -> &SnapshotMap<usize, T> {
        self.rows.get_or_init(|| {
            SnapshotMap::from_shared_sorted(
                self.keys
                    .iter()
                    .zip(&self.values)
                    .filter_map(|(k, v)| v.as_ref().map(|v| (k.clone(), v.clone())))
                    .collect(),
            )
        })
    }
}
impl<T: Clone + 'static, S: AdditiveScore> Node<S> for FixedSourceNode<T> {
    fn apply_batch(
        &mut self,
        inputs: &[(bool, Event)],
        arithmetic: NodeArithmetic,
    ) -> Result<Outputs, GraphError> {
        let mut latest = BTreeMap::new();
        for (_, event) in inputs {
            latest.insert(typed::<usize, T>(event).id, event.clone());
        }
        let mut out = Vec::new();
        for event in latest.into_values() {
            out.extend(<Self as Node<S>>::apply(self, false, &event, arithmetic)?);
        }
        Ok(out)
    }
    fn apply_source(&mut self, event: &Event) -> Result<(), GraphError> {
        self.update(event)
    }
    fn source_count(&self) -> Option<usize> {
        Some(self.values.iter().filter(|v| v.is_some()).count())
    }
    fn clear(&mut self) {
        for i in 0..self.values.len() {
            self.replace(i, None);
        }
    }
    fn source_events(&self) -> Vec<Event> {
        erase(
            self.keys
                .iter()
                .zip(&self.values)
                .filter_map(|(key, value)| {
                    value
                        .as_ref()
                        .map(|value| TupleChange::put(**key, value.as_ref().clone()))
                }),
        )
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(Saved(self.values.clone()))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        let token = self.journals.len();
        self.journals.push(self.spare.pop().unwrap_or_default());
        Box::new(Token(token))
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let token = saved
            .downcast::<Token>()
            .expect("fixed source journal token");
        assert_eq!(
            self.journals.len(),
            token.0 + 1,
            "fixed source journal order"
        );
        let mut undo = self.journals.pop().unwrap();
        if let Some(parent) = self.journals.last_mut() {
            if parent.is_empty() {
                std::mem::swap(parent, &mut undo);
            } else {
                parent.append(&mut undo);
            }
        }
        undo.clear();
        self.spare.push(undo);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<Token>() {
            Ok(token) => {
                assert_eq!(
                    self.journals.len(),
                    token.0 + 1,
                    "fixed source journal order"
                );
                self.rows.take();
                let mut undo = self.journals.pop().unwrap();
                for (index, value) in undo.drain(..).rev() {
                    self.values[index] = value;
                }
                self.spare.push(undo);
            }
            Err(saved) => {
                let Saved(values) = *saved.downcast::<Saved<T>>().expect("fixed source snapshot");
                assert_eq!(
                    values.len(),
                    self.values.len(),
                    "fixed source snapshot universe"
                );
                for (index, value) in values.into_iter().enumerate() {
                    self.replace(index, value);
                }
            }
        }
    }
    fn source_rows(&self) -> Option<&dyn Any> {
        Some(self.rows())
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        self.update(event)?;
        Ok(vec![event.clone()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SimpleScore;
    use std::cell::Cell;

    struct Payload(Rc<Cell<usize>>, usize);
    impl Clone for Payload {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone(), self.1)
        }
    }
    #[test]
    fn upgrade_retains_keys_payloads_and_checks_the_entire_universe() {
        let clones = Rc::new(Cell::new(0));
        let mut rows = SnapshotMap::new();
        for i in 0..3 {
            rows.insert(i, Payload(clones.clone(), i));
        }
        let original = rows.shared_entries();
        let source = SourceNode(rows);
        assert!(
            <SourceNode<usize, Payload> as Node<SimpleScore>>::bind_fixed_planning_source(
                &source, 2
            )
            .is_none()
        );
        let fixed = <SourceNode<usize, Payload> as Node<SimpleScore>>::bind_fixed_planning_source(
            &source, 3,
        )
        .unwrap();
        let view = fixed
            .source_rows()
            .unwrap()
            .downcast_ref::<SnapshotMap<usize, Payload>>()
            .unwrap();
        for ((k, v), (old_k, old_v)) in view.iter().zip(&original) {
            assert!(std::ptr::eq(k, old_k.as_ref()));
            assert!(std::ptr::eq(v, old_v.as_ref()));
        }
        assert_eq!(clones.get(), 0);
        let mut rows = SnapshotMap::new();
        rows.insert(0, 0);
        rows.insert(2, 2);
        assert!(
            <SourceNode<usize, usize> as Node<SimpleScore>>::bind_fixed_planning_source(
                &SourceNode(rows),
                2
            )
            .is_none()
        );
    }
    #[test]
    // The storage must also support single-threaded user payloads behind Arc.
    #[allow(clippy::arc_with_non_send_sync)]
    fn nested_installs_clear_and_failed_ids_preserve_outer_rollback() {
        let clones = Rc::new(Cell::new(0));
        let value = Arc::new(Payload(clones.clone(), 10));
        let key = Arc::new(0);
        let mut node: Box<dyn Node<SimpleScore>> =
            Box::new(FixedSourceNode::new(vec![(key.clone(), value.clone())]));
        let outer = node.begin_checkpoint();
        let saved = node.checkpoint();
        let change: Event = Rc::new(TupleChange::put(0usize, Payload(clones.clone(), 20)));
        node.apply_source(&change).unwrap();
        let inner = node.begin_checkpoint();
        node.clear();
        node.restore(saved);
        let view = node
            .source_rows()
            .unwrap()
            .downcast_ref::<SnapshotMap<usize, Payload>>()
            .unwrap()
            .clone();
        assert!(std::ptr::eq(&view[&0], value.as_ref()));
        let invalid: Event = Rc::new(TupleChange::put(1usize, Payload(clones.clone(), 99)));
        assert!(matches!(
            node.apply_source(&invalid),
            Err(GraphError::Dependency(_))
        ));
        assert_eq!(clones.get(), 1);
        node.commit_checkpoint(inner);
        node.restore(outer);
        let view_after = node
            .source_rows()
            .unwrap()
            .downcast_ref::<SnapshotMap<usize, Payload>>()
            .unwrap();
        assert!(std::ptr::eq(&view_after[&0], value.as_ref()));
        assert!(std::ptr::eq(
            view_after.iter().next().unwrap().0,
            key.as_ref()
        ));
        assert_eq!(view[&0].1, 10);
        assert_eq!(clones.get(), 1);
    }
}
