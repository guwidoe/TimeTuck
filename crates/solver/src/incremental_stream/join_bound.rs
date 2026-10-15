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
    left_index: SnapshotMap<K, SnapshotMap<I, ()>>,
    right_index: SnapshotMap<K, SnapshotMap<J, ()>>,
    left_poisoned: bool,
    right_poisoned: bool,
}
impl<I, T, K> JoinSide<I, T, K> {
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
        self.index = saved.index;
        self.poisoned = saved.poisoned;
    }
}
impl<I: 'static, T, K> JoinSide<I, T, K> {
    fn bind_fixed(&mut self, count: usize) -> bool {
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
            left_index: self.left.index.clone(),
            right_index: self.right.index.clone(),
            left_poisoned: self.left.poisoned,
            right_poisoned: self.right.poisoned,
        }))
    }
    pub(crate) fn commit_bound_checkpoint(&mut self, saved: Box<dyn Any>) {
        if let Ok(saved) = saved.downcast::<BoundCheckpoint<I, J, L, R, K>>() {
            self.left.commit_bound(saved.left);
            self.right.commit_bound(saved.right);
        }
    }
    pub(crate) fn restore_bound_checkpoint(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<BoundCheckpoint<I, J, L, R, K>>() {
            Ok(saved) => {
                self.left.restore_bound(saved.left);
                self.right.restore_bound(saved.right);
                self.left.index = saved.left_index;
                self.right.index = saved.right_index;
                self.left.poisoned = saved.left_poisoned;
                self.right.poisoned = saved.right_poisoned;
            }
            Err(saved) => self.restore(saved),
        }
    }
}
