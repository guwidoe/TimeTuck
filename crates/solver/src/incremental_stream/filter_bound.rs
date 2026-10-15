//! Reuse bounded key-preserving rows for complete planning-filter input caches.
//! Sparse/unbound filters and ordinary facts keep their existing maps. Binding
//! is private, occurs after initialization and never changes the ID universe.
use super::*;
use join_fixed_rows::FixedRows;
use std::any::{Any, TypeId};

enum Saved<I, V> {
    Frame(usize),
    Full(Box<FixedRows<I, V>>),
}

impl<I: Ord + Clone, T: Clone> FilterNode<'_, I, T> {
    pub(super) fn row(&self, id: &I) -> Option<&(bool, T)> {
        match &self.fixed_rows {
            Some(rows) => rows.get(id),
            None => self.rows.get(id),
        }
    }
    pub(super) fn row_iter(&self) -> impl Iterator<Item = (&I, &(bool, T))> {
        self.fixed_rows
            .iter()
            .flat_map(|rows| rows.iter())
            .chain(self.rows.iter())
    }
    pub(super) fn insert_row(&mut self, id: I, value: (bool, T)) {
        match &mut self.fixed_rows {
            Some(rows) => rows.insert(id, value),
            None => self.rows.insert(id, value),
        }
    }
    pub(super) fn remove_row(&mut self, id: &I) {
        match &mut self.fixed_rows {
            Some(rows) => rows.remove(id),
            None => {
                self.rows.remove(id);
            }
        }
    }
    pub(super) fn clear_rows(&mut self) {
        match &mut self.fixed_rows {
            Some(rows) => rows.clear(),
            None => self.rows.clear(),
        }
    }
}

impl<I: Ord + Clone + 'static, T: Clone + 'static> FilterNode<'_, I, T> {
    pub(super) fn checkpoint_rows(&self) -> Box<dyn Any> {
        match &self.fixed_rows {
            Some(rows) => Box::new(Saved::Full(rows.clone())),
            None => Box::new(self.rows.clone()),
        }
    }
    pub(super) fn begin_rows(&mut self) -> Box<dyn Any> {
        match &mut self.fixed_rows {
            Some(rows) => Box::new(Saved::<I, (bool, T)>::Frame(rows.begin())),
            None => self.rows.begin_checkpoint(),
        }
    }
    pub(super) fn commit_rows(&mut self, saved: Box<dyn Any>) {
        match &mut self.fixed_rows {
            Some(rows) => {
                if let Saved::Frame(token) = *saved
                    .downcast::<Saved<I, (bool, T)>>()
                    .expect("bound filter checkpoint")
                {
                    rows.commit(token);
                }
            }
            None => self.rows.commit_checkpoint(saved),
        }
    }
    pub(super) fn restore_rows(&mut self, saved: Box<dyn Any>) {
        match &mut self.fixed_rows {
            Some(rows) => match *saved
                .downcast::<Saved<I, (bool, T)>>()
                .expect("bound filter checkpoint")
            {
                Saved::Frame(token) => rows.rollback(token),
                Saved::Full(old) => rows.install(*old),
            },
            None => self.rows.restore_checkpoint(saved),
        }
    }
    pub(crate) fn bind_fixed_inputs(&mut self, count: usize) -> bool {
        if TypeId::of::<I>() != TypeId::of::<usize>()
            || count > 256
            || self.fixed_rows.is_some()
            || self.rows.len() != count
        {
            return false;
        }
        fn index<I: 'static>(id: &I) -> usize {
            *(id as &dyn Any)
                .downcast_ref::<usize>()
                .expect("bound usize ID")
        }
        let rows = self.rows.shared_entries();
        if rows
            .iter()
            .enumerate()
            .any(|(i, (id, _))| index(id.as_ref()) != i)
        {
            return false;
        }
        self.fixed_rows = Some(Box::new(FixedRows::new(rows, index::<I>)));
        self.rows.clear();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    struct Payload(i64, Rc<Cell<usize>>);
    impl Clone for Payload {
        fn clone(&self) -> Self {
            self.1.set(self.1.get() + 1);
            Self(self.0, self.1.clone())
        }
    }

    #[test]
    fn binding_checks_entire_universe_and_retains_actual_keys_payloads_and_iteration() {
        let clones = Rc::new(Cell::new(0));
        let mut node = FilterNode::new(|v: &Payload| v.0 > 0);
        node.apply(TupleChange::put(0usize, Payload(1, clones.clone())));
        node.apply(TupleChange::put(1, Payload(0, clones.clone())));
        let held = node.rows.shared_entries();
        clones.set(0);
        assert!(node.bind_fixed_inputs(2));
        assert!(!node.bind_fixed_inputs(2));
        let (id, row) = node.rows().next().unwrap();
        assert!(std::ptr::eq(id, held[0].0.as_ref()));
        assert!(std::ptr::eq(row, &held[0].1.as_ref().1));
        assert_eq!(node.rows().count(), 1);
        let saved = node.checkpoint();
        node.clear();
        assert_eq!(node.rows().count(), 0);
        node.restore(saved);
        assert_eq!(node.rows().count(), 1);
        assert_eq!(clones.get(), 0);
        let mut zero = FilterNode::<usize, i64>::new(|_| true);
        assert!(zero.bind_fixed_inputs(0));
        for count in [1usize, 256, 257] {
            let mut sparse = FilterNode::new(|_: &i64| true);
            sparse.apply(TupleChange::put(count - 1, 1));
            assert_eq!(sparse.bind_fixed_inputs(count), count == 1);
        }
        let mut wrong = FilterNode::new(|_: &i64| true);
        wrong.apply(TupleChange::put(1usize, 1));
        assert!(!wrong.bind_fixed_inputs(1));
        let mut custom = FilterNode::new(|_: &i64| true);
        custom.apply(TupleChange::put("id", 1));
        assert!(!custom.bind_fixed_inputs(1));
    }

    #[test]
    fn nested_and_full_restore_inside_outer_frame_keep_predicates_and_old_owners() {
        let calls = Rc::new(Cell::new(0));
        let mut node = FilterNode::new(|v: &i64| {
            calls.set(calls.get() + 1);
            *v > 0
        });
        node.apply(TupleChange::put(0usize, 1));
        assert!(node.bind_fixed_inputs(1));
        let outer = node.begin_checkpoint();
        node.apply(TupleChange::put(0, 2));
        let full = node.checkpoint();
        let inner = node.begin_checkpoint();
        node.apply(TupleChange::put(0, -3));
        assert!(node.rows().next().is_none());
        node.commit_checkpoint(inner);
        node.restore(full);
        assert_eq!(*node.rows().next().unwrap().1, 2);
        let before = calls.get();
        assert_eq!(
            node.apply(TupleChange::retract(0)),
            Some(TupleChange::retract(0))
        );
        assert_eq!(calls.get(), before);
        node.restore(outer);
        assert_eq!(*node.rows().next().unwrap().1, 1);
        assert_eq!(calls.get(), before);
        assert!(!node.is_poisoned());
    }
}
