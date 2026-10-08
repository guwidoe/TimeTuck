//! Native engine lifecycle cases; no upstream method credit.
use MoveProviderEvent::*;
use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    rc::Rc,
};
use timetuck::*;

type Log = Rc<RefCell<Vec<&'static str>>>;
struct Tracked<Q> {
    inner: Q,
    log: Log,
    cursors: Rc<Cell<usize>>,
}
fn tracked<Q>(inner: Q, log: &Log) -> Tracked<Q> {
    Tracked {
        inner,
        log: log.clone(),
        cursors: Rc::new(Cell::new(0)),
    }
}
struct Cursor<I> {
    inner: I,
    log: Log,
    cursors: Rc<Cell<usize>>,
}
impl<I: Iterator> Iterator for Cursor<I> {
    type Item = I::Item;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}
impl<I> Drop for Cursor<I> {
    fn drop(&mut self) {
        self.cursors.set(self.cursors.get() - 1);
        self.log.borrow_mut().push("drop");
    }
}
impl<P, A, M, Q: MoveProvider<P, A, M>> MoveProvider<P, A, M> for Tracked<Q> {
    fn event(&mut self, event: MoveProviderEvent) {
        assert_eq!(self.cursors.get(), 0, "event before cursor destruction");
        self.log.borrow_mut().push(match event {
            SolveStarted => "solve+",
            SolveEnded => "solve-",
            PhaseStarted => "phase+",
            StepStarted => "step+",
            StepEnded => "step-",
            PhaseEnded => "phase-",
        });
        self.inner.event(event);
    }
    fn construction_placement_completed(&mut self, installed: bool) -> bool {
        assert_eq!(self.cursors.get(), 0);
        self.log
            .borrow_mut()
            .push(if installed { "installed" } else { "rejected" });
        self.inner.construction_placement_completed(installed)
    }
    fn moves<'a>(&'a mut self, p: &'a P, a: &'a A, step: u64) -> Box<dyn Iterator<Item = M> + 'a>
    where
        M: 'a,
    {
        self.log.borrow_mut().push("open");
        let inner = self.inner.moves(p, a, step);
        self.cursors.set(self.cursors.get() + 1);
        Box::new(Cursor {
            inner,
            log: self.log.clone(),
            cursors: self.cursors.clone(),
        })
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
fn change(value: i32) -> Move<i32> {
    Move::Change {
        variable: 0,
        value: Some(value),
    }
}
fn insert() -> ListMove<i32> {
    ListMove::Assign {
        value: 7,
        destination: ListPosition {
            entity: 0,
            index: 0,
        },
    }
}
fn relocate() -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: 0,
            index: 0,
        },
        destination: ListPosition {
            entity: 1,
            index: 0,
        },
    }
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        limits: SolverLimits {
            assert_phase_scores: true,
            ..Default::default()
        },
        phases: vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                acceptance: Acceptance::HillClimbing,
                ..Default::default()
            }),
        ],
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: plan().limits,
        phases: vec![
            MixedSolverPhase::Construction(Default::default()),
            MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                search: LocalSearchConfig {
                    max_steps: 1,
                    acceptance: Acceptance::HillClimbing,
                    ..Default::default()
                },
                ..Default::default()
            }),
        ],
    }
}
#[test]
fn actual_phase_and_scan_boundaries_in_all_nine_backends_without_diagnostic_replay() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let l = ListProblem::new(
        vec![7],
        vec![ListEntity::new(vec![]), ListEntity::new(vec![])],
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
        ($solve:ident,$p:expr,$scorer:expr,$plan:expr,$p_ty:ty,$a_ty:ty,$construction:expr,$search:expr) => {{
            let log = Log::default();
            let mut construction =
                tracked(|_: &$p_ty, _: &$a_ty, _: u64| vec![$construction], &log);
            let mut search = tracked(|_: &$p_ty, _: &$a_ty, _: u64| vec![$search], &log);
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind(0, &mut construction);
            ext.move_providers.bind(1, &mut search);
            let result = $solve(&$p, $scorer, $plan, ext).unwrap();
            assert!(result.complete);
            assert_eq!(result.work.construction_steps, 1);
            assert_eq!(result.work.completed_steps, 1);
            assert_eq!(
                *log.borrow(),
                vec![
                    "solve+",
                    "solve+",
                    "phase+",
                    "step+",
                    "open",
                    "drop",
                    "installed",
                    "step-",
                    "phase-",
                    "phase+",
                    "step+",
                    "open",
                    "drop",
                    "step-",
                    "phase-",
                    "solve-",
                    "solve-",
                ]
            );
        }};
    }
    check!(
        solve_with_extensions,
        b,
        &bn,
        plan(),
        FiniteProblem<i32>,
        Assignment<i32>,
        change(1),
        change(0)
    );
    check!(
        solve_stream_with_extensions,
        b,
        bg,
        plan(),
        FiniteProblem<i32>,
        Assignment<i32>,
        change(1),
        change(0)
    );
    check!(
        solve_calculator_with_extensions,
        b,
        &zero::<Vec<usize>>,
        plan(),
        FiniteProblem<i32>,
        Assignment<i32>,
        change(1),
        change(0)
    );
    check!(
        solve_list_with_extensions,
        l,
        &ln,
        plan(),
        ListProblem<i32>,
        ListAssignment<i32>,
        insert(),
        relocate()
    );
    check!(
        solve_list_stream_with_extensions,
        l,
        lg,
        plan(),
        ListProblem<i32>,
        ListAssignment<i32>,
        insert(),
        relocate()
    );
    check!(
        solve_list_calculator_with_extensions,
        l,
        &zero::<Vec<usize>>,
        plan(),
        ListProblem<i32>,
        ListAssignment<i32>,
        insert(),
        relocate()
    );
    check!(solve_mixed_with_extensions,m,&mn,mixed_plan(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,MixedMove { basic:change(1),lists:insert() },MixedMove { basic:change(0),lists:relocate() });
    check!(solve_mixed_stream_with_extensions,m,mg,mixed_plan(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,MixedMove { basic:change(1),lists:insert() },MixedMove { basic:change(0),lists:relocate() });
    check!(solve_mixed_calculator_with_extensions,m,&zero::<MixedCalculatorChange>,mixed_plan(),MixedProblem<i32,i32>,MixedAssignment<i32,i32>,MixedMove { basic:change(1),lists:insert() },MixedMove { basic:change(0),lists:relocate() });
}
#[test]
fn empty_error_cancelled_and_discarded_scans_close_without_claiming_completion() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    for mode in 0..4 {
        let log = Log::default();
        let mut provider = tracked(
            |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| match mode {
                0 => vec![],
                1 => vec![Move::Change {
                    variable: 99,
                    value: Some(1),
                }],
                _ => vec![change(1), change(0)],
            },
            &log,
        );
        if mode == 1 {
            let mut initial = p.initial_assignment();
            initial.apply(&change(0)).unwrap().commit();
            assert!(
                local_search_from_with_provider(
                    &p,
                    &n,
                    initial,
                    LocalSearchConfig::default(),
                    &mut provider
                )
                .is_err()
            );
        } else {
            let r = construction_from_with_provider(
                &p,
                &n,
                p.initial_assignment(),
                ProviderConstructionConfig {
                    max_scanned_moves: if mode == 2 { 1 } else { 100 },
                    max_evaluations: if mode == 3 { 0 } else { 100 },
                    interruption: ConstructionInterruption::DiscardIncompleteStep,
                    ..Default::default()
                },
                &mut provider,
            )
            .unwrap();
            assert_eq!(r.completed_steps, 0);
        }
        assert_eq!(
            *log.borrow(),
            if mode == 3 {
                vec![]
            } else {
                vec!["phase+", "step+", "open", "drop", "step-", "phase-"]
            }
        );
    }
    let cancel = CancellationToken::new();
    cancel.cancel();
    let log = Log::default();
    let mut provider = tracked(
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| vec![change(1)],
        &log,
    );
    construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig {
            cancellation: Some(&cancel),
            ..Default::default()
        },
        &mut provider,
    )
    .unwrap();
    assert!(log.borrow().is_empty());
}
#[test]
fn panic_drops_cursors_without_running_more_user_callbacks() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("panic", vec![0], |v| {
        assert!(v[0].is_none(), "score failure");
        SimpleScore::ZERO
    })
    .unwrap();
    let log = Log::default();
    let mut provider = tracked(
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| vec![change(1)],
        &log,
    );
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig::default(),
            &mut provider,
        )
    }));
    assert!(r.is_err());
    assert_eq!(*log.borrow(), vec!["phase+", "step+", "open", "drop"]);
}
#[test]
fn filter_forwards_optional_rejection_and_phase_reset_rewinds_an_exhausted_queue() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).allowing_unassigned(),
        Variable::new(vec![1]).allowing_unassigned(),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("value", vec![0, 1], |v| {
        SimpleScore::new(
            -i64::from(v[0].copied().unwrap_or(0)) + i64::from(v[1].copied().unwrap_or(0)),
        )
    })
    .unwrap();
    let queue = QueuedMoveProvider::new(
        vec![0usize, 1],
        PlacementQueueOrder::Once,
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, &key: &usize| {
            vec![Move::Change {
                variable: key,
                value: Some(1),
            }]
        },
    );
    let reset = PhaseResetMoveProvider::new(queue, |q| q.reset());
    let mut filter = FilteredMoveProvider::new(
        reset,
        100,
        100,
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64, _: &Move<i32>| true,
    );
    for _ in 0..2 {
        let r = construction_from_with_provider(
            &p,
            &n,
            p.initial_assignment(),
            ProviderConstructionConfig {
                optional: OptionalConstruction::EvaluateUnassigned,
                ..Default::default()
            },
            &mut filter,
        )
        .unwrap();
        assert_eq!(r.assignment.values(), &[None, Some(1)]);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
    }
}
#[test]
fn union_forwards_boundaries_to_enabled_children_without_opening_lazy_cursors() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let logs: [Log; 3] = std::array::from_fn(|_| Log::default());
    let mut a = tracked(
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| vec![change(1)],
        &logs[0],
    );
    let mut b = tracked(
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| panic!("unopened child"),
        &logs[1],
    );
    let mut c = tracked(
        |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| panic!("disabled child"),
        &logs[2],
    );
    let mut union = ProviderUnion::new(
        vec![
            (&mut a as &mut dyn MoveProvider<_, _, _>, 1),
            (&mut b, 1),
            (&mut c, 0),
        ],
        UnionOrder::Sequential,
    )
    .unwrap();
    let r = construction_from_with_provider(
        &p,
        &n,
        p.initial_assignment(),
        ProviderConstructionConfig {
            max_evaluations: 2,
            ..Default::default()
        },
        &mut union,
    )
    .unwrap();
    assert!(r.complete);
    assert_eq!(
        *logs[0].borrow(),
        vec!["phase+", "step+", "open", "drop", "step-", "phase-"]
    );
    assert_eq!(
        *logs[1].borrow(),
        vec!["phase+", "step+", "step-", "phase-"]
    );
    assert!(logs[2].borrow().is_empty());
}

struct LiveCalculator {
    value: Rc<Cell<Option<i32>>>,
    fail_reset: bool,
}
impl IncrementalCalculator<Assignment<i32>> for LiveCalculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = Vec<usize>;
    fn reset(
        &mut self,
        a: &Assignment<i32>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        if self.fail_reset {
            return Err(MatchRegistryError::Calculator("reset failed".into()));
        }
        self.value.set(a.values()[0]);
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<i32>,
        _: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.value.set(a.values()[0]);
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        SimpleScore::new(i64::from(self.value.get().unwrap_or(0)))
    }
}
struct ObserveEnd {
    value: Rc<Cell<Option<i32>>>,
    ended: Rc<Cell<bool>>,
}
impl MoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for ObserveEnd {
    fn event(&mut self, e: MoveProviderEvent) {
        if e == StepEnded {
            assert_eq!(
                self.value.get(),
                Some(1),
                "winner must already be installed"
            );
            self.ended.set(true);
        }
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
        Box::new(std::iter::once(change(1)))
    }
}
#[test]
fn end_callback_observes_committed_calculator_state_and_initial_errors_close_phase() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    for fail_reset in [false, true] {
        let value = Rc::new(Cell::new(None));
        let ended = Rc::new(Cell::new(false));
        let log = Log::default();
        let mut provider = tracked(
            ObserveEnd {
                value: value.clone(),
                ended: ended.clone(),
            },
            &log,
        );
        let result = calculator_local_search_with_provider(
            &p,
            p.initial_assignment(),
            || LiveCalculator {
                value: value.clone(),
                fail_reset,
            },
            LocalSearchConfig {
                max_steps: 1,
                ..Default::default()
            },
            &mut provider,
        );
        assert_eq!(result.is_err(), fail_reset);
        assert_eq!(ended.get(), !fail_reset);
        assert_eq!(
            *log.borrow(),
            if fail_reset {
                vec!["phase+", "phase-"]
            } else {
                vec!["phase+", "step+", "open", "drop", "step-", "phase-"]
            }
        );
    }
}

struct Factory {
    log: Log,
}
impl PlacementMoveFactory<FiniteProblem<i32>, Assignment<i32>, usize, Move<i32>> for Factory {
    fn event(&mut self, e: MoveProviderEvent) {
        self.log.borrow_mut().push(match e {
            SolveStarted => "solve+",
            SolveEnded => "solve-",
            PhaseStarted => "phase+",
            StepStarted => "step+",
            StepEnded => "step-",
            PhaseEnded => "phase-",
        });
    }
    fn candidates<'a>(
        &mut self,
        _: &'a FiniteProblem<i32>,
        _: &'a Assignment<i32>,
        key: &usize,
    ) -> Box<dyn Iterator<Item = Move<i32>> + 'a>
    where
        Move<i32>: 'a,
    {
        Box::new(std::iter::once(Move::Change {
            variable: *key,
            value: Some(1),
        }))
    }
}
#[test]
fn placement_factories_receive_boundaries_through_mixed_composition() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let l = ListProblem::new(vec![7], vec![ListEntity::new(vec![])]).unwrap();
    let m = MixedProblem::new(b, l);
    let log = Log::default();
    let queue = QueuedMoveProvider::new(
        vec![0usize],
        PlacementQueueOrder::Once,
        Factory { log: log.clone() },
    );
    let lists = SortedListConstruction::new(
        m.lists(),
        ConstructionPlacement::ValueQueue,
        OptionalConstruction::Preserve,
        |a, b| a.cmp(&b),
        |a, b| a.cmp(b),
    );
    let mut provider = PhaseResetMoveProvider::new(
        MixedConstructionProvider::new(&m, queue, lists, MixedConstructionOrder::BasicThenLists),
        |p| {
            p.reset_with(|b, l| {
                b.reset();
                l.reset();
            })
        },
    );
    let n = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1);
    for _ in 0..2 {
        let r = mixed_construction_from_with_provider(
            &m,
            &n,
            m.initial_assignment(),
            ProviderConstructionConfig::default(),
            &mut provider,
        )
        .unwrap();
        assert!(r.complete);
        assert_eq!(provider.provider().component_steps(), [1, 1]);
        assert_eq!(
            log.take(),
            vec!["phase+", "step+", "step-", "step+", "step-", "phase-"]
        );
    }
}
