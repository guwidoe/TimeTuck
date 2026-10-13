// Adapted from pinned Community UniformRandomUnionMoveIteratorTest, Apache-2.0.
// See compat/provenance.json and THIRD_PARTY_NOTICES.md.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
struct CountingMoveCursor {
    index: usize,
    polls: Rc<Cell<usize>>,
}
impl AvailableMoveCursor for CountingMoveCursor {
    type Move = Move<usize>;
    fn poll_available(&mut self) -> bool {
        self.polls.set(self.polls.get() + 1);
        true
    }
    // Like source CountingMoveIterator.next, drawing itself never polls.
    fn draw_move(&mut self) -> Option<Self::Move> {
        Some(Move::Change {
            variable: self.index,
            value: Some(self.index),
        })
    }
}
#[test]
fn has_next_only_polls_the_drawn_iterator() {
    let counts: [Rc<Cell<usize>>; 3] = std::array::from_fn(|_| Rc::new(Cell::new(0)));
    let children = counts.iter().enumerate().map(|(index, polls)| {
        (
            CountingMoveCursor {
                index,
                polls: polls.clone(),
            },
            1,
        )
    });
    let mut union = PrimedMoveUnion::new(children, UnionOrder::Random { seed: 0 });
    for count in &counts {
        assert_eq!(count.get(), 1);
    }
    let Move::Change {
        variable: drawn, ..
    } = union.next().unwrap()
    else {
        panic!("native labeled move")
    };
    for (index, count) in counts.iter().enumerate() {
        assert_eq!(count.get(), if index == drawn { 2 } else { 1 });
    }
}
