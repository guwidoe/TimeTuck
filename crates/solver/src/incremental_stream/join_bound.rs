//! Private activation/checkpoints for ordinary direct-planning join inputs.
use super::*;
use join_fixed_rows::FixedRows;
use std::any::{Any, TypeId};

enum SideSaved<I, V> {
    Fixed(usize),
    Map(crate::primitive_rows::PrimitiveRows<I, V>),
}
struct BoundCheckpoint<I, J, L, R, K> {
    left: SideSaved<I, (K, L)>,
    right: SideSaved<J, (K, R)>,
    left_index: usize,
    right_index: usize,
    left_poisoned: bool,
    right_poisoned: bool,
}
enum CapturedRows<I, V> {
    Fixed(join_fixed_rows::Capture<V>),
    Map(crate::primitive_rows::PrimitiveRows<I, V>),
}
enum CapturedIndex<I, K> {
    Fixed(join_fixed_index::Capture<K>),
    Map(SnapshotMap<K, SnapshotMap<I, ()>>),
}
struct CapturedSide<I, T, K> {
    rows: CapturedRows<I, (K, T)>,
    index: CapturedIndex<I, K>,
    poisoned: bool,
}
struct BoundCapture<I, J, L, R, K> {
    left: CapturedSide<I, L, K>,
    right: CapturedSide<J, R, K>,
}
impl<I, T, K> JoinSide<I, T, K> {
    pub(super) fn begin_index(&mut self) -> usize {
        match &mut self.fixed_index {
            Some(index) => index.begin(),
            None => self.index.begin_journal(),
        }
    }
    pub(super) fn commit_index(&mut self, token: usize) {
        match &mut self.fixed_index {
            Some(index) => index.commit(token),
            None => self.index.commit_journal(token),
        }
    }
    pub(super) fn rollback_index(&mut self, token: usize) {
        match &mut self.fixed_index {
            Some(index) => index.rollback(token),
            None => self.index.rollback_journal(token),
        }
    }
    fn begin_bound(&mut self) -> SideSaved<I, (K, T)> {
        match &mut self.fixed_rows {
            Some(rows) => SideSaved::Fixed(rows.begin()),
            None => SideSaved::Map(self.rows.clone()),
        }
    }
    fn commit_bound(&mut self, saved: SideSaved<I, (K, T)>) {
        if let SideSaved::Fixed(token) = saved {
            self.fixed_rows.as_mut().expect("bound rows").commit(token);
        }
    }
    fn restore_bound(&mut self, saved: SideSaved<I, (K, T)>) {
        match saved {
            SideSaved::Fixed(token) => self
                .fixed_rows
                .as_mut()
                .expect("bound rows")
                .rollback(token),
            SideSaved::Map(rows) => self.rows.install_snapshot(rows),
        }
    }
    fn capture_side(&self, original: &SideSaved<I, (K, T)>, index: usize) -> CapturedSide<I, T, K> {
        CapturedSide {
            rows: match original {
                SideSaved::Fixed(token) => CapturedRows::Fixed(
                    self.fixed_rows
                        .as_ref()
                        .expect("bound capture rows")
                        .capture_current(*token),
                ),
                SideSaved::Map(_) => CapturedRows::Map(self.rows.clone()),
            },
            index: match &self.fixed_index {
                Some(rows) => CapturedIndex::Fixed(rows.capture_current(index)),
                None => CapturedIndex::Map(self.index.clone()),
            },
            poisoned: self.poisoned,
        }
    }
    fn install_captured_side(&mut self, saved: CapturedSide<I, T, K>) {
        match saved.rows {
            CapturedRows::Fixed(rows) => self
                .fixed_rows
                .as_mut()
                .expect("bound capture rows")
                .install_capture(rows),
            CapturedRows::Map(rows) => self.rows.install_snapshot(rows),
        }
        match saved.index {
            CapturedIndex::Fixed(index) => self
                .fixed_index
                .as_mut()
                .expect("bound capture index")
                .install_capture(index),
            CapturedIndex::Map(index) => self.index.install_snapshot(index),
        }
        self.poisoned = saved.poisoned;
    }
    pub(super) fn restore_side(&mut self, saved: Self) {
        match (&mut self.fixed_rows, saved.fixed_rows) {
            (Some(rows), Some(old)) => rows.install(*old),
            (None, None) => self.rows.install_snapshot(saved.rows),
            _ => unreachable!("join binding is immutable after initialization"),
        }
        match (&mut self.fixed_index, saved.fixed_index) {
            (Some(index), Some(old)) => index.install(*old),
            (None, None) => self.index.install_snapshot(saved.index),
            _ => unreachable!("join binding is immutable after initialization"),
        }
        self.poisoned = saved.poisoned;
    }
}
impl<I: 'static, T, K: 'static> JoinSide<I, T, K> {
    fn bind_fixed(&mut self, count: usize) -> bool {
        if TypeId::of::<I>() != TypeId::of::<usize>()
            || count > 4096
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
        self.fixed_index = Some(Box::new(join_fixed_index::FixedIndex::new(
            rows.iter().map(|(key, _)| key.clone()).collect(),
            self.index.shared_entries(),
            index::<I>,
        )));
        self.fixed_index.as_mut().unwrap().enable_delta_capture();
        let mut fixed = FixedRows::new(rows, index::<I>);
        if let Some(copy) = self.consuming_copy {
            fixed.enable_copied(copy);
        }
        self.fixed_rows = Some(Box::new(fixed));
        self.rows.clear();
        self.index.clear();
        true
    }
}
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
> JoinNode<'_, I, J, L, R, K>
{
    // Only whole-graph ownership-consuming search activation calls this while
    // empty. Public/retained and unbounded side payloads keep original owners.
    // The index and its canonical I owners are independent of copied payloads.
    pub(crate) fn enable_consuming_rows(&mut self) -> bool {
        if !self.primitive_graph_journals
            || self.is_poisoned()
            || TypeId::of::<I>() != TypeId::of::<usize>()
            || TypeId::of::<J>() != TypeId::of::<usize>()
            || !self.left.rows.is_empty()
            || !self.right.rows.is_empty()
            || self.left.fixed_rows.is_some()
            || self.right.fixed_rows.is_some()
        {
            return false;
        }
        let Some(left) = join_fixed_rows::copied::certify::<K, L>() else {
            return false;
        };
        let Some(right) = join_fixed_rows::copied::certify::<K, R>() else {
            return false;
        };
        self.left.consuming_copy = Some(left);
        self.right.consuming_copy = Some(right);
        true
    }
    pub(crate) fn bind_fixed_input(&mut self, right: bool, count: usize) -> bool {
        if right {
            self.right.bind_fixed(count)
        } else {
            self.left.bind_fixed(count)
        }
    }
    pub(crate) fn capture_checkpoint(&self, original: &dyn Any) -> Box<dyn Any> {
        healthy(self.is_poisoned());
        // Unchanged captured owners with user destructors must retain their
        // original full-snapshot lifetime. Primitive ID vectors have no effects.
        if !std::mem::needs_drop::<(K, L, R)>()
            && let Some(saved) = original.downcast_ref::<BoundCheckpoint<I, J, L, R, K>>()
        {
            return Box::new(BoundCapture {
                left: self.left.capture_side(&saved.left, saved.left_index),
                right: self.right.capture_side(&saved.right, saved.right_index),
            });
        }
        if self.primitive_graph_journals
            && let Some(captured) = self.capture_journal_checkpoint(original)
        {
            return captured;
        }
        self.checkpoint()
    }
    pub(crate) fn begin_bound_checkpoint(&mut self) -> Option<Box<dyn Any>> {
        if self.left.fixed_rows.is_none() && self.right.fixed_rows.is_none() {
            return self
                .primitive_graph_journals
                .then(|| self.begin_journal_checkpoint());
        }
        healthy(self.is_poisoned());
        Some(Box::new(BoundCheckpoint {
            left: self.left.begin_bound(),
            right: self.right.begin_bound(),
            left_index: self.left.begin_index(),
            right_index: self.right.begin_index(),
            left_poisoned: self.left.poisoned,
            right_poisoned: self.right.poisoned,
        }))
    }
    pub(crate) fn commit_bound_checkpoint(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<BoundCheckpoint<I, J, L, R, K>>() {
            Ok(saved) => {
                self.left.commit_bound(saved.left);
                self.right.commit_bound(saved.right);
                self.left.commit_index(saved.left_index);
                self.right.commit_index(saved.right_index);
            }
            Err(saved) => self.commit_if_journal_checkpoint(saved),
        }
    }
    pub(crate) fn restore_bound_checkpoint(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<BoundCheckpoint<I, J, L, R, K>>() {
            Ok(saved) => {
                self.left.restore_bound(saved.left);
                self.right.restore_bound(saved.right);
                self.left.rollback_index(saved.left_index);
                self.right.rollback_index(saved.right_index);
                self.left.poisoned = saved.left_poisoned;
                self.right.poisoned = saved.right_poisoned;
            }
            Err(saved) => match saved.downcast::<BoundCapture<I, J, L, R, K>>() {
                Ok(saved) => {
                    self.left.install_captured_side(saved.left);
                    self.right.install_captured_side(saved.right);
                }
                Err(saved) => self.restore_checkpoint(saved),
            },
        }
    }
}

#[cfg(test)]
mod join_bound_tests {
    use super::*;
    const COMPARISONS: [JoinComparison; 5] = [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ];
    type Node = JoinNode<'static, usize, usize, (usize, i64), (usize, i64), usize>;
    fn node(comparison: JoinComparison, count: usize) -> Node {
        let mut node =
            JoinNode::comparing(comparison, |r: &(usize, i64)| r.0, |r: &(usize, i64)| r.0);
        for i in 0..count {
            node.apply_left(TupleChange::put(i, (i % 9, i as i64)));
            node.apply_right(TupleChange::put(i, ((i + 2) % 9, -(i as i64))));
        }
        node
    }
    type Match = ((usize, usize), ((usize, i64), (usize, i64)));
    fn matches(node: &Node) -> Vec<Match> {
        node.matches()
            .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
            .collect()
    }
    #[test]
    fn join_binding_accepts_complete_bounded_universes_only() {
        for count in [0, 1, 256, 257, 320, 573, 4096, 4097] {
            let mut node: JoinNode<'_, usize, usize, usize, usize, usize> =
                JoinNode::new(|r: &usize| *r, |r: &usize| *r);
            for i in 0..count {
                node.apply_left(TupleChange::put(i, i));
            }
            assert_eq!(node.bind_fixed_input(false, count), count <= 4096);
            if count <= 4096 {
                assert!(!node.bind_fixed_input(false, count));
            }
        }
        let mut sparse = node(JoinComparison::Equal, 2);
        assert!(!sparse.bind_fixed_input(false, 1));
        sparse.apply_left(TupleChange::retract(0));
        assert!(!sparse.bind_fixed_input(false, 1));
        let mut custom = JoinNode::new(|r: &usize| *r, |r: &usize| *r);
        custom.apply_left(TupleChange::put(0u32, 0));
        custom.apply_right(TupleChange::put(0usize, 0));
        assert!(!custom.bind_fixed_input(false, 1));
    }
    #[test]
    fn join_bound_membership_matches_generic_outputs_and_order_across_word_boundaries() {
        for comparison in COMPARISONS {
            for bound_right in [false, true] {
                let mut generic = node(comparison, 131);
                let mut bound = node(comparison, 131);
                assert!(bound.bind_fixed_input(false, 131));
                if bound_right {
                    assert!(bound.bind_fixed_input(true, 131));
                }
                assert_eq!(matches(&bound), matches(&generic));
                let mut seed = 813u64;
                for step in 0..100 {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                    let id = (seed >> 32) as usize % 131;
                    let value = (step % 7 != 0).then_some(((seed >> 28) as usize % 11, step));
                    let change = TupleChange { id, value };
                    let (actual, expected) = if step % 2 == 0 {
                        (bound.apply_left(change.clone()), generic.apply_left(change))
                    } else {
                        (
                            bound.apply_right(change.clone()),
                            generic.apply_right(change),
                        )
                    };
                    assert_eq!(actual, expected, "{comparison:?}, step {step}");
                    assert_eq!(
                        matches(&bound),
                        matches(&generic),
                        "{comparison:?}, step {step}"
                    );
                }
            }
        }
    }
    #[test]
    fn join_bound_nested_trials_snapshots_clear_and_restore_keep_memberships() {
        for comparison in COMPARISONS {
            for bound_right in [false, true] {
                let mut node = node(comparison, 8);
                assert!(node.bind_fixed_input(false, 8));
                if bound_right {
                    assert!(node.bind_fixed_input(true, 8));
                }
                let initial = matches(&node);
                let snapshot = node.checkpoint();
                let outer = node.begin_bound_checkpoint().unwrap();
                node.apply_left(TupleChange::put(0, (20, 200)));
                node.apply_right(TupleChange::retract(1));
                let middle = matches(&node);
                let inner = node.begin_bound_checkpoint().unwrap();
                node.apply_left(TupleChange::retract(2));
                node.apply_right(TupleChange::put(1, (20, 300)));
                node.commit_bound_checkpoint(inner);
                let retained = node.checkpoint();
                let final_rows = matches(&node);
                node.restore_bound_checkpoint(outer);
                assert_eq!(matches(&node), initial);
                let outer = node.begin_bound_checkpoint().unwrap();
                node.restore(retained);
                assert_eq!(matches(&node), final_rows);
                node.restore_bound_checkpoint(outer);
                assert_eq!(matches(&node), initial);
                let clear = node.begin_bound_checkpoint().unwrap();
                node.clear();
                assert!(matches(&node).is_empty());
                node.restore_bound_checkpoint(clear);
                assert_eq!(matches(&node), initial);
                let outer = node.begin_bound_checkpoint().unwrap();
                node.apply_left(TupleChange::put(0, (20, 200)));
                node.apply_right(TupleChange::retract(1));
                assert_eq!(matches(&node), middle);
                node.commit_bound_checkpoint(outer);
                node.restore(snapshot);
                assert_eq!(matches(&node), initial);
            }
        }
    }
    #[test]
    fn join_unbound_index_journals_survive_batch_installs_and_full_restore() {
        let mut node = node(JoinComparison::Equal, 8);
        let initial = matches(&node);
        let snapshot = node.checkpoint();
        let outer = node.begin_journal_checkpoint();
        node.apply_left(TupleChange::put(0, (20, 200)));
        let inner = node.begin_journal_checkpoint();
        node.apply_batch(
            [TupleChange::put(1, (20, 300))],
            [TupleChange::put(2, (20, 400))],
        );
        node.commit_journal_checkpoint(inner);
        node.restore_checkpoint(outer);
        assert_eq!(matches(&node), initial);
        let outer = node.begin_journal_checkpoint();
        node.apply_right(TupleChange::retract(1));
        node.restore(snapshot);
        assert_eq!(matches(&node), initial);
        node.restore_checkpoint(outer);
        assert_eq!(matches(&node), initial);
    }
}

#[cfg(test)]
mod join_capture_tests {
    use super::*;
    use std::{cell::Cell, rc::Rc, sync::Arc};
    type Node<'a> = JoinNode<'a, usize, usize, (usize, i64), (usize, i64), usize>;
    type Match = ((usize, usize), ((usize, i64), (usize, i64)));
    const COMPARISONS: [JoinComparison; 5] = [
        JoinComparison::Equal,
        JoinComparison::LessThan,
        JoinComparison::LessThanOrEqual,
        JoinComparison::GreaterThan,
        JoinComparison::GreaterThanOrEqual,
    ];
    fn contents(node: &Node<'_>) -> Vec<Match> {
        node.matches()
            .map(|((i, j), (l, r))| ((*i, *j), (*l, *r)))
            .collect()
    }
    #[test]
    fn join_capture_all_relations_partial_binding_alternate_trials_and_outer_undo() {
        for comparison in COMPARISONS {
            for binding in 1..=3 {
                let calls = Cell::new(0);
                let mut node = JoinNode::comparing(
                    comparison,
                    |row: &(usize, i64)| {
                        calls.set(calls.get() + 1);
                        row.0
                    },
                    |row: &(usize, i64)| {
                        calls.set(calls.get() + 1);
                        row.0
                    },
                );
                for id in 0..24 {
                    node.apply_left(TupleChange::put(id, (id % 4, id as i64)));
                    node.apply_right(TupleChange::put(id, ((id + 1) % 4, -(id as i64))));
                }
                if binding & 1 != 0 {
                    assert!(node.bind_fixed_input(false, 24));
                }
                if binding & 2 != 0 {
                    assert!(node.bind_fixed_input(true, 24));
                }
                let base = contents(&node);
                let full_base = node.checkpoint();
                let original = node.begin_bound_checkpoint().unwrap();
                node.apply_left(TupleChange::put(0, (9, 90)));
                node.apply_right(TupleChange::put(1, (9, 91)));
                node.apply_left(TupleChange::retract(23));
                let inner = node.begin_bound_checkpoint().unwrap();
                node.apply_right(TupleChange::retract(22));
                node.apply_right(TupleChange::put(1, (10, 101)));
                node.apply_left(TupleChange::put(0, (10, 100)));
                let want = contents(&node);
                let before = calls.get();
                let saved = node.capture_checkpoint(original.as_ref());
                let nested = node.capture_checkpoint(original.as_ref());
                assert!(
                    saved.is::<BoundCapture<usize, usize, (usize, i64), (usize, i64), usize>>()
                );
                assert_eq!(calls.get(), before);
                node.commit_bound_checkpoint(inner);
                node.restore_bound_checkpoint(original);
                assert_eq!(contents(&node), base);
                let other = node.begin_bound_checkpoint().unwrap();
                node.apply_left(TupleChange::put(3, (12, 300)));
                node.apply_right(TupleChange::retract(2));
                node.restore_bound_checkpoint(other);
                assert_eq!(contents(&node), base);
                let outer = node.begin_bound_checkpoint().unwrap();
                let before = calls.get();
                node.restore_bound_checkpoint(nested);
                assert_eq!(contents(&node), want, "{comparison:?}, binding {binding}");
                node.restore_bound_checkpoint(outer);
                assert_eq!(contents(&node), base);
                node.restore_bound_checkpoint(saved);
                assert_eq!(contents(&node), want);
                assert_eq!(
                    calls.get(),
                    before,
                    "capture installation does not project keys"
                );
                node.restore(full_base);
                assert_eq!(contents(&node), base);
                // Full legacy snapshots restored inside a frame must remain
                // visible to subsequent sparse captures and outer undo.
                let original = node.begin_bound_checkpoint().unwrap();
                let legacy = node.checkpoint();
                node.clear();
                node.restore(legacy);
                node.apply_left(TupleChange::put(4, (8, 400)));
                let wanted = contents(&node);
                let saved = node.capture_checkpoint(original.as_ref());
                node.restore_bound_checkpoint(original);
                node.restore_bound_checkpoint(saved);
                assert_eq!(contents(&node), wanted);
            }
        }
    }
    #[test]
    fn join_capture_self_join_empty_sparse_states_keep_matching_rows() {
        for comparison in COMPARISONS {
            let mut node =
                JoinNode::comparing(comparison, |r: &(usize, i64)| r.0, |r: &(usize, i64)| r.0);
            for id in 0usize..8 {
                node.apply_left(TupleChange::put(id, (id % 3, id as i64)));
                node.apply_right(TupleChange::put(id, (id % 3, id as i64)));
            }
            assert!(node.bind_fixed_input(false, 8));
            assert!(node.bind_fixed_input(true, 8));
            let base = contents(&node);
            let original = node.begin_bound_checkpoint().unwrap();
            node.clear();
            for id in [0, 7] {
                node.apply_right(TupleChange::put(id, (20, id as i64 * 2)));
                node.apply_left(TupleChange::put(id, (20, id as i64 * 2)));
            }
            let want = contents(&node);
            let saved = node.capture_checkpoint(original.as_ref());
            node.restore_bound_checkpoint(original);
            assert_eq!(contents(&node), base);
            node.restore_bound_checkpoint(saved);
            assert_eq!(contents(&node), want);
            let original = node.begin_bound_checkpoint().unwrap();
            node.clear();
            let saved = node.capture_checkpoint(original.as_ref());
            node.restore_bound_checkpoint(original);
            node.restore_bound_checkpoint(saved);
            assert!(contents(&node).is_empty());
        }
        let mut empty: Node<'_> =
            JoinNode::new(|row: &(usize, i64)| row.0, |row: &(usize, i64)| row.0);
        assert!(empty.bind_fixed_input(false, 0));
        assert!(empty.bind_fixed_input(true, 0));
        let original = empty.begin_bound_checkpoint().unwrap();
        let saved = empty.capture_checkpoint(original.as_ref());
        empty.restore_bound_checkpoint(original);
        empty.restore_bound_checkpoint(saved);
        assert!(contents(&empty).is_empty());
    }
    #[test]
    fn join_capture_custom_drop_payloads_keep_full_untouched_owners_and_clone_behavior() {
        struct Row(usize, Arc<()>, Rc<Cell<usize>>);
        impl Clone for Row {
            fn clone(&self) -> Self {
                self.2.set(self.2.get() + 1);
                Self(self.0, self.1.clone(), self.2.clone())
            }
        }
        let clones = Rc::new(Cell::new(0));
        let marker = Arc::new(());
        let weak = Arc::downgrade(&marker);
        let mut node = JoinNode::new(|row: &Row| row.0, |row: &usize| *row);
        node.apply_left(TupleChange::put(
            0usize,
            Row(0, Arc::new(()), clones.clone()),
        ));
        node.apply_left(TupleChange::put(1, Row(1, marker, clones.clone())));
        node.apply_right(TupleChange::put(0usize, 0usize));
        assert!(node.bind_fixed_input(false, 2));
        assert!(node.bind_fixed_input(true, 1));
        let original = node.begin_bound_checkpoint().unwrap();
        node.apply_left(TupleChange::put(0, Row(2, Arc::new(()), clones.clone())));
        let before = clones.get();
        let saved = node.capture_checkpoint(original.as_ref());
        assert!(saved.is::<(JoinSide<usize, Row, usize>, JoinSide<usize, usize, usize>)>());
        node.restore_bound_checkpoint(original);
        node.clear();
        assert!(
            weak.upgrade().is_some(),
            "full capture retains untouched custom payload owners"
        );
        node.restore_bound_checkpoint(saved);
        assert_eq!(clones.get(), before);
        assert!(weak.upgrade().is_some());
        drop(node);
        assert!(weak.upgrade().is_none());
    }
}
