//! Original expression-cooling contracts; no upstream source credit.
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
fn cooling(scope: TerminationProgressScope) -> Acceptance {
    Acceptance::SimulatedAnnealingTermination {
        starting_temperature: SimpleScore::new(1_000_000_000),
        scope,
        seed: 42,
    }
}
fn gradient(a: &Acceptor<SimpleScore>) -> f64 {
    match a {
        Acceptor::SimulatedAnnealingTermination { gradient, .. } => *gradient,
        _ => panic!("wrong acceptor"),
    }
}
#[test]
fn supplied_progress_freezes_holds_and_reheats_without_consuming_draws() {
    let scope = TerminationProgressScope::Phase;
    let mut a = cooling(scope).start(SimpleScore::ZERO).unwrap();
    assert_eq!(gradient(&a), 0.0);
    a.termination_step_started(scope, Some(1.0)).unwrap();
    assert!(!a.accepts(SimpleScore::ZERO, SimpleScore::new(-1), &[]));
    a.termination_step_started(scope, None).unwrap();
    assert_eq!(gradient(&a), 1.0);
    a.termination_step_started(TerminationProgressScope::Solve, Some(0.0))
        .unwrap();
    assert_eq!(gradient(&a), 1.0);
    for bad in [f64::NAN, f64::INFINITY, -0.1, 1.01] {
        assert_eq!(
            a.termination_step_started(scope, Some(bad)),
            Err(AcceptanceError::InvalidGradient)
        );
        assert_eq!(gradient(&a), 1.0);
    }
    a.termination_step_started(scope, Some(0.0)).unwrap();
    let mut reference = SimulatedAnnealing::new(SimpleScore::new(1_000_000_000), 2, 42).unwrap();
    for p in [0.0, 0.5, 1.0] {
        a.termination_step_started(scope, Some(p)).unwrap();
        for loss in [1, 500_000_000, 1_000_000_000, 2_000_000_000] {
            assert_eq!(
                a.accepts(SimpleScore::ZERO, SimpleScore::new(-loss), &[]),
                reference.accepts(SimpleScore::ZERO, SimpleScore::new(-loss))
            );
            a.step_ended(SimpleScore::ZERO, &[]);
            assert_eq!(gradient(&a), p, "completed steps do not invent a schedule");
        }
        reference.step_ended();
    }
}
#[test]
fn composite_notifications_preserve_short_circuit_random_streams_and_scope() {
    let scope = TerminationProgressScope::Phase;
    let mut a = MoveAcceptor::from_policies(
        &[
            Acceptance::MoveTabu {
                tenure: 1,
                aspiration: false,
            },
            cooling(scope),
        ],
        SimpleScore::ZERO,
    )
    .unwrap();
    let mut reference = cooling(scope).start(SimpleScore::ZERO).unwrap();
    a.step_ended(SimpleScore::ZERO, &[0], &0);
    a.termination_step_started(scope, Some(0.5)).unwrap();
    reference
        .termination_step_started(scope, Some(0.5))
        .unwrap();
    for _ in 0..20 {
        assert!(!a.accepts(SimpleScore::ZERO, SimpleScore::new(-1), &[0], &0));
    }
    for _ in 0..100 {
        assert_eq!(
            a.accepts(SimpleScore::ZERO, SimpleScore::new(-500_000_000), &[1], &1),
            reference.accepts(SimpleScore::ZERO, SimpleScore::new(-500_000_000), &[1])
        );
    }
    let mut values =
        ValueAcceptor::<_, i32, i32>::from_policies(&[cooling(scope)], SimpleScore::ZERO).unwrap();
    values.termination_step_started(scope, Some(1.0)).unwrap();
    assert!(!values.accepts(SimpleScore::ZERO, SimpleScore::new(-1), &[0], &0, &[0]));
}
#[test]
fn context_validation_precedes_clocks_factories_and_callbacks() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<_, SimpleScore>::new(1);
    n.add_constraint("must not score", vec![0], |_| panic!("no scoring"))
        .unwrap();
    for scope in [
        TerminationProgressScope::Solve,
        TerminationProgressScope::Phase,
    ] {
        let config = LocalSearchConfig {
            acceptance: cooling(scope),
            ..Default::default()
        };
        assert!(matches!(
            local_search(&p, &n, config),
            Err(LocalSearchError::Acceptance(
                AcceptanceError::TerminationContextRequired
            ))
        ));
        let clock = SolverClock::new(|| panic!("no clock"));
        let mut lifecycle = |_| panic!("no lifecycle");
        let r = solve_stream_with_extensions(
            &p,
            || -> Result<(StreamGraph<'_, SimpleScore>, PlanningStreamSource<i64>), GraphError> {
                panic!("no factory")
            },
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(config)],
                ..Default::default()
            },
            SolverExtensions {
                clock: Some(&clock),
                lifecycle: Some(&mut lifecycle),
                ..Default::default()
            },
        );
        assert!(matches!(
            r,
            Err(SolverError::InvalidAcceptance {
                phase: 0,
                error: AcceptanceError::TerminationContextRequired
            })
        ));
    }
}
#[test]
fn and_or_progress_changes_real_acceptance_and_scan_uses_one_snapshot() {
    // At first step time=5: AND(Time10,Steps100) progress0, OR progress0.5.
    // Seed11's first draw exceeds exp(-1/(1-0.5)) but is below exp(-1).
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, -1, -2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
        .unwrap();
    for all in [true, false] {
        let ticks = Arc::new(AtomicU64::new(0));
        let source = ticks.clone();
        let clock = SolverClock::new(move || ms(source.load(Ordering::SeqCst)));
        let leaves = [
            SearchTermination::Condition(SearchLimit::Time(ms(10))),
            SearchTermination::Condition(SearchLimit::Steps(100)),
        ];
        let condition = if all {
            SearchTermination::All(&leaves)
        } else {
            SearchTermination::Any(&leaves)
        };
        let mut lifecycle = |e| {
            if matches!(e, SolverLifecycleEvent::StepStarted { .. }) {
                ticks.store(5, Ordering::SeqCst);
            }
        };
        let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(-1),
            }]
        };
        let mut ext = SolverExtensions {
            clock: Some(&clock),
            lifecycle: Some(&mut lifecycle),
            ..Default::default()
        };
        ext.move_providers.bind(0, &mut provider);
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 1,
                    acceptance: Acceptance::SimulatedAnnealingTermination {
                        starting_temperature: SimpleScore::new(1),
                        scope: TerminationProgressScope::Solve,
                        seed: 11,
                    },
                    ..Default::default()
                })],
            },
            ext,
        )
        .unwrap();
        assert_eq!(r.work.completed_steps, u64::from(all));
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
    // A score goal without a gradient keeps the expression false after Time5
    // reaches one, allowing the next step to demonstrate frozen versus cooled.
    let ticks = Arc::new(AtomicU64::new(0));
    let source = ticks.clone();
    let clock = SolverClock::new(move || ms(source.load(Ordering::SeqCst)));
    let children = [
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Time(ms(5)))),
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::new(100))),
    ];
    let condition = ScoredSearchTermination::All(&children);
    let seen = RefCell::new(vec![]);
    let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, step: u64| {
        seen.borrow_mut().push(a.values()[0].unwrap());
        ticks.store(10, Ordering::SeqCst);
        if step == 0 {
            vec![
                Move::Change {
                    variable: 0,
                    value: Some(-2),
                },
                Move::Change {
                    variable: 0,
                    value: Some(-1),
                },
            ]
        } else {
            vec![Move::Change {
                variable: 0,
                value: Some(-2),
            }]
        }
    };
    let mut ext = SolverExtensions {
        clock: Some(&clock),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut provider);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            limits: SolverLimits {
                scored_termination: Some(&condition),
                ..Default::default()
            },
            phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 3,
                selection: MoveSelection::BestAccepted,
                acceptance: cooling(TerminationProgressScope::Solve),
                ..Default::default()
            })],
        },
        ext,
    )
    .unwrap();
    assert_eq!(*seen.borrow(), vec![0, -1]);
    assert_eq!(r.work.completed_steps, 1);
    assert_eq!(r.work.move_evaluations, 3);
    assert_eq!(r.assignment.values(), &[Some(0)]);
}

#[test]
fn stateful_threshold_reset_reheats_in_both_scopes() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| SimpleScore::new(*v[0].unwrap()))
        .unwrap();
    for scope in [
        TerminationProgressScope::Solve,
        TerminationProgressScope::Phase,
    ] {
        let ticks = Arc::new(AtomicU64::new(0));
        let source = ticks.clone();
        let clock = SolverClock::new(move || ms(source.load(Ordering::SeqCst)));
        let condition =
            StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(
                SignificantImprovementConfig::new(ms(10), SimpleScore::new(1)).unwrap(),
            ));
        let bindings = [PhaseTerminationBinding::stateful(0, &condition)];
        let mut lifecycle = |e| {
            if let SolverLifecycleEvent::StepStarted { step, .. } = e {
                ticks.store(if step == 0 { 4 } else { 5 }, Ordering::SeqCst);
            }
        };
        let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, step: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(if step == 0 { 1 } else { 0 }),
            }]
        };
        let mut ext = SolverExtensions {
            clock: Some(&clock),
            lifecycle: Some(&mut lifecycle),
            stateful_termination: (scope == TerminationProgressScope::Solve).then_some(&condition),
            phase_terminations: if scope == TerminationProgressScope::Phase {
                &bindings
            } else {
                &[]
            },
            ..Default::default()
        };
        ext.move_providers.bind(0, &mut provider);
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 2,
                    acceptance: Acceptance::SimulatedAnnealingTermination {
                        starting_temperature: SimpleScore::new(1),
                        scope,
                        seed: 11,
                    },
                    ..Default::default()
                })],
                ..Default::default()
            },
            ext,
        )
        .unwrap();
        assert_eq!(
            r.work.completed_steps, 2,
            "reset .4 -> .1 admits the seeded worsening step"
        );
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(r.assignment.values(), &[Some(1)]);
    }
}
#[test]
fn live_step_move_and_score_counts_drive_cooling_without_extra_scoring() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    for leaf in [
        SearchLimit::Steps(4),
        SearchLimit::MoveEvaluations(2),
        SearchLimit::ScoreEvaluations(4),
    ] {
        let calls = std::cell::Cell::new(0);
        let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
        n.add_constraint("value", vec![0], |v| {
            calls.set(calls.get() + 1);
            SimpleScore::new(*v[0].unwrap())
        })
        .unwrap();
        let condition = SearchTermination::Condition(leaf);
        let mut provider = |_: &FiniteProblem<i64>, _: &Assignment<i64>, step: u64| {
            vec![Move::Change {
                variable: 0,
                value: Some(if step == 0 { 1 } else { 0 }),
            }]
        };
        let mut ext = SolverExtensions::default();
        ext.move_providers.bind(0, &mut provider);
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                limits: SolverLimits {
                    termination: Some(&condition),
                    ..Default::default()
                },
                phases: vec![SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 2,
                    acceptance: Acceptance::SimulatedAnnealingTermination {
                        starting_temperature: SimpleScore::new(1),
                        scope: TerminationProgressScope::Solve,
                        seed: 11,
                    },
                    ..Default::default()
                })],
            },
            ext,
        )
        .unwrap();
        assert_eq!(r.work.completed_steps, 1);
        assert_eq!(calls.get(), 3);
        assert_eq!(r.work.evaluations, 3);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
    }
}
#[test]
fn repair_attempts_use_phase_expression_progress() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| SimpleScore::new(-*v[0].unwrap()))
        .unwrap();
    for cold in [false, true] {
        let leaves = [
            ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Time(
                if cold { Duration::ZERO } else { Duration::MAX },
            ))),
            ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::new(100))),
        ];
        let condition = ScoredSearchTermination::All(&leaves);
        let bindings = [PhaseTerminationBinding::scored(0, &condition)];
        let clock = SolverClock::new(|| Duration::ZERO);
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                    minimum_ruined: 1,
                    maximum_ruined: 1,
                    max_attempts: 2,
                    move_weights: RepairMoveWeights {
                        change: 1,
                        swap: 0,
                        repair: 0,
                    },
                    acceptance: cooling(TerminationProgressScope::Phase),
                    ..Default::default()
                })],
                ..Default::default()
            },
            SolverExtensions {
                phase_terminations: &bindings,
                clock: Some(&clock),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.work.accepted_moves, if cold { 0 } else { 2 });
        assert_eq!(r.assignment, p.initial_assignment());
        assert_eq!(r.score, Some(SimpleScore::ZERO));
    }
}
