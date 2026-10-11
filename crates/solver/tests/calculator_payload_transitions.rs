//! Original retained calculator payload-transition regressions; no upstream credit.
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use timetuck::*;

#[derive(Clone, Debug)]
struct Value {
    id: u8,
    payload: i64,
}
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Value {}
fn value(id: u8, payload: i64) -> Value {
    Value { id, payload }
}
#[derive(Default)]
struct Counts {
    resets: Cell<u64>,
    scores: Cell<u64>,
    before: Cell<u64>,
    after: Cell<u64>,
    reset_payloads: RefCell<Vec<Vec<i64>>>,
    fail_payload: Cell<Option<i64>>,
    panic: Cell<bool>,
}
trait Payloads {
    type Change;
    fn payloads(&self) -> Vec<i64>;
}
impl Payloads for Assignment<Value> {
    type Change = Vec<usize>;
    fn payloads(&self) -> Vec<i64> {
        self.values().iter().flatten().map(|v| v.payload).collect()
    }
}
impl Payloads for ListAssignment<Value> {
    type Change = Vec<usize>;
    fn payloads(&self) -> Vec<i64> {
        self.lists().iter().flatten().map(|v| v.payload).collect()
    }
}
impl Payloads for MixedAssignment<Value, Value> {
    type Change = MixedCalculatorChange;
    fn payloads(&self) -> Vec<i64> {
        self.basic()
            .payloads()
            .into_iter()
            .chain(self.lists().payloads())
            .collect()
    }
}
#[derive(Clone)]
struct Calculator {
    total: i64,
    handle: Option<MatchRegistration<SimpleScore, ()>>,
    counts: Rc<Counts>,
}
impl Calculator {
    fn new(counts: &Rc<Counts>) -> Self {
        Self {
            total: 0,
            handle: None,
            counts: counts.clone(),
        }
    }
}
impl<A: Payloads> IncrementalCalculator<A> for Calculator {
    type Score = SimpleScore;
    type Justification = ();
    type Change = A::Change;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        a: &A,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.resets.set(self.counts.resets.get() + 1);
        let payloads = a.payloads();
        self.counts
            .reset_payloads
            .borrow_mut()
            .push(payloads.clone());
        if self
            .counts
            .fail_payload
            .get()
            .is_some_and(|bad| payloads.contains(&bad))
        {
            assert!(!self.counts.panic.get(), "reset panic");
            return Err(MatchRegistryError::NeedsReset);
        }
        self.total = payloads.into_iter().sum();
        self.handle = if r.is_enabled() {
            Some(r.register("payload", SimpleScore::new(self.total), ())?)
        } else {
            None
        };
        Ok(())
    }
    fn before_change(
        &mut self,
        _: &A,
        _: &A::Change,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.before.set(self.counts.before.get() + 1);
        if let Some(handle) = self.handle.take() {
            handle.cancel(r)?;
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &A,
        _: &A::Change,
        r: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.after.set(self.counts.after.get() + 1);
        self.total = a.payloads().into_iter().sum();
        self.handle = if r.is_enabled() {
            Some(r.register("payload", SimpleScore::new(self.total), ())?)
        } else {
            None
        };
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.counts.scores.set(self.counts.scores.get() + 1);
        SimpleScore::new(self.total)
    }
}
fn exact(strategy: ExactSearchStrategy) -> SolverSequence<'static> {
    SolverSequence {
        phases: vec![SolverPhase::Exact(ExactPhaseConfig {
            strategy,
            ..Default::default()
        })],
        ..Default::default()
    }
}
fn strategies() -> Vec<ExactSearchStrategy> {
    vec![
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
        ExactSearchStrategy::BranchAndBound(Default::default()),
        ExactSearchStrategy::CompleteBranchAndBound(Default::default()),
    ]
}
fn basic() -> FiniteProblem<Value> {
    FiniteProblem::new(vec![
        Variable::new(vec![value(0, 10)]).with_initial(value(0, 100)),
    ])
    .unwrap()
}
fn lists() -> ListProblem<Value> {
    ListProblem::new(
        vec![value(1, 20)],
        vec![ListEntity::new(vec![value(1, 200)])],
    )
    .unwrap()
}
fn mixed(c: SolverSequence<'static>) -> MixedSolverSequence<'static> {
    MixedSolverSequence {
        limits: c.limits,
        phases: c
            .phases
            .into_iter()
            .map(|p| match p {
                SolverPhase::Exact(c) => MixedSolverPhase::Exact(c),
                SolverPhase::Custom(c) => MixedSolverPhase::Custom(c),
                SolverPhase::RuinRecreate(c) => {
                    MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                        search: c,
                        ..Default::default()
                    })
                }
                _ => panic!("unsupported fixture phase"),
            })
            .collect(),
    }
}
#[test]
fn retained_exact_refreshes_identity_equal_payload() {
    let p = basic();
    let c = exact(ExactSearchStrategy::Enumerate);
    let counts = Rc::new(Counts::default());
    let factory = || Calculator::new(&counts);
    let fresh = solve_calculator(&p, &factory, c.clone()).unwrap();
    assert_eq!(fresh.score, Some(SimpleScore::new(10)));
    let mut d = CalculatorScoreDirector::new(&p, p.initial_assignment(), factory, true).unwrap();
    let retained = d.run_solver(c).unwrap();
    assert_eq!(retained.assignment.payloads(), vec![10]);
    assert_eq!(retained.score, fresh.score);
    assert_eq!(d.assignment().payloads(), vec![10]);
    assert_eq!(
        *d.registry().unwrap().total().unwrap(),
        SimpleScore::new(10)
    );
}
macro_rules! exact_matrix {
    ($test:ident,$problem:expr,$director:ident,$solve:ident,$convert:expr) => {
        #[test]
        fn $test() {
            for strategy in strategies() {
                for budget in [0, 1, 2, 20] {
                    let p = $problem;
                    let mut config = exact(strategy.clone());
                    config.limits.max_evaluations = budget;
                    let fresh_counts = Rc::new(Counts::default());
                    let fresh = $solve(
                        &p,
                        &|| Calculator::new(&fresh_counts),
                        ($convert)(config.clone()),
                    )
                    .unwrap();
                    let counts = Rc::new(Counts::default());
                    let mut d = $director::new(
                        &p,
                        p.initial_assignment(),
                        || Calculator::new(&counts),
                        true,
                    )
                    .unwrap();
                    let before = counts.scores.get();
                    let r = d.run_solver(($convert)(config)).unwrap();
                    assert_eq!(r.score, fresh.score, "{strategy:?}, budget {budget}");
                    assert_eq!(r.assignment.payloads(), fresh.assignment.payloads());
                    assert_eq!(r.work.evaluations, fresh.work.evaluations);
                    assert_eq!(
                        r.work.constraint_evaluations,
                        u128::from(counts.scores.get() - before)
                    );
                    assert_eq!(counts.scores.get(), counts.resets.get());
                    assert_eq!((counts.before.get(), counts.after.get()), (0, 0));
                    assert_eq!(d.assignment().payloads(), r.assignment.payloads());
                    if let Some(score) = r.score {
                        assert_eq!(d.score().unwrap(), score);
                        assert_eq!(
                            *d.registry().unwrap().total().unwrap(),
                            SimpleScore::new(r.assignment.payloads().iter().sum())
                        );
                    }
                    if budget == 0 {
                        assert_eq!(counts.resets.get(), 1);
                    } else {
                        assert_eq!(
                            counts.resets.get(),
                            1 + r.work.constraint_evaluations as u64
                        );
                    }
                }
            }
        }
    };
}
exact_matrix!(
    basic_exact_routes,
    basic(),
    CalculatorScoreDirector,
    solve_calculator,
    |c| c
);
exact_matrix!(
    list_exact_routes,
    lists(),
    ListCalculatorScoreDirector,
    solve_list_calculator,
    |c| c
);
exact_matrix!(
    mixed_exact_routes,
    MixedProblem::new(basic(), lists()),
    MixedCalculatorScoreDirector,
    solve_mixed_calculator,
    mixed
);

#[test]
fn prior_best_payloads_and_match_handles_survive_exact_refresh_and_restore() {
    for strategy in strategies() {
        let p = basic();
        let counts = Rc::new(Counts::default());
        let mut d = CalculatorScoreDirector::new(
            &p,
            p.initial_assignment(),
            || Calculator::new(&counts),
            true,
        )
        .unwrap();
        let mut config = exact(strategy);
        config
            .phases
            .insert(0, SolverPhase::Custom(Default::default()));
        let mut command = |_,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<Value>,
            Move<Value>,
            SimpleScore,
            ScoringError,
        >| {
            c.execute(&Move::Change {
                variable: 0,
                value: Some(value(0, 300)),
            })?;
            Ok(())
        };
        let r = d
            .run_solver_with_extensions(
                config,
                SolverExtensions {
                    command: Some(&mut command),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(300)));
        assert_eq!(r.assignment.payloads(), vec![300]);
        assert_eq!(d.assignment().payloads(), vec![300]);
        assert_eq!(
            *d.registry().unwrap().total().unwrap(),
            SimpleScore::new(300)
        );
        assert_eq!(
            r.work.constraint_evaluations,
            u128::from(counts.scores.get() - 1)
        );
        // Cancels the restored calculator's original registration, not a handle
        // issued by a discarded exact node or reset generation.
        d.apply(&Move::Change {
            variable: 0,
            value: Some(value(0, 400)),
        })
        .unwrap()
        .commit();
        assert_eq!(
            *d.registry().unwrap().total().unwrap(),
            SimpleScore::new(400)
        );
        assert_eq!(d.score().unwrap(), SimpleScore::new(400));
    }
}

#[test]
fn refresh_reset_errors_and_panics_poison_until_explicit_recovery() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for panic in [false, true] {
        let p = basic();
        let counts = Rc::new(Counts::default());
        let mut d = CalculatorScoreDirector::new(
            &p,
            p.initial_assignment(),
            || Calculator::new(&counts),
            true,
        )
        .unwrap();
        counts.fail_payload.set(Some(10));
        counts.panic.set(panic);
        let r = catch_unwind(AssertUnwindSafe(|| {
            d.run_solver(exact(ExactSearchStrategy::Enumerate))
        }));
        if panic {
            assert!(r.is_err());
        } else {
            assert!(r.unwrap().is_err());
        }
        assert!(d.score().is_err());
        assert!(d.registry().is_err());
        assert_eq!(counts.scores.get(), 1);
        counts.fail_payload.set(None);
        d.reset(p.initial_assignment()).unwrap();
        let r = d.run_solver(exact(ExactSearchStrategy::Enumerate)).unwrap();
        assert_eq!(r.score, Some(SimpleScore::new(10)));
    }
}

macro_rules! repair_matrix {
    ($test:ident,$problem:expr,$director:ident,$solve:ident,$convert:expr) => {
        #[test]
        fn $test() {
            for seed in 0..8 {
                let p = $problem;
                let config = SolverSequence {
                    phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                        minimum_ruined: 1,
                        maximum_ruined: 1,
                        max_attempts: 6,
                        seed,
                        ..Default::default()
                    })],
                    ..Default::default()
                };
                let fresh_counts = Rc::new(Counts::default());
                let fresh = $solve(
                    &p,
                    &|| Calculator::new(&fresh_counts),
                    ($convert)(config.clone()),
                )
                .unwrap();
                let counts = Rc::new(Counts::default());
                let mut d = $director::new(
                    &p,
                    p.initial_assignment(),
                    || Calculator::new(&counts),
                    true,
                )
                .unwrap();
                let r = d.run_solver(($convert)(config)).unwrap();
                assert_eq!(r.assignment.payloads(), fresh.assignment.payloads());
                assert_eq!(r.score, fresh.score);
                assert_eq!(r.work, fresh.work);
                assert_eq!(
                    r.work.constraint_evaluations,
                    u128::from(counts.scores.get() - 1)
                );
                assert_eq!(counts.resets.get(), 1 + fresh_counts.resets.get());
                assert_eq!(
                    (counts.before.get(), counts.after.get()),
                    (fresh_counts.before.get(), fresh_counts.after.get())
                );
                assert_eq!(d.assignment().payloads(), r.assignment.payloads());
                assert_eq!(
                    *d.registry().unwrap().total().unwrap(),
                    SimpleScore::new(r.assignment.payloads().iter().sum())
                );
                assert_eq!(
                    counts.reset_payloads.borrow()[1..],
                    fresh_counts.reset_payloads.borrow()[..]
                );
            }
        }
    };
}
repair_matrix!(
    basic_repairs,
    FiniteProblem::new(vec![
        Variable::new(vec![value(0, 10)]).with_initial(value(0, 100)),
        Variable::new(vec![value(1, 20)]).allowing_unassigned()
    ])
    .unwrap(),
    CalculatorScoreDirector,
    solve_calculator,
    |c| c
);
repair_matrix!(
    list_repairs,
    ListProblem::new(
        vec![value(0, 10), value(1, 20)],
        vec![ListEntity::new(vec![value(0, 100)])]
    )
    .unwrap()
    .allowing_unassigned(),
    ListCalculatorScoreDirector,
    solve_list_calculator,
    |c| c
);
repair_matrix!(
    mixed_repairs,
    MixedProblem::new(
        basic(),
        ListProblem::new(
            vec![value(0, 10), value(1, 20)],
            vec![ListEntity::new(vec![value(0, 100)])]
        )
        .unwrap()
        .allowing_unassigned()
    ),
    MixedCalculatorScoreDirector,
    solve_mixed_calculator,
    mixed
);
