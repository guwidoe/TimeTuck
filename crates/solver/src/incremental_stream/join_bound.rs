//! Private activation/checkpoints for ordinary direct-planning join inputs.
use super::*;
use join_fixed_rows::FixedRows;
use std::any::{Any, TypeId};

enum SideSaved<I, V> {
    Fixed(usize),
    Map(SnapshotMap<I, V>),
}
struct BoundCheckpoint<I, J, L, R, K> {
    left: SideSaved<I, (K, L)>,
    right: SideSaved<J, (K, R)>,
    left_index: usize,
    right_index: usize,
    left_poisoned: bool,
    right_poisoned: bool,
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
impl<I: 'static, T, K> JoinSide<I, T, K> {
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
        self.fixed_rows = Some(Box::new(FixedRows::new(rows, index::<I>)));
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
    pub(crate) fn bind_fixed_input(&mut self, right: bool, count: usize) -> bool {
        if right {
            self.right.bind_fixed(count)
        } else {
            self.left.bind_fixed(count)
        }
    }
    pub(crate) fn begin_bound_checkpoint(&mut self) -> Option<Box<dyn Any>> {
        if self.left.fixed_rows.is_none() && self.right.fixed_rows.is_none() {
            return None;
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
        if let Ok(saved) = saved.downcast::<BoundCheckpoint<I, J, L, R, K>>() {
            self.left.commit_bound(saved.left);
            self.right.commit_bound(saved.right);
            self.left.commit_index(saved.left_index);
            self.right.commit_index(saved.right_index);
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
            Err(saved) => self.restore(saved),
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
