//! Reuse bounded key-preserving rows for complete planning-filter input caches.
//! Sparse/unbound filters and ordinary facts keep their existing maps. Binding
//! is private, occurs after initialization and never changes the ID universe.
use super::*;
use filter_pairs::PairRows;
use join_fixed_rows::FixedRows;
use std::any::{Any, TypeId};

enum Saved<I, V> {
    Frame(usize),
    Full(Box<FixedRows<I, V>>),
    PairFrame(usize),
    PairFull(Box<PairRows<I, V>>),
}

impl<I: Ord + Clone, T: Clone> FilterNode<'_, I, T> {
    pub(super) fn apply_filtered(
        &mut self,
        id: I,
        new: Option<(bool, T)>,
    ) -> Option<TupleChange<I, T>> {
        fn output<I: Clone, T: Clone>(
            id: &I,
            new: &Option<(bool, T)>,
            was_accepted: bool,
        ) -> Option<TupleChange<I, T>> {
            match new {
                Some((true, row)) => Some(TupleChange::put(id.clone(), row.clone())),
                _ if was_accepted => Some(TupleChange::retract(id.clone())),
                _ => None,
            }
        }
        if let Some(rows) = &mut self.pair_rows {
            let was_accepted = rows.get(&id).is_some_and(|(accepted, _)| *accepted);
            let result = output(&id, &new, was_accepted);
            self.poisoned = true;
            match new {
                Some(row) => rows.insert(id, row),
                None => rows.remove(&id),
            }
            self.poisoned = false;
            return result;
        }
        match &mut self.fixed_rows {
            Some(rows) => {
                let was_accepted = rows.get(&id).is_some_and(|(accepted, _)| *accepted);
                let result = output(&id, &new, was_accepted);
                self.poisoned = true;
                match new {
                    Some(row) => rows.insert(id, row),
                    None => rows.remove(&id),
                }
                self.poisoned = false;
                result
            }
            None => {
                let (prepared, was_accepted) = self
                    .rows
                    .prepare_change(&id, |row| row.is_some_and(|(accepted, _)| *accepted));
                let result = output(&id, &new, was_accepted);
                self.poisoned = true;
                match new {
                    Some(row) => prepared.insert(id, row),
                    None => prepared.remove(),
                }
                self.poisoned = false;
                result
            }
        }
    }
    pub(super) fn row_iter(&self) -> impl Iterator<Item = (&I, &(bool, T))> {
        self.fixed_rows
            .iter()
            .flat_map(|rows| rows.iter())
            .chain(self.pair_rows.iter().flat_map(|rows| rows.iter()))
            .chain(self.rows.iter())
    }
    pub(super) fn clear_rows(&mut self) {
        if let Some(rows) = &mut self.pair_rows {
            rows.clear();
            return;
        }
        match &mut self.fixed_rows {
            Some(rows) => rows.clear(),
            None => self.rows.clear(),
        }
    }
}

impl<I: Ord + Clone + 'static, T: Clone + 'static> FilterNode<'_, I, T> {
    pub(super) fn checkpoint_rows(&self) -> Box<dyn Any> {
        if let Some(rows) = &self.pair_rows {
            return Box::new(Saved::PairFull(rows.clone()));
        }
        match &self.fixed_rows {
            Some(rows) => Box::new(Saved::Full(rows.clone())),
            None => Box::new(self.rows.clone()),
        }
    }
    pub(super) fn begin_rows(&mut self) -> Box<dyn Any> {
        if let Some(rows) = &mut self.pair_rows {
            return Box::new(Saved::<I, (bool, T)>::PairFrame(rows.begin()));
        }
        match &mut self.fixed_rows {
            Some(rows) => Box::new(Saved::<I, (bool, T)>::Frame(rows.begin())),
            None => self.rows.begin_checkpoint(),
        }
    }
    pub(super) fn commit_rows(&mut self, saved: Box<dyn Any>) {
        if let Some(rows) = &mut self.pair_rows {
            if let Saved::PairFrame(token) = *saved
                .downcast::<Saved<I, (bool, T)>>()
                .expect("pair filter checkpoint")
            {
                rows.commit(token);
            }
            return;
        }
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
        if let Some(rows) = &mut self.pair_rows {
            match *saved
                .downcast::<Saved<I, (bool, T)>>()
                .expect("pair filter checkpoint")
            {
                Saved::PairFrame(token) => rows.rollback(token),
                Saved::PairFull(old) => rows.install(*old),
                _ => unreachable!("pair filter checkpoint kind"),
            }
            return;
        }
        match &mut self.fixed_rows {
            Some(rows) => match *saved
                .downcast::<Saved<I, (bool, T)>>()
                .expect("bound filter checkpoint")
            {
                Saved::Frame(token) => rows.rollback(token),
                Saved::Full(old) => rows.install(*old),
                _ => unreachable!("bound filter checkpoint kind"),
            },
            None => self.rows.restore_checkpoint(saved),
        }
    }
    pub(crate) fn bind_fixed_pairs(&mut self, left: usize, right: usize) -> bool {
        if self.fixed_rows.is_some() || self.pair_rows.is_some() {
            return false;
        }
        let Some(rows) = PairRows::bind(&self.rows, left, right) else {
            return false;
        };
        self.pair_rows = Some(Box::new(rows));
        self.rows.clear();
        true
    }
    pub(crate) fn bind_fixed_inputs(&mut self, count: usize) -> bool {
        if TypeId::of::<I>() != TypeId::of::<usize>()
            || count > 4096
            || self.fixed_rows.is_some()
            || self.pair_rows.is_some()
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
        for count in [1usize, 256, 257, 320, 573, 4096, 4097] {
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
    fn larger_complete_planning_universes_bind_and_restore_their_final_slot() {
        for count in [257usize, 320, 573, 4096, 4097] {
            let mut node = FilterNode::new(|value: &i64| *value > 0);
            for id in 0..count {
                node.apply(TupleChange::put(id, 1));
            }
            assert_eq!(node.bind_fixed_inputs(count), count <= 4096);
            let saved = node.begin_checkpoint();
            assert_eq!(
                node.apply(TupleChange::put(count - 1, -1)),
                Some(TupleChange::retract(count - 1))
            );
            assert_eq!(node.rows().count(), count - 1);
            node.restore(saved);
            assert_eq!(node.rows().count(), count);
            assert_eq!(node.rows().last(), Some((&(count - 1), &1)));
        }
    }

    #[test]
    fn pair_binding_keeps_rejections_keys_and_nested_saved_membership() {
        let clones = Rc::new(Cell::new(0));
        let mut node = FilterNode::new(|value: &Payload| value.0 > 0);
        node.apply(TupleChange::put(
            (0usize, 63usize),
            Payload(1, clones.clone()),
        ));
        node.apply(TupleChange::put((2, 64), Payload(-1, clones.clone())));
        let held = node.rows.shared_entries();
        clones.set(0);
        assert!(node.bind_fixed_pairs(320, 320));
        assert_eq!(clones.get(), 0);
        let (id, value) = node.rows().next().unwrap();
        assert!(std::ptr::eq(id, held[0].0.as_ref()));
        assert!(std::ptr::eq(value, &held[0].1.as_ref().1));
        let outer = node.begin_checkpoint();
        node.apply(TupleChange::put((2, 64), Payload(3, clones.clone())));
        let saved = node.checkpoint();
        let inner = node.begin_checkpoint();
        node.apply(TupleChange::retract((0, 63)));
        node.commit_checkpoint(inner);
        node.restore(saved);
        assert_eq!(node.rows().count(), 2);
        node.restore(outer);
        assert_eq!(node.rows().count(), 1);
        assert!(node.apply(TupleChange::retract((2, 64))).is_none());
        assert_eq!(node.rows().count(), 1);
    }

    #[test]
    fn pair_filter_clone_failure_precedes_membership_mutation_and_remains_retryable() {
        struct Fallible(i64, Rc<Cell<bool>>);
        impl Clone for Fallible {
            fn clone(&self) -> Self {
                assert!(!self.1.replace(false), "accepted pair payload clone");
                Self(self.0, self.1.clone())
            }
        }
        let fail = Rc::new(Cell::new(false));
        let calls = Cell::new(0);
        let mut node = FilterNode::new(|_: &Fallible| {
            calls.set(calls.get() + 1);
            true
        });
        node.apply(TupleChange::put(
            (1usize, 2usize),
            Fallible(1, fail.clone()),
        ));
        assert!(node.bind_fixed_pairs(320, 320));
        fail.set(true);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                node.apply(TupleChange::put((1, 2), Fallible(2, fail.clone())))
            }))
            .is_err()
        );
        assert!(!node.is_poisoned());
        assert_eq!(node.rows().next().unwrap().1.0, 1);
        assert!(
            node.apply(TupleChange::put((1, 2), Fallible(3, fail.clone())))
                .is_some()
        );
        let before = calls.get();
        assert!(
            node.apply(TupleChange::retract((1, 2)))
                .unwrap()
                .value
                .is_none()
        );
        assert_eq!(calls.get(), before);
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
