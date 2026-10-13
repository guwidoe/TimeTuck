//! Opt-in availability-polled union feeding ordinary native move providers.
use crate::{UnionOrder, random::SplitMix64};
use std::iter::{FusedIterator, Peekable};

/// Separate availability observation from drawing a move. A false poll retires
/// this cursor permanently within one union. Calls must return cooperatively.
/// A successful poll normally guarantees a move; an unexpected empty draw also
/// retires the cursor. External effects and caught panics cannot be rolled back.
pub trait AvailableMoveCursor {
    type Move;
    fn poll_available(&mut self) -> bool;
    fn draw_move(&mut self) -> Option<Self::Move>;
}
/// Bridge ordinary borrowed/owned iterators to availability polling. A poll uses
/// one-item lookahead, which may generate/buffer a candidate but never scores it.
/// Repeated polls of that buffer do not call the child iterator again.
pub struct IteratorMoveCursor<I: Iterator>(Peekable<I>);
impl<I: Iterator> IteratorMoveCursor<I> {
    pub fn new(iterator: I) -> Self {
        Self(iterator.peekable())
    }
}
impl<I: Iterator> AvailableMoveCursor for IteratorMoveCursor<I> {
    type Move = I::Item;
    fn poll_available(&mut self) -> bool {
        self.0.peek().is_some()
    }
    fn draw_move(&mut self) -> Option<Self::Move> {
        self.0.next()
    }
}

/// Prime every enabled child exactly once at construction, then draw one family
/// and poll only that selected child after consuming its move. has_next reads
/// cached availability without polling or drawing. Weights choose families;
/// internal child order and duplicate occurrences are retained. Zero weights
/// neither poll nor draw. Native seeded selection does not match Java PRNG traces.
///
/// Each productive next draws exactly one child and performs one availability
/// poll; exhausted cursors are dropped immediately and never reopened. Ordinary
/// iterator adapters may prefetch one candidate per enabled child. This changes
/// generator timing, so ProviderUnion enables it only by explicit opt-in.
/// Infinite sources need a consumer/search budget. A caught callback panic fuses
/// the union; a move consumed before a post-draw poll panic cannot be recovered.
/// Family selection scans live weights; ordinary MoveUnion paths are unchanged.
pub struct PrimedMoveUnion<C: AvailableMoveCursor> {
    children: Vec<Option<(C, u32)>>,
    total: u128,
    random: Option<SplitMix64>,
    ended: bool,
}
impl<C: AvailableMoveCursor> PrimedMoveUnion<C> {
    pub fn new(children: impl IntoIterator<Item = (C, u32)>, order: UnionOrder) -> Self {
        let mut total = 0;
        let children = children
            .into_iter()
            .filter(|(_, w)| *w != 0)
            .map(|(mut child, weight)| {
                if child.poll_available() {
                    total += u128::from(weight);
                    Some((child, weight))
                } else {
                    None
                }
            })
            .collect();
        Self {
            children,
            total,
            random: match order {
                UnionOrder::Sequential => None,
                UnionOrder::Random { seed } => Some(SplitMix64::new(seed)),
            },
            ended: false,
        }
    }
    pub fn has_next(&self) -> bool {
        !self.ended && self.total != 0
    }
    fn retire(&mut self, index: usize) {
        let (_, weight) = self.children[index].take().expect("live selected child");
        self.total -= u128::from(weight);
    }
}
impl<I: Iterator> PrimedMoveUnion<IteratorMoveCursor<I>> {
    pub fn from_iterators(children: impl IntoIterator<Item = (I, u32)>, order: UnionOrder) -> Self {
        Self::new(
            children
                .into_iter()
                .map(|(cursor, weight)| (IteratorMoveCursor::new(cursor), weight)),
            order,
        )
    }
}
impl<C: AvailableMoveCursor> Iterator for PrimedMoveUnion<C> {
    type Item = C::Move;
    fn next(&mut self) -> Option<Self::Item> {
        if !self.has_next() {
            self.ended = true;
            return None;
        }
        self.ended = true;
        while self.total != 0 {
            let mut rank = self
                .random
                .as_mut()
                .map_or(0, |random| random.below_wide(self.total));
            let index = self
                .children
                .iter()
                .position(|child| {
                    let Some((_, weight)) = child else {
                        return false;
                    };
                    if rank < u128::from(*weight) {
                        true
                    } else {
                        rank -= u128::from(*weight);
                        false
                    }
                })
                .expect("positive live weight");
            let child = &mut self.children[index].as_mut().unwrap().0;
            let Some(movement) = child.draw_move() else {
                self.retire(index);
                continue;
            };
            if !child.poll_available() {
                self.retire(index)
            }
            self.ended = false;
            return Some(movement);
        }
        None
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, (!self.has_next()).then_some(0))
    }
}
impl<C: AvailableMoveCursor> FusedIterator for PrimedMoveUnion<C> {}
