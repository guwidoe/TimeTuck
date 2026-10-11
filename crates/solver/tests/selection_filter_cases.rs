// Adapted from Timefold Community filtering/count-limiting selector tests.
// Apache-2.0; native sources/scopes/string identities replace Java mocks.
use SelectionLifecycleEvent::*;
use timetuck::*;
struct Source {
    values: Vec<&'static str>,
    events: Vec<SelectionLifecycleEvent>,
    opens: usize,
    sizes: usize,
}
impl Source {
    fn new(values: &[&'static str]) -> Self {
        Self {
            values: values.to_vec(),
            events: vec![],
            opens: 0,
            sizes: 0,
        }
    }
}
impl SelectionSource<&'static str, &'static str> for Source {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &&'static str) {
        self.events.push(e);
    }
    fn size(&mut self, context: &&'static str) -> Option<usize> {
        assert_eq!(*context, "e1");
        self.sizes += 1;
        Some(self.values.len())
    }
    fn open<'a>(
        &'a mut self,
        context: &'a &'static str,
    ) -> Box<dyn Iterator<Item = &'static str> + 'a>
    where
        &'static str: 'a,
    {
        assert_eq!(*context, "e1");
        self.opens += 1;
        Box::new(self.values.iter().copied())
    }
}
fn counts(source: &Source, builds: usize) {
    for (e, n) in [
        (SolveStarted, 1),
        (SolveEnded, 1),
        (PhaseStarted, 2),
        (PhaseEnded, 2),
        (StepStarted, 5),
        (StepEnded, 5),
    ] {
        assert_eq!(source.events.iter().filter(|&&event| event == e).count(), n);
    }
    assert_eq!(source.opens, builds);
    assert_eq!(source.sizes, builds);
}
fn codes(mut it: impl Iterator<Item = &'static str>, expected: &[&str]) {
    for value in expected {
        assert_eq!(it.next(), Some(*value));
    }
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None);
}
fn reject_three(_: &&'static str, value: &&'static str) -> bool {
    !value.ends_with('3')
}
type Filter = FilteredSelectionSource<Source, fn(&&'static str, &&'static str) -> bool>;
fn filtered(values: &[&'static str]) -> Filter {
    Filter::new(Source::new(values), reject_three)
}
fn cached_filter(scope: SelectionCacheScope, builds: usize, values: &[&'static str]) {
    let mut cache =
        SelectionCache::uniform(filtered(values), scope, CachedSelectionOrder::Original, 4)
            .unwrap();
    cache.event(SolveStarted, &"e1").unwrap();
    for steps in [2, 3] {
        cache.event(PhaseStarted, &"e1").unwrap();
        for _ in 0..steps {
            cache.event(StepStarted, &"e1").unwrap();
            assert_eq!(cache.size().unwrap(), 3);
            assert!(!cache.is_never_ending());
            codes(
                cache.iter(0).unwrap().copied(),
                &[values[0], values[1], values[3]],
            );
            cache.event(StepEnded, &"e1").unwrap();
        }
        cache.event(PhaseEnded, &"e1").unwrap();
    }
    cache.event(SolveEnded, &"e1").unwrap();
    counts(cache.source().source(), builds);
}
fn raw_filter(values: &[&'static str]) {
    let mut source = filtered(values);
    source.event(SolveStarted, &"e1");
    for steps in [2, 3] {
        source.event(PhaseStarted, &"e1");
        for _ in 0..steps {
            source.event(StepStarted, &"e1");
            // Upstream getSize reports the unfiltered size; native API labels it an upper bound.
            assert_eq!(source.upper_bound(&"e1"), Some(4));
            assert!(!source.is_never_ending());
            codes(source.open(&"e1"), &[values[0], values[1], values[3]]);
            source.event(StepEnded, &"e1");
        }
        source.event(PhaseEnded, &"e1");
    }
    source.event(SolveEnded, &"e1");
    counts(source.source(), 5);
}
macro_rules! filter_cases {
    ($solve:ident,$phase:ident,$step:ident,$raw:ident,$values:expr) => {
        #[test]
        fn $solve() {
            cached_filter(SelectionCacheScope::Solve, 1, $values);
        }
        #[test]
        fn $phase() {
            cached_filter(SelectionCacheScope::Phase, 2, $values);
        }
        #[test]
        fn $step() {
            cached_filter(SelectionCacheScope::Step, 5, $values);
        }
        #[test]
        fn $raw() {
            raw_filter($values);
        }
    };
}
filter_cases!(
    filter_entity_solve,
    filter_entity_phase,
    filter_entity_step,
    filter_entity_uncached,
    &["e1", "e2", "e3", "e4"]
);
filter_cases!(
    filter_move_solve,
    filter_move_phase,
    filter_move_step,
    filter_move_uncached,
    &["a1", "a2", "a3", "a4"]
);
#[test]
fn filter_iterable_value() {
    raw_filter(&["v1", "v2", "v3", "v4"]);
}
#[test]
fn filter_entity_dependent_value() {
    raw_filter(&["v1", "v2", "v3", "v4"]);
}

#[test]
fn filter_entity_ordered_uncached() {
    // Despite its Solver suffix, the source method specifies JUST_IN_TIME.
    let mut source = filtered(&["e1", "e2", "e3", "e4"]);
    source.event(SolveStarted, &"e1");
    for steps in [2, 3] {
        source.event(PhaseStarted, &"e1");
        for _ in 0..steps {
            source.event(StepStarted, &"e1");
            assert_eq!(source.upper_bound(&"e1"), Some(4));
            assert!(!source.is_never_ending());
            {
                let mut cursor = source.cursor(&"e1", 16);
                for expected in ["e1", "e2", "e4"] {
                    assert!(cursor.has_next());
                    assert_eq!(cursor.next_item(), Some(&expected));
                }
                assert!(!cursor.has_next());
                assert_eq!(cursor.next_item(), None);
                for expected in ["e4", "e2", "e1"] {
                    assert!(cursor.has_previous());
                    assert_eq!(cursor.previous_item(), Some(&expected));
                }
                assert!(!cursor.has_previous());
                assert_eq!(cursor.previous_item(), None);
            }
            source.event(StepEnded, &"e1");
        }
        source.event(PhaseEnded, &"e1");
    }
    source.event(SolveEnded, &"e1");
    counts(source.source(), 5);
}
fn limited(values: &[&'static str], maximum: usize) {
    let mut source = LimitedSelectionSource::new(Source::new(values), maximum);
    source.event(SolveStarted, &"e1");
    for steps in [2, 3] {
        source.event(PhaseStarted, &"e1");
        for _ in 0..steps {
            source.event(StepStarted, &"e1");
            assert_eq!(source.size(&"e1"), Some(3));
            assert!(!source.is_never_ending());
            codes(source.open(&"e1"), &values[..3]);
            source.event(StepEnded, &"e1");
        }
        source.event(PhaseEnded, &"e1");
    }
    source.event(SolveEnded, &"e1");
    counts(source.source(), 5);
}
macro_rules! limit_cases {
    ($lower:ident,$higher:ident,$values:expr) => {
        #[test]
        fn $lower() {
            limited($values, 3);
        }
        #[test]
        fn $higher() {
            limited(&$values[..3], 5);
        }
    };
}
limit_cases!(
    limit_entity_lower,
    limit_entity_higher,
    &["e1", "e2", "e3", "e4", "e5"]
);
limit_cases!(
    limit_move_lower,
    limit_move_higher,
    &["a1", "a2", "a3", "a4", "a5"]
);
limit_cases!(
    limit_value_lower,
    limit_value_higher,
    &["v1", "v2", "v3", "v4", "v5"]
);
limit_cases!(
    limit_entity_dependent_value_lower,
    limit_entity_dependent_value_higher,
    &["v1", "v2", "v3", "v4", "v5"]
);
#[test]
fn limit_entity_is_never_ending() {
    assert!(!LimitedSelectionSource::new(Source::new(&[]), 5).is_never_ending());
}
#[test]
fn limit_entity_size() {
    let mut s = LimitedSelectionSource::new(Source::new(&["e1"]), 5);
    for n in [1, 5, 10] {
        s.source_mut().values = vec!["e1"; n];
        assert_eq!(s.size(&"e1"), Some(n.min(5)));
    }
}
#[test]
fn limit_entity_ending_original() {
    let mut s = LimitedSelectionSource::new(Source::new(&["e1", "e2", "e3", "e4"]), 2);
    codes(s.open(&"e1"), &["e1", "e2"]);
}
