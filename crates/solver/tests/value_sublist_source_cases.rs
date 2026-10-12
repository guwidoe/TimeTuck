// Adapted from pinned Community RandomSubListSelectorTest and the random
// sublist change/swap selector lifecycle tests, Apache-2.0.
// See compat/provenance.json for identities, notices and native substitutions.
use std::{cell::RefCell, num::NonZeroUsize, rc::Rc};
use timetuck::*;
#[derive(Clone)]
struct Source<T> {
    rows: Vec<T>,
    events: Rc<RefCell<Vec<SelectionLifecycleEvent>>>,
    repeating: bool,
}
impl<T> Source<T> {
    fn new(rows: Vec<T>, repeating: bool) -> Self {
        Self {
            rows,
            events: Default::default(),
            repeating,
        }
    }
}
impl<C, T: Clone> SelectionSource<C, T> for Source<T> {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &C) {
        self.events.borrow_mut().push(e);
    }
    fn is_never_ending(&self) -> bool {
        self.repeating
    }
    fn size(&mut self, _: &C) -> Option<usize> {
        Some(self.rows.len())
    }
    fn open<'a>(&'a mut self, _: &'a C) -> Box<dyn Iterator<Item = T> + 'a>
    where
        T: 'a,
    {
        if self.repeating {
            Box::new(self.rows.iter().cloned().cycle())
        } else {
            Box::new(self.rows.iter().cloned())
        }
    }
}
fn limits() -> ValueSubListSourceLimits {
    ValueSubListSourceLimits {
        entity_reads: 100,
        seed_reads_per_item: NonZeroUsize::new(100).unwrap(),
    }
}
fn events() -> [SelectionLifecycleEvent; 8] {
    use SelectionLifecycleEvent::*;
    [
        SolveStarted,
        PhaseStarted,
        StepStarted,
        StepEnded,
        StepStarted,
        StepEnded,
        PhaseEnded,
        SolveEnded,
    ]
}
fn lifecycle<C, T>(source: &mut impl SelectionSource<C, T>, context: &C) {
    for e in events() {
        source.event(e, context);
    }
}
fn empty() -> ListProblem<i32> {
    ListProblem::new(vec![], vec![]).unwrap()
}
#[test]
fn empty_when_minimum_sublist_size_greater_than_list_size() {
    let p = ListProblem::new(vec![1, 2, 3], vec![ListEntity::new(vec![1, 2, 3])]).unwrap();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let mut source = ValueSubListSource::new(
        Source::new(vec![0], false),
        Source::new(vec![], true),
        SubListSize::new(4, usize::MAX).unwrap(),
        limits(),
        0,
    );
    assert!(repeats(&source, &c));
    assert_eq!(source.span_count(&c), 0);
    assert_eq!(source.size(&c), Some(0));
    let mut cursor = source.open(&c);
    assert_eq!(cursor.next(), None);
    assert_eq!(cursor.next(), None);
}
#[test]
fn sublist_phase_lifecycle() {
    let p = empty();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let entities = Source::new(vec![], false);
    let seeds = Source::<i32>::new(vec![], true);
    let e = entities.events.clone();
    let s = seeds.events.clone();
    let mut source = ValueSubListSource::new(
        entities,
        seeds,
        SubListSize::new(1, usize::MAX).unwrap(),
        limits(),
        0,
    );
    lifecycle(&mut source, &c);
    assert_eq!(*e.borrow(), events());
    assert_eq!(*s.borrow(), events());
}
#[test]
fn change_phase_lifecycle() {
    let p = empty();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let entities = Source::new(vec![], false);
    let seeds = Source::<i32>::new(vec![], true);
    let destinations = Source::<Option<ListPosition>>::new(vec![], true);
    let e = entities.events.clone();
    let s = seeds.events.clone();
    let d = destinations.events.clone();
    let spans = ValueSubListSource::new(
        entities,
        seeds,
        SubListSize::new(1, usize::MAX).unwrap(),
        limits(),
        0,
    );
    let mut source = SubListChangeSource::new(spans, destinations, false, 0);
    lifecycle(&mut source, &c);
    for trace in [e, s, d] {
        assert_eq!(*trace.borrow(), events());
    }
}
#[test]
fn swap_phase_lifecycle() {
    let p = empty();
    let a = p.initial_assignment();
    let c = ListSelectionContext::new(&p, &a).unwrap();
    let entities = Source::new(vec![], false);
    let left = Source::<i32>::new(vec![], true);
    let right = Source::<i32>::new(vec![], true);
    let e = entities.events.clone();
    let l = left.events.clone();
    let r = right.events.clone();
    let bounds = SubListSize::new(1, usize::MAX).unwrap();
    let left = ValueSubListSource::new(entities.clone(), left, bounds, limits(), 0);
    let right = ValueSubListSource::new(entities, right, bounds, limits(), 0);
    let mut source = SubListSwapSource::new(left, right, false, 0);
    lifecycle(&mut source, &c);
    let expected: Vec<_> = events().into_iter().flat_map(|e| [e, e]).collect();
    assert_eq!(*e.borrow(), expected);
    assert_eq!(*l.borrow(), events());
    assert_eq!(*r.borrow(), events());
}
#[test]
fn validate_constructor_arguments() {
    assert_eq!(
        SubListSize::new(0, 5),
        Err(ListError::InvalidSampleSize {
            minimum: 0,
            maximum: 5
        })
    );
    assert_eq!(
        SubListSize::new(2, 1),
        Err(ListError::InvalidSampleSize {
            minimum: 2,
            maximum: 1
        })
    );
    let _source = ValueSubListSource::new(
        Source::<usize>::new(vec![], false),
        Source::<i32>::new(vec![], true),
        SubListSize::new(1, 1).unwrap(),
        limits(),
        0,
    );
}

fn repeats<C>(s: &impl SelectionSource<C, ListRange>, _: &C) -> bool {
    s.is_never_ending()
}
