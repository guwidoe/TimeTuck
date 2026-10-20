//! Reuse bounded key-preserving rows for complete planning-filter input caches.
//! Unbounded primitive IDs may use sparse pages; generic/custom-Drop inputs
//! retain tree maps. Fixed binding is private and never changes the ID universe.
use super::*;
use crate::primitive_rows::{DeltaRows, PrimitiveRows, RowsCapture};
use filter_pairs::PairRows;
use join_fixed_rows::FixedRows;
use std::any::{Any, TypeId};

enum Saved<I, V> {
    Frame(usize),
    Full(Box<FixedRows<I, V>>),
    PairFrame(usize),
    PairFull(Box<PairRows<I, V>>),
    PairDelta(filter_pairs::Capture<I, V>),
    FixedDelta(join_fixed_rows::Capture<V>),
    RowsFrame(usize),
    RowsFull(Box<PrimitiveRows<I, V>>),
    RowsDelta(DeltaRows<I, V>),
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
        let elide_rejected = self.elide_rejected && matches!(&new, Some((false, _)));
        if let Some(rows) = &mut self.pair_rows {
            let was_accepted = rows.get(&id).is_some_and(|(accepted, _)| *accepted);
            let result = output(&id, &new, was_accepted);
            self.poisoned = true;
            match new {
                Some(_) if elide_rejected => {
                    // Rejected puts still validate the declared universe at the
                    // original publication boundary, including poison on failure.
                    rows.validate_insert(&id);
                    rows.remove(&id);
                }
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
                    Some(_) if elide_rejected => {
                        rows.validate_insert(&id);
                        rows.remove(&id);
                    }
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
                    Some(_) if elide_rejected => {
                        prepared.remove();
                        self.rejected_universe_incomplete = true;
                    }
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
            None => Box::new(Saved::RowsFull(Box::new(self.rows.clone()))),
        }
    }
    pub(crate) fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any> {
        healthy(self.poisoned);
        // A saved candidate must retain untouched custom payload owners too.
        // Primitive payloads have no observable destruction schedule.
        if std::mem::needs_drop::<T>() {
            return self.checkpoint_rows();
        }
        if let Some(saved) = original.downcast_ref::<Saved<I, (bool, T)>>() {
            match saved {
                Saved::PairFrame(token) => {
                    let rows = self.pair_rows.as_ref().expect("bound pair rows");
                    return Box::new(Saved::PairDelta(rows.capture_current(*token)));
                }
                Saved::Frame(token) => {
                    let rows = self.fixed_rows.as_ref().expect("bound fixed rows");
                    return Box::new(Saved::<I, (bool, T)>::FixedDelta(
                        rows.capture_current(*token),
                    ));
                }
                Saved::RowsFrame(token) => {
                    return match self.rows.capture_current(*token) {
                        RowsCapture::Full(rows) => Box::new(Saved::RowsFull(Box::new(rows))),
                        RowsCapture::Delta(delta) => Box::new(Saved::RowsDelta(delta)),
                    };
                }
                _ => {}
            }
        }
        self.checkpoint_rows()
    }
    pub(super) fn begin_rows(&mut self) -> Box<dyn Any> {
        if let Some(rows) = &mut self.pair_rows {
            return Box::new(Saved::<I, (bool, T)>::PairFrame(rows.begin()));
        }
        match &mut self.fixed_rows {
            Some(rows) => Box::new(Saved::<I, (bool, T)>::Frame(rows.begin())),
            None => Box::new(Saved::<I, (bool, T)>::RowsFrame(self.rows.begin_journal())),
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
            None => {
                if let Saved::RowsFrame(token) = *saved
                    .downcast::<Saved<I, (bool, T)>>()
                    .expect("filter row checkpoint")
                {
                    self.rows.commit_journal(token);
                }
            }
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
                Saved::PairDelta(delta) => rows.install_capture(delta),
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
                Saved::FixedDelta(delta) => rows.install_capture(delta),
                _ => unreachable!("bound filter checkpoint kind"),
            },
            None => match *saved
                .downcast::<Saved<I, (bool, T)>>()
                .expect("filter row checkpoint")
            {
                Saved::RowsFrame(token) => self.rows.rollback_journal(token),
                Saved::RowsFull(old) => self.rows.install_snapshot(*old),
                Saved::RowsDelta(delta) => self.rows.install_capture(RowsCapture::Delta(delta)),
                _ => unreachable!("filter row checkpoint kind"),
            },
        }
    }
    /// Bind only exact primitive IDs with payloads whose destruction cannot be
    /// observed. Generic keys and payloads with Drop retain ordinary map state.
    pub(crate) fn bind_primitive_inputs(&mut self) -> bool {
        if self.fixed_rows.is_some() || self.pair_rows.is_some() || std::mem::needs_drop::<T>() {
            return false;
        }
        // Existing rows may be retained by full snapshots or journals. Never
        // change their rejection ownership policy through a later conversion.
        let initially_empty = self.rows.len() == 0;
        let bound = self.rows.bind_primitive();
        self.elide_rejected |= bound && initially_empty;
        bound
    }
    pub(crate) fn bind_fixed_pairs(&mut self, left: usize, right: usize) -> bool {
        if self.rejected_universe_incomplete
            || self.fixed_rows.is_some()
            || self.pair_rows.is_some()
        {
            return false;
        }
        let Some(rows) = PairRows::bind(self.rows.materialized(), left, right) else {
            return false;
        };
        self.pair_rows = Some(Box::new(rows));
        self.rows.clear();
        true
    }
    pub(crate) fn bind_fixed_inputs(&mut self, count: usize) -> bool {
        if self.rejected_universe_incomplete
            || TypeId::of::<I>() != TypeId::of::<usize>()
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

#[cfg(test)]
mod capture_tests {
    use super::*;
    use std::{cell::Cell, fmt::Debug, rc::Rc};

    #[test]
    fn bound_filter_capture_retains_untouched_custom_payload_owners() {
        let mut node = FilterNode::new(|_: &Rc<usize>| true);
        let value = Rc::new(7usize);
        let weak = Rc::downgrade(&value);
        node.apply(TupleChange::put(0usize, value));
        assert!(node.bind_fixed_inputs(1));
        let original = node.begin_checkpoint();
        let saved = node.capture_checkpoint(original.as_ref());
        assert!(matches!(
            saved.downcast_ref::<Saved<usize, (bool, Rc<usize>)>>(),
            Some(Saved::Full(_))
        ));
        node.restore(original);
        node.clear_rows();
        assert!(weak.upgrade().is_some());
        drop(saved);
        assert!(weak.upgrade().is_none());
    }

    fn exercise<I: Ord + Clone + Debug + 'static>(ids: [I; 3], pair: bool) {
        let calls = Rc::new(Cell::new(0));
        let mut node = FilterNode::new(|value: &i64| {
            calls.set(calls.get() + 1);
            *value > 0
        });
        for (id, value) in ids.iter().zip([1, -2, 3]) {
            node.apply(TupleChange::put(id.clone(), value));
        }
        if pair {
            assert!(node.bind_fixed_pairs(3, 65));
        } else {
            assert!(node.bind_fixed_inputs(3));
        }
        let contents = |node: &FilterNode<'_, I, i64>| {
            node.rows()
                .map(|(id, value)| (id.clone(), *value))
                .collect::<Vec<_>>()
        };
        let base = contents(&node);
        let original = node.begin_checkpoint();
        node.apply(TupleChange::put(ids[0].clone(), -10));
        node.apply(TupleChange::retract(ids[2].clone()));
        let inner = node.begin_checkpoint();
        node.apply(TupleChange::put(ids[1].clone(), 20));
        node.apply(TupleChange::put(ids[2].clone(), 30));
        let want = contents(&node);
        let saved = node.capture_checkpoint(original.as_ref());
        let saved_nested = node.capture_checkpoint(original.as_ref());
        node.commit_checkpoint(inner);
        node.restore(original);
        assert_eq!(contents(&node), base);
        let full_base = node.checkpoint();
        let other = node.begin_checkpoint();
        node.apply(TupleChange::put(ids[0].clone(), 100));
        node.apply(TupleChange::retract(ids[1].clone()));
        node.restore(other);
        node.restore(full_base);
        let outer = node.begin_checkpoint();
        let before = calls.get();
        node.restore(saved_nested);
        assert_eq!(contents(&node), want);
        assert_eq!(
            calls.get(),
            before,
            "capture installation never evaluates predicates"
        );
        node.restore(outer);
        assert_eq!(contents(&node), base);
        node.restore(saved);
        assert_eq!(contents(&node), want);
        assert_eq!(calls.get(), before);
        assert!(!node.is_poisoned());
    }
    #[test]
    fn pair_filter_capture_survives_full_saves_other_trials_and_nested_installs() {
        exercise([(0usize, 1usize), (0, 63), (2, 64)], true);
    }
    #[test]
    fn fixed_filter_capture_survives_full_saves_other_trials_and_nested_installs() {
        exercise([0usize, 1, 2], false);
    }
    #[test]
    fn ordinary_filter_capture_retains_full_snapshot_fallback() {
        let calls = Cell::new(0);
        let mut node = FilterNode::new(|value: &i64| {
            calls.set(calls.get() + 1);
            *value > 0
        });
        node.apply(TupleChange::put("a", 1));
        let original = node.begin_checkpoint();
        node.apply(TupleChange::put("b", 2));
        let saved = node.capture_checkpoint(original.as_ref());
        assert!(
            matches!(saved.downcast_ref::<Saved<&'static str, (bool, i64)>>(), Some(Saved::RowsFull(rows)) if !rows.is_primitive())
        );
        node.restore(original);
        let before = calls.get();
        node.restore(saved);
        assert_eq!(
            node.rows().collect::<Vec<_>>(),
            vec![(&"a", &1), (&"b", &2)]
        );
        assert_eq!(calls.get(), before);
    }
}

#[cfg(test)]
mod primitive_tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    #[test]
    fn unbounded_primitive_filter_capture_preserves_rejections_sparse_keys_and_nested_restores() {
        let calls = Cell::new(0);
        let mut node = FilterNode::new(|value: &i64| {
            calls.set(calls.get() + 1);
            *value > 0
        });
        let a = ((usize::MAX, 0usize), (usize::MAX, usize::MAX));
        let b = ((0usize, usize::MAX), (0usize, 15usize));
        let c = ((1usize, 2usize), (3usize, 16usize));
        node.apply(TupleChange::put(a, 1));
        node.apply(TupleChange::put(b, -2));
        let owner = node.rows().next().unwrap().1 as *const i64;
        assert!(node.bind_primitive_inputs());
        assert!(node.rows.is_primitive());
        assert_eq!(node.rows().next().unwrap().1 as *const i64, owner);
        let original = node.begin_checkpoint();
        assert_eq!(
            node.apply(TupleChange::put(a, -10)),
            Some(TupleChange::retract(a))
        );
        node.apply(TupleChange::put(c, 3));
        let nested = node.begin_checkpoint();
        node.apply(TupleChange::put(b, 2));
        node.commit_checkpoint(nested);
        let saved = node.capture_checkpoint(original.as_ref());
        assert!(matches!(
            saved.downcast_ref::<Saved<((usize, usize), (usize, usize)), (bool, i64)>>(),
            Some(Saved::RowsDelta(_))
        ));
        let want = node
            .rows()
            .map(|(key, value)| (*key, *value))
            .collect::<Vec<_>>();
        node.restore(original);
        let other = node.begin_checkpoint();
        node.apply(TupleChange::put(c, 30));
        node.apply(TupleChange::retract(a));
        node.restore(other);
        let outer = node.begin_checkpoint();
        let before = calls.get();
        node.restore(saved);
        assert_eq!(
            node.rows()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            want
        );
        assert_eq!(calls.get(), before);
        node.restore(outer);
        assert_eq!(
            node.rows()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            vec![(a, 1)]
        );
        assert!(node.apply(TupleChange::retract(b)).is_none());
        assert_eq!(calls.get(), before);
    }

    #[test]
    fn primitive_filter_keeps_exact_output_cloning_and_prepublication_failures() {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        static CLONES: AtomicUsize = AtomicUsize::new(0);
        static FAIL: AtomicBool = AtomicBool::new(false);
        struct Counted(i64);
        impl Clone for Counted {
            fn clone(&self) -> Self {
                CLONES.fetch_add(1, Ordering::SeqCst);
                assert!(
                    !FAIL.swap(false, Ordering::SeqCst),
                    "filter output clone failure"
                );
                Self(self.0)
            }
        }
        CLONES.store(0, Ordering::SeqCst);
        let calls = Cell::new(0);
        let mut node = FilterNode::new(|value: &Counted| {
            calls.set(calls.get() + 1);
            value.0 > 0
        });
        assert!(node.bind_primitive_inputs());
        node.apply(TupleChange::put(usize::MAX, Counted(1)));
        assert_eq!(CLONES.load(Ordering::SeqCst), 1);
        FAIL.store(true, Ordering::SeqCst);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                node.apply(TupleChange::put(usize::MAX, Counted(2)))
            }))
            .is_err()
        );
        assert!(!node.is_poisoned());
        assert_eq!(node.rows().next().unwrap().1.0, 1);
        assert_eq!(CLONES.load(Ordering::SeqCst), 2);
        assert!(
            node.apply(TupleChange::put(usize::MAX, Counted(-3)))
                .unwrap()
                .value
                .is_none()
        );
        assert_eq!(CLONES.load(Ordering::SeqCst), 2);
        let original = node.begin_checkpoint();
        node.apply(TupleChange::put(usize::MAX - 1, Counted(4)));
        let saved = node.capture_checkpoint(original.as_ref());
        node.restore(original);
        node.restore(saved);
        assert_eq!(CLONES.load(Ordering::SeqCst), 3);
        let before = calls.get();
        assert!(node.apply(TupleChange::retract(usize::MAX)).is_none());
        assert_eq!(calls.get(), before);
    }

    #[test]
    fn custom_drop_and_key_types_keep_full_tree_owners_and_borrowed_keys_still_work() {
        let value = Rc::new(7usize);
        let weak = Rc::downgrade(&value);
        let mut node = FilterNode::new(|_: &Rc<usize>| false);
        node.apply(TupleChange::put(usize::MAX, value));
        assert!(!node.bind_primitive_inputs());
        assert!(!node.rows.is_primitive());
        let original = node.begin_checkpoint();
        let saved = node.capture_checkpoint(original.as_ref());
        assert!(matches!(
            saved.downcast_ref::<Saved<usize, (bool, Rc<usize>)>>(),
            Some(Saved::RowsFull(_))
        ));
        node.restore(original);
        node.clear();
        assert!(weak.upgrade().is_some());
        drop(saved);
        assert!(weak.upgrade().is_none());
        let mut custom = FilterNode::new(|_: &i64| true);
        custom.apply(TupleChange::put(String::from("custom"), 1));
        assert!(!custom.bind_primitive_inputs());
        let borrowed = String::from("borrowed");
        let mut node = FilterNode::new(|_: &i64| true);
        node.apply(TupleChange::put(borrowed.as_str(), 2));
        assert_eq!(node.rows().next(), Some((&borrowed.as_str(), &2)));
        assert!(!node.rows.is_primitive());
    }
}
