//! Original transactional calculator coverage; no upstream method credit.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Problem {
    values: Vec<i64>,
    structural: i64,
}
#[derive(Clone, Copy)]
struct Change {
    index: usize,
    reset_matches: bool,
}
const CHANGE: Change = Change {
    index: 0,
    reset_matches: false,
};
#[derive(Default, Debug)]
struct Counts {
    clones: usize,
    resets: usize,
    before: usize,
    after: usize,
    scores: usize,
}
type Registration = MatchRegistration<SimpleScore, Vec<i64>>;
struct Calculator {
    issued: Rc<RefCell<Vec<Registration>>>,
    values: Vec<i64>,
    structural: i64,
    matched: Option<MatchRegistration<SimpleScore, Vec<i64>>>,
    counts: Rc<RefCell<Counts>>,
    fail: Rc<Cell<u8>>,
}
impl Clone for Calculator {
    fn clone(&self) -> Self {
        self.counts.borrow_mut().clones += 1;
        assert_ne!(self.fail.get(), 9, "clone panic");
        Self {
            values: self.values.clone(),
            structural: self.structural,
            matched: self.matched.clone(),
            counts: self.counts.clone(),
            fail: self.fail.clone(),
            issued: self.issued.clone(),
        }
    }
}
impl Calculator {
    fn stage(&self, stage: u8) -> Result<(), MatchRegistryError> {
        if self.fail.get() == stage {
            return Err(MatchRegistryError::Calculator(format!("stage {stage}")));
        }
        assert_ne!(self.fail.get(), stage + 10, "stage panic {stage}");
        Ok(())
    }
    fn register(
        &mut self,
        registry: &mut MatchRegistry<SimpleScore, Vec<i64>>,
    ) -> Result<(), MatchRegistryError> {
        if registry.is_enabled() {
            let handle = registry.register(
                "sum",
                SimpleScore::new(self.values.iter().sum()),
                self.values.clone(),
            )?;
            self.issued.borrow_mut().push(handle.clone());
            self.matched = Some(handle);
        }
        Ok(())
    }
}
impl IncrementalCalculator<Problem> for Calculator {
    type Score = SimpleScore;
    type Justification = Vec<i64>;
    type Change = Change;
    fn supports_matches(&self) -> bool {
        true
    }
    fn reset(
        &mut self,
        p: &Problem,
        registry: &mut MatchRegistry<SimpleScore, Vec<i64>>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.borrow_mut().resets += 1;
        self.values = p.values.clone();
        self.structural = p.structural;
        self.register(registry)
    }
    fn before_change(
        &mut self,
        _: &Problem,
        _: &Change,
        registry: &mut MatchRegistry<SimpleScore, Vec<i64>>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.borrow_mut().before += 1;
        if let Some(handle) = self.matched.take() {
            handle.cancel(registry)?;
        }
        self.stage(1)
    }
    fn after_change(
        &mut self,
        p: &Problem,
        change: &Change,
        registry: &mut MatchRegistry<SimpleScore, Vec<i64>>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.borrow_mut().after += 1;
        self.values[change.index] = p.values[change.index];
        self.structural = p.structural;
        if change.reset_matches {
            registry.reset();
        }
        self.register(registry)?;
        self.stage(2)
    }
    fn calculate_score(&mut self) -> SimpleScore {
        unreachable!("fallible hook required")
    }
    fn calculate_score_with_matches(
        &mut self,
        _: &mut MatchRegistry<SimpleScore, Vec<i64>>,
    ) -> Result<SimpleScore, MatchRegistryError> {
        self.counts.borrow_mut().scores += 1;
        self.stage(3)?;
        Ok(SimpleScore::new(self.values.iter().sum()).with_structural(self.structural))
    }
}
fn session(matches: bool) -> CalculatorSession<Problem, Calculator> {
    CalculatorSession::new(
        Problem {
            values: vec![2, 3],
            structural: 0,
        },
        Calculator {
            values: vec![],
            structural: 0,
            matched: None,
            counts: Default::default(),
            fail: Default::default(),
            issued: Default::default(),
        },
        matches,
    )
    .unwrap()
}

#[test]
fn scored_what_if_restores_stateful_cache_without_reset_or_inverse_callbacks() {
    for matches in [false, true] {
        let mut session = session(matches);
        let identity = session.calculator().counts.clone();
        let result = session
            .evaluate_change(
                &CHANGE,
                |p| p.values[0] = 19,
                |p, score, registry| {
                    assert_eq!(p.values, [19, 3]);
                    assert_eq!(*score, SimpleScore::new(22));
                    if matches {
                        assert_eq!(registry.total().unwrap(), score);
                    } else {
                        assert_eq!(registry.total(), Err(MatchRegistryError::Disabled));
                    }
                    p.values.clone()
                },
            )
            .unwrap();
        assert_eq!(result, [19, 3]);
        assert_eq!(session.solution().values, [2, 3]);
        assert_eq!(session.calculator().values, [2, 3]);
        assert!(Rc::ptr_eq(&identity, &session.calculator().counts));
        let c = identity.borrow();
        assert_eq!(
            (c.clones, c.resets, c.before, c.after, c.scores),
            (1, 1, 1, 1, 1)
        );
        drop(c);
        assert_eq!(session.score().unwrap(), SimpleScore::new(5));
        session.change(&CHANGE, |p| p.values[0] = 7).unwrap();
        assert_eq!(session.score().unwrap(), SimpleScore::new(10));
    }
}

#[test]
fn callback_errors_and_panics_restore_matches_cache_and_ready_state() {
    for stage in 1..=3 {
        for panic in [false, true] {
            let mut session = session(true);
            let original = session.calculator().matched.clone().unwrap();
            let fail = session.calculator().fail.clone();
            fail.set(stage + if panic { 10 } else { 0 });
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                session.evaluate_change(
                    &CHANGE,
                    |p| p.values[0] = 100,
                    |_, _, _| panic!("failed candidate visited"),
                )
            }));
            if panic {
                assert!(outcome.is_err());
            } else {
                assert_eq!(
                    outcome.unwrap(),
                    Err(MatchRegistryError::Calculator(format!("stage {stage}")))
                );
            }
            assert_eq!(session.solution().values, [2, 3]);
            assert_eq!(session.calculator().values, [2, 3]);
            assert_eq!(session.calculator().counts.borrow().resets, 1);
            assert_eq!(
                session.registry().unwrap().total().unwrap(),
                &SimpleScore::new(5)
            );
            // The restored calculator's old cancellation handle works normally.
            fail.set(0);
            assert_eq!(session.score().unwrap(), SimpleScore::new(5));
            original.cancel(session.registry_mut().unwrap()).unwrap();
        }
    }
}

#[test]
fn mutation_and_visitor_panics_restore_before_unwinding_to_caller() {
    for panic_in_mutation in [false, true] {
        let mut session = session(true);
        let result = catch_unwind(AssertUnwindSafe(|| {
            session.evaluate_change(
                &CHANGE,
                |p| {
                    p.values[0] = 88;
                    assert!(!panic_in_mutation, "mutation panic");
                },
                |_, _, _| panic!("visitor panic"),
            )
        }));
        assert!(result.is_err());
        assert_eq!(session.solution().values, [2, 3]);
        assert_eq!(session.score().unwrap(), SimpleScore::new(5));
        assert_eq!(
            session.registry().unwrap().total().unwrap(),
            &SimpleScore::new(5)
        );
        assert_eq!(session.calculator().counts.borrow().resets, 1);
    }
}

#[test]
fn failed_snapshot_clone_or_unready_session_never_mutates_or_scores() {
    let mut session = session(true);
    let fail = session.calculator().fail.clone();
    fail.set(9);
    let result = catch_unwind(AssertUnwindSafe(|| {
        session.evaluate_change(
            &CHANGE,
            |_| panic!("mutation after failed snapshot"),
            |_, _, _| panic!("visitor"),
        )
    }));
    assert!(result.is_err());
    assert_eq!(session.solution().values, [2, 3]);
    assert_eq!(session.calculator().counts.borrow().before, 0);
    assert_eq!(session.calculator().counts.borrow().scores, 0);
    fail.set(1);
    assert!(
        session
            .change(&CHANGE, |_| panic!("mutation after failed before"))
            .is_err()
    );
    let clones = session.calculator().counts.borrow().clones;
    assert_eq!(
        session.evaluate_change(&CHANGE, |_| panic!("unready mutation"), |_, _, _| ()),
        Err(MatchRegistryError::NeedsReset)
    );
    assert_eq!(session.calculator().counts.borrow().clones, clones);
    fail.set(0);
    session.reset_current().unwrap();
    assert_eq!(session.score().unwrap(), SimpleScore::new(5));
}

#[test]
fn escaped_trial_handles_do_not_alias_future_matches_even_after_registry_reset() {
    for reset_matches in [false, true] {
        let mut session = session(true);
        let original = session.calculator().matched.clone().unwrap();
        let issued = session.calculator().issued.clone();
        let escaped = session
            .evaluate_change(
                &Change {
                    reset_matches,
                    ..CHANGE
                },
                |p| p.values[0] = 8,
                |_, _, registry| {
                    let handle = issued.borrow().last().unwrap().clone();
                    assert_eq!(registry.total().unwrap(), &handle.record().score);
                    handle
                },
            )
            .unwrap();
        assert_eq!(escaped.record().score, SimpleScore::new(11));
        assert_eq!(
            escaped.cancel(session.registry_mut().unwrap()),
            Err(if reset_matches {
                MatchRegistryError::ForeignOrExpiredRegistration
            } else {
                MatchRegistryError::AlreadyCanceled
            })
        );
        assert_eq!(
            session.calculator().matched.as_ref().unwrap().id(),
            original.id()
        );
        session.change(&CHANGE, |p| p.values[0] = 9).unwrap();
        let next = session.calculator().matched.as_ref().unwrap();
        assert!(next.id() > escaped.id());
        assert_eq!(
            original.cancel(session.registry_mut().unwrap()),
            Err(MatchRegistryError::AlreadyCanceled)
        );
        assert_eq!(session.score().unwrap(), SimpleScore::new(12));
    }
}

#[test]
fn structurally_incomplete_scores_and_visitor_errors_are_read_only_trials() {
    let mut session = session(false);
    for structural in [0, -1] {
        let selected = session
            .evaluate_change(
                &CHANGE,
                |p| p.structural = structural,
                |p, score, _| {
                    assert_eq!(p.structural, structural);
                    if score.structural() < 0 {
                        Err("flawed")
                    } else {
                        Ok("valid")
                    }
                },
            )
            .unwrap();
        assert_eq!(
            selected,
            if structural == 0 {
                Ok("valid")
            } else {
                Err("flawed")
            }
        );
        assert_eq!(session.solution().structural, 0);
        assert_eq!(session.calculator().structural, 0);
    }
    let c = session.calculator().counts.borrow();
    assert_eq!((c.before, c.after, c.scores, c.resets), (2, 2, 2, 1));
}
