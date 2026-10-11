//! Original provider snapshot lifetime, work bounds and real solver regressions.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;
struct Recorded {
    calls: Rc<Cell<usize>>,
    reads: Rc<Cell<usize>>,
    events: Rc<RefCell<Vec<MoveProviderEvent>>>,
    infinite: bool,
}
impl MoveProvider<(), (), u64> for Recorded {
    fn event(&mut self, e: MoveProviderEvent) {
        self.events.borrow_mut().push(e);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        !installed
    }
    fn moves<'a>(
        &'a mut self,
        _: &'a (),
        _: &'a (),
        step: u64,
    ) -> Box<dyn Iterator<Item = u64> + 'a>
    where
        u64: 'a,
    {
        self.calls.set(self.calls.get() + 1);
        let reads = self.reads.clone();
        let n = if self.infinite { usize::MAX } else { 3 };
        Box::new((0..n).map(move |i| {
            reads.set(reads.get() + 1);
            step * 100 + (i.min(1) as u64)
        }))
    }
}
#[test]
fn cache_lifetimes_preserve_duplicates_and_forward_all_events() {
    use MoveProviderEvent::*;
    for scope in [
        ProviderCacheScope::Step,
        ProviderCacheScope::Phase,
        ProviderCacheScope::Solve,
    ] {
        let calls = Rc::new(Cell::new(0));
        let reads = Rc::new(Cell::new(0));
        let events = Rc::new(RefCell::new(vec![]));
        let child = Recorded {
            calls: calls.clone(),
            reads: reads.clone(),
            events: events.clone(),
            infinite: false,
        };
        let mut provider = CachedProviderMoves::new(child, scope, 3);
        let mut expected = vec![SolveStarted];
        provider.event(SolveStarted);
        for phase in 0..2 {
            provider.event(PhaseStarted);
            expected.push(PhaseStarted);
            for index in 0..2 {
                let step = phase * 2 + index;
                provider.event(StepStarted);
                expected.push(StepStarted);
                let first = match scope {
                    ProviderCacheScope::Step => step,
                    ProviderCacheScope::Phase => phase * 2,
                    ProviderCacheScope::Solve => 0,
                };
                let want = [first * 100, first * 100 + 1, first * 100 + 1];
                assert_eq!(provider.moves(&(), &(), step).collect::<Vec<_>>(), want);
                assert_eq!(
                    provider.moves(&(), &(), step + 100).collect::<Vec<_>>(),
                    want
                );
                provider.event(StepEnded);
                expected.push(StepEnded);
            }
            provider.event(PhaseEnded);
            expected.push(PhaseEnded);
        }
        provider.event(SolveEnded);
        expected.push(SolveEnded);
        assert_eq!(*events.borrow(), expected);
        assert!(provider.cached().is_none());
        let opens = match scope {
            ProviderCacheScope::Step => 4,
            ProviderCacheScope::Phase => 2,
            ProviderCacheScope::Solve => 1,
        };
        assert_eq!(calls.get(), opens);
        assert_eq!(reads.get(), opens * 3);
        assert!(provider.construction_placement_completed(false));
        assert!(!provider.construction_placement_completed(true));
    }
}
#[test]
fn overflow_is_bounded_and_never_publishes_a_partial_snapshot() {
    let calls = Rc::new(Cell::new(0));
    let reads = Rc::new(Cell::new(0));
    let child = Recorded {
        calls: calls.clone(),
        reads: reads.clone(),
        events: Default::default(),
        infinite: true,
    };
    let mut provider = CachedProviderMoves::new(child, ProviderCacheScope::Phase, 5);
    for attempt in 1..=2 {
        assert!(matches!(
            provider.try_moves(&(), &(), 0),
            Err(ProviderSnapshotError { maximum: 5 })
        ));
        assert_eq!(reads.get(), attempt * 6);
        assert!(provider.cached().is_none());
    }
    assert_eq!(calls.get(), 2);
    let mut limited = LimitedMoveProvider::new(provider, 0);
    assert_eq!(limited.moves(&(), &(), 0).count(), 0);
    assert_eq!(calls.get(), 2);
    let mut empty_cache = CachedProviderMoves::new(limited, ProviderCacheScope::Step, 0);
    assert_eq!(empty_cache.moves(&(), &(), 0).count(), 0);
    assert_eq!(calls.get(), 2);
}
#[test]
fn limit_consumes_only_requested_items_and_editing_the_child_invalidates_cache() {
    let calls = Rc::new(Cell::new(0));
    let reads = Rc::new(Cell::new(0));
    let child = Recorded {
        calls: calls.clone(),
        reads: reads.clone(),
        events: Default::default(),
        infinite: true,
    };
    let mut limited = LimitedMoveProvider::new(child, 5);
    assert_eq!(
        limited.moves(&(), &(), 2).collect::<Vec<_>>(),
        [200, 201, 201, 201, 201]
    );
    assert_eq!(reads.get(), 5);
    let mut cached = CachedProviderMoves::new(limited, ProviderCacheScope::Phase, 5);
    assert_eq!(cached.moves(&(), &(), 3).count(), 5);
    assert_eq!(reads.get(), 10);
    let _ = cached.provider_mut();
    assert!(cached.cached().is_none());
    assert_eq!(cached.moves(&(), &(), 4).next(), Some(400));
    assert_eq!(reads.get(), 15);
    assert_eq!(calls.get(), 3);
    assert!(cached.construction_placement_completed(false));
}
#[test]
fn metadata_never_runs_factories_and_propagates_nested_errors_and_overflow() {
    struct Huge;
    impl MoveProvider<(), (), i32> for Huge {
        fn neighborhood_count(&self) -> Result<usize, NeighborhoodCountError> {
            Ok(usize::MAX)
        }
        fn moves<'a>(
            &'a mut self,
            _: &'a (),
            _: &'a (),
            _: u64,
        ) -> Box<dyn Iterator<Item = i32> + 'a>
        where
            i32: 'a,
        {
            panic!("metadata only")
        }
    }
    let mut huge = Huge;
    let mut ordinary = |_: &(), _: &(), _: u64| panic!("metadata only");
    let union = ProviderUnion::unweighted(
        vec![
            &mut huge as &mut dyn MoveProvider<(), (), i32>,
            &mut ordinary,
        ],
        UnionOrder::Sequential,
    )
    .unwrap();
    let wrapped = PhaseResetMoveProvider::new(
        LimitedMoveProvider::new(
            CachedProviderMoves::<_, i32>::new(union, ProviderCacheScope::Phase, 0),
            0,
        ),
        |_| panic!("metadata must not reset"),
    );
    assert_eq!(
        wrapped.neighborhood_count(),
        Err(NeighborhoodCountError::Overflow)
    );
    let mut a = |_: &(), _: &(), _: u64| vec![0_i32];
    let weighted = ProviderUnion::new(
        vec![(&mut a as &mut dyn MoveProvider<(), (), i32>, 1)],
        UnionOrder::Sequential,
    )
    .unwrap();
    assert_eq!(
        LimitedMoveProvider::new(weighted, 0).neighborhood_count(),
        Err(NeighborhoodCountError::WeightedUnion)
    );
}
#[test]
fn cached_stable_basic_moves_reach_known_optimum_in_both_score_backends() {
    for stream in [false, true] {
        let p = FiniteProblem::new(
            (0..2)
                .map(|_| Variable::new(vec![0_i32, 1, 2]).with_initial(0))
                .collect(),
        )
        .unwrap();
        let mut n = ConstraintNetwork::new(2);
        for variable in 0..2 {
            n.add_constraint(format!("v{variable}"), vec![variable], |v| {
                SimpleScore::new(i64::from(v[0].copied().unwrap()))
            })
            .unwrap();
        }
        let openings = Rc::new(Cell::new(0));
        let count = openings.clone();
        let factory = move |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| {
            count.set(count.get() + 1);
            (0..2)
                .flat_map(|variable| {
                    (1..=2).map(move |v| Move::Change {
                        variable,
                        value: Some(v),
                    })
                })
                .collect()
        };
        let mut provider = LimitedMoveProvider::new(
            CachedProviderMoves::new(factory, ProviderCacheScope::Phase, 4),
            4,
        );
        let config = LocalSearchConfig {
            assert_scores: true,
            ..Default::default()
        };
        let result = if stream {
            let mut b = StreamGraphBuilder::new();
            let source = b.source::<usize, (usize, Option<i32>)>();
            b.score("value", source.port(), |(_, v)| {
                SimpleScore::new(i64::from(v.unwrap()))
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
        assert_eq!(result.best.unwrap().score, SimpleScore::new(4));
        assert_eq!(openings.get(), 1);
        assert!(provider.provider().cached().is_none());
    }
}
