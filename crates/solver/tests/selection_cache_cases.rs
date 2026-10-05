// Adapted from Community caching/probability selector tests and PlannerAssert.
// Apache-2.0. Native lifecycle events replace mocked Java scopes; see provenance.
use SelectionLifecycleEvent::*;
use std::cell::Cell;
use timetuck::*;
struct Source {
    values: Vec<&'static str>,
    queries: Cell<usize>,
    sizes: usize,
    opens: usize,
    events: Vec<SelectionLifecycleEvent>,
    infinite: bool,
}
impl Source {
    fn new(values: &[&'static str]) -> Self {
        Self {
            values: values.to_vec(),
            queries: Cell::new(0),
            sizes: 0,
            opens: 0,
            events: vec![],
            infinite: false,
        }
    }
}
impl SelectionSource<(), &'static str> for Source {
    fn is_never_ending(&self) -> bool {
        self.queries.set(self.queries.get() + 1);
        self.infinite
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
type Cache = SelectionCache<(), &'static str, Source, fn(&(), &&'static str) -> f64>;
fn cache(
    values: &[&'static str],
    scope: SelectionCacheScope,
    order: CachedSelectionOrder,
) -> Cache {
    Cache::uniform(Source::new(values), scope, order, 100).unwrap()
}
fn event(cache: &mut Cache, event: SelectionLifecycleEvent) {
    cache.event(event, &()).unwrap();
}
fn lifecycle(source: &Source, phases: usize, steps: usize, builds: usize) {
    for (event, count) in [
        (SolveStarted, 1),
        (SolveEnded, 1),
        (PhaseStarted, phases),
        (PhaseEnded, phases),
        (StepStarted, steps),
        (StepEnded, steps),
    ] {
        assert_eq!(source.events.iter().filter(|&&e| e == event).count(), count);
    }
    assert_eq!(source.opens, builds);
    assert_eq!(source.sizes, builds);
}
fn original(scope: SelectionCacheScope, builds: usize, values: &[&'static str]) {
    let mut c = cache(values, scope, CachedSelectionOrder::Original);
    assert_eq!(c.source().queries.get(), 1);
    event(&mut c, SolveStarted);
    for steps in [2, 3] {
        event(&mut c, PhaseStarted);
        for _ in 0..steps {
            event(&mut c, StepStarted);
            let mut it = c.iter(0).unwrap();
            for value in values {
                assert!(it.has_next());
                assert_eq!(it.next(), Some(value));
            }
            assert!(!it.has_next());
            assert_eq!(it.next(), None);
            assert_eq!(it.next(), None);
            drop(it);
            assert!(!c.is_never_ending());
            assert_eq!(c.size().unwrap(), values.len());
            event(&mut c, StepEnded);
        }
        event(&mut c, PhaseEnded);
    }
    event(&mut c, SolveEnded);
    lifecycle(c.source(), 2, 5, builds);
}
macro_rules! original_cases {
    ($solve:ident,$phase:ident,$step:ident,$values:expr) => {
        #[test]
        fn $solve() {
            original(SelectionCacheScope::Solve, 1, $values);
        }
        #[test]
        fn $phase() {
            original(SelectionCacheScope::Phase, 2, $values);
        }
        #[test]
        fn $step() {
            original(SelectionCacheScope::Step, 5, $values);
        }
    };
}
original_cases!(
    entity_original_solve,
    entity_original_phase,
    entity_original_step,
    &["e1", "e2", "e3"]
);
original_cases!(
    value_original_solve,
    value_original_phase,
    value_original_step,
    &["v1", "v2", "v3"]
);
original_cases!(
    move_original_solve,
    move_original_phase,
    move_original_step,
    &["a1", "a2", "a3"]
);
fn random_prefix(c: &Cache, draws: &[f64], expected: &[&str], size: usize) {
    let mut draws = draws.iter().copied();
    let mut it = c.iter_with(move || draws.next().unwrap()).unwrap();
    for value in expected {
        assert!(it.has_next());
        assert!(it.has_next());
        assert_eq!(it.next().copied(), Some(*value));
    }
    assert!(it.has_next());
    assert!(c.is_never_ending());
    assert_eq!(c.size().unwrap(), size);
}
fn random_moves(scope: SelectionCacheScope, builds: usize) {
    let mut c = cache(&["a1", "a2", "a3"], scope, CachedSelectionOrder::Random);
    assert_eq!(c.source().queries.get(), 1);
    event(&mut c, SolveStarted);
    event(&mut c, PhaseStarted);
    event(&mut c, StepStarted);
    random_prefix(&c, &[0.5, 1.0 / 6.0, 5.0 / 6.0], &["a2", "a1", "a3"], 3);
    event(&mut c, StepEnded);
    event(&mut c, StepStarted);
    random_prefix(&c, &[5.0 / 6.0, 1.0 / 6.0, 0.5], &["a3", "a1", "a2"], 3);
    event(&mut c, StepEnded);
    event(&mut c, PhaseEnded);
    event(&mut c, PhaseStarted);
    event(&mut c, StepStarted);
    random_prefix(&c, &[0.5, 5.0 / 6.0, 1.0 / 6.0], &["a2", "a3", "a1"], 3);
    event(&mut c, StepEnded);
    event(&mut c, PhaseEnded);
    event(&mut c, SolveEnded);
    lifecycle(c.source(), 2, 3, builds);
}
#[test]
fn move_random_solve() {
    random_moves(SelectionCacheScope::Solve, 1);
}
#[test]
fn move_random_phase() {
    random_moves(SelectionCacheScope::Phase, 2);
}
#[test]
fn move_random_step() {
    random_moves(SelectionCacheScope::Step, 3);
}
#[test]
fn entity_list_iterator_with_random_selection() {
    let c = cache(
        &[],
        SelectionCacheScope::Phase,
        CachedSelectionOrder::Random,
    );
    assert!(matches!(
        c.list_iter(),
        Err(SelectionCacheError::RandomListAccess)
    ));
}
#[test]
fn entity_indexed_list_iterator_with_random_selection() {
    let c = cache(
        &[],
        SelectionCacheScope::Phase,
        CachedSelectionOrder::Random,
    );
    assert!(matches!(
        c.list_iter_from(0),
        Err(SelectionCacheError::RandomListAccess)
    ));
}
#[test]
fn entity_is_never_ending() {
    for order in [CachedSelectionOrder::Original, CachedSelectionOrder::Random] {
        let c = cache(&[], SelectionCacheScope::Phase, order);
        assert_eq!(c.is_never_ending(), order == CachedSelectionOrder::Random);
    }
}
fn weight(_: &(), value: &&'static str) -> f64 {
    match *value {
        "e1" => 1000.0,
        "e2" => 200.0,
        "e3" => 30.0,
        "e4" => 4.0,
        _ => panic!("unknown code"),
    }
}
fn probability() {
    let mut c = Cache::new(
        Source::new(&["e1", "e2", "e3", "e4"]),
        SelectionCacheScope::Step,
        CachedSelectionOrder::Random,
        4,
        weight,
    )
    .unwrap();
    event(&mut c, SolveStarted);
    event(&mut c, PhaseStarted);
    event(&mut c, StepStarted);
    random_prefix(
        &c,
        &[
            1222.0 / 1234.0,
            111.0 / 1234.0,
            0.0,
            1230.0 / 1234.0,
            1199.0 / 1234.0,
        ],
        &["e3", "e1", "e1", "e4", "e2"],
        4,
    );
    event(&mut c, StepEnded);
    event(&mut c, PhaseEnded);
    event(&mut c, SolveEnded);
    lifecycle(c.source(), 1, 1, 1);
}
#[test]
fn probability_entity_random_selection() {
    probability();
}
#[test]
fn probability_move_random_selection() {
    probability();
}
#[test]
fn probability_entity_is_never_ending() {
    assert!(cache(&[], SelectionCacheScope::Step, CachedSelectionOrder::Random).is_never_ending());
}
#[test]
fn probability_entity_with_never_ending_selection() {
    let mut source = Source::new(&[]);
    source.infinite = true;
    assert!(matches!(
        Cache::uniform(
            source,
            SelectionCacheScope::Step,
            CachedSelectionOrder::Random,
            10
        ),
        Err(SelectionCacheError::UnboundedSource)
    ));
}
#[test]
fn probability_entity_without_cached_selection_type() {
    assert!(matches!(
        Cache::uniform(
            Source::new(&[]),
            SelectionCacheScope::Uncached,
            CachedSelectionOrder::Random,
            10
        ),
        Err(SelectionCacheError::UncachedScope)
    ));
}
