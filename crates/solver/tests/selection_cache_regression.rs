use SelectionLifecycleEvent::*;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Source {
    count: usize,
    declared: Option<usize>,
    events: Vec<SelectionLifecycleEvent>,
    opens: usize,
    reads: Rc<Cell<usize>>,
    panic_open: bool,
}
impl Source {
    fn new(count: usize) -> Self {
        Self {
            count,
            declared: Some(count),
            events: vec![],
            opens: 0,
            reads: Rc::new(Cell::new(0)),
            panic_open: false,
        }
    }
}
impl SelectionSource<usize, usize> for Source {
    fn event(&mut self, event: SelectionLifecycleEvent, _: &usize) {
        self.events.push(event);
    }
    fn size(&mut self, _: &usize) -> Option<usize> {
        self.declared
    }
    fn open<'a>(&'a mut self, context: &'a usize) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.opens += 1;
        assert!(!self.panic_open, "supplier failed");
        Box::new((0..self.count).map(move |i| {
            self.reads.set(self.reads.get() + 1);
            context + i
        }))
    }
}
type Cache = SelectionCache<usize, usize, Source, fn(&usize, &usize) -> f64>;
fn uniform(scope: SelectionCacheScope) -> Cache {
    Cache::uniform(Source::new(2), scope, CachedSelectionOrder::Original, 4).unwrap()
}
#[test]
fn each_scope_captures_current_context_and_releases_only_at_its_boundary() {
    for scope in [
        SelectionCacheScope::Solve,
        SelectionCacheScope::Phase,
        SelectionCacheScope::Step,
    ] {
        let mut c = uniform(scope);
        assert!(matches!(c.size(), Err(SelectionCacheError::NotReady)));
        c.event(SolveStarted, &0).unwrap();
        for phase in [10, 20] {
            c.event(PhaseStarted, &phase).unwrap();
            for step in [phase + 1, phase + 2] {
                c.event(StepStarted, &step).unwrap();
                let expected = match scope {
                    SelectionCacheScope::Solve => 0,
                    SelectionCacheScope::Phase => phase,
                    _ => step,
                };
                assert_eq!(
                    c.iter(0).unwrap().copied().collect::<Vec<_>>(),
                    [expected, expected + 1]
                );
                assert_eq!(
                    c.list_iter_from(1).unwrap().copied().collect::<Vec<_>>(),
                    [expected + 1]
                );
                assert_eq!(c.list_iter_from(2).unwrap().count(), 0);
                assert!(matches!(
                    c.list_iter_from(3),
                    Err(SelectionCacheError::IndexOutOfBounds { index: 3, len: 2 })
                ));
                let held = c.snapshot().unwrap();
                c.event(StepEnded, &step).unwrap();
                assert_eq!(held.items(), &[expected, expected + 1]);
                assert_eq!(c.size().is_ok(), scope != SelectionCacheScope::Step);
            }
            c.event(PhaseEnded, &phase).unwrap();
            assert_eq!(c.size().is_ok(), scope == SelectionCacheScope::Solve);
        }
        c.event(SolveEnded, &0).unwrap();
        assert!(c.snapshot().is_err());
        assert_eq!(c.state(), SelectionCacheState::Idle);
        assert_eq!(
            c.source().opens,
            match scope {
                SelectionCacheScope::Solve => 1,
                SelectionCacheScope::Phase => 2,
                _ => 4,
            }
        );
    }
}
#[test]
fn invalid_transitions_do_not_forward_and_failed_builds_can_be_closed_and_retried() {
    let mut c = uniform(SelectionCacheScope::Step);
    assert!(matches!(
        c.event(StepStarted, &0),
        Err(SelectionCacheError::InvalidTransition { .. })
    ));
    assert!(c.source().events.is_empty());
    c.event(SolveStarted, &0).unwrap();
    c.event(PhaseStarted, &0).unwrap();
    c.source_mut().declared = Some(1);
    assert_eq!(
        c.event(StepStarted, &0),
        Err(SelectionCacheError::SizeMismatch {
            declared: 1,
            actual: 2
        })
    );
    assert_eq!(c.state(), SelectionCacheState::Step);
    assert!(c.snapshot().is_err());
    let count = c.source().events.len();
    assert!(c.event(StepStarted, &0).is_err());
    assert!(c.event(PhaseEnded, &0).is_err());
    assert_eq!(c.source().events.len(), count);
    c.event(StepEnded, &0).unwrap();
    c.source_mut().declared = Some(2);
    c.event(StepStarted, &5).unwrap();
    assert_eq!(c.snapshot().unwrap().items(), &[5, 6]);
    c.event(StepEnded, &5).unwrap();
    c.event(PhaseEnded, &5).unwrap();
    c.event(SolveEnded, &5).unwrap();
}
#[test]
fn unknown_infinite_sources_are_bounded_and_declared_oversize_sources_are_not_opened() {
    for declared in [None, Some(usize::MAX)] {
        let mut source = Source::new(usize::MAX);
        source.declared = declared;
        let mut c = Cache::uniform(
            source,
            SelectionCacheScope::Solve,
            CachedSelectionOrder::Original,
            3,
        )
        .unwrap();
        assert_eq!(
            c.event(SolveStarted, &0),
            Err(SelectionCacheError::Weights(
                WeightedSelectionError::CandidateLimit { maximum: 3 }
            ))
        );
        assert_eq!(c.source().opens, usize::from(declared.is_none()));
        assert_eq!(
            c.source().reads.get(),
            if declared.is_none() { 4 } else { 0 }
        );
        assert!(c.snapshot().is_err());
        c.event(SolveEnded, &0).unwrap();
    }
}
#[test]
fn callback_unwinding_and_bad_weights_never_install_a_stale_snapshot() {
    let mut c = uniform(SelectionCacheScope::Step);
    c.event(SolveStarted, &0).unwrap();
    c.event(PhaseStarted, &0).unwrap();
    c.event(StepStarted, &0).unwrap();
    let held = c.snapshot().unwrap();
    c.event(StepEnded, &0).unwrap();
    c.source_mut().panic_open = true;
    assert!(catch_unwind(AssertUnwindSafe(|| c.event(StepStarted, &10))).is_err());
    assert_eq!(c.state(), SelectionCacheState::Step);
    assert!(c.snapshot().is_err());
    assert_eq!(held.items(), &[0, 1]);
    c.event(StepEnded, &10).unwrap();
    c.source_mut().panic_open = false;
    c.event(StepStarted, &20).unwrap();
    assert_eq!(c.snapshot().unwrap().items(), &[20, 21]);
    let mut bad = SelectionCache::new(
        Source::new(2),
        SelectionCacheScope::Solve,
        CachedSelectionOrder::Random,
        2,
        |_: &usize, _: &usize| -1.0,
    )
    .unwrap();
    assert!(matches!(
        bad.event(SolveStarted, &0),
        Err(SelectionCacheError::Weights(
            WeightedSelectionError::InvalidWeight { index: 0 }
        ))
    ));
    assert!(bad.snapshot().is_err());
    bad.event(SolveEnded, &0).unwrap();
    let mut original = SelectionCache::new(
        Source::new(2),
        SelectionCacheScope::Solve,
        CachedSelectionOrder::Original,
        2,
        |_: &usize, _: &usize| panic!("unused weights"),
    )
    .unwrap();
    original.event(SolveStarted, &0).unwrap();
    assert_eq!(original.size().unwrap(), 2);
}
#[test]
fn held_nonclone_snapshots_outlive_scope_release_and_random_empty_iteration_does_not_draw() {
    struct Item(Rc<Cell<usize>>);
    impl Drop for Item {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    struct Items(Rc<Cell<usize>>);
    impl SelectionSource<(), Item> for Items {
        fn size(&mut self, _: &()) -> Option<usize> {
            Some(1)
        }
        fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Item> + 'a>
        where
            Item: 'a,
        {
            Box::new(std::iter::once(Item(self.0.clone())))
        }
    }
    let drops = Rc::new(Cell::new(0));
    let mut c = SelectionCache::uniform(
        Items(drops.clone()),
        SelectionCacheScope::Solve,
        CachedSelectionOrder::Original,
        1,
    )
    .unwrap();
    c.event(SolveStarted, &()).unwrap();
    let held = c.snapshot().unwrap();
    c.event(SolveEnded, &()).unwrap();
    assert_eq!(drops.get(), 0);
    assert_eq!(held.items().len(), 1);
    drop(held);
    assert_eq!(drops.get(), 1);
    let mut c = Cache::uniform(
        Source::new(0),
        SelectionCacheScope::Solve,
        CachedSelectionOrder::Random,
        0,
    )
    .unwrap();
    c.event(SolveStarted, &0).unwrap();
    assert!(c.is_never_ending());
    let mut it = c.iter_with(|| panic!("empty draw")).unwrap();
    assert!(!it.has_next());
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None);
    assert_eq!(it.size_hint(), (0, Some(0)));
}
struct Changes {
    problem: FiniteProblem<i32>,
    events: Vec<SelectionLifecycleEvent>,
    opens: usize,
}
impl SelectionSource<Assignment<i32>, Move<i32>> for Changes {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &Assignment<i32>) {
        self.events.push(e);
    }
    fn size(&mut self, _: &Assignment<i32>) -> Option<usize> {
        None
    }
    fn open<'a>(&'a mut self, a: &'a Assignment<i32>) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        self.opens += 1;
        Box::new(change_moves(&self.problem, a).unwrap())
    }
}
type MoveCache =
    SelectionCache<Assignment<i32>, Move<i32>, Changes, fn(&Assignment<i32>, &Move<i32>) -> f64>;
struct Provider(MoveCache);
struct Cursor<'a> {
    cache: &'a mut MoveCache,
    context: &'a Assignment<i32>,
    samples: Box<dyn Iterator<Item = Move<i32>> + 'a>,
}
impl Iterator for Cursor<'_> {
    type Item = Move<i32>;
    fn next(&mut self) -> Option<Self::Item> {
        self.samples.next()
    }
}
impl Drop for Cursor<'_> {
    fn drop(&mut self) {
        self.cache.event(StepEnded, self.context).unwrap();
    }
}
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Provider {
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i32>,
        a: &'a Assignment<i32>,
        step: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        if self.0.state() == SelectionCacheState::Idle {
            self.0.event(SolveStarted, a).unwrap();
            self.0.event(PhaseStarted, a).unwrap();
        }
        self.0.event(StepStarted, a).unwrap();
        let samples = self.0.snapshot().unwrap().into_samples(step + 7).take(4);
        Box::new(Cursor {
            cache: &mut self.0,
            context: a,
            samples: Box::new(samples),
        })
    }
}
#[test]
fn step_scoped_weighted_move_provider_balances_events_in_both_backends() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2, 3]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    let config = LocalSearchConfig {
        max_steps: 3,
        max_move_evaluations: Some(20),
        assert_scores: true,
        ..Default::default()
    };
    for stream in [false, true] {
        let mut provider = Provider(
            MoveCache::new(
                Changes {
                    problem: p.clone(),
                    events: vec![],
                    opens: 0,
                },
                SelectionCacheScope::Step,
                CachedSelectionOrder::Random,
                3,
                |_, m| match m {
                    Move::Change { value: Some(3), .. } => 1000.0,
                    _ => 0.5,
                },
            )
            .unwrap(),
        );
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
                config.clone(),
                &mut provider,
            )
            .unwrap()
        } else {
            local_search_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                config.clone(),
                &mut provider,
            )
            .unwrap()
        };
        let best = result.best.unwrap();
        assert_eq!(best.score, SimpleScore::new(3));
        assert_eq!(provider.0.state(), SelectionCacheState::Phase);
        assert!(provider.0.snapshot().is_err());
        provider.0.event(PhaseEnded, &best.assignment).unwrap();
        provider.0.event(SolveEnded, &best.assignment).unwrap();
        let source = provider.0.source();
        for e in [SolveStarted, SolveEnded, PhaseStarted, PhaseEnded] {
            assert_eq!(source.events.iter().filter(|&&v| v == e).count(), 1);
        }
        for e in [StepStarted, StepEnded] {
            assert_eq!(
                source.events.iter().filter(|&&v| v == e).count(),
                source.opens
            );
        }
        assert!(source.opens >= 2);
    }
}
