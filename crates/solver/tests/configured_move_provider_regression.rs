//! Original configured-provider integration; no upstream method credit.
struct Key(usize);
use SelectionLifecycleEvent as E;
use std::{cell::Cell, marker::PhantomData, rc::Rc};
use timetuck::*;
struct Entries {
    keys: Vec<usize>,
    events: Vec<(E, i32)>,
    builds: Vec<i32>,
    declared: Option<usize>,
}
impl Entries {
    fn new(keys: Vec<usize>) -> Self {
        Self {
            keys,
            events: vec![],
            builds: vec![],
            declared: None,
        }
    }
}
impl SelectionSource<i32, Key> for Entries {
    fn event(&mut self, e: E, c: &i32) {
        self.events.push((e, *c));
    }
    fn size(&mut self, _: &i32) -> Option<usize> {
        self.declared.or(Some(self.keys.len()))
    }
    fn open<'a>(&'a mut self, c: &'a i32) -> Box<dyn Iterator<Item = Key> + 'a>
    where
        Key: 'a,
    {
        self.builds.push(*c);
        Box::new(self.keys.iter().map(|&key| Key(key)))
    }
}
struct Zero<C>(PhantomData<C>);
fn zero<C>() -> Zero<C> {
    Zero(PhantomData)
}
impl<A, C> IncrementalCalculator<A> for Zero<C> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = C;
    fn reset(
        &mut self,
        _: &A,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::ZERO
    }
}
fn basic_plan() -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
            acceptance: Acceptance::HillClimbing,
            max_steps: 2,
            ..Default::default()
        })],
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: basic_plan().limits,
        phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
            search: LocalSearchConfig {
                acceptance: Acceptance::HillClimbing,
                max_steps: 2,
                ..Default::default()
            },
            ..Default::default()
        })],
    }
}
fn value(a: &Assignment<i32>) -> i32 {
    a.values()[0].unwrap()
}
fn owner(a: &ListAssignment<i32>) -> i32 {
    i32::from(a.lists()[0].is_empty())
}
fn basic_move(a: &Assignment<i32>) -> Move<i32> {
    Move::Change {
        variable: 0,
        value: Some(value(a) + 1),
    }
}
fn list_move(a: &ListAssignment<i32>) -> ListMove<i32> {
    let source = owner(a) as usize;
    ListMove::Change {
        source: ListPosition {
            entity: source,
            index: 0,
        },
        destination: ListPosition {
            entity: 1 - source,
            index: 0,
        },
    }
}
#[test]
fn configured_nonclone_sources_drive_all_nine_backends_and_shared_phases() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let l = ListProblem::new(
        vec![7],
        vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
    )
    .unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let bn = ConstraintNetwork::<_, SimpleScore>::new(1);
    let ln = ListConstraintNetwork::<_, SimpleScore>::new(2, 1);
    let mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 2, 1);
    let bg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = g.source();
        Ok((g.build(), s))
    };
    let lg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut g);
        Ok((g.build(), s))
    };
    let mg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::new(&mut g);
        Ok((g.build(), s))
    };
    macro_rules! check {
        ($solve:ident,$p:expr,$scorer:expr,$plan:expr,$p_ty:ty,$a_ty:ty,$capture:expr,$movement:expr) => {{
            for scope in [
                SelectionCacheScope::Solve,
                SelectionCacheScope::Phase,
                SelectionCacheScope::Step,
            ] {
                let selection =
                    SelectionBuilder::new(selection_config(SelectionOrder::Sorted, scope))
                        .with_comparator_factory(|context: &i32| {
                            let descending = *context % 2 != 0;
                            move |a: &Key, b: &Key| {
                                if descending {
                                    b.0.cmp(&a.0)
                                } else {
                                    a.0.cmp(&b.0)
                                }
                            }
                        })
                        .build(ConfiguredSelectionPolicy::default(), |_| {
                            Entries::new(vec![2, 0, 1])
                        })
                        .unwrap();
                let mut provider = ConfiguredMoveProvider::new(
                    selection,
                    ConfiguredProviderConfig::default(),
                    |_: &$p_ty, a: &$a_ty, _: u64| ($capture)(a),
                    |_: &$p_ty, a: &$a_ty, _: u64, key: &Key| (key.0 == 0).then(|| ($movement)(a)),
                );
                for run in 1..=2 {
                    let mut ext = SolverExtensions::default();
                    ext.move_providers
                        .bind_shared([0, 1, 1], &mut provider)
                        .unwrap();
                    let mut plan = $plan;
                    plan.phases.push(plan.phases[0].clone());
                    let r = $solve(&$p, $scorer, plan, ext).unwrap();
                    assert!(r.complete);
                    assert_eq!(r.work.completed_steps, 4);
                    assert_eq!(r.work.move_evaluations, 4);
                    assert_eq!(provider.selection().state(), SelectionCacheState::Idle);
                    let events = vec![
                        (E::SolveStarted, 0),
                        (E::PhaseStarted, 0),
                        (E::StepStarted, 0),
                        (E::StepEnded, 0),
                        (E::StepStarted, 1),
                        (E::StepEnded, 1),
                        (E::PhaseEnded, 1),
                        (E::PhaseStarted, 0),
                        (E::StepStarted, 0),
                        (E::StepEnded, 0),
                        (E::StepStarted, 1),
                        (E::StepEnded, 1),
                        (E::PhaseEnded, 1),
                        (E::SolveEnded, 1),
                    ];
                    assert_eq!(provider.selection().source().events, events.repeat(run));
                    let builds = match scope {
                        SelectionCacheScope::Solve => vec![0],
                        SelectionCacheScope::Phase => vec![0, 0],
                        SelectionCacheScope::Step => vec![0, 1, 0, 1],
                        SelectionCacheScope::Uncached => unreachable!(),
                    };
                    assert_eq!(provider.selection().source().builds, builds.repeat(run));
                }
            }
        }};
    }
    check!(
        solve_with_extensions,
        b,
        &bn,
        basic_plan(),
        FiniteProblem<i32>,
        Assignment<i32>,
        value,
        basic_move
    );
    check!(
        solve_stream_with_extensions,
        b,
        bg,
        basic_plan(),
        FiniteProblem<i32>,
        Assignment<i32>,
        value,
        basic_move
    );
    check!(
        solve_calculator_with_extensions,
        b,
        &zero::<Vec<usize>>,
        basic_plan(),
        FiniteProblem<i32>,
        Assignment<i32>,
        value,
        basic_move
    );
    check!(
        solve_list_with_extensions,
        l,
        &ln,
        basic_plan(),
        ListProblem<i32>,
        ListAssignment<i32>,
        owner,
        list_move
    );
    check!(
        solve_list_stream_with_extensions,
        l,
        lg,
        basic_plan(),
        ListProblem<i32>,
        ListAssignment<i32>,
        owner,
        list_move
    );
    check!(
        solve_list_calculator_with_extensions,
        l,
        &zero::<Vec<usize>>,
        basic_plan(),
        ListProblem<i32>,
        ListAssignment<i32>,
        owner,
        list_move
    );
    let mv = |a: &MixedAssignment<i32, i32>| MixedMove {
        basic: basic_move(a.basic()),
        lists: list_move(a.lists()),
    };
    let capture = |a: &MixedAssignment<i32, i32>| value(a.basic());
    check!(solve_mixed_with_extensions,m,&mn,mixed_plan(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,capture,mv);
    check!(solve_mixed_stream_with_extensions,m,mg,mixed_plan(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,capture,mv);
    check!(solve_mixed_calculator_with_extensions,m,&zero::<MixedCalculatorChange>,mixed_plan(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,capture,mv);
}

fn selection_config(order: SelectionOrder, scope: SelectionCacheScope) -> SelectionConfig {
    SelectionConfig {
        cache_scope: Some(scope),
        order: Some(order),
        ..Default::default()
    }
}
fn event<P: MoveProvider<(), i32, i32>>(p: &mut P, e: MoveProviderEvent) {
    p.event(e);
}
#[test]
fn phase_source_step_order_and_current_mapping_have_independent_contexts() {
    let builds = Rc::new(Cell::new(0));
    let observed = builds.clone();
    let selection = SelectionBuilder::new(selection_config(
        SelectionOrder::Sorted,
        SelectionCacheScope::Step,
    ))
    .with_comparator_factory(move |c: &i32| {
        observed.set(observed.get() + 1);
        let descending = *c != 0;
        move |a: &Key, b: &Key| {
            if descending {
                b.0.cmp(&a.0)
            } else {
                a.0.cmp(&b.0)
            }
        }
    })
    .build(
        ConfiguredSelectionPolicy {
            source_minimum_scope: SelectionCacheScope::Phase,
            ..Default::default()
        },
        |_| Entries::new(vec![2, 0, 1]),
    )
    .unwrap();
    let mut p = ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig::default(),
        |_: &(), a: &i32, _: u64| *a,
        |_: &(), a: &i32, _: u64, k: &Key| Some(*a + k.0 as i32),
    );
    assert!(matches!(
        p.try_moves(&(), &0, 0),
        Err(CachedProviderError::OutsideStep)
    ));
    event(&mut p, MoveProviderEvent::PhaseStarted);
    for (context, expected) in [(0, vec![0, 1, 2]), (10, vec![12, 11, 10])] {
        event(&mut p, MoveProviderEvent::StepStarted);
        assert_eq!(
            p.try_moves(&(), &context, 0).unwrap().collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            p.try_moves(&(), &context, 0).unwrap().collect::<Vec<_>>(),
            expected
        );
        event(&mut p, MoveProviderEvent::StepEnded);
    }
    assert_eq!(builds.get(), 2);
    assert_eq!(p.selection().source().builds, vec![0]);
    event(&mut p, MoveProviderEvent::PhaseEnded);
    assert_eq!(p.selection().state(), SelectionCacheState::Idle);
}
#[test]
fn all_orders_map_nonclone_keys_with_caps_and_replayable_openings() {
    for order in [
        SelectionOrder::Original,
        SelectionOrder::Random,
        SelectionOrder::Sorted,
        SelectionOrder::Shuffled,
        SelectionOrder::Probabilistic,
    ] {
        let mut config = selection_config(order, SelectionCacheScope::Step);
        config.selected_count_limit = Some(5);
        let mut b = SelectionBuilder::new(config);
        if order == SelectionOrder::Sorted {
            b = b.with_comparator(|_, a: &Key, b: &Key| a.0.cmp(&b.0));
        }
        if order == SelectionOrder::Probabilistic {
            b = b.with_weight(|_, k: &Key| if k.0 == 1 { 1.0 } else { 0.0 });
        }
        let selection = b
            .build(ConfiguredSelectionPolicy::default(), |_| {
                Entries::new(vec![2, 0, 1, 1])
            })
            .unwrap();
        let mut p = ConfiguredMoveProvider::new(
            selection,
            ConfiguredProviderConfig {
                maximum_draws: 3,
                seed: 17,
            },
            |_: &(), _: &i32, _: u64| 0,
            |_: &(), _: &i32, _: u64, k: &Key| Some(k.0 as i32),
        );
        event(&mut p, MoveProviderEvent::PhaseStarted);
        event(&mut p, MoveProviderEvent::StepStarted);
        let first = p.try_moves(&(), &0, 9).unwrap().collect::<Vec<_>>();
        assert_eq!(first, p.try_moves(&(), &0, 9).unwrap().collect::<Vec<_>>());
        assert_eq!(first.len(), 3);
        match order {
            SelectionOrder::Original => assert_eq!(first, [2, 0, 1]),
            SelectionOrder::Sorted => assert_eq!(first, [0, 1, 1]),
            SelectionOrder::Probabilistic => assert_eq!(first, [1, 1, 1]),
            _ => assert!(first.iter().all(|v| (0..=2).contains(v))),
        }
        event(&mut p, MoveProviderEvent::StepEnded);
        event(&mut p, MoveProviderEvent::PhaseEnded);
    }
    let mut config = selection_config(SelectionOrder::Original, SelectionCacheScope::Uncached);
    config.selected_count_limit = Some(2);
    let selection = SelectionBuilder::new(config)
        .build(ConfiguredSelectionPolicy::default(), |_| {
            Entries::new(vec![2, 0, 1])
        })
        .unwrap();
    let mut p = ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig::default(),
        |_: &(), _: &i32, _: u64| 0,
        |_: &(), _: &i32, _: u64, k: &Key| Some(k.0 as i32),
    );
    event(&mut p, MoveProviderEvent::PhaseStarted);
    event(&mut p, MoveProviderEvent::StepStarted);
    assert_eq!(p.try_moves(&(), &0, 0).unwrap().collect::<Vec<_>>(), [2, 0]);
    event(&mut p, MoveProviderEvent::StepEnded);
    event(&mut p, MoveProviderEvent::PhaseEnded);
}
#[test]
fn rejected_random_draws_and_zero_draw_scans_remain_bounded() {
    for maximum_draws in [0, 7] {
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let selection = SelectionBuilder::new(selection_config(
            SelectionOrder::Random,
            SelectionCacheScope::Step,
        ))
        .build(ConfiguredSelectionPolicy::default(), |_| {
            Entries::new(vec![0])
        })
        .unwrap();
        let mut p = ConfiguredMoveProvider::new(
            selection,
            ConfiguredProviderConfig {
                maximum_draws,
                seed: 0,
            },
            |_: &(), _: &i32, _: u64| 0,
            move |_: &(), _: &i32, _: u64, _: &Key| {
                observed.set(observed.get() + 1);
                None::<i32>
            },
        );
        event(&mut p, MoveProviderEvent::PhaseStarted);
        event(&mut p, MoveProviderEvent::StepStarted);
        assert_eq!(p.try_moves(&(), &0, 0).unwrap().count(), 0);
        assert_eq!(calls.get(), maximum_draws);
        event(&mut p, MoveProviderEvent::StepEnded);
        event(&mut p, MoveProviderEvent::PhaseEnded);
        if maximum_draws == 0 {
            assert!(p.selection().source().events.is_empty());
            assert!(p.selection().source().builds.is_empty());
        }
    }
}
#[test]
fn comparator_panics_and_build_failures_can_be_reset_without_stale_output() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let panic = Rc::new(Cell::new(true));
    let observed = panic.clone();
    let selection = SelectionBuilder::new(selection_config(
        SelectionOrder::Sorted,
        SelectionCacheScope::Step,
    ))
    .with_comparator_factory(move |_: &i32| {
        assert!(!observed.get(), "comparator factory failure");
        |a: &Key, b: &Key| a.0.cmp(&b.0)
    })
    .build(ConfiguredSelectionPolicy::default(), |_| {
        Entries::new(vec![2, 0, 1])
    })
    .unwrap();
    let mut p = ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig::default(),
        |_: &(), _: &i32, _: u64| 0,
        |_: &(), _: &i32, _: u64, k: &Key| Some(k.0 as i32),
    );
    event(&mut p, MoveProviderEvent::PhaseStarted);
    event(&mut p, MoveProviderEvent::StepStarted);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = p.try_moves(&(), &0, 0);
        }))
        .is_err()
    );
    p.reset().unwrap();
    panic.set(false);
    p.source_mut().declared = Some(9);
    event(&mut p, MoveProviderEvent::PhaseStarted);
    event(&mut p, MoveProviderEvent::StepStarted);
    assert!(p.try_moves(&(), &0, 0).is_err());
    p.reset().unwrap();
    p.source_mut().declared = None;
    event(&mut p, MoveProviderEvent::PhaseStarted);
    event(&mut p, MoveProviderEvent::StepStarted);
    assert_eq!(
        p.try_moves(&(), &0, 0).unwrap().collect::<Vec<_>>(),
        [0, 1, 2]
    );
    event(&mut p, MoveProviderEvent::StepEnded);
    event(&mut p, MoveProviderEvent::PhaseEnded);
    assert_eq!(p.selection().state(), SelectionCacheState::Idle);
}
#[test]
fn sorted_provider_drives_real_construction_without_a_second_cache() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 3]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    let selection = SelectionBuilder::new(selection_config(
        SelectionOrder::Sorted,
        SelectionCacheScope::Phase,
    ))
    .with_comparator(|_, a: &Key, b: &Key| a.0.cmp(&b.0))
    .build(ConfiguredSelectionPolicy::default(), |_| {
        Entries::new(vec![2, 0, 1])
    })
    .unwrap();
    let mut provider = ConfiguredMoveProvider::new(
        selection,
        ConfiguredProviderConfig::default(),
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| 0,
        |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64, k: &Key| {
            a.values()[k.0].is_none().then_some(Move::Change {
                variable: k.0,
                value: Some(1),
            })
        },
    );
    let r = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut provider,
    )
    .unwrap();
    assert_eq!(r.assignment.values(), [Some(1), Some(1), Some(1)]);
    assert_eq!(provider.selection().source().builds.len(), 1);
    assert_eq!(provider.selection().state(), SelectionCacheState::Idle);
}

#[test]
fn owned_uncached_iteration_and_borrowed_mapping_do_not_clone_yielded_payloads() {
    struct Counted(Rc<Cell<usize>>);
    impl Clone for Counted {
        fn clone(&self) -> Self {
            self.0.set(self.0.get() + 1);
            Self(self.0.clone())
        }
    }
    struct Fresh(Rc<Cell<usize>>);
    impl SelectionSource<(), Counted> for Fresh {
        fn size(&mut self, _: &()) -> Option<usize> {
            Some(3)
        }
        fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = Counted> + 'a>
        where
            Counted: 'a,
        {
            Box::new((0..3).map(|_| Counted(self.0.clone())))
        }
    }
    let calls = Rc::new(Cell::new(0));
    let mut selection = SelectionBuilder::new(selection_config(
        SelectionOrder::Original,
        SelectionCacheScope::Uncached,
    ))
    .build(ConfiguredSelectionPolicy::default(), |_| {
        Fresh(calls.clone())
    })
    .unwrap();
    for e in [E::SolveStarted, E::PhaseStarted, E::StepStarted] {
        selection.event(e, &()).unwrap();
    }
    assert_eq!(selection.iter(&(), 0).unwrap().count(), 3);
    assert_eq!(selection.map_iter(&(), 0, |_| 1).unwrap().sum::<i32>(), 3);
    assert_eq!(calls.get(), 0);
}
