//! Original transactional decision and work-accounting contracts.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};
use timetuck::*;
fn movement(value: i64) -> Move<i64> {
    Move::Change {
        variable: 0,
        value: Some(value),
    }
}
fn problem() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0)]).unwrap()
}
fn sequence(c: CustomPhaseConfig<'_>) -> SolverSequence<'_> {
    SolverSequence {
        phases: vec![SolverPhase::Custom(c)],
        ..Default::default()
    }
}
#[derive(Default, Debug)]
struct Counts {
    reset: Cell<u64>,
    score: Cell<u64>,
    before: Cell<u64>,
    after: Cell<u64>,
}
struct Sum {
    sum: i64,
    counts: Rc<Counts>,
}
impl IncrementalCalculator<Assignment<i64>> for Sum {
    type Score = SimpleScore;
    type Change = Vec<usize>;
    type Justification = ();
    fn reset(
        &mut self,
        a: &Assignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.reset.set(self.counts.reset.get() + 1);
        self.sum = a.values().iter().flatten().sum();
        Ok(())
    }
    fn before_change(
        &mut self,
        a: &Assignment<i64>,
        changed: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.before.set(self.counts.before.get() + 1);
        assert_eq!(self.sum, a.values().iter().flatten().sum::<i64>());
        for &i in changed {
            self.sum -= a.values()[i].unwrap_or(0);
        }
        Ok(())
    }
    fn after_change(
        &mut self,
        a: &Assignment<i64>,
        changed: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        self.counts.after.set(self.counts.after.get() + 1);
        for &i in changed {
            self.sum += a.values()[i].unwrap_or(0);
        }
        assert_eq!(self.sum, a.values().iter().flatten().sum::<i64>());
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.counts.score.set(self.counts.score.get() + 1);
        SimpleScore::new(self.sum)
    }
}
#[test]
fn stateful_calculator_acceptance_avoids_a_second_transaction_and_score_callback() {
    let run = |conditional| {
        let p = problem();
        let counts = Rc::new(Counts::default());
        let events = RefCell::new(vec![]);
        let r = calculator_custom_phase(
            &p,
            p.initial_assignment(),
            || Sum {
                sum: 0,
                counts: counts.clone(),
            },
            Default::default(),
            |c| {
                for (value, accept) in [(2, false), (1, true), (2, false), (0, true)] {
                    if conditional {
                        assert_eq!(
                            c.execute_if(&movement(value), |_, _| accept)?
                                .unwrap()
                                .committed,
                            accept
                        );
                    } else {
                        c.evaluate(&movement(value), |_, _| ())?;
                        if accept {
                            c.execute(&movement(value))?;
                        }
                    }
                }
                Ok(())
            },
            |e| events.borrow_mut().push(e.score),
        )
        .unwrap();
        assert_eq!(r.assignment.values(), &[Some(1)]);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(r.work.committed_moves, 2);
        assert_eq!(
            *events.borrow(),
            vec![SimpleScore::ZERO, SimpleScore::new(1)]
        );
        (
            r.work,
            (
                counts.reset.get(),
                counts.score.get(),
                counts.before.get(),
                counts.after.get(),
            ),
        )
    };
    let (baseline, b) = run(false);
    let (current, c) = run(true);
    assert_eq!(b, (1, 7, 10, 10));
    assert_eq!(c, (1, 5, 6, 6));
    assert_eq!(baseline.evaluations, 7);
    assert_eq!(current.evaluations, 5);
    assert_eq!(baseline.constraint_evaluations, 7);
    assert_eq!(current.constraint_evaluations, 5);
}
#[test]
fn decision_panic_rolls_back_and_known_work_survives_catch() {
    let p = problem();
    let counts = Rc::new(Counts::default());
    let r = calculator_custom_phase(
        &p,
        p.initial_assignment(),
        || Sum {
            sum: 0,
            counts: counts.clone(),
        },
        Default::default(),
        |c| {
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = c.execute_if(&movement(2), |_, _| panic!("decision"));
                }))
                .is_err()
            );
            assert_eq!(c.assignment(), &p.initial_assignment());
            assert_eq!(c.score(), SimpleScore::ZERO);
            assert_eq!(c.work().evaluations, 2);
            assert_eq!(c.work().committed_moves, 0);
            assert!(!c.is_terminated());
            c.execute_if(&movement(1), |_, _| true)?;
            Ok(())
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(counts.score.get(), 3);
    assert_eq!(counts.before.get(), 3);
    assert_eq!(counts.after.get(), 3);
}
#[test]
fn rejected_candidates_spend_quota_but_emit_no_step_or_best_events() {
    let p = problem();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("score", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap())
    })
    .unwrap();
    for budget in 1..=5 {
        let decisions = Cell::new(0);
        let events = RefCell::new(vec![]);
        let best = RefCell::new(vec![]);
        let mut command = |_: usize,
                           c: &mut CustomPhaseContext<
            '_,
            Assignment<i64>,
            Move<i64>,
            SimpleScore,
            ScoringError,
        >| {
            for (value, accept) in [(2, false), (1, true), (2, false), (0, true)] {
                let result = c.execute_if(&movement(value), |_, _| {
                    decisions.set(decisions.get() + 1);
                    accept
                })?;
                if result.is_none() {
                    break;
                }
            }
            assert!(
                c.execute_if(&movement(1), |_, _| panic!(
                    "exhausted budget called decision"
                ))?
                .is_none()
            );
            Ok(())
        };
        let mut listener = |e| events.borrow_mut().push(e);
        let mut observer = |e: SolverEvent<'_, Assignment<i64>, SimpleScore>| {
            if let SolverEvent::Best(u) = e {
                best.borrow_mut().push(u.score);
            }
        };
        let r = solve_with_extensions(
            &p,
            &n,
            sequence(CustomPhaseConfig {
                max_evaluations: budget,
                ..Default::default()
            }),
            SolverExtensions {
                command: Some(&mut command),
                lifecycle: Some(&mut listener),
                events: Some(&mut observer),
                ..Default::default()
            },
        )
        .unwrap();
        let commits = u64::from(budget >= 3) + u64::from(budget >= 5);
        assert_eq!(decisions.get(), budget - 1);
        assert_eq!(r.work.evaluations, budget);
        assert_eq!(r.work.completed_steps, 0); // Existing solver summary excludes custom steps.
        let started = events
            .borrow()
            .iter()
            .filter(|e| matches!(e, SolverLifecycleEvent::StepStarted { .. }))
            .count();
        let ended = events
            .borrow()
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    SolverLifecycleEvent::StepEnded {
                        completed: true,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(started, commits as usize);
        assert_eq!(ended, commits as usize);
        assert_eq!(r.score, Some(SimpleScore::new(i64::from(budget >= 3))));
        assert!(!best.borrow().contains(&SimpleScore::new(2)));
    }
}
#[test]
fn cancellation_inside_decision_commits_the_current_trial_then_stops() {
    let p = problem();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("score", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap())
    })
    .unwrap();
    let token = CancellationToken::new();
    let mut command = |_: usize,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        SimpleScore,
        ScoringError,
    >| {
        assert!(
            c.execute_if(&movement(1), |_, _| {
                token.cancel();
                true
            })?
            .unwrap()
            .committed
        );
        assert!(
            c.execute_if(&movement(2), |_, _| panic!("cancelled"))?
                .is_none()
        );
        Ok(())
    };
    let mut plan = sequence(Default::default());
    plan.limits.cancellation = Some(&token);
    let r = solve_with_extensions(
        &p,
        &n,
        plan,
        SolverExtensions {
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Cancelled);
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(r.work.completed_steps, 0); // Custom commits are reported by the command context.
    assert_eq!(r.work.evaluations, 2);
}
#[test]
fn scoring_errors_and_panics_invalidate_the_phase_without_calling_decision() {
    let p = problem();
    for panics in [false, true] {
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("score", vec![0], |v| {
            if panics && v[0] == Some(&1) {
                panic!("score");
            }
            SimpleScore::new(v[0].copied().unwrap())
        })
        .unwrap();
        let r = custom_phase(&p, &n, p.initial_assignment(), Default::default(), |c| {
            if panics {
                assert!(
                    catch_unwind(AssertUnwindSafe(|| {
                        let _ = c.execute_if(&movement(1), |_, _| panic!("decision must not run"));
                    }))
                    .is_err()
                );
            } else {
                assert!(matches!(
                    c.execute_if(&movement(99), |_, _| panic!("decision must not run")),
                    Err(CustomPhaseError::Scoring(_))
                ));
            }
            assert!(c.is_terminated());
            assert_eq!(c.assignment(), &p.initial_assignment());
            assert!(c.execute_if(&movement(2), |_, _| panic!())?.is_none());
            Ok(())
        });
        assert!(matches!(r, Err(CustomPhaseError::IgnoredOperationError)));
    }
}
#[test]
fn temporary_completion_does_not_arm_goal_but_committed_completion_does() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1])]).unwrap();
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("score", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap_or(0))
    })
    .unwrap();
    let condition =
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::new(1)));
    let mut command = |_: usize,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        SimpleScore,
        ScoringError,
    >| {
        assert!(!c.execute_if(&movement(1), |_, _| false)?.unwrap().committed);
        assert!(!c.is_terminated());
        assert!(c.score().structural() < 0);
        assert!(c.execute_if(&movement(1), |_, _| true)?.unwrap().committed);
        assert!(c.is_terminated());
        Ok(())
    };
    let mut plan = sequence(Default::default());
    plan.limits.scored_termination = Some(&condition);
    let r = solve_with_extensions(
        &p,
        &n,
        plan,
        SolverExtensions {
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.complete);
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.completed_steps, 0); // Custom commits are reported by the command context.
}
