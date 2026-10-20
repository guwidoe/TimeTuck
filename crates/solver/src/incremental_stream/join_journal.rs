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
            Err(snapshot) => self.restore(snapshot),
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
