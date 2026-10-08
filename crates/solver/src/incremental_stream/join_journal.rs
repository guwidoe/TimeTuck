//! Graph trial checkpoints avoid sharing row roots until a snapshot is requested.
use super::*;
struct Checkpoint<I, J, K> {
    left: usize,
    right: usize,
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
    pub(crate) fn begin_journal_checkpoint(&mut self) -> Box<dyn Any> {
        healthy(self.is_poisoned());
        Box::new(Checkpoint {
            left: self.left.rows.begin_journal(),
            right: self.right.rows.begin_journal(),
            left_index: self.left.index.clone(),
            right_index: self.right.index.clone(),
            left_poisoned: self.left.poisoned,
            right_poisoned: self.right.poisoned,
        })
    }
    pub(crate) fn commit_journal_checkpoint(&mut self, saved: Box<dyn Any>) {
        let saved = saved
            .downcast::<Checkpoint<I, J, K>>()
            .expect("join journal checkpoint");
        self.left.rows.commit_journal(saved.left);
        self.right.rows.commit_journal(saved.right);
    }
    pub(crate) fn restore_checkpoint(&mut self, saved: Box<dyn Any>) {
        match saved.downcast::<Checkpoint<I, J, K>>() {
            Ok(saved) => {
                self.left.rows.rollback_journal(saved.left);
                self.right.rows.rollback_journal(saved.right);
                self.left.index = saved.left_index;
                self.right.index = saved.right_index;
                self.left.poisoned = saved.left_poisoned;
                self.right.poisoned = saved.right_poisoned;
            }
            Err(snapshot) => self.restore(snapshot),
        }
    }
}
