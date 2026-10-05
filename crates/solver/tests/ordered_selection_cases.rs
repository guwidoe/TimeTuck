// Adapted from pinned Timefold Community sorting/shuffling selector tests.
// Apache-2.0. Native scopes and string codes replace Java mocks; see provenance.
use SelectionLifecycleEvent::*;
use std::{cell::Cell, collections::VecDeque, rc::Rc};
use timetuck::*;
struct Source {
    values: Vec<&'static str>,
    queries: Cell<usize>,
    opens: usize,
    sizes: usize,
    events: Vec<SelectionLifecycleEvent>,
}
impl Source {
    fn new(values: &[&'static str]) -> Self {
        Self {
            values: values.to_vec(),
            queries: Cell::new(0),
            opens: 0,
            sizes: 0,
            events: vec![],
        }
    }
}
impl SelectionSource<(), &'static str> for Source {
    fn is_never_ending(&self) -> bool {
        self.queries.set(self.queries.get() + 1);
        false
    }
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        self.sizes += 1;
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        self.opens += 1;
        Box::new(self.values.iter().copied())
    }
}
type Cache = OrderedSelectionCache<(), &'static str, Source>;
fn event(c: &mut Cache, e: SelectionLifecycleEvent) {
    c.event(e, &()).unwrap();
}
fn codes(c: &Cache, expected: &[&str]) {
    let snapshot = c.snapshot().unwrap();
    let mut it = snapshot.iter();
    for expected in expected {
        assert_eq!(it.next().copied(), Some(*expected));
    }
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None);
    assert!(!c.is_never_ending());
    assert_eq!(c.size().unwrap(), expected.len());
}
fn lifecycle(c: &Cache, steps: usize, builds: usize) {
    for (event, count) in [
        (SolveStarted, 1),
        (SolveEnded, 1),
        (PhaseStarted, 2),
        (PhaseEnded, 2),
        (StepStarted, steps),
        (StepEnded, steps),
    ] {
        assert_eq!(
            c.source().events.iter().filter(|&&e| e == event).count(),
            count
        );
    }
    assert_eq!(c.source().opens, builds);
    assert_eq!(c.source().sizes, builds);
}
fn sorted(scope: SelectionCacheScope) -> Result<Cache, OrderedSelectionError> {
    Cache::sorted_by(
        Source::new(&["jan", "feb", "mar", "apr", "may", "jun"]),
        scope,
        6,
        |_, a, b| a.cmp(b),
    )
}
fn sorting(scope: SelectionCacheScope, builds: usize) {
    let mut c = sorted(scope).unwrap();
    event(&mut c, SolveStarted);
    for steps in [2, 3] {
        event(&mut c, PhaseStarted);
        for _ in 0..steps {
            event(&mut c, StepStarted);
            codes(&c, &["apr", "feb", "jan", "jun", "mar", "may"]);
            event(&mut c, StepEnded);
        }
        event(&mut c, PhaseEnded);
    }
    event(&mut c, SolveEnded);
    lifecycle(&c, 5, builds);
}
macro_rules! sorting_cases {
    ($solve:ident,$phase:ident,$step:ident,$jit:ident) => {
        #[test]
        fn $solve() {
            sorting(SelectionCacheScope::Solve, 1);
        }
        #[test]
        fn $phase() {
            sorting(SelectionCacheScope::Phase, 2);
        }
        #[test]
        fn $step() {
            sorting(SelectionCacheScope::Step, 5);
        }
        #[test]
        fn $jit() {
            assert!(matches!(
                sorted(SelectionCacheScope::Uncached),
                Err(OrderedSelectionError::Cache(
                    SelectionCacheError::UncachedScope
                ))
            ));
        }
    };
}
sorting_cases!(
    sort_entity_solve,
    sort_entity_phase,
    sort_entity_step,
    sort_entity_uncached
);
sorting_cases!(
    sort_move_solve,
    sort_move_phase,
    sort_move_step,
    sort_move_uncached
);
#[test]
fn sort_entity_is_never_ending() {
    assert!(
        !sorted(SelectionCacheScope::Phase)
            .unwrap()
            .is_never_ending()
    );
}
#[test]
fn shuffle_entity_is_never_ending() {
    assert!(
        !Cache::shuffled(Source::new(&[]), SelectionCacheScope::Phase, 0, 0)
            .unwrap()
            .is_never_ending()
    );
}
fn shuffling(scope: SelectionCacheScope, builds: usize) {
    let draws = Rc::new(std::cell::RefCell::new(VecDeque::from([2, 0])));
    let supplied = draws.clone();
    let mut c = Cache::shuffled_with(Source::new(&["a1", "a2", "a3"]), scope, 3, move |_| {
        supplied.borrow_mut().pop_front().unwrap()
    })
    .unwrap();
    assert_eq!(c.source().queries.get(), 1);
    event(&mut c, SolveStarted);
    event(&mut c, PhaseStarted);
    event(&mut c, StepStarted);
    codes(&c, &["a2", "a1", "a3"]);
    event(&mut c, StepEnded);
    *draws.borrow_mut() = VecDeque::from([0, 1]);
    event(&mut c, StepStarted);
    if scope == SelectionCacheScope::Step {
        codes(&c, &["a3", "a2", "a1"]);
    } else {
        codes(&c, &["a3", "a1", "a2"]);
    }
    event(&mut c, StepEnded);
    event(&mut c, PhaseEnded);
    event(&mut c, PhaseStarted);
    *draws.borrow_mut() = VecDeque::from([1, 0]);
    event(&mut c, StepStarted);
    if scope == SelectionCacheScope::Solve {
        codes(&c, &["a2", "a3", "a1"]);
    } else {
        codes(&c, &["a3", "a1", "a2"]);
    }
    event(&mut c, StepEnded);
    event(&mut c, PhaseEnded);
    event(&mut c, SolveEnded);
    lifecycle(&c, 3, builds);
    assert!(draws.borrow().is_empty());
}
#[test]
fn shuffle_move_solve() {
    shuffling(SelectionCacheScope::Solve, 1);
}
#[test]
fn shuffle_move_phase() {
    shuffling(SelectionCacheScope::Phase, 2);
}
#[test]
fn shuffle_move_step() {
    shuffling(SelectionCacheScope::Step, 3);
}
