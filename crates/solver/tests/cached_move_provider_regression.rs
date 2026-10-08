//! Native cached-source integration; no upstream method credit.
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
impl SelectionSource<i32, usize> for Entries {
    fn event(&mut self, e: E, c: &i32) {
        self.events.push((e, *c));
    }
    fn size(&mut self, _: &i32) -> Option<usize> {
        self.declared.or(Some(self.keys.len()))
    }
    fn open<'a>(&'a mut self, c: &'a i32) -> Box<dyn Iterator<Item = usize> + 'a>
    where
        usize: 'a,
    {
        self.builds.push(*c);
        Box::new(self.keys.iter().copied())
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
fn cached_sources_drive_all_nine_backends_and_release_at_real_phase_boundaries() {
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
                ProviderCacheScope::Solve,
                ProviderCacheScope::Phase,
                ProviderCacheScope::Step,
            ] {
                let mut provider = CachedMoveProvider::uniform(
                    Entries::new(vec![0]),
                    CachedProviderConfig {
                        scope,
                        ..Default::default()
                    },
                    |_: &$p_ty, a: &$a_ty, _: u64| ($capture)(a),
                    |_: &$p_ty, a: &$a_ty, _: u64, _: &usize| Some(($movement)(a)),
                )
                .unwrap();
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
                    assert_eq!(provider.cache().state(), SelectionCacheState::Idle);
                    assert!(matches!(
                        provider.cache().size(),
                        Err(SelectionCacheError::NotReady)
                    ));
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
                    assert_eq!(provider.cache().source().events, events.repeat(run));
                    let builds = match scope {
                        ProviderCacheScope::Solve => vec![0],
                        ProviderCacheScope::Phase => vec![0, 0],
                        ProviderCacheScope::Step => vec![0, 1, 0, 1],
                    };
                    assert_eq!(provider.cache().source().builds, builds.repeat(run));
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
#[test]
fn construction_maps_retained_keys_against_current_assignments() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1]); 3]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(3);
    for scope in [ProviderCacheScope::Phase, ProviderCacheScope::Step] {
        let mut provider = CachedMoveProvider::uniform(
            Entries::new(vec![2, 0, 1]),
            CachedProviderConfig {
                scope,
                ..Default::default()
            },
            |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64| {
                a.values().iter().filter(|x| x.is_none()).count() as i32
            },
            |_: &FiniteProblem<i32>, a: &Assignment<i32>, _: u64, &key: &usize| {
                a.values()[key].is_none().then_some(Move::Change {
                    variable: key,
                    value: Some(1),
                })
            },
        )
        .unwrap();
        let r = construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig::default(),
            &mut provider,
        )
        .unwrap();
        assert!(r.complete);
        assert_eq!(r.completed_steps, 3);
        assert_eq!(r.evaluations, 7);
        assert_eq!(
            provider.cache().source().builds,
            if scope == ProviderCacheScope::Phase {
                vec![3]
            } else {
                vec![3, 2, 1]
            }
        );
        assert_eq!(provider.cache().state(), SelectionCacheState::Idle);
    }
}
#[test]
fn random_draws_are_weighted_replayable_and_bounded_even_when_every_key_is_rejected() {
    let calls = Cell::new(0);
    let mut provider = CachedMoveProvider::new(
        Entries::new(vec![0, 1, 2]),
        CachedProviderConfig {
            order: CachedSelectionOrder::Random,
            maximum_draws: 17,
            seed: 91,
            ..Default::default()
        },
        |_: &i32, &key: &usize| if key == 0 { 0.0 } else { 1.0 },
        |_: &(), _: &(), _: u64| 0,
        |_: &(), _: &(), _: u64, &key: &usize| Some(key),
    )
    .unwrap();
    provider.event(MoveProviderEvent::PhaseStarted);
    provider.event(MoveProviderEvent::StepStarted);
    let first = provider.moves(&(), &(), u64::MAX).collect::<Vec<_>>();
    assert_eq!(first.len(), 17);
    assert!(first.iter().all(|x| *x != 0));
    assert_eq!(
        first,
        provider.moves(&(), &(), u64::MAX).collect::<Vec<_>>()
    );
    provider.event(MoveProviderEvent::StepEnded);
    provider.event(MoveProviderEvent::PhaseEnded);
    assert_eq!(provider.cache().source().builds, vec![0]);
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let mut rejected = CachedMoveProvider::uniform(
        Entries::new(vec![0]),
        CachedProviderConfig {
            order: CachedSelectionOrder::Random,
            maximum_draws: 17,
            ..Default::default()
        },
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| 0,
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, _: &usize| -> Option<Move<i32>> {
            calls.set(calls.get() + 1);
            None
        },
    )
    .unwrap();
    let r = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig::default(),
        &mut rejected,
    )
    .unwrap();
    assert_eq!(calls.get(), 17);
    assert_eq!(r.evaluations, 1);
    assert_eq!(r.termination, ProviderConstructionTermination::NoCandidates);
}
#[test]
fn unopened_scans_and_zero_draws_do_not_build_or_capture_context() {
    for maximum_draws in [0, 1] {
        let mut provider = CachedMoveProvider::uniform(
            Entries::new(vec![0]),
            CachedProviderConfig {
                maximum_draws,
                ..Default::default()
            },
            |_: &(), _: &(), _: u64| panic!("context must remain lazy"),
            |_: &(), _: &(), _: u64, &key: &usize| Some(key),
        )
        .unwrap();
        assert!(matches!(
            provider.try_moves(&(), &(), 0),
            Err(CachedProviderError::OutsideStep)
        ));
        provider.event(MoveProviderEvent::PhaseStarted);
        provider.event(MoveProviderEvent::StepStarted);
        if maximum_draws == 0 {
            assert_eq!(provider.moves(&(), &(), 0).count(), 0);
        }
        provider.event(MoveProviderEvent::StepEnded);
        provider.event(MoveProviderEvent::PhaseEnded);
        assert!(provider.cache().source().events.is_empty());
        assert!(provider.cache().source().builds.is_empty());
    }
}
#[test]
fn source_failures_return_errors_or_panic_in_engine_and_reset_allows_reuse() {
    for scope in [ProviderCacheScope::Phase, ProviderCacheScope::Step] {
        let mut entries = Entries::new(vec![0]);
        entries.declared = Some(2);
        let mut provider = CachedMoveProvider::uniform(
            entries,
            CachedProviderConfig {
                scope,
                ..Default::default()
            },
            |_: &(), _: &(), _: u64| 0,
            |_: &(), _: &(), _: u64, &key: &usize| Some(key),
        )
        .unwrap();
        provider.event(MoveProviderEvent::PhaseStarted);
        provider.event(MoveProviderEvent::StepStarted);
        assert!(matches!(
            provider.try_moves(&(), &(), 0),
            Err(CachedProviderError::Cache(
                SelectionCacheError::SizeMismatch {
                    declared: 2,
                    actual: 1
                }
            ))
        ));
        assert!(matches!(
            provider.cache().size(),
            Err(SelectionCacheError::NotReady)
        ));
        provider.reset().unwrap();
        assert_eq!(provider.cache().state(), SelectionCacheState::Idle);
        provider.source_mut().declared = None;
        provider.event(MoveProviderEvent::PhaseStarted);
        provider.event(MoveProviderEvent::StepStarted);
        assert_eq!(provider.moves(&(), &(), 0).collect::<Vec<_>>(), vec![0]);
        provider.event(MoveProviderEvent::StepEnded);
        provider.event(MoveProviderEvent::PhaseEnded);
    }
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let mut provider = CachedMoveProvider::uniform(
        Entries::new(vec![0, 1]),
        CachedProviderConfig {
            maximum_candidates: 1,
            ..Default::default()
        },
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| 0,
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, &key: &usize| {
            Some(Move::Change {
                variable: key,
                value: Some(1),
            })
        },
    )
    .unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            construction_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                ProviderConstructionConfig::default(),
                &mut provider,
            )
        }))
        .is_err()
    );
    provider.reset().unwrap();
    provider.source_mut().keys.pop();
    assert!(
        construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig::default(),
            &mut provider
        )
        .unwrap()
        .complete
    );
}
struct OwnedKey(Rc<Cell<usize>>);
impl Drop for OwnedKey {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
struct OwnedSource(Rc<Cell<usize>>);
impl SelectionSource<(), OwnedKey> for OwnedSource {
    fn size(&mut self, _: &()) -> Option<usize> {
        Some(1)
    }
    fn open<'a>(&'a mut self, _: &'a ()) -> Box<dyn Iterator<Item = OwnedKey> + 'a>
    where
        OwnedKey: 'a,
    {
        Box::new(std::iter::once(OwnedKey(self.0.clone())))
    }
}
#[test]
fn nonclone_keys_release_at_scope_end_but_owned_snapshots_remain_valid() {
    for scope in [ProviderCacheScope::Phase, ProviderCacheScope::Step] {
        let drops = Rc::new(Cell::new(0));
        let mut provider = CachedMoveProvider::uniform(
            OwnedSource(drops.clone()),
            CachedProviderConfig {
                scope,
                ..Default::default()
            },
            |_: &(), _: &(), _: u64| (),
            |_: &(), _: &(), _: u64, _: &OwnedKey| Some(7),
        )
        .unwrap();
        provider.event(MoveProviderEvent::PhaseStarted);
        provider.event(MoveProviderEvent::StepStarted);
        assert_eq!(provider.moves(&(), &(), 0).collect::<Vec<_>>(), vec![7]);
        let snapshot = provider.cache().snapshot().unwrap();
        provider.event(MoveProviderEvent::StepEnded);
        assert_eq!(
            provider.cache().size().is_ok(),
            scope == ProviderCacheScope::Phase
        );
        provider.event(MoveProviderEvent::PhaseEnded);
        assert_eq!(drops.get(), 0);
        assert_eq!(snapshot.len(), 1);
        drop(snapshot);
        assert_eq!(drops.get(), 1);
    }
}
