//! Graph trial checkpoints avoid sharing row roots until a snapshot is requested.
use super::*;
struct Checkpoint {
    left: usize,
    right: usize,
    left_index: usize,
    right_index: usize,
    left_poisoned: bool,
    right_poisoned: bool,
}
struct Capture<I, J, L, R, K> {
    left: crate::primitive_rows::RowsCapture<I, (K, L)>,
    right: crate::primitive_rows::RowsCapture<J, (K, R)>,
    left_index: SnapshotMap<K, SnapshotMap<I, ()>>,
    right_index: SnapshotMap<K, SnapshotMap<J, ()>>,
    left_poisoned: bool,
    right_poisoned: bool,
}
impl<
    I: Ord + Clone + 'static,
    J: Ord + Clone + 'static,
    L: Clone + 'static,
    R: Clone + 'static,
    K: Ord + Clone + 'static,
> JoinNode<'_, I, J, L, R, K>
{
    pub(crate) fn bind_primitive_graph_journals(&mut self) -> bool {
        if primitive::<I>()
            && primitive::<J>()
            && primitive::<K>()
            && !std::mem::needs_drop::<(L, R)>()
        {
            // Default standalone/custom nodes retain the original tree. Only
            // ordinary graph construction takes direct primitive row lookups;
            // key buckets and their matching order remain unchanged.
            self.left.rows.bind_primitive();
            self.right.rows.bind_primitive();
            if self.comparison == JoinComparison::Equal {
                if let Some(index) = &mut self.left.fixed_index {
                    index.enable_directory_buckets();
                }
                if let Some(index) = &mut self.right.fixed_index {
                    index.enable_directory_buckets();
                }
            }
            self.primitive_graph_journals = true;
        }
        self.primitive_graph_journals
    }
    pub(super) fn commit_if_journal_checkpoint(&mut self, saved: Box<dyn Any>) {
        if saved.is::<Checkpoint>() {
            self.commit_journal_checkpoint(saved);
        }
    }
    #[cfg(test)]
    pub(crate) fn is_ordinary_journal_for_test(saved: &dyn Any) -> bool {
        saved.is::<Checkpoint>()
    }
    pub(crate) fn begin_journal_checkpoint(&mut self) -> Box<dyn Any> {
        healthy(self.is_poisoned());
        Box::new(Checkpoint {
            left: self.left.rows.begin_journal(),
            right: self.right.rows.begin_journal(),
            left_index: self.left.begin_index(),
            right_index: self.right.begin_index(),
            left_poisoned: self.left.poisoned,
            right_poisoned: self.right.poisoned,
        })
    }
    pub(crate) fn commit_journal_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<Checkpoint>()
            .expect("join journal checkpoint");
        self.left.rows.commit_journal(saved.left);
        self.right.rows.commit_journal(saved.right);
        self.left.commit_index(saved.left_index);
        self.right.commit_index(saved.right_index);
    }
    pub(super) fn capture_journal_checkpoint(&self, original: &dyn Any) -> Option<Box<dyn Any>> {
        let saved = original.downcast_ref::<Checkpoint>()?;
        Some(Box::new(Capture {
            left: self.left.rows.capture_current(saved.left),
            right: self.right.rows.capture_current(saved.right),
            left_index: self.left.index.clone(),
            right_index: self.right.index.clone(),
            left_poisoned: self.left.poisoned,
            right_poisoned: self.right.poisoned,
        }))
    }
    pub(crate) fn restore_checkpoint(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<Checkpoint>() {
            Ok(saved) => {
                self.left.rows.rollback_journal(saved.left);
                self.right.rows.rollback_journal(saved.right);
                self.left.rollback_index(saved.left_index);
                self.right.rollback_index(saved.right_index);
                self.left.poisoned = saved.left_poisoned;
                self.right.poisoned = saved.right_poisoned;
            }
            Err(snapshot) => match snapshot.downcast::<Capture<I, J, L, R, K>>() {
                Ok(saved) => {
                    self.left.rows.install_capture(saved.left);
                    self.right.rows.install_capture(saved.right);
                    self.left.index.install_snapshot(saved.left_index);
                    self.right.index.install_snapshot(saved.right_index);
                    self.left.poisoned = saved.left_poisoned;
                    self.right.poisoned = saved.right_poisoned;
                }
                Err(snapshot) => self.restore(snapshot),
            },
        }
    }
}

// Exact identities only: a custom Clone/Ord/Drop implementation must retain its
// original full-root ownership path. This gate adds no ID-universe restriction.
fn primitive<T: 'static>() -> bool {
    use std::any::TypeId;
    let kind = TypeId::of::<T>();
    kind == TypeId::of::<usize>()
        || kind == TypeId::of::<(usize, usize)>()
        || kind == TypeId::of::<(usize, usize, usize)>()
        || kind == TypeId::of::<((usize, usize), usize)>()
        || kind == TypeId::of::<(usize, (usize, usize))>()
        || kind == TypeId::of::<((usize, usize), (usize, usize))>()
        || kind == TypeId::of::<(usize, usize, usize, usize)>()
}

#[cfg(test)]
mod primitive_rows_tests;
