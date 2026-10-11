//! Original orchestration/observation contracts; no upstream method credit.
use std::{
    cell::Cell,
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
fn history(window: u64, difference: i64) -> StatefulSearchTermination<'static> {
    StatefulSearchTermination::Condition(StatefulSearchLimit::SignificantImprovement(
        SignificantImprovementConfig::new(ms(window), SimpleScore::new(difference)).unwrap(),
    ))
}
fn clock() -> (Arc<AtomicU64>, SolverClock) {
    let ticks = Arc::new(AtomicU64::new(0));
    let a = ticks.clone();
    (
        ticks,
        SolverClock::new(move || ms(a.load(Ordering::SeqCst))),
    )
}
fn local(steps: u64) -> SolverPhase<'static> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: steps,
        acceptance: Acceptance::HillClimbing,
        ..Default::default()
    })
}
#[test]
fn phase_reset_differs_from_solve_history_and_preserves_global_best() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let condition = history(5, 10);
    let bindings = [
        PhaseTerminationBinding::stateful(0, &condition),
        PhaseTerminationBinding::stateful(1, &condition),
    ];
    for phase in [false, true] {
        let (ticks, source) = clock();
        let mut event = |event| {
            if matches!(event, SolverLifecycleEvent::PhaseStarted { phase: 1, .. }) {
                ticks.fetch_add(2, Ordering::SeqCst);
            }
            if matches!(event, SolverLifecycleEvent::StepStarted { .. }) {
                ticks.fetch_add(4, Ordering::SeqCst);
            }
        };
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![local(1), local(1)],
                ..Default::default()
            },
            SolverExtensions {
                clock: Some(&source),
                lifecycle: Some(&mut event),
                stateful_termination: (!phase).then_some(&condition),
                phase_terminations: if phase { &bindings } else { &[] },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.work.completed_steps, if phase { 2 } else { 1 });
        assert_eq!(
            r.termination,
            if phase {
                SolverTermination::PlanCompleted
            } else {
                SolverTermination::Condition
            }
        );
        assert_eq!(r.assignment, p.initial_assignment());
    }
}
#[test]
fn later_lower_phase_scores_do_not_rebase_solve_history() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2, 3]).with_initial(0)]).unwrap();
    let phase = Cell::new(0);
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("phase score", vec![0], |v| {
        SimpleScore::new(if phase.get() == 0 {
            10 * *v[0].unwrap()
        } else {
            *v[0].unwrap() - 1
        })
    })
    .unwrap();
    let (ticks, source) = clock();
    let mut event = |event| match event {
        SolverLifecycleEvent::PhaseStarted { phase: i, .. } => phase.set(i),
        SolverLifecycleEvent::StepStarted { phase: 0, .. } => {
            ticks.store(1, Ordering::SeqCst);
        }
        SolverLifecycleEvent::StepStarted { phase: 1, step, .. } => {
            ticks.store(if step == 0 { 3 } else { 7 }, Ordering::SeqCst);
        }
        _ => {}
    };
    let mut first = |_: &FiniteProblem<i64>, _: &Assignment<i64>, _: u64| {
        vec![Move::Change {
            variable: 0,
            value: Some(1),
        }]
    };
    let mut later = |_: &FiniteProblem<i64>, a: &Assignment<i64>, _: u64| {
        vec![Move::Change {
            variable: 0,
            value: Some(a.values()[0].unwrap() + 1),
        }]
    };
    let condition = history(5, 10);
    let mut ext = SolverExtensions {
        clock: Some(&source),
        lifecycle: Some(&mut event),
        stateful_termination: Some(&condition),
        ..Default::default()
    };
    ext.move_providers.bind(0, &mut first);
    ext.move_providers.bind(1, &mut later);
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![local(1), local(2)],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.phases[0].score, Some(SimpleScore::new(10)));
    assert_eq!(r.phases[1].score, Some(SimpleScore::new(1)));
    assert_eq!(r.phases[1].work.completed_steps, 1);
    assert_eq!(r.score, Some(SimpleScore::new(10)));
    assert_eq!(r.assignment.values(), &[Some(1)]);
}
#[test]
fn later_construction_improvement_cannot_reset_an_armed_search_anchor() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![0_i64, 1, 2]).with_initial(0),
        Variable::new(vec![200]).allowing_unassigned(),
    ])
    .unwrap();
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(2);
    n.add_constraint("sum", vec![0, 1], |v| {
        SimpleScore::new(v.iter().flatten().map(|v| **v).sum())
    })
    .unwrap();
    let (ticks, source) = clock();
    let mut event = |event| {
        if let SolverLifecycleEvent::PhaseStarted { phase, .. } = event {
            ticks.store(
                match phase {
                    0 => 0,
                    1 => 3,
                    _ => 6,
                },
                Ordering::SeqCst,
            );
        }
    };
    let condition = history(5, 100);
    let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, _: u64| {
        vec![Move::Change {
            variable: 0,
            value: Some(a.values()[0].unwrap() + 1),
        }]
    };
    let mut ext = SolverExtensions {
        clock: Some(&source),
        lifecycle: Some(&mut event),
        stateful_termination: Some(&condition),
        ..Default::default()
    };
    ext.move_providers
        .bind_shared([0, 2], &mut provider)
        .unwrap();
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![
                local(1),
                SolverPhase::Construction(ConstructionConfig {
                    optional: OptionalConstruction::NonWorsening,
                    ..Default::default()
                }),
                local(1),
            ],
            ..Default::default()
        },
        ext,
    )
    .unwrap();
    assert_eq!(r.phases.len(), 3);
    assert_eq!(r.work.completed_steps, 1);
    assert_eq!(r.work.construction_steps, 1);
    assert_eq!(r.score, Some(SimpleScore::new(201)));
    assert_eq!(r.termination, SolverTermination::Condition);
}
#[test]
fn first_complete_exact_candidate_arms_history_after_partial_roots() {
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::BranchAndBound(Default::default()),
    ] {
        let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]); 3]).unwrap();
        let (ticks, source) = clock();
        let complete = Cell::new(0);
        let mut n = ConstraintNetwork::<i64, SimpleScore>::new(3);
        n.add_constraint("advance", vec![0, 1, 2], |v| {
            ticks.fetch_add(10, Ordering::SeqCst);
            if v.iter().all(Option::is_some) {
                complete.set(complete.get() + 1);
            }
            SimpleScore::ZERO
        })
        .unwrap();
        let condition = history(5, 10);
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![SolverPhase::Exact(ExactPhaseConfig {
                    strategy,
                    ..Default::default()
                })],
                ..Default::default()
            },
            SolverExtensions {
                clock: Some(&source),
                stateful_termination: Some(&condition),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(r.complete);
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert_eq!(r.termination, SolverTermination::Condition);
        assert!(complete.get() >= 1, "partial roots must not arm the timer");
        assert!(!r.is_optimal());
    }
}
#[test]
fn static_counter_subtrees_allocate_live_score_counts_and_bad_bindings_precede_callbacks() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1]).with_initial(0)]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let counter = ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
        SearchLimit::ScoreEvaluations(2),
    ));
    let condition = StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&counter));
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![local(10)],
            ..Default::default()
        },
        SolverExtensions {
            stateful_termination: Some(&condition),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.evaluations, 2);
    assert_eq!(r.termination, SolverTermination::Condition);
    let invalid = [PhaseTerminationBinding::stateful(1, &condition)];
    let duplicate = [
        PhaseTerminationBinding::stateful(0, &condition),
        PhaseTerminationBinding::stateful(0, &condition),
    ];
    for (bindings, duplicate) in [(&invalid[..], false), (&duplicate[..], true)] {
        let clock = SolverClock::new(|| panic!("invalid binding must precede clock"));
        let mut event = |_| panic!("invalid binding must precede lifecycle");
        let r = solve_with_extensions(
            &p,
            &n,
            SolverSequence {
                phases: vec![local(1)],
                ..Default::default()
            },
            SolverExtensions {
                clock: Some(&clock),
                lifecycle: Some(&mut event),
                phase_terminations: bindings,
                ..Default::default()
            },
        );
        assert!(if duplicate {
            matches!(r, Err(SolverError::DuplicatePhaseTermination { phase: 0 }))
        } else {
            matches!(r, Err(SolverError::InvalidPhaseTermination { phase: 1 }))
        });
    }
}

#[test]
fn custom_moves_share_history_and_live_counts_without_extra_scoring() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2, 3]).with_initial(0)]).unwrap();
    let calls = Cell::new(0);
    let mut n = ConstraintNetwork::<i64, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| {
        calls.set(calls.get() + 1);
        SimpleScore::new(*v[0].unwrap())
    })
    .unwrap();
    let (ticks, source) = clock();
    let count = ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
        SearchLimit::MoveEvaluations(2),
    ));
    let children = [
        history(2, 10),
        StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&count)),
    ];
    let condition = StatefulSearchTermination::All(&children);
    let mut command = |_: usize,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        SimpleScore,
        ScoringError,
    >| {
        for v in 1..=3 {
            ticks.store(v as u64, Ordering::SeqCst);
            let result = c.execute(&Move::Change {
                variable: 0,
                value: Some(v),
            })?;
            assert_eq!(result.is_some(), v < 3);
        }
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
            clock: Some(&source),
            stateful_termination: Some(&condition),
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(calls.get(), 3);
    assert_eq!(r.score, Some(SimpleScore::new(2)));
    assert_eq!(r.termination, SolverTermination::Condition);
}
#[test]
fn repair_history_combines_with_provisional_reconstruction_counts_inside_scan() {
    let p = FiniteProblem::new(vec![Variable::new(vec![0_i64, 1, 2]).with_initial(0); 2]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let (ticks, source) = clock();
    let count = ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
        SearchLimit::MoveEvaluationsOf {
            kind: MoveEvaluationKind::Reconstruction,
            limit: 2,
        },
    ));
    let children = [
        history(5, 10),
        StatefulSearchTermination::Condition(StatefulSearchLimit::Static(&count)),
    ];
    let condition = StatefulSearchTermination::All(&children);
    let mut event = |event| {
        if matches!(event, SolverLifecycleEvent::StepStarted { .. }) {
            ticks.store(10, Ordering::SeqCst);
        }
    };
    let r = solve_with_extensions(
        &p,
        &n,
        SolverSequence {
            phases: vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 2,
                maximum_ruined: 2,
                max_attempts: 2,
                ..Default::default()
            })],
            ..Default::default()
        },
        SolverExtensions {
            clock: Some(&source),
            stateful_termination: Some(&condition),
            lifecycle: Some(&mut event),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.work.repair_attempts, 1);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(r.termination, SolverTermination::Condition);
    assert_eq!(r.assignment, p.initial_assignment());
}
