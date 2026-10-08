//! Native shared-provider ownership and solve-boundary failures.
use MoveProviderEvent::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;
type Log = Rc<RefCell<Vec<MoveProviderEvent>>>;
struct Provider {
    log: Log,
}
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Provider {
    fn event(&mut self, e: MoveProviderEvent) {
        self.log.borrow_mut().push(e);
    }
    fn moves<'a>(
        &'a mut self,
        _: &'a FiniteProblem<i32>,
        _: &'a Assignment<i32>,
        _: u64,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(std::iter::once(Move::Change {
            variable: 0,
            value: Some(1),
        }))
    }
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            });
            2
        ],
        ..Default::default()
    }
}
fn problem() -> FiniteProblem<i32> {
    FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap()
}
#[test]
fn shared_registration_is_atomic_and_releases_only_the_final_binding() {
    let logs: [Log; 3] = std::array::from_fn(|_| Log::default());
    let mut a = Provider {
        log: logs[0].clone(),
    };
    let mut b = Provider {
        log: logs[1].clone(),
    };
    let mut c = Provider {
        log: logs[2].clone(),
    };
    let mut providers = PhaseMoveProviders::default();
    providers.bind_shared([0, 2, 0], &mut a).unwrap();
    assert_eq!(
        providers.bind_shared([1, 2], &mut b),
        Err(MoveProviderBindingError::AlreadyBound { phase: 2 })
    );
    assert!(!providers.contains(1));
    assert_eq!(
        providers.bind_shared([], &mut c),
        Err(MoveProviderBindingError::EmptyPhases)
    );
    assert!(providers.remove(0).is_none());
    assert!(!providers.contains(0));
    assert!(providers.contains(2));
    let released = providers.remove(2).unwrap();
    released.event(SolveStarted);
    assert_eq!(*logs[0].borrow(), vec![SolveStarted]);
    assert!(providers.remove(2).is_none());
}
#[test]
fn replacing_one_alias_preserves_the_other_and_notifies_distinct_providers_once() {
    let p = problem();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let logs: [Log; 2] = std::array::from_fn(|_| Log::default());
    let mut a = Provider {
        log: logs[0].clone(),
    };
    let mut b = Provider {
        log: logs[1].clone(),
    };
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind_shared([0, 1], &mut a).unwrap();
    assert!(ext.move_providers.bind(0, &mut b).is_none());
    let r = solve_with_extensions(&p, &n, plan(), ext).unwrap();
    assert_eq!(r.work.completed_steps, 2);
    for log in logs {
        assert_eq!(
            *log.borrow(),
            vec![
                SolveStarted,
                PhaseStarted,
                StepStarted,
                StepEnded,
                PhaseEnded,
                SolveEnded
            ]
        );
    }
}
#[test]
fn invalid_plans_skip_events_and_early_stops_close_registered_providers() {
    let p = problem();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    for mode in 0..4 {
        let log = Log::default();
        let mut provider = Provider { log: log.clone() };
        let mut ext = SolverExtensions::default();
        ext.move_providers
            .bind_shared(if mode == 0 { vec![9] } else { vec![0, 1] }, &mut provider)
            .unwrap();
        let mut config = plan();
        if mode == 1 {
            config.limits.max_evaluations = 0;
        }
        if mode == 2 {
            config.phases[1] = SolverPhase::LocalSearch(LocalSearchConfig {
                acceptance: Acceptance::VariableTabu {
                    tenure: 0,
                    aspiration: false,
                },
                ..Default::default()
            });
        }
        if mode == 3 {
            config.limits.max_move_evaluations = Some(1);
        }
        let result = solve_with_extensions(&p, &n, config, ext);
        match mode {
            0 | 2 => assert!(result.is_err()),
            1 => assert_eq!(result.unwrap().work.evaluations, 0),
            _ => assert_eq!(result.unwrap().phases.len(), 1),
        }
        if mode == 3 {
            assert_eq!(
                *log.borrow(),
                vec![
                    SolveStarted,
                    PhaseStarted,
                    StepStarted,
                    StepEnded,
                    PhaseEnded,
                    SolveEnded
                ]
            );
        } else if mode == 1 {
            assert_eq!(*log.borrow(), vec![SolveStarted, SolveEnded]);
        } else {
            assert!(log.borrow().is_empty(), "mode {mode}");
        }
    }
}
struct Keys {
    events: Vec<SelectionLifecycleEvent>,
    builds: usize,
}
impl SelectionSource<(), usize> for Keys {
    fn event(&mut self, e: SelectionLifecycleEvent, _: &()) {
        self.events.push(e);
    }
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.builds += 1;
        Box::new(std::iter::once(0))
    }
}
#[test]
fn solve_cache_survives_an_unbound_phase_and_closes_on_a_later_factory_error() {
    let p = problem();
    for fail in [false, true] {
        let mut provider = CachedMoveProvider::uniform(
            Keys {
                events: vec![],
                builds: 0,
            },
            CachedProviderConfig {
                scope: ProviderCacheScope::Solve,
                ..Default::default()
            },
            |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| (),
            |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, &key: &usize| {
                Some(Move::Change {
                    variable: key,
                    value: Some(1),
                })
            },
        )
        .unwrap();
        let mut ext = SolverExtensions::default();
        ext.move_providers
            .bind_shared([0, 2], &mut provider)
            .unwrap();
        let mut config = plan();
        config.phases.push(config.phases[0].clone());
        let factories = Cell::new(0);
        let r = solve_stream_with_extensions(
            &p,
            || {
                factories.set(factories.get() + 1);
                if fail && factories.get() == 3 {
                    return Err(GraphError::Dependency("later factory failed".into()));
                }
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let s = b.source();
                Ok((b.build(), s))
            },
            config,
            ext,
        );
        assert_eq!(r.is_err(), fail);
        assert_eq!(factories.get(), 3);
        assert_eq!(provider.cache().state(), SelectionCacheState::Idle);
        assert_eq!(provider.cache().source().builds, 1);
        let events = &provider.cache().source().events;
        assert_eq!(
            events
                .iter()
                .filter(|&&e| e == SelectionLifecycleEvent::SolveStarted)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|&&e| e == SelectionLifecycleEvent::SolveEnded)
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|&&e| e == SelectionLifecycleEvent::PhaseStarted)
                .count(),
            if fail { 1 } else { 2 }
        );
    }
}
#[test]
fn panic_skips_end_callbacks_and_explicit_reset_allows_a_new_solve() {
    let p = problem();
    let panic = Cell::new(true);
    let mut provider = CachedMoveProvider::uniform(
        Keys {
            events: vec![],
            builds: 0,
        },
        CachedProviderConfig {
            scope: ProviderCacheScope::Solve,
            ..Default::default()
        },
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| (),
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, &key: &usize| {
            assert!(!panic.get(), "mapper failed");
            Some(Move::Change {
                variable: key,
                value: Some(1),
            })
        },
    )
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut ext = SolverExtensions::default();
        ext.move_providers
            .bind_shared([0, 1], &mut provider)
            .unwrap();
        solve_with_extensions(&p, &n, plan(), ext)
    }));
    assert!(r.is_err());
    assert_eq!(
        provider.cache().source().events,
        vec![
            SelectionLifecycleEvent::SolveStarted,
            SelectionLifecycleEvent::PhaseStarted,
            SelectionLifecycleEvent::StepStarted
        ]
    );
    provider.reset().unwrap();
    panic.set(false);
    let mut ext = SolverExtensions::default();
    ext.move_providers
        .bind_shared([0, 1], &mut provider)
        .unwrap();
    assert_eq!(
        solve_with_extensions(&p, &n, plan(), ext)
            .unwrap()
            .work
            .completed_steps,
        2
    );
    assert_eq!(provider.cache().state(), SelectionCacheState::Idle);
}
