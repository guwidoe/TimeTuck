// Adapted from FilteringMoveSelectorTest.bailOutByTermination.
// Timefold Community, Apache-2.0; see compat/provenance.json and notices.
use std::{cell::Cell, num::NonZeroUsize, rc::Rc};
use timetuck::*;
struct RepeatingMoves(Rc<Cell<usize>>);
impl SelectionSource<(), ()> for RepeatingMoves {
    fn is_never_ending(&self) -> bool {
        true
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(1000)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = ()> + 'a>
    where
        (): 'a,
    {
        Box::new(std::iter::repeat_with(move || self.0.set(self.0.get() + 1)))
    }
}
#[test]
fn bail_out_by_termination() {
    let reads = Rc::new(Cell::new(0));
    let polls = Cell::new(0);
    let mut selector =
        FilteredSelectionSource::new(RepeatingMoves(reads.clone()), |_: &(), _: &()| false)
            .with_termination(NonZeroUsize::new(1000).unwrap(), || {
                polls.set(polls.get() + 1);
                polls.get() == 2
            });
    selector.event(SelectionLifecycleEvent::PhaseStarted, &());
    assert_eq!(selector.open(&()).next(), None);
    assert_eq!(reads.get(), 2000);
    assert_eq!(polls.get(), 2);
    assert_eq!(selector.stats().stop, Some(CandidateFilterStop::Terminated));
}
