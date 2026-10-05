use SelectionLifecycleEvent::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;
#[derive(Clone)]
struct Counters {
    opens: Rc<Cell<usize>>,
    polls: Rc<Cell<usize>>,
    drops: Rc<Cell<usize>>,
}
impl Counters {
    fn new() -> Self {
        Self {
            opens: Rc::new(Cell::new(0)),
            polls: Rc::new(Cell::new(0)),
            drops: Rc::new(Cell::new(0)),
        }
    }
}
struct ProbeProvider {
    counts: Counters,
    length: Option<usize>,
}
struct Cursor {
    counts: Counters,
    next: usize,
    length: Option<usize>,
}
impl Iterator for Cursor {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        self.counts.polls.set(self.counts.polls.get() + 1);
        if self.length == Some(self.next) {
            return None;
        }
        let result = self.next;
        self.next += 1;
        Some(result)
    }
}
impl Drop for Cursor {
    fn drop(&mut self) {
        self.counts.drops.set(self.counts.drops.get() + 1);
    }
}
impl MoveProvider<(), usize, usize> for ProbeProvider {
    fn moves<'a>(
        &'a mut self,
        _: &'a (),
        _: &'a usize,
        _: u64,
    ) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.counts.opens.set(self.counts.opens.get() + 1);
        Box::new(Cursor {
            counts: self.counts.clone(),
            next: 0,
            length: self.length,
        })
    }
}
#[test]
fn zero_budgets_are_lazy_and_early_drop_releases_the_child_without_false_exhaustion() {
    for (scan, selected, stop) in [
        (0, 10, CandidateFilterStop::ScanLimit),
        (10, 0, CandidateFilterStop::SelectionLimit),
    ] {
        let counts = Counters::new();
        let mut p = FilteredMoveProvider::new(
            ProbeProvider {
                counts: counts.clone(),
                length: None,
            },
            scan,
            selected,
            |_: &(), _: &usize, _: u64, _: &usize| true,
        );
        let mut it = p.moves(&(), &0, 0);
        assert_eq!(it.next(), None);
        assert_eq!(it.next(), None);
        drop(it);
        assert_eq!(counts.opens.get(), 0);
        assert_eq!(counts.polls.get(), 0);
        assert_eq!(p.stats().stop, Some(stop));
    }
    let counts = Counters::new();
    let mut p = FilteredMoveProvider::new(
        ProbeProvider {
            counts: counts.clone(),
            length: None,
        },
        10,
        10,
        |_: &(), _: &usize, _: u64, _: &usize| true,
    );
    drop(p.moves(&(), &0, 0));
    assert_eq!(counts.opens.get(), 0);
    let mut it = p.moves(&(), &0, 0);
    assert_eq!(it.next(), Some(0));
    drop(it);
    assert_eq!(counts.drops.get(), 1);
    assert_eq!(counts.polls.get(), 1);
    assert_eq!(
        p.stats(),
        CandidateFilterStats {
            scanned: 1,
            rejected: 0,
            yielded: 1,
            stop: None
        }
    );
}
#[test]
fn scan_and_selection_limits_count_different_work_and_reset_per_step() {
    let contexts = Rc::new(RefCell::new(vec![]));
    let seen = contexts.clone();
    let counts = Counters::new();
    let mut p = FilteredMoveProvider::new(
        ProbeProvider {
            counts: counts.clone(),
            length: None,
        },
        7,
        2,
        move |_: &(), threshold: &usize, step: u64, item: &usize| {
            seen.borrow_mut().push((*threshold, step, *item));
            item >= threshold
        },
    );
    assert_eq!(p.moves(&(), &3, 9).collect::<Vec<_>>(), [3, 4]);
    assert_eq!(
        p.stats(),
        CandidateFilterStats {
            scanned: 5,
            rejected: 3,
            yielded: 2,
            stop: Some(CandidateFilterStop::SelectionLimit)
        }
    );
    assert_eq!(
        p.moves(&(), &100, 10).collect::<Vec<_>>(),
        Vec::<usize>::new()
    );
    assert_eq!(
        p.stats(),
        CandidateFilterStats {
            scanned: 7,
            rejected: 7,
            yielded: 0,
            stop: Some(CandidateFilterStop::ScanLimit)
        }
    );
    assert_eq!(counts.polls.get(), 12);
    assert_eq!(counts.opens.get(), 2);
    assert_eq!(counts.drops.get(), 2);
    assert_eq!(
        &contexts.borrow()[..5],
        &[(3, 9, 0), (3, 9, 1), (3, 9, 2), (3, 9, 3), (3, 9, 4)]
    );
    assert!(
        contexts.borrow()[5..]
            .iter()
            .all(|&(a, s, _)| a == 100 && s == 10)
    );
}
#[test]
fn source_exhaustion_and_size_hints_do_not_poll_or_reopen_a_child() {
    let counts = Counters::new();
    let mut p = FilteredMoveProvider::new(
        ProbeProvider {
            counts: counts.clone(),
            length: Some(3),
        },
        10,
        8,
        |_: &(), _: &usize, _: u64, item: &usize| item.is_multiple_of(2),
    );
    let mut it = p.moves(&(), &0, 0);
    assert_eq!(it.size_hint(), (0, Some(8)));
    assert_eq!(counts.opens.get(), 0);
    assert_eq!(it.next(), Some(0));
    assert_eq!(it.next(), Some(2));
    assert_eq!(it.next(), None);
    assert_eq!(it.size_hint(), (0, Some(0)));
    assert_eq!(it.next(), None);
    drop(it);
    assert_eq!(counts.polls.get(), 4);
    assert_eq!(counts.opens.get(), 1);
    assert_eq!(
        p.stats(),
        CandidateFilterStats {
            scanned: 3,
            rejected: 1,
            yielded: 2,
            stop: Some(CandidateFilterStop::SourceExhausted)
        }
    );
}
struct Source {
    values: Vec<usize>,
    repeating: bool,
    opens: usize,
}
impl SelectionSource<usize, usize> for Source {
    fn is_never_ending(&self) -> bool {
        self.repeating
    }
    fn size(&mut self, _: &usize) -> Option<usize> {
        Some(self.values.len())
    }
    fn open<'a>(&'a mut self, _: &'a usize) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens += 1;
        if self.repeating {
            Box::new(self.values.iter().copied().cycle())
        } else {
            Box::new(self.values.iter().copied())
        }
    }
}
#[test]
fn filtering_size_is_unknown_until_cached_and_context_refreshes_at_the_selected_scope() {
    let source = Source {
        values: vec![0, 1, 2, 3, 4, 5],
        repeating: false,
        opens: 0,
    };
    let mut filtered =
        FilteredSelectionSource::new(source, |threshold: &usize, item: &usize| item >= threshold);
    assert_eq!(filtered.upper_bound(&2), Some(6));
    assert_eq!(filtered.size(&2), None);
    assert_eq!(filtered.source().opens, 0);
    let mut cache = SelectionCache::uniform(
        filtered,
        SelectionCacheScope::Step,
        CachedSelectionOrder::Original,
        6,
    )
    .unwrap();
    cache.event(SolveStarted, &0).unwrap();
    cache.event(PhaseStarted, &0).unwrap();
    for threshold in [2, 4, 9] {
        cache.event(StepStarted, &threshold).unwrap();
        assert_eq!(
            cache.snapshot().unwrap().items(),
            &(0..6).filter(|&i| i >= threshold).collect::<Vec<_>>()
        );
        cache.event(StepEnded, &threshold).unwrap();
    }
    assert_eq!(cache.source().source().opens, 3);
}
#[test]
fn limit_before_filter_bounds_rejections_and_limit_after_filter_bounds_matches() {
    let source = || Source {
        values: vec![0, 1, 2, 3],
        repeating: true,
        opens: 0,
    };
    let mut finite = LimitedSelectionSource::new(source(), 7);
    assert_eq!(finite.size(&0), Some(7));
    assert_eq!(finite.open(&0).collect::<Vec<_>>(), [0, 1, 2, 3, 0, 1, 2]);
    let mut filtered = FilteredSelectionSource::new(
        LimitedSelectionSource::new(source(), 7),
        |_: &usize, item: &usize| *item == 3,
    );
    assert_eq!(filtered.upper_bound(&0), Some(7));
    assert_eq!(filtered.open(&0).collect::<Vec<_>>(), [3]);
    let mut capped = LimitedSelectionSource::new(
        FilteredSelectionSource::new(source(), |_: &usize, item: &usize| *item == 3),
        2,
    );
    assert_eq!(capped.open(&0).collect::<Vec<_>>(), [3, 3]);
    // A filtered repeating source has unknown accepted size even when the child repeats.
    // Materialization must not interpret the child's distinct-value count as its length.
    let mut cache = SelectionCache::uniform(
        capped,
        SelectionCacheScope::Solve,
        CachedSelectionOrder::Original,
        2,
    )
    .unwrap();
    cache.event(SolveStarted, &0).unwrap();
    assert_eq!(cache.snapshot().unwrap().items(), [3, 3]);
    let mut zero = LimitedSelectionSource::new(source(), 0);
    assert_eq!(zero.open(&0).next(), None);
    assert_eq!(zero.source().opens, 0);
}
#[test]
fn filtered_move_providers_solve_in_both_basic_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for stream in [false, true] {
        let base = |p: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
            change_moves(p, a).unwrap().collect::<Vec<_>>()
        };
        let mut provider = FilteredMoveProvider::new(
            base,
            3,
            1,
            |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, m: &Move<i32>| {
                matches!(m, Move::Change { value: Some(3), .. })
            },
        );
        let config = LocalSearchConfig {
            max_steps: 1,
            max_move_evaluations: Some(1),
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |r| {
                SimpleScore::new(i64::from(r.1.unwrap()))
            })
            .unwrap();
            stream_local_search_from_with_provider(
                &p,
                b.build(),
                source,
                p.initial_assignment(),
                config,
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(&p, &n, p.initial_assignment(), config, &mut provider)
                .unwrap()
        };
        assert_eq!(result.best.unwrap().score, SimpleScore::new(3));
        assert_eq!(provider.stats().yielded, 1);
        assert_eq!(provider.stats().scanned, 3);
    }
}
