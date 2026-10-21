//! Borrow complete exact numeric join scratch rows through the original route.
//! Four direction/type tags share one pointer; Option encodes completion without
//! widening the original 16-byte carrier or adding graph/workspace metadata.
use super::*;
use std::any::TypeId;
type Triple = (usize, usize, usize);
type Quad = (usize, usize, usize, usize);
type Pair = (usize, usize);
type Change = TupleChange<Pair, (Triple, Triple)>;
type MemberChange = TupleChange<(Pair, Pair), (Quad, Quad)>;
pub(super) fn supports<I: 'static, J: 'static, L: 'static, R: 'static>() -> bool {
    TypeId::of::<I>() == TypeId::of::<usize>()
        && TypeId::of::<J>() == TypeId::of::<usize>()
        && TypeId::of::<L>() == TypeId::of::<Triple>()
        && TypeId::of::<R>() == TypeId::of::<Triple>()
}
pub(super) fn supports_member<I: 'static, J: 'static, L: 'static, R: 'static>() -> bool {
    TypeId::of::<I>() == TypeId::of::<Pair>()
        && TypeId::of::<J>() == TypeId::of::<Pair>()
        && TypeId::of::<L>() == TypeId::of::<Quad>()
        && TypeId::of::<R>() == TypeId::of::<Quad>()
}
enum Rows<'node> {
    TripleLeft(&'node mut Vec<Change>),
    TripleRight(&'node mut Vec<Change>),
    MemberLeft(&'node mut Vec<MemberChange>),
    MemberRight(&'node mut Vec<MemberChange>),
}
pub(super) struct JoinOutput<'node> {
    rows: Option<Rows<'node>>,
}
impl<'node> JoinOutput<'node> {
    pub(super) fn new(rows: &'node mut dyn Any, right: bool) -> Self {
        let rows = rows
            .downcast_mut::<Vec<Change>>()
            .expect("certified fixed join output");
        Self {
            rows: Some(if right {
                Rows::TripleRight(rows)
            } else {
                Rows::TripleLeft(rows)
            }),
        }
    }
    pub(super) fn new_member(rows: &'node mut dyn Any, right: bool) -> Self {
        let rows = rows
            .downcast_mut::<Vec<MemberChange>>()
            .expect("certified complete member output");
        Self {
            rows: Some(if right {
                Rows::MemberRight(rows)
            } else {
                Rows::MemberLeft(rows)
            }),
        }
    }
    fn right(&self) -> bool {
        matches!(self.rows, Some(Rows::TripleRight(_) | Rows::MemberRight(_)))
    }
    fn finish(&mut self) {
        fn trim<T>(rows: &mut Vec<T>) {
            debug_assert!(rows.is_empty());
            if rows.capacity() > JOIN_BUFFER_BYTES / std::mem::size_of::<T>() {
                *rows = Vec::new();
            }
        }
        match self.rows.as_mut().expect("live output carrier") {
            Rows::TripleLeft(rows) | Rows::TripleRight(rows) => trim(rows),
            Rows::MemberLeft(rows) | Rows::MemberRight(rows) => trim(rows),
        }
        self.rows = None;
    }
    pub(super) fn for_each(mut self, mut publish: impl FnMut(Event)) {
        let right = self.right();
        match self.rows.as_mut().expect("live output carrier") {
            Rows::TripleLeft(rows) | Rows::TripleRight(rows) => {
                for mut change in rows.drain(..) {
                    if right {
                        change.id = (change.id.1, change.id.0);
                        change.value = change.value.map(|(r, l)| (l, r));
                    }
                    publish(Event::new(change));
                }
            }
            Rows::MemberLeft(rows) | Rows::MemberRight(rows) => {
                for mut change in rows.drain(..) {
                    if right {
                        change.id = (change.id.1, change.id.0);
                        change.value = change.value.map(|(r, l)| (l, r));
                    }
                    publish(Event::new(change));
                }
            }
        }
        self.finish();
    }
    pub(super) fn publish(
        mut self,
        edges: &[Edge],
        pending: &mut VecDeque<(Edge, event_queue::QueuedEvent)>,
        arena: &mut Vec<Event>,
        shared: bool,
    ) {
        if shared && matches!(self.rows, Some(Rows::TripleLeft(_) | Rows::TripleRight(_))) {
            self.for_each(|output| event_queue::enqueue(edges, output, pending, arena, true));
            return;
        }
        let right = self.right();
        match self.rows.as_mut().expect("live output carrier") {
            Rows::TripleLeft(rows) => publish_triples::<false>(rows, edges, pending, arena),
            Rows::TripleRight(rows) => publish_triples::<true>(rows, edges, pending, arena),
            Rows::MemberLeft(rows) | Rows::MemberRight(rows) => {
                for mut change in rows.drain(..) {
                    if right {
                        change.id = (change.id.1, change.id.0);
                        change.value = change.value.map(|(r, l)| (l, r));
                    }
                    let owner: Rc<dyn Any> = Rc::new(change);
                    event_queue::enqueue_shared(edges, owner, pending);
                }
            }
        }
        self.finish();
    }
}
// Hoist immutable direction out of the per-row loop and keep this numeric
// publication loop outside the generic propagation frame. Original scratch Vec,
// full fields, scalar queue/FIFO/fanout and carrier finish/abandon stay intact.
// The shared-producer route above retains the original for_each path.
#[inline(never)]
fn publish_triples<const RIGHT: bool>(
    rows: &mut Vec<Change>,
    edges: &[Edge],
    pending: &mut VecDeque<(Edge, event_queue::QueuedEvent)>,
    arena: &mut Vec<Event>,
) {
    for mut change in rows.drain(..) {
        if RIGHT {
            change.id = (change.id.1, change.id.0);
            change.value = change.value.map(|(r, l)| (l, r));
        }
        match Event::new(change) {
            Event::Pair(row) => {
                if !edges.is_empty() {
                    let index = arena.len();
                    arena.push(Event::Pair(row));
                    for &edge in edges {
                        pending.push_back((edge, event_queue::QueuedEvent::Inline(index)));
                    }
                }
            }
            output => event_queue::enqueue(edges, output, pending, arena, false),
        }
    }
}
impl Drop for JoinOutput<'_> {
    fn drop(&mut self) {
        fn abandon<T>(rows: &mut Vec<T>) {
            rows.clear();
            *rows = Vec::new();
        }
        if let Some(rows) = self.rows.as_mut() {
            match rows {
                Rows::TripleLeft(rows) | Rows::TripleRight(rows) => abandon(rows),
                Rows::MemberLeft(rows) | Rows::MemberRight(rows) => abandon(rows),
            }
        }
    }
}
#[cfg(test)]
mod member_tests;
#[cfg(test)]
mod tests;
