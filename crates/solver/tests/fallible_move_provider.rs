//! Native fallible provider contracts; no upstream source-method credit.
use MoveProviderEvent::*;
use std::{cell::RefCell, error::Error, fmt, marker::PhantomData, rc::Rc};
use timetuck::*;

#[derive(Debug)]
struct Failed;
impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("provider failed")
    }
}
impl Error for Failed {}
fn failure() -> MoveProviderError {
    MoveProviderError::new(Failed)
}
fn assert_failure(error: &(dyn Error + 'static)) {
    let mut current = Some(error);
    while let Some(e) = current {
        if e.is::<Failed>() {
            return;
        }
        current = e.source();
    }
    panic!("missing typed provider failure: {error:?}");
}
#[derive(Default)]
struct Trace {
    events: Vec<MoveProviderEvent>,
    opens: usize,
    polls: usize,
    drops: usize,
    completed: usize,
}
struct Script<M> {
    steps: Vec<Vec<Result<M, MoveProviderError>>>,
    setup_error: bool,
    trace: Rc<RefCell<Trace>>,
}
impl<M> Script<M> {
    fn new(steps: Vec<Vec<Result<M, MoveProviderError>>>) -> Self {
        Self {
            steps,
            setup_error: false,
            trace: Default::default(),
        }
    }
}
struct Cursor<M> {
    inner: std::vec::IntoIter<Result<M, MoveProviderError>>,
    trace: Rc<RefCell<Trace>>,
}
impl<M> Iterator for Cursor<M> {
    type Item = Result<M, MoveProviderError>;
    fn next(&mut self) -> Option<Self::Item> {
        self.trace.borrow_mut().polls += 1;
        self.inner.next()
    }
}
impl<M> Drop for Cursor<M> {
    fn drop(&mut self) {
        self.trace.borrow_mut().drops += 1;
    }
}
impl<P, A, M: Clone> FallibleMoveProvider<P, A, M> for Script<M> {
    fn event(&mut self, e: MoveProviderEvent) {
        let mut t = self.trace.borrow_mut();
        if matches!(e, StepEnded | PhaseEnded | SolveEnded) {
            assert_eq!(t.opens, t.drops + usize::from(self.setup_error));
        }
        t.events.push(e);
    }
    fn construction_placement_completed(&mut self, _: bool) -> bool {
        self.trace.borrow_mut().completed += 1;
        false
    }
    fn try_moves<'a>(
        &'a mut self,
        _: &'a P,
        _: &'a A,
        step: u64,
    ) -> Result<Box<dyn Iterator<Item = Result<M, MoveProviderError>> + 'a>, MoveProviderError>
    where
        M: 'a,
    {
        self.trace.borrow_mut().opens += 1;
        if self.setup_error {
            return Err(failure());
        }
        Ok(Box::new(Cursor {
            inner: self.steps[step as usize].clone().into_iter(),
            trace: self.trace.clone(),
        }))
    }
}
fn change(v: i32) -> Move<i32> {
    Move::Change {
        variable: 0,
        value: Some(v),
    }
}
fn relocate(from: usize, to: usize) -> ListMove<i32> {
    ListMove::Change {
        source: ListPosition {
            entity: from,
            index: 0,
        },
        destination: ListPosition {
            entity: to,
            index: 0,
        },
    }
}
fn config() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 2,
        acceptance: Acceptance::HillClimbing,
        selection: MoveSelection::BestAccepted,
        assert_scores: true,
        ..Default::default()
    }
}
fn plan() -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::LocalSearch(config())],
        ..Default::default()
    }
}
fn mixed_plan() -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        phases: vec![MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
            search: config(),
            ..Default::default()
        })],
        ..Default::default()
    }
}
struct Zero<C>(PhantomData<C>);
impl<C> Clone for Zero<C> {
    fn clone(&self) -> Self {
        Zero(PhantomData)
    }
}
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

#[test]
fn retained_nine_backends_discard_failed_scan_and_keep_last_completed_step() {
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
    macro_rules! check {
        ($director:expr,$problem:expr,$plan:expr,$first:expr,$second:expr) => {{
            let mut director = $director;
            let mut expected = $problem.initial_assignment();
            // Establish the reference through the ordinary validated transaction.
            let first = $first;
            let mut provider = Script::new(vec![
                vec![Ok(first.clone())],
                vec![Ok($second), Err(failure())],
            ]);
            let trace = provider.trace.clone();
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind_fallible(0, &mut provider);
            let error = director.run_solver_with_extensions($plan, ext).unwrap_err();
            assert_failure(&error);
            let mut reference = $director;
            reference.apply(&first).unwrap().commit();
            expected.clone_from(reference.assignment());
            assert_eq!(director.assignment(), &expected);
            assert_eq!(director.score(), reference.score());
            director.assert_consistent().unwrap();
            let t = trace.borrow();
            assert_eq!(
                t.events,
                vec![
                    SolveStarted,
                    PhaseStarted,
                    StepStarted,
                    StepEnded,
                    StepStarted,
                    StepEnded,
                    PhaseEnded,
                    SolveEnded
                ]
            );
            assert_eq!((t.opens, t.drops, t.completed), (2, 2, 0));
        }};
    }
    check!(
        ScoreDirector::new(&b, &bn, b.initial_assignment()).unwrap(),
        b,
        plan(),
        change(1),
        change(2)
    );
    check!(
        ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap(),
        l,
        plan(),
        relocate(0, 1),
        relocate(1, 0)
    );
    check!(
        MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap(),
        m,
        mixed_plan(),
        MixedMove {
            basic: change(1),
            lists: relocate(0, 1)
        },
        MixedMove {
            basic: change(2),
            lists: relocate(1, 0)
        }
    );
    let bg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = g.source();
        StreamScoreDirector::new(&b, g.build(), s, b.initial_assignment()).unwrap()
    };
    let lg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut g);
        ListStreamScoreDirector::new(&l, g.build(), s, l.initial_assignment()).unwrap()
    };
    let mg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::new(&mut g);
        MixedStreamScoreDirector::new(&m, g.build(), s, m.initial_assignment()).unwrap()
    };
    check!(bg(), b, plan(), change(1), change(2));
    check!(lg(), l, plan(), relocate(0, 1), relocate(1, 0));
    check!(
        mg(),
        m,
        mixed_plan(),
        MixedMove {
            basic: change(1),
            lists: relocate(0, 1)
        },
        MixedMove {
            basic: change(2),
            lists: relocate(1, 0)
        }
    );
    check!(
        CalculatorScoreDirector::new(&b, b.initial_assignment(), zero::<Vec<usize>>, false)
            .unwrap(),
        b,
        plan(),
        change(1),
        change(2)
    );
    check!(
        ListCalculatorScoreDirector::new(&l, l.initial_assignment(), zero::<Vec<usize>>, false)
            .unwrap(),
        l,
        plan(),
        relocate(0, 1),
        relocate(1, 0)
    );
    check!(
        MixedCalculatorScoreDirector::new(
            &m,
            m.initial_assignment(),
            zero::<MixedCalculatorChange>,
            false
        )
        .unwrap(),
        m,
        mixed_plan(),
        MixedMove {
            basic: change(1),
            lists: relocate(0, 1)
        },
        MixedMove {
            basic: change(2),
            lists: relocate(1, 0)
        }
    );
}

#[test]
fn setup_shuffle_and_unpolled_errors_respect_transaction_and_lazy_boundaries() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let calls = RefCell::new(Vec::new());
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        calls.borrow_mut().push(v[0].copied());
        SimpleScore::new(i64::from(*v[0].unwrap()))
    })
    .unwrap();
    for mode in 0..4 {
        let mut provider = Script::new(vec![vec![Ok(change(1)), Err(failure())]]);
        provider.setup_error = mode == 0;
        let mut c = config();
        c.max_steps = 1;
        c.assert_scores = false;
        if mode == 1 {
            c.move_order = MoveOrder::ShuffledMoves {
                seed: 4,
                window: 4.try_into().unwrap(),
            };
        }
        if mode == 2 {
            c.max_evaluations = 2;
        }
        if mode == 3 {
            c.selection = MoveSelection::FirstAccepted;
        }
        calls.borrow_mut().clear();
        let result = local_search_from_with_fallible_provider(
            &p,
            &n,
            p.initial_assignment(),
            c,
            &mut provider,
        );
        if mode < 3 {
            assert_failure(&result.unwrap_err());
            assert_eq!(
                *calls.borrow(),
                if mode == 2 {
                    vec![Some(0), Some(1)]
                } else {
                    vec![Some(0)]
                }
            );
        } else {
            assert_eq!(
                result.unwrap().best.unwrap().assignment.values(),
                &[Some(1)]
            );
            assert_eq!(provider.trace.borrow().polls, 1);
        }
    }
}

#[test]
fn shared_binding_removal_and_replacement_never_lose_fallible_borrows() {
    let mut a = Script::<Move<i32>>::new(vec![]);
    let mut b = Script::<Move<i32>>::new(vec![]);
    let mut c = Script::<Move<i32>>::new(vec![]);
    let mut old = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| vec![change(1)];
    let mut bindings =
        PhaseMoveProviders::<FiniteProblem<i32>, Assignment<i32>, Move<i32>>::default();
    bindings.bind_shared_fallible([0, 1, 0], &mut a).unwrap();
    assert!(bindings.remove(0).is_none());
    assert!(bindings.contains(0));
    assert!(bindings.bind_shared_fallible([2, 1], &mut b).is_err());
    assert!(!bindings.contains(2));
    assert!(bindings.take_binding(0).is_none());
    assert!(!bindings.contains(0));
    assert!(matches!(
        bindings.bind_provider(1, MoveProviderBinding::Infallible(&mut old)),
        Some(MoveProviderBinding::Fallible(_))
    ));
    assert!(matches!(
        bindings.bind_fallible(1, &mut c),
        Some(MoveProviderBinding::Infallible(_))
    ));
    assert!(matches!(
        bindings.take_binding(1),
        Some(MoveProviderBinding::Fallible(_))
    ));
    fn send_sync<T: Send + Sync>() {}
    send_sync::<MoveProviderError>();
    let e = failure();
    assert_eq!(e, e.clone());
    assert_ne!(e, failure());
    assert!(e.inner().is::<Failed>());
}

#[test]
fn construction_failure_discards_prefix_in_all_nine_retained_backends() {
    let b = FiniteProblem::new(vec![Variable::new(vec![0, 1])]).unwrap();
    let l = ListProblem::new(vec![7], vec![ListEntity::new(vec![])]).unwrap();
    let m = MixedProblem::new(b.clone(), l.clone());
    let bn = ConstraintNetwork::<_, SimpleScore>::new(1);
    let ln = ListConstraintNetwork::<_, SimpleScore>::new(1, 1);
    let mn = MixedConstraintNetwork::<_, _, SimpleScore>::new(1, 1, 1);
    let plan = || SolverSequence {
        phases: vec![SolverPhase::Construction(Default::default())],
        ..Default::default()
    };
    let mixed_plan = || MixedSolverSequence {
        phases: vec![MixedSolverPhase::Construction(Default::default())],
        ..Default::default()
    };
    let insert = ListMove::Assign {
        value: 7,
        destination: ListPosition {
            entity: 0,
            index: 0,
        },
    };
    macro_rules! check {
        ($director:expr,$p:expr,$plan:expr,$movement:expr) => {{
            let mut director = $director;
            let initial_score = director.score();
            let mut provider = Script::new(vec![vec![Ok($movement), Err(failure())]]);
            let trace = provider.trace.clone();
            let mut ext = SolverExtensions::default();
            ext.move_providers.bind_fallible(0, &mut provider);
            assert_failure(&director.run_solver_with_extensions($plan, ext).unwrap_err());
            assert_eq!(director.assignment(), &$p.initial_assignment());
            assert_eq!(director.score(), initial_score);
            director.assert_consistent().unwrap();
            let t = trace.borrow();
            assert_eq!(t.completed, 0);
            assert_eq!(t.polls, 2);
            assert_eq!(
                t.events,
                vec![
                    SolveStarted,
                    PhaseStarted,
                    StepStarted,
                    StepEnded,
                    PhaseEnded,
                    SolveEnded
                ]
            );
        }};
    }
    check!(
        ScoreDirector::new(&b, &bn, b.initial_assignment()).unwrap(),
        b,
        plan(),
        change(1)
    );
    check!(
        ListScoreDirector::new(&l, &ln, l.initial_assignment()).unwrap(),
        l,
        plan(),
        insert.clone()
    );
    check!(
        MixedScoreDirector::new(&m, &mn, m.initial_assignment()).unwrap(),
        m,
        mixed_plan(),
        MixedMove {
            basic: change(1),
            lists: insert.clone()
        }
    );
    let bg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = g.source();
        StreamScoreDirector::new(&b, g.build(), s, b.initial_assignment()).unwrap()
    };
    let lg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = ListStreamSources::new(&mut g);
        ListStreamScoreDirector::new(&l, g.build(), s, l.initial_assignment()).unwrap()
    };
    let mg = || {
        let mut g = StreamGraphBuilder::<SimpleScore>::new();
        let s = MixedStreamSources::new(&mut g);
        MixedStreamScoreDirector::new(&m, g.build(), s, m.initial_assignment()).unwrap()
    };
    check!(bg(), b, plan(), change(1));
    check!(lg(), l, plan(), insert.clone());
    check!(
        mg(),
        m,
        mixed_plan(),
        MixedMove {
            basic: change(1),
            lists: insert.clone()
        }
    );
    check!(
        CalculatorScoreDirector::new(&b, b.initial_assignment(), zero::<Vec<usize>>, false)
            .unwrap(),
        b,
        plan(),
        change(1)
    );
    check!(
        ListCalculatorScoreDirector::new(&l, l.initial_assignment(), zero::<Vec<usize>>, false)
            .unwrap(),
        l,
        plan(),
        insert.clone()
    );
    check!(
        MixedCalculatorScoreDirector::new(
            &m,
            m.initial_assignment(),
            zero::<MixedCalculatorChange>,
            false
        )
        .unwrap(),
        m,
        mixed_plan(),
        MixedMove {
            basic: change(1),
            lists: insert
        }
    );
}

#[test]
fn model_errors_precede_later_provider_errors_and_panics_remain_panics() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let invalid = Move::Change {
        variable: 99,
        value: Some(1),
    };
    let mut provider = Script::new(vec![vec![Ok(invalid), Err(failure())]]);
    let e = local_search_from_with_fallible_provider(
        &p,
        &n,
        p.initial_assignment(),
        config(),
        &mut provider,
    )
    .unwrap_err();
    assert!(matches!(
        e,
        LocalSearchError::Scoring(ScoringError::Model(_))
    ));
    assert_eq!(provider.trace.borrow().polls, 1);
    // Setup panics are not repackaged and unwinding emits no cleanup callbacks.
    struct Panics(Rc<RefCell<Vec<MoveProviderEvent>>>);
    impl FallibleMoveProvider<FiniteProblem<i32>, Assignment<i32>, Move<i32>> for Panics {
        fn event(&mut self, e: MoveProviderEvent) {
            self.0.borrow_mut().push(e);
        }
        fn try_moves<'a>(
            &'a mut self,
            _: &'a FiniteProblem<i32>,
            _: &'a Assignment<i32>,
            _: u64,
        ) -> Result<
            Box<dyn Iterator<Item = Result<Move<i32>, MoveProviderError>> + 'a>,
            MoveProviderError,
        >
        where
            Move<i32>: 'a,
        {
            panic!("unwrapped panic")
        }
    }
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut panics = Panics(log.clone());
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        local_search_from_with_fallible_provider(
            &p,
            &n,
            p.initial_assignment(),
            config(),
            &mut panics,
        )
    }));
    assert!(r.is_err());
    assert_eq!(*log.borrow(), vec![PhaseStarted, StepStarted]);
}

#[test]
fn closure_provider_setup_error_is_typed_and_zero_budget_does_not_open_it() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let calls = std::cell::Cell::new(0);
    let mut provider = |_: &FiniteProblem<i32>,
                        _: &Assignment<i32>,
                        _: u64|
     -> Result<Vec<Move<i32>>, MoveProviderError> {
        calls.set(calls.get() + 1);
        Err(failure())
    };
    let mut c = config();
    c.max_evaluations = 0;
    assert!(
        local_search_from_with_fallible_provider(&p, &n, p.initial_assignment(), c, &mut provider)
            .is_ok()
    );
    assert_eq!(calls.get(), 0);
    assert_failure(
        &local_search_from_with_fallible_provider(
            &p,
            &n,
            p.initial_assignment(),
            config(),
            &mut provider,
        )
        .unwrap_err(),
    );
    assert_eq!(calls.get(), 1);
    assert!(provider.try_moves(&p, &p.initial_assignment(), 0).is_err());
    assert_eq!(calls.get(), 2);
}
