use std::{
    cell::Cell,
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
struct Hook<D> {
    mode: Rc<Cell<u8>>,
    calls: Rc<Cell<usize>>,
    handle: Option<MatchRegistration<SimpleScore, ()>>,
    marker: PhantomData<D>,
}
impl<D> Hook<D> {
    fn new(mode: Rc<Cell<u8>>, calls: Rc<Cell<usize>>) -> Self {
        Self {
            mode,
            calls,
            handle: None,
            marker: PhantomData,
        }
    }
}
impl<P, D> IncrementalCalculator<P> for Hook<D> {
    type Score = SimpleScore;
    type Justification = ();
    type Change = D;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        _: &P,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.handle = None;
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        panic!("fallible override must be used")
    }
    fn calculate_score_with_matches(
        &mut self,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<SimpleScore, MatchRegistryError> {
        self.calls.set(self.calls.get() + 1);
        if r.is_enabled() {
            if let Some(h) = self.handle.take() {
                h.cancel(r)?;
            }
            self.handle = Some(r.register("late match", SimpleScore::new(9), ())?);
        }
        match self.mode.get() {
            1 => Err(MatchRegistryError::Calculator("late score failed".into())),
            2 => panic!("late score panic"),
            _ => Ok(SimpleScore::new(9)),
        }
    }
}
#[test]
fn session_repeated_scoring_owns_cancellation_and_recovers_after_error_or_panic() {
    let mode = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut session =
        CalculatorSession::new((), Hook::<()>::new(mode.clone(), calls.clone()), true).unwrap();
    for _ in 0..3 {
        assert_eq!(session.score().unwrap(), SimpleScore::new(9));
        assert_eq!(
            *session.registry().unwrap().total().unwrap(),
            SimpleScore::new(9)
        );
        assert_eq!(
            session.registry().unwrap().constraints().unwrap()["late match"]
                .matches()
                .len(),
            1
        );
    }
    assert_eq!(calls.get(), 3);
    mode.set(1);
    assert_eq!(
        session.score(),
        Err(MatchRegistryError::Calculator("late score failed".into()))
    );
    assert_eq!(session.score(), Err(MatchRegistryError::NeedsReset));
    assert!(matches!(
        session.registry(),
        Err(MatchRegistryError::NeedsReset)
    ));
    assert_eq!(calls.get(), 4);
    mode.set(0);
    session.reset(()).unwrap();
    assert_eq!(session.score().unwrap(), SimpleScore::new(9));
    mode.set(2);
    assert!(catch_unwind(AssertUnwindSafe(|| session.score())).is_err());
    assert_eq!(session.score(), Err(MatchRegistryError::NeedsReset));
    mode.set(0);
    session.reset(()).unwrap();
    assert_eq!(session.score().unwrap(), SimpleScore::new(9));
}
macro_rules! director_case {
    ($test:ident,$director:ident,$change:ty,$problem:expr,$movement:expr) => {
        #[test]
        fn $test() {
            let mode = Rc::new(Cell::new(0));
            let calls = Rc::new(Cell::new(0));
            let p = $problem;
            let factory = || Hook::<$change>::new(mode.clone(), calls.clone());
            let mut d = $director::new(&p, p.initial_assignment(), factory, true).unwrap();
            assert_eq!(calls.get(), 1);
            assert_eq!(d.score().unwrap().with_structural(0), SimpleScore::new(9));
            assert!(d.explain_default(|_| vec![1]).unwrap().matches_agree());
            assert_eq!(calls.get(), 1);
            let initial = d.assignment().clone();
            mode.set(1);
            assert!(d.apply(&$movement).is_err());
            assert_eq!(d.assignment(), &initial);
            assert!(d.score().is_err());
            mode.set(0);
            d.reset(initial.clone()).unwrap();
            assert!(d.explain_default(|_| vec![1]).unwrap().matches_agree());
            mode.set(2);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _guard = d.apply(&$movement);
                }))
                .is_err()
            );
            assert_eq!(d.assignment(), &initial);
            assert!(d.score().is_err());
            mode.set(0);
            d.reset(initial).unwrap();
            assert!(d.explain_default(|_| vec![1]).unwrap().matches_agree());
            let disabled = $director::new(&p, p.initial_assignment(), factory, false).unwrap();
            assert_eq!(
                disabled.score().unwrap().with_structural(0),
                SimpleScore::new(9)
            );
        }
    };
}
fn basic() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![2, 4]); 2]).unwrap()
}
fn lists() -> ListProblem<i64> {
    ListProblem::new(vec![2, 4], vec![ListEntity::new(vec![]); 2]).unwrap()
}
fn assign() -> ListMove<i64> {
    ListMove::Assign {
        value: 2,
        destination: ListPosition {
            entity: 0,
            index: 0,
        },
    }
}
director_case!(
    basic_hook,
    CalculatorScoreDirector,
    Vec<usize>,
    basic(),
    Move::Change {
        variable: 0,
        value: Some(2)
    }
);
director_case!(
    list_hook,
    ListCalculatorScoreDirector,
    Vec<usize>,
    lists(),
    assign()
);
director_case!(
    mixed_hook,
    MixedCalculatorScoreDirector,
    MixedCalculatorChange,
    MixedProblem::new(basic(), lists()),
    MixedMove {
        basic: Move::Change {
            variable: 0,
            value: Some(2)
        },
        lists: assign()
    }
);
#[test]
fn complete_plans_use_fallible_scoring_with_matches_disabled() {
    let mode = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let factory = || Hook::<Vec<usize>>::new(mode.clone(), calls.clone());
    let plan = SolverSequence {
        phases: vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ],
        ..Default::default()
    };
    let a = solve_calculator(&basic(), &factory, plan.clone()).unwrap();
    let b = solve_list_calculator(&lists(), &factory, plan).unwrap();
    assert!(a.is_optimal() && b.is_optimal());
    assert_eq!(a.score.unwrap(), SimpleScore::new(9));
    assert_eq!(b.score.unwrap(), SimpleScore::new(9));
    let m = MixedProblem::new(basic(), lists());
    let c = solve_mixed_calculator(
        &m,
        &|| Hook::<MixedCalculatorChange>::new(mode.clone(), calls.clone()),
        MixedSolverSequence {
            phases: vec![
                MixedSolverPhase::Construction(MixedConstructionConfig::default()),
                MixedSolverPhase::LocalSearch(MixedLocalSearchConfig {
                    search: LocalSearchConfig {
                        max_steps: 1,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                MixedSolverPhase::Exact(ExactPhaseConfig::default()),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    assert!(c.is_optimal());
    assert_eq!(c.score.unwrap(), SimpleScore::new(9));
    assert!(calls.get() > 3);
    mode.set(1);
    assert!(solve_calculator(&basic(), &factory, SolverConfig::default()).is_err());
}
