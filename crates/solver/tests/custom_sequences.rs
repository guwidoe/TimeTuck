//! Original shared-budget multi-command contracts, motivated by public custom-phase docs.
use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
type Context<'a> = CustomPhaseContext<'a, Assignment<i64>, Move<i64>, SimpleScore, ScoringError>;
type CommandFn = fn(&mut Context<'_>) -> Result<(), CustomPhaseError<ScoringError>>;
type Command<'a, 'c> =
    dyn FnMut(&mut Context<'c>) -> Result<(), CustomPhaseError<ScoringError>> + 'a;
fn movement(value: i64) -> Move<i64> {
    Move::Change {
        variable: 0,
        value: Some(value),
    }
}
fn problem() -> FiniteProblem<i64> {
    FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0)]).unwrap()
}
fn network() -> ConstraintNetwork<'static, i64, SimpleScore> {
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap())
    })
    .unwrap();
    n
}
#[derive(Default)]
struct Counts {
    fail_reset: Cell<bool>,
    factory: Cell<u64>,
    reset: Cell<u64>,
    score: Cell<u64>,
    before: Cell<u64>,
    after: Cell<u64>,
}
struct Calculator {
    sum: i64,
    id: u64,
    c: Rc<Counts>,
}
impl IncrementalCalculator<Assignment<i64>> for Calculator {
    type Score = SimpleScore;
    type Change = Vec<usize>;
    type Justification = ();
    fn reset(
        &mut self,
        a: &Assignment<i64>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        assert_eq!(self.id, 1);
        self.c.reset.set(self.c.reset.get() + 1);
        if self.c.fail_reset.get() {
            return Err(MatchRegistryError::Calculator("restoration failed".into()));
        }
        self.sum = a.values().iter().flatten().sum();
        Ok(())
    }
    fn before_change(
        &mut self,
        a: &Assignment<i64>,
        changed: &Vec<usize>,
        _: &mut MatchRegistry<SimpleScore, ()>,
    ) -> Result<(), MatchRegistryError> {
        assert_eq!(self.sum, a.values().iter().flatten().sum::<i64>());
        self.c.before.set(self.c.before.get() + 1);
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
        self.c.after.set(self.c.after.get() + 1);
        for &i in changed {
            self.sum += a.values()[i].unwrap_or(0);
        }
        assert_eq!(self.sum, a.values().iter().flatten().sum::<i64>());
        Ok(())
    }
    fn calculate_score(&mut self) -> SimpleScore {
        self.c.score.set(self.c.score.get() + 1);
        SimpleScore::new(self.sum)
    }
}
#[test]
fn calculator_instance_survives_handoffs_and_provably_current_best_avoids_resets() {
    let p = problem();
    let c = Rc::new(Counts::default());
    let r = calculator_custom_phase(
        &p,
        p.initial_assignment(),
        || {
            c.factory.set(c.factory.get() + 1);
            Calculator {
                sum: 0,
                id: c.factory.get(),
                c: c.clone(),
            }
        },
        Default::default(),
        |ctx| {
            let mut first = |ctx: &mut Context<'_>| {
                ctx.execute(&movement(1))?;
                Ok(())
            };
            let mut preview = |ctx: &mut Context<'_>| {
                assert_eq!(ctx.score(), SimpleScore::new(1));
                ctx.evaluate(&movement(2), |_, _| ())?;
                ctx.execute_if(&movement(0), |_, _| false)?;
                Ok(())
            };
            let mut worsen = |ctx: &mut Context<'_>| {
                assert_eq!(ctx.work().restorations, 0);
                ctx.execute(&movement(0))?;
                Ok(())
            };
            let mut improve = |ctx: &mut Context<'_>| {
                assert_eq!(ctx.assignment().values(), &[Some(1)]);
                assert_eq!(ctx.work().restorations, 1);
                ctx.execute(&movement(2))?;
                Ok(())
            };
            let cmds: [&mut Command<'_, '_>; 4] =
                [&mut first, &mut preview, &mut worsen, &mut improve];
            assert_eq!(ctx.run_commands(cmds)?, 4);
            Ok(())
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    assert_eq!(r.work.evaluations, 7);
    assert_eq!(r.work.move_evaluations, 5);
    assert_eq!(r.work.restorations, 1);
    assert_eq!(r.work.committed_moves, 3);
    assert_eq!(
        (
            c.factory.get(),
            c.reset.get(),
            c.score.get(),
            c.before.get(),
            c.after.get()
        ),
        (1, 2, 7, 7, 7)
    );
}
#[test]
fn every_score_budget_prefix_shares_one_limit_and_restoration_is_not_a_move() {
    let p = problem();
    let n = network();
    for budget in 1..=6 {
        let invoked = RefCell::new(vec![]);
        let r = custom_phase(
            &p,
            &n,
            p.initial_assignment(),
            CustomPhaseConfig {
                max_evaluations: budget,
                ..Default::default()
            },
            |c| {
                let commands = (0..3).map(|i| {
                    let invoked = &invoked;
                    move |c: &mut Context<'_>| {
                        invoked.borrow_mut().push(i);
                        if i == 0 {
                            c.execute(&movement(1))?;
                            c.execute(&movement(0))?;
                        }
                        if i == 1 {
                            assert_eq!(c.assignment().values(), &[Some(1)]);
                            c.execute(&movement(2))?;
                        }
                        Ok(())
                    }
                });
                c.run_commands(commands)?;
                Ok(())
            },
        )
        .unwrap();
        let expected = match budget {
            1 => vec![],
            2..=4 => vec![0],
            5 => vec![0, 1],
            _ => vec![0, 1, 2],
        };
        assert_eq!(*invoked.borrow(), expected, "budget {budget}");
        assert_eq!(r.work.evaluations, budget.min(5));
        assert_eq!(r.work.restorations, u64::from(budget >= 4));
        assert_eq!(
            r.work.move_evaluations,
            if budget <= 3 {
                budget - 1
            } else if budget == 4 {
                2
            } else {
                3
            }
        );
        assert_eq!(
            r.score,
            Some(SimpleScore::new(if budget == 1 {
                0
            } else if budget < 5 {
                1
            } else {
                2
            }))
        );
    }
}
#[test]
fn empty_nested_and_terminal_sequences_do_not_reinitialize_or_reset_history() {
    let p = problem();
    let n = network();
    let r = custom_phase(&p, &n, p.initial_assignment(), Default::default(), |c| {
        let empty: [CommandFn; 0] = [];
        assert_eq!(c.run_commands(empty)?, 0);
        let mut outer = |c: &mut Context<'_>| {
            let mut first = |c: &mut Context<'_>| {
                c.execute(&movement(1))?;
                c.execute(&movement(0))?;
                Ok(())
            };
            let mut second = |c: &mut Context<'_>| {
                assert_eq!(c.assignment().values(), &[Some(1)]);
                Ok(())
            };
            let nested: [&mut Command<'_, '_>; 2] = [&mut first, &mut second];
            assert_eq!(c.run_commands(nested)?, 2);
            Ok(())
        };
        assert_eq!(c.run_commands([&mut outer])?, 1);
        assert_eq!(c.work().restorations, 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(r.work.evaluations, 4);
    assert_eq!(r.work.committed_moves, 2);
}
#[test]
fn command_errors_and_original_panic_payloads_poison_the_enclosing_context() {
    let p = problem();
    let n = network();
    for panics in [false, true] {
        let later = Cell::new(false);
        let r = custom_phase(&p, &n, p.initial_assignment(), Default::default(), |c| {
            let mut first = |c: &mut Context<'_>| {
                c.execute(&movement(1))?;
                if panics {
                    std::panic::panic_any(123_u32);
                }
                Err(CustomPhaseError::Command("command failed".into()))
            };
            let mut second = |_: &mut Context<'_>| {
                later.set(true);
                Ok(())
            };
            let commands: [&mut Command<'_, '_>; 2] = [&mut first, &mut second];
            if panics {
                let payload =
                    catch_unwind(AssertUnwindSafe(|| c.run_commands(commands))).unwrap_err();
                assert_eq!(payload.downcast_ref::<u32>(), Some(&123));
            } else {
                assert!(
                    matches!(c.run_commands(commands),Err(CustomPhaseError::Command(s)) if s=="command failed")
                );
            }
            assert!(c.is_terminated());
            assert!(c.execute(&movement(2))?.is_none());
            Ok(())
        });
        assert!(matches!(r, Err(CustomPhaseError::IgnoredOperationError)));
        assert!(!later.get());
    }
}
#[test]
fn restoration_panic_prevents_later_commands_and_keeps_transactional_state() {
    let p = problem();
    let fail = Cell::new(false);
    let mut n = ConstraintNetwork::new(1);
    n.add_constraint("score", vec![0], |v| {
        if fail.get() {
            panic!("restoration scoring");
        }
        SimpleScore::new(v[0].copied().unwrap())
    })
    .unwrap();
    let later = Cell::new(false);
    let r = custom_phase(&p, &n, p.initial_assignment(), Default::default(), |c| {
        let mut first = |c: &mut Context<'_>| {
            c.execute(&movement(1))?;
            c.execute(&movement(0))?;
            fail.set(true);
            Ok(())
        };
        let mut second = |_: &mut Context<'_>| {
            later.set(true);
            Ok(())
        };
        let commands: [&mut Command<'_, '_>; 2] = [&mut first, &mut second];
        assert!(catch_unwind(AssertUnwindSafe(|| c.run_commands(commands))).is_err());
        assert!(c.is_terminated());
        assert_eq!(c.assignment().values(), &[Some(0)]);
        assert_eq!(c.score(), SimpleScore::ZERO);
        assert_eq!(c.work().restorations, 1);
        Ok(())
    });
    assert!(matches!(r, Err(CustomPhaseError::IgnoredOperationError)));
    assert!(!later.get());
}
#[test]
fn cancellation_during_a_command_prevents_restoration_and_iterator_advance() {
    let p = problem();
    let n = network();
    let token = CancellationToken::new();
    let pulled = Cell::new(0);
    let r = custom_phase(
        &p,
        &n,
        p.initial_assignment(),
        CustomPhaseConfig {
            cancellation: Some(&token),
            ..Default::default()
        },
        |c| {
            let commands = (0..3).map(|i| {
                pulled.set(pulled.get() + 1);
                let token = &token;
                move |c: &mut Context<'_>| {
                    assert_eq!(i, 0);
                    c.execute(&movement(1))?;
                    c.execute(&movement(0))?;
                    token.cancel();
                    Ok(())
                }
            });
            assert_eq!(c.run_commands(commands)?, 1);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(pulled.get(), 1);
    assert_eq!(r.work.restorations, 0);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.termination, CustomPhaseTermination::Cancelled);
    assert_eq!(r.assignment.values(), &[Some(1)]);
}
#[test]
fn phase_score_quota_counts_restoration_without_resetting_lifecycle_or_stagnation() {
    let p = problem();
    let n = network();
    let entered = Cell::new(0);
    let events = RefCell::new(vec![]);
    let condition = SearchTermination::Condition(SearchLimit::ScoreEvaluations(4));
    let bindings = [PhaseTerminationBinding::search(0, &condition)];
    let mut command = |_: usize, c: &mut Context<'_>| {
        let commands = (0..2).map(|i| {
            let entered = &entered;
            move |c: &mut Context<'_>| {
                entered.set(entered.get() + 1);
                assert_eq!(i, 0);
                c.execute(&movement(1))?;
                c.execute(&movement(0))?;
                Ok(())
            }
        });
        assert_eq!(c.run_commands(commands)?, 1);
        Ok(())
    };
    let mut lifecycle = |e| events.borrow_mut().push(e);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(Default::default())],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            phase_terminations: &bindings,
            lifecycle: Some(&mut lifecycle),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.evaluations, 4);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(entered.get(), 1);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Custom(CustomPhaseTermination::Condition)
    );
    let events = events.borrow();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SolverLifecycleEvent::PhaseStarted { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, SolverLifecycleEvent::StepStarted { .. }))
            .count(),
        2
    );
}
#[test]
fn phase_time_and_unimproved_history_continue_across_command_handoffs() {
    let p = problem();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let t = Arc::new(AtomicU64::new(0));
    let ticks = t.clone();
    let clock = SolverClock::new(move || Duration::from_millis(ticks.load(Ordering::SeqCst)));
    let condition =
        SearchTermination::Condition(SearchLimit::UnimprovedTime(Duration::from_millis(5)));
    let bindings = [PhaseTerminationBinding::search(0, &condition)];
    let entered = Cell::new(0);
    let mut command = |_: usize, c: &mut Context<'_>| {
        let commands = (0..3).map(|_| {
            let entered = &entered;
            let t = &t;
            move |c: &mut Context<'_>| {
                entered.set(entered.get() + 1);
                t.fetch_add(3, Ordering::SeqCst);
                c.execute(&movement(1))?;
                Ok(())
            }
        });
        assert_eq!(c.run_commands(commands)?, 2);
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::Custom(Default::default())],
            ..Default::default()
        },
        SolverExtensions {
            command: Some(&mut command),
            phase_terminations: &bindings,
            clock: Some(&clock),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(entered.get(), 2);
    assert_eq!(r.work.move_evaluations, 1);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Custom(CustomPhaseTermination::Condition)
    );
}

#[derive(Clone, Debug)]
struct Payload {
    id: u8,
    value: i64,
}
impl PartialEq for Payload {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Payload {}
fn payload_script<E>(
    c: &mut CustomPhaseContext<'_, Assignment<Payload>, Move<Payload>, SimpleScore, E>,
) -> Result<(), CustomPhaseError<E>> {
    let commands = (0..2).map(|i| {
        move |c: &mut CustomPhaseContext<'_, Assignment<Payload>, Move<Payload>, SimpleScore, E>| {
            if i == 0 {
                c.execute(&Move::Change {
                    variable: 0,
                    value: Some(Payload { id: 1, value: 200 }),
                })?;
                c.execute(&Move::Change {
                    variable: 0,
                    value: Some(Payload { id: 1, value: 100 }),
                })?;
                assert_eq!(c.assignment(), c.best_assignment()); // Identity equality hides worsening payload.
                assert_eq!(c.score(), SimpleScore::new(100));
            } else {
                assert_eq!(c.assignment().values()[0].as_ref().unwrap().value, 200);
                assert_eq!(c.score(), SimpleScore::new(200));
                c.assert_consistent()?;
            }
            Ok(())
        }
    });
    assert_eq!(c.run_commands(commands)?, 2);
    assert_eq!(c.work().restorations, 1);
    Ok(())
}
#[test]
fn identity_equal_payloads_restore_actual_best_and_scoped_and_stream_caches() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![Payload { id: 1, value: 0 }]).with_initial(Payload { id: 1, value: 10 }),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<Payload, SimpleScore>::new(1);
    n.add_constraint("payload", vec![0], |v| {
        SimpleScore::new(v[0].unwrap().value)
    })
    .unwrap();
    let r = custom_phase(
        &p,
        &n,
        p.initial_assignment(),
        Default::default(),
        payload_script,
    )
    .unwrap();
    assert_eq!(r.assignment.values()[0].as_ref().unwrap().value, 200);
    let mut b = StreamGraphBuilder::new();
    let source = b.source::<usize, (usize, Option<Payload>)>();
    b.score("payload", source.port(), |(_, v)| {
        SimpleScore::new(v.as_ref().unwrap().value)
    })
    .unwrap();
    let r = stream_custom_phase(
        &p,
        b.build(),
        source,
        p.initial_assignment(),
        Default::default(),
        payload_script,
    )
    .unwrap();
    assert_eq!(r.assignment.values()[0].as_ref().unwrap().value, 200);
}
#[test]
fn calculator_restoration_error_is_returned_and_later_commands_are_not_invoked() {
    let p = problem();
    let counts = Rc::new(Counts::default());
    let entered = Cell::new(0);
    let r = calculator_custom_phase(
        &p,
        p.initial_assignment(),
        || {
            counts.factory.set(counts.factory.get() + 1);
            Calculator {
                sum: 0,
                id: counts.factory.get(),
                c: counts.clone(),
            }
        },
        Default::default(),
        |c| {
            let commands = (0..2).map(|_| {
                |c: &mut Context<'_>| {
                    entered.set(entered.get() + 1);
                    c.execute(&movement(1))?;
                    c.execute(&movement(0))?;
                    counts.fail_reset.set(true);
                    Ok(())
                }
            });
            let error = c.run_commands(commands).unwrap_err();
            assert!(
                matches!(error, CustomPhaseError::Scoring(ScoringError::Calculator(MatchRegistryError::Calculator(ref text))) if text == "restoration failed")
            );
            assert!(c.is_terminated());
            assert_eq!(c.work().evaluations, 4);
            assert_eq!(c.work().restorations, 1);
            assert!(c.execute(&movement(2))?.is_none());
            Err(error)
        },
        |_| {},
    );
    assert!(matches!(r, Err(CustomPhaseError::Scoring(_))));
    assert_eq!(entered.get(), 1);
    assert_eq!(counts.factory.get(), 1);
    assert_eq!(counts.reset.get(), 2);
    assert_eq!(counts.score.get(), 3); // Failing reset never calculates a score.
}
