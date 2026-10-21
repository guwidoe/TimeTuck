//! Borrow completed fixed-join scratch rows without building an erased Vec.
//! Only exact builtin Copy rows use this representation; public/custom/batch and
//! unbounded joins retain the original Outputs path. No input event is retained.
use super::*;
use std::any::TypeId;

type Triple = (usize, usize, usize);
type Change = TupleChange<(usize, usize), (Triple, Triple)>;

pub(super) fn supports<I: 'static, J: 'static, L: 'static, R: 'static>() -> bool {
    TypeId::of::<I>() == TypeId::of::<usize>()
        && TypeId::of::<J>() == TypeId::of::<usize>()
        && TypeId::of::<L>() == TypeId::of::<Triple>()
        && TypeId::of::<R>() == TypeId::of::<Triple>()
}

pub(super) struct JoinOutput<'node> {
    rows: &'node mut Vec<Change>,
    right: bool,
    finished: bool,
}
impl<'node> JoinOutput<'node> {
    pub(super) fn new(rows: &'node mut dyn Any, right: bool) -> Self {
        Self {
            rows: rows
                .downcast_mut::<Vec<Change>>()
                .expect("certified fixed join output"),
            right,
            finished: false,
        }
    }
    pub(super) fn for_each(mut self, mut publish: impl FnMut(Event)) {
        for mut change in self.rows.drain(..) {
            if self.right {
                change.id = (change.id.1, change.id.0);
                change.value = change.value.map(|(right, left)| (left, right));
            }
            publish(Event::new(change));
        }
        self.finished = true;
    }
}
impl Drop for JoinOutput<'_> {
    fn drop(&mut self) {
        // Drain's own unwinding drops all remaining staged rows first. On panic
        // release capacity just as the original local typed Vec did. A successful
        // drain retains only the original bounded empty scratch allocation.
        self.rows.clear();
        if !self.finished
            || self.rows.capacity() > JOIN_BUFFER_BYTES / std::mem::size_of::<Change>()
        {
            *self.rows = Vec::new();
        }
    }
}

#[cfg(test)]
mod tests;
