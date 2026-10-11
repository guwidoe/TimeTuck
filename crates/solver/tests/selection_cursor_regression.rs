//! Original bounded and move-only reversible filtering regressions.
use std::{cell::Cell, rc::Rc};
use timetuck::*;
struct Token {
    value: usize,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Source {
    opens: Rc<Cell<usize>>,
    reads: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
}
impl SelectionSource<(), Token> for Source {
    fn size(&mut self, _: &()) -> Option<usize> {
        None
    }
    fn is_never_ending(&self) -> bool {
        true
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Token> + 'a>
    where
        Token: 'a,
    {
        self.opens.set(self.opens.get() + 1);
        let reads = self.reads.clone();
        let drops = self.drops.clone();
        Box::new((0..).map(move |value| {
            reads.set(reads.get() + 1);
            Token {
                value,
                drops: drops.clone(),
            }
        }))
    }
}
#[test]
fn backward_and_repeated_probes_reuse_move_only_occurrences_without_filtering_again() {
    let opens = Rc::new(Cell::new(0));
    let reads = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let predicates = Cell::new(0);
    let child = Source {
        opens: opens.clone(),
        reads: reads.clone(),
        drops: drops.clone(),
    };
    let mut filter = FilteredSelectionSource::new(child, |_: &(), t: &Token| {
        predicates.set(predicates.get() + 1);
        t.value.is_multiple_of(2)
    });
    {
        let mut cursor = filter.cursor(&(), 5);
        assert_eq!(opens.get(), 0);
        assert!(!cursor.has_previous());
        assert!(cursor.has_next());
        assert!(cursor.has_next());
        assert_eq!(reads.get(), 1);
        assert_eq!(cursor.next_item().map(|t| t.value), Some(0));
        assert_eq!(cursor.next_item().map(|t| t.value), Some(2));
        assert_eq!(cursor.previous_item().map(|t| t.value), Some(2));
        assert_eq!(cursor.previous_item().map(|t| t.value), Some(0));
        assert_eq!(cursor.previous_item().map(|t| t.value), None);
        assert_eq!(cursor.next_item().map(|t| t.value), Some(0));
        assert_eq!(cursor.next_item().map(|t| t.value), Some(2));
        assert_eq!(reads.get(), 3);
        assert_eq!(cursor.next_item().map(|t| t.value), Some(4));
        assert_eq!(cursor.position(), 3);
        assert!(!cursor.has_next());
        assert!(!cursor.has_next());
        assert_eq!(reads.get(), 5);
        assert_eq!(predicates.get(), 5);
        for expected in [4, 2, 0] {
            assert_eq!(cursor.previous_item().map(|t| t.value), Some(expected));
        }
        assert_eq!(cursor.position(), 0);
        assert_eq!(drops.get(), 2); // Only rejected occurrences released so far.
    }
    assert_eq!(opens.get(), 1);
    assert_eq!(drops.get(), 5);
    assert_eq!(filter.stats().stop, Some(CandidateFilterStop::ScanLimit));
}
#[test]
fn zero_and_all_rejected_traversals_stop_without_unbounded_hidden_work() {
    for maximum in [0, 17] {
        let opens = Rc::new(Cell::new(0));
        let reads = Rc::new(Cell::new(0));
        let drops = Rc::new(Cell::new(0));
        let child = Source {
            opens: opens.clone(),
            reads: reads.clone(),
            drops: drops.clone(),
        };
        let mut filter = FilteredSelectionSource::new(child, |_: &(), _: &Token| false);
        {
            let mut cursor = filter.cursor(&(), maximum);
            assert!(!cursor.has_next());
            assert!(!cursor.has_next());
            assert_eq!(cursor.next_item().map(|t| t.value), None);
            assert_eq!(cursor.previous_item().map(|t| t.value), None);
            assert_eq!(cursor.position(), 0);
        }
        assert_eq!(opens.get(), usize::from(maximum > 0));
        assert_eq!(reads.get(), maximum);
        assert_eq!(drops.get(), maximum);
        assert_eq!(filter.stats().scanned, maximum);
        assert_eq!(filter.stats().stop, Some(CandidateFilterStop::ScanLimit));
    }
}
