//! Bounded, private planning-source storage. Ordinary sources and planning
//! sources above 256 rows retain SnapshotMap. Contiguous IDs are checked once
//! after initial binding; later writes check their universe before mutation.
//!
//! Slots avoid AVL path writes during trial/rollback. Undo records move the old
//! row rather than cloning user payloads. Trial rows stay owned until inspection
//! or a full checkpoint promotes them to shared storage. Full checkpoints copy
//! at most 256 Arc slots; inspection lazily builds an ordered tree sharing keys
//! and payloads. Slots and journals are larger, trading bounded storage against
//! frequent trial allocations. Neither operation invokes user Clone or Ord.
use super::*;
use std::{
    cell::{Cell, OnceCell},
    sync::Arc,
};

pub(super) const MAX_ROWS: usize = 256;
// Trial rows need ownership, but most never escape a rollback. Share lazily on
// inspection/full snapshots, without invoking user Clone or retaining candidates.
struct Row<T> {
    shared: OnceCell<Arc<T>>,
    owned: Cell<Option<T>>,
}
impl<T> Row<T> {
    fn from_shared(value: Arc<T>) -> Self {
        Self {
            shared: OnceCell::from(value),
            owned: Cell::new(None),
        }
    }
    fn owned(value: T) -> Self {
        Self {
            shared: OnceCell::new(),
            owned: Cell::new(Some(value)),
        }
    }
    fn shared(&self) -> &Arc<T> {
        self.shared
            .get_or_init(|| Arc::new(self.owned.take().expect("unshared row")))
    }
}
type Slots<T> = Vec<Option<Row<T>>>;
type Undo<T> = Vec<(usize, Option<Row<T>>)>;
struct Token(usize);
struct Saved<T>(Vec<Option<Arc<T>>>);
pub(super) struct FixedSourceNode<T> {
    keys: Vec<Arc<usize>>,
    values: Slots<T>,
    rows: OnceCell<SnapshotMap<usize, T>>,
    journals: Vec<Undo<T>>,
    spare: Vec<Undo<T>>,
    checkpoint_box: CheckpointBox<Token>,
    // Reusable batch positions only; no event or user payload ownership.
    latest: Vec<(usize, usize)>,
}
impl<T> FixedSourceNode<T> {
    pub(super) fn new(rows: Vec<(Arc<usize>, Arc<T>)>) -> Self {
        let (keys, values) = rows
            .into_iter()
            .map(|(k, v)| (k, Some(Row::from_shared(v))))
            .unzip();
        Self {
            keys,
            values,
            rows: OnceCell::new(),
            journals: Vec::new(),
            spare: Vec::new(),
            checkpoint_box: CheckpointBox::new(),
            latest: Vec::new(),
        }
    }
    fn replace(&mut self, index: usize, value: Option<Row<T>>) {
        self.rows.take();
        let old = std::mem::replace(&mut self.values[index], value);
        if let Some(journal) = self.journals.last_mut() {
            journal.push((index, old));
        }
    }
    pub(super) fn put_owned(&mut self, index: usize, value: T) -> Result<(), GraphError> {
        if index >= self.values.len() {
            return Err(GraphError::Dependency(
                "bound planning source ID outside its validated universe".into(),
            ));
        }
        self.replace(index, Some(Row::owned(value)));
        Ok(())
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
        let value = change.value.as_ref().map(|v| Row::owned(v.clone()));
        self.replace(change.id, value);
        Ok(())
    }
    fn rows(&self) -> &SnapshotMap<usize, T> {
        self.rows.get_or_init(|| {
            SnapshotMap::from_shared_sorted(
                self.keys
                    .iter()
                    .zip(&self.values)
                    .filter_map(|(k, v)| v.as_ref().map(|v| (k.clone(), v.shared().clone())))
                    .collect(),
            )
        })
    }
}
impl<T: Clone + 'static, S: AdditiveScore> Node<S> for FixedSourceNode<T> {
    fn fixed_source_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
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
                .map(|(index, (_, event))| (typed::<usize, T>(event).id, index)),
        );
        // The input position breaks ID ties, preserving the last write without
        // downcasting events in the sorting or deduplication comparisons.
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
                        .map(|value| TupleChange::put(**key, value.shared().as_ref().clone()))
                }),
        )
    }
    fn checkpoint(&self) -> Box<dyn Any> {
        Box::new(Saved(
            self.values
                .iter()
                .map(|v| v.as_ref().map(|v| v.shared().clone()))
                .collect(),
        ))
    }
    fn begin_checkpoint(&mut self) -> Box<dyn Any> {
        let token = self.journals.len();
        self.journals.push(self.spare.pop().unwrap_or_default());
        self.checkpoint_box.begin(Token(token))
    }
    fn commit_checkpoint(&mut self, saved: Box<dyn Any>) {
        let (token, empty) =
            CheckpointBox::<Token>::unpack(saved).expect("fixed source journal token");
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
        self.checkpoint_box.recycle(empty);
    }
    fn restore(&mut self, saved: Box<dyn Any>) {
        match CheckpointBox::<Token>::unpack(saved) {
            Ok((token, empty)) => {
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
                self.checkpoint_box.recycle(empty);
            }
            Err(saved) => {
                let Saved(values) = *saved.downcast::<Saved<T>>().expect("fixed source snapshot");
                assert_eq!(
                    values.len(),
                    self.values.len(),
                    "fixed source snapshot universe"
                );
                for (index, value) in values.into_iter().enumerate() {
                    self.replace(index, value.map(Row::from_shared));
                }
            }
        }
    }
    fn source_rows(&self) -> Option<&dyn Any> {
        Some(self.rows())
    }
    fn apply(&mut self, _: bool, event: &Event, _: NodeArithmetic) -> Result<Outputs, GraphError> {
        self.update(event)?;
        Ok(Outputs::one(event.clone()))
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
    #[allow(clippy::arc_with_non_send_sync)]
    fn owned_rows_inspection_and_rollback_do_not_require_payload_clone() {
        struct CloneBomb(usize);
        impl Clone for CloneBomb {
            fn clone(&self) -> Self {
                panic!("unexpected payload clone")
            }
        }
        let original = Arc::new(CloneBomb(10));
        let mut node = FixedSourceNode::new(vec![(Arc::new(0), original.clone())]);
        let held = node.rows().clone();
        let saved = <FixedSourceNode<CloneBomb> as Node<SimpleScore>>::checkpoint(&node);
        let undo = <FixedSourceNode<CloneBomb> as Node<SimpleScore>>::begin_checkpoint(&mut node);
        node.put_owned(0, CloneBomb(20)).unwrap();
        let changed = node.rows().clone();
        assert_eq!(changed[&0].0, 20);
        assert_eq!(held[&0].0, 10);
        assert!(matches!(
            node.put_owned(1, CloneBomb(99)),
            Err(GraphError::Dependency(_))
        ));
        assert!(std::ptr::eq(&node.rows()[&0], &changed[&0]));
        <FixedSourceNode<CloneBomb> as Node<SimpleScore>>::restore(&mut node, undo);
        assert!(std::ptr::eq(&node.rows()[&0], original.as_ref()));
        node.put_owned(0, CloneBomb(30)).unwrap();
        <FixedSourceNode<CloneBomb> as Node<SimpleScore>>::restore(&mut node, saved);
        assert!(std::ptr::eq(&node.rows()[&0], original.as_ref()));
        assert_eq!(changed[&0].0, 20);
    }
    #[test]
    #[allow(clippy::arc_with_non_send_sync)]
    fn reused_markers_preserve_nested_snapshot_install_and_payload_release() {
        use std::cell::RefCell;
        struct Owned(usize, Rc<RefCell<Vec<usize>>>);
        impl Clone for Owned {
            fn clone(&self) -> Self {
                panic!("checkpoint must not clone a payload");
            }
        }
        impl Drop for Owned {
            fn drop(&mut self) {
                self.1.borrow_mut().push(self.0);
            }
        }
        type Source = FixedSourceNode<Owned>;
        fn marker_address(saved: &dyn Any) -> *const Token {
            CheckpointBox::<Token>::get(saved).unwrap()
        }
        let drops = Rc::new(RefCell::new(Vec::new()));
        let mut node = Source::new(vec![(Arc::new(0), Arc::new(Owned(10, drops.clone())))]);
        let saved = <Source as Node<SimpleScore>>::checkpoint(&node);
        let outer = <Source as Node<SimpleScore>>::begin_checkpoint(&mut node);
        let address = marker_address(outer.as_ref());
        node.put_owned(0, Owned(20, drops.clone())).unwrap();
        let inner = <Source as Node<SimpleScore>>::begin_checkpoint(&mut node);
        assert_ne!(address, marker_address(inner.as_ref()));
        node.put_owned(0, Owned(30, drops.clone())).unwrap();
        <Source as Node<SimpleScore>>::commit_checkpoint(&mut node, inner);
        assert!(drops.borrow().is_empty());
        // Installing an owned snapshot while a journal is live must itself undo.
        <Source as Node<SimpleScore>>::restore(&mut node, saved);
        node.put_owned(0, Owned(40, drops.clone())).unwrap();
        <Source as Node<SimpleScore>>::restore(&mut node, outer);
        assert_eq!(node.values[0].as_ref().unwrap().shared().as_ref().0, 10);
        assert_eq!(*drops.borrow(), [40, 30, 20]);
        let next = <Source as Node<SimpleScore>>::begin_checkpoint(&mut node);
        assert_eq!(address, marker_address(next.as_ref()));
        node.put_owned(0, Owned(50, drops.clone())).unwrap();
        <Source as Node<SimpleScore>>::commit_checkpoint(&mut node, next);
        assert_eq!(*drops.borrow(), [40, 30, 20, 10]);
        let last = <Source as Node<SimpleScore>>::begin_checkpoint(&mut node);
        assert_eq!(address, marker_address(last.as_ref()));
        <Source as Node<SimpleScore>>::clear(&mut node);
        <Source as Node<SimpleScore>>::restore(&mut node, last);
        assert_eq!(node.values[0].as_ref().unwrap().shared().as_ref().0, 50);
        assert_eq!(*drops.borrow(), [40, 30, 20, 10]);
        drop(node);
        assert_eq!(*drops.borrow(), [40, 30, 20, 10, 50]);
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

#[cfg(test)]
#[path = "source_batch_tests.rs"]
mod batch_tests;

#[cfg(test)]
mod lazy_row_tests {
    use super::*;
    use crate::SimpleScore;
    use std::{
        cell::RefCell,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    struct NonClone(usize, Rc<RefCell<Vec<usize>>>);
    impl Drop for NonClone {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    #[test]
    fn unshared_rows_need_no_clone_and_promote_once_without_replacing_identity() {
        let drops = Rc::new(RefCell::new(Vec::new()));
        let row = Row::owned(NonClone(1, drops.clone()));
        assert!(row.shared.get().is_none());
        let first = row.shared().clone();
        let held = Arc::downgrade(&first);
        assert!(std::ptr::eq(first.as_ref(), row.shared().as_ref()));
        assert_eq!(Arc::strong_count(&first), 2);
        drop(row);
        assert!(drops.borrow().is_empty());
        drop(first);
        assert!(held.upgrade().is_none());
        assert_eq!(*drops.borrow(), [1]);
        let unshared = Row::owned(NonClone(2, drops.clone()));
        drop(unshared);
        assert_eq!(*drops.borrow(), [1, 2]);
    }
    #[test]
    fn full_snapshots_promote_owned_rows_without_payload_clone_and_keep_old_views() {
        #[derive(Debug)]
        struct CloneBomb(usize);
        impl Clone for CloneBomb {
            fn clone(&self) -> Self {
                panic!("unexpected Clone");
            }
        }
        type Source = FixedSourceNode<CloneBomb>;
        let mut node = Source::new(vec![(Arc::new(0), Arc::new(CloneBomb(1)))]);
        let trial = <Source as Node<SimpleScore>>::begin_checkpoint(&mut node);
        node.put_owned(0, CloneBomb(2)).unwrap();
        assert!(node.values[0].as_ref().unwrap().shared.get().is_none());
        let saved = <Source as Node<SimpleScore>>::checkpoint(&node);
        let view = node.rows().clone();
        let pointer = &view[&0] as *const CloneBomb;
        node.put_owned(0, CloneBomb(3)).unwrap();
        assert!(node.values[0].as_ref().unwrap().shared.get().is_none());
        <Source as Node<SimpleScore>>::restore(&mut node, saved);
        assert_eq!(&node.rows()[&0] as *const CloneBomb, pointer);
        <Source as Node<SimpleScore>>::restore(&mut node, trial);
        assert_eq!(node.rows()[&0].0, 1);
        assert_eq!(view[&0].0, 2);
    }
    #[test]
    #[allow(clippy::arc_with_non_send_sync)]
    fn lazy_promotion_and_source_event_clone_failure_preserve_rows_and_rollback() {
        struct Value(usize, Rc<Cell<usize>>, Rc<Cell<usize>>);
        impl Clone for Value {
            fn clone(&self) -> Self {
                let calls = self.1.get() + 1;
                self.1.set(calls);
                assert_ne!(calls, self.2.get(), "source event Clone");
                Self(self.0, self.1.clone(), self.2.clone())
            }
        }
        type Source = FixedSourceNode<Value>;
        let calls = Rc::new(Cell::new(0));
        let fail = Rc::new(Cell::new(0));
        let value = |v| Value(v, calls.clone(), fail.clone());
        let mut node = Source::new(vec![
            (Arc::new(0), Arc::new(value(1))),
            (Arc::new(1), Arc::new(value(2))),
        ]);
        let trial = <Source as Node<SimpleScore>>::begin_checkpoint(&mut node);
        node.put_owned(0, value(3)).unwrap();
        node.put_owned(1, value(4)).unwrap();
        assert!(
            node.values
                .iter()
                .all(|v| v.as_ref().unwrap().shared.get().is_none())
        );
        fail.set(2);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                <Source as Node<SimpleScore>>::source_events(&node)
            }))
            .is_err()
        );
        assert_eq!(calls.get(), 2);
        fail.set(0);
        assert_eq!((node.rows()[&0].0, node.rows()[&1].0), (3, 4));
        let events = <Source as Node<SimpleScore>>::source_events(&node);
        assert_eq!(events.len(), 2);
        assert_eq!(calls.get(), 4);
        drop(events);
        <Source as Node<SimpleScore>>::restore(&mut node, trial);
        assert_eq!((node.rows()[&0].0, node.rows()[&1].0), (1, 2));
        assert_eq!(calls.get(), 4);
    }
}
