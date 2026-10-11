//! Original phase expression contracts; no upstream source-method credit.
use std::{
    cell::Cell,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use timetuck::*;
fn problem(initialized: bool) -> FiniteProblem<i64> {
    FiniteProblem::new(vec![
        if initialized {
            Variable::new(vec![0, 1, 2]).with_initial(0)
        } else {
            Variable::new(vec![0, 1, 2])
        };
        2
    ])
    .unwrap()
}
fn sequence(phases: Vec<SolverPhase<'_>>) -> SolverSequence<'_> {
    SolverSequence {
        phases,
        ..Default::default()
    }
}
fn local() -> SolverPhase<'static> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        acceptance: Acceptance::HillClimbing,
        max_steps: 100,
        ..Default::default()
    })
}
#[test]
fn phase_time_and_move_origins_restart_and_global_deadline_keeps_precedence() {
    let p = problem(true);
    for global in [None, Some(Duration::from_millis(11))] {
        let ticks = Arc::new(AtomicU64::new(1000));
        let read = ticks.clone();
        let clock = SolverClock::new(move || Duration::from_millis(read.load(Ordering::SeqCst)));
        let mut n = ConstraintNetwork::new(2);
        n.add_constraint("ticks", vec![0, 1], move |_| {
            ticks.fetch_add(3, Ordering::SeqCst);
            SimpleScore::ZERO
        })
        .unwrap();
        let leaves = [
            SearchTermination::Condition(SearchLimit::Time(Duration::from_millis(5))),
            SearchTermination::Condition(SearchLimit::MoveEvaluations(1)),
        ];
        let condition = SearchTermination::All(&leaves);
        let bindings = [
            PhaseTerminationBinding::search(0, &condition),
            PhaseTerminationBinding::search(1, &condition),
        ];
        let mut plan = sequence(vec![local(), local()]);
        plan.limits.max_time = global;
        let r = solve_with_extensions(
            &p,
            &n,
            plan,
            SolverExtensions {
                clock: Some(&clock),
                phase_terminations: &bindings,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.phases[0].work.evaluations, 2);
        assert_eq!(r.phases[1].work.evaluations, 2);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
        );
        assert_eq!(
            r.termination,
            if global.is_some() {
                SolverTermination::TimeLimit
            } else {
                SolverTermination::PlanCompleted
            }
        );
    }
}
#[test]
fn partial_construction_never_arms_score_goal_and_handoff_can_finish() {
    let p = problem(false);
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let all = [
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::ZERO)),
        ScoredSearchTermination::Condition(ScoredSearchLimit::Search(SearchLimit::Steps(0))),
    ];
    let goal = ScoredSearchTermination::All(&all);
    let count = ScoredSearchTermination::Condition(ScoredSearchLimit::Search(
        SearchLimit::ScoreEvaluations(2),
    ));
    let any = [goal, count];
    let condition = ScoredSearchTermination::Any(&any);
    let bindings = [PhaseTerminationBinding::scored(0, &condition)];
    let r = solve_with_extensions(
        &p,
        &n,
        sequence(vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::Construction(Default::default()),
        ]),
        SolverExtensions {
            phase_terminations: &bindings,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.phases[0].work.evaluations, 2);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Construction(ConstructionTermination::Condition)
    );
    assert!(r.complete);
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
}
#[test]
fn every_exact_strategy_can_stop_its_phase_without_stopping_the_plan() {
    let p = problem(true);
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let condition = SearchTermination::Condition(SearchLimit::ScoreEvaluations(1));
    let bindings = [PhaseTerminationBinding::search(0, &condition)];
    for strategy in [
        ExactSearchStrategy::Enumerate,
        ExactSearchStrategy::Complete,
        ExactSearchStrategy::BranchAndBound(Default::default()),
        ExactSearchStrategy::CompleteBranchAndBound(Default::default()),
    ] {
        let r = solve_with_extensions(
            &p,
            &n,
            sequence(vec![
                SolverPhase::Exact(ExactPhaseConfig {
                    strategy,
                    ..Default::default()
                }),
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 0,
                    ..Default::default()
                }),
            ]),
            SolverExtensions {
                phase_terminations: &bindings,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.phases[0].work.evaluations, 1);
        // A singleton completion can finish naturally before another poll.
        assert!(matches!(
            r.phases[0].termination,
            PhaseTermination::Exact(
                ExactPhaseTermination::Condition | ExactPhaseTermination::CompletionExhausted
            )
        ));
        assert_eq!(r.phases[1].work.evaluations, 1);
        assert_eq!(r.termination, SolverTermination::PlanCompleted);
        assert!(!r.is_optimal());
    }
}
#[test]
fn custom_worsening_move_keeps_incoming_best_for_next_phase_scored_goal() {
    let p = problem(true);
    let mut n = ConstraintNetwork::new(2);
    n.add_constraint("negative", vec![0, 1], |v| {
        SimpleScore::new(-v.iter().flatten().map(|v| **v).sum::<i64>())
    })
    .unwrap();
    let first = SearchTermination::Condition(SearchLimit::MoveEvaluations(1));
    let second =
        ScoredSearchTermination::Condition(ScoredSearchLimit::BestScore(SimpleScore::ZERO));
    let bindings = [
        PhaseTerminationBinding::search(0, &first),
        PhaseTerminationBinding::scored(1, &second),
    ];
    let mut seen = vec![];
    let mut command = |phase,
                       c: &mut CustomPhaseContext<
        '_,
        Assignment<i64>,
        Move<i64>,
        SimpleScore,
        ScoringError,
    >| {
        seen.push(phase);
        if phase == 0 {
            assert!(
                c.execute(&Move::Change {
                    variable: 0,
                    value: Some(1)
                })?
                .is_some()
            );
            assert!(c.is_terminated());
        } else {
            assert_eq!(c.assignment().values(), &[Some(0), Some(0)]);
            assert!(c.is_terminated());
            assert!(
                c.execute(&Move::Change {
                    variable: 0,
                    value: Some(2)
                })?
                .is_none()
            );
        }
        Ok(())
    };
    let r = solve_with_extensions(
        &p,
        &n,
        sequence(vec![SolverPhase::Custom(Default::default()); 2]),
        SolverExtensions {
            phase_terminations: &bindings,
            command: Some(&mut command),
            ..Default::default()
        },
    )
    .unwrap();
    // The next phase is entered and initialized, but its incoming retained best
    // already reaches the scored goal, so the custom command is skipped.
    assert_eq!(seen, vec![0]);
    assert_eq!(r.phases.len(), 2);
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::Custom(CustomPhaseTermination::Condition)
    );
    assert_eq!(r.phases[1].work.evaluations, 1);
    assert_eq!(r.work.move_evaluations, 1);
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
    assert_eq!(r.score, Some(SimpleScore::ZERO));
}
#[test]
fn phase_stagnation_rearms_after_initialization_in_each_phase() {
    let p = problem(true);
    for timed in [false, true] {
        let ticks = Arc::new(AtomicU64::new(0));
        let read = ticks.clone();
        let clock = SolverClock::new(move || Duration::from_millis(read.load(Ordering::SeqCst)));
        let mut n = ConstraintNetwork::new(2);
        n.add_constraint("ticks", vec![0, 1], move |_| {
            ticks.fetch_add(1, Ordering::SeqCst);
            SimpleScore::ZERO
        })
        .unwrap();
        let condition = SearchTermination::Condition(if timed {
            SearchLimit::UnimprovedTime(Duration::from_millis(3))
        } else {
            SearchLimit::UnimprovedSteps(2)
        });
        let bindings = [
            PhaseTerminationBinding::search(0, &condition),
            PhaseTerminationBinding::search(1, &condition),
        ];
        let r = solve_with_extensions(
            &p,
            &n,
            sequence(vec![local(), local()]),
            SolverExtensions {
                clock: Some(&clock),
                phase_terminations: &bindings,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.termination, SolverTermination::PlanCompleted);
        assert_eq!(r.phases[0].work.evaluations, r.phases[1].work.evaluations);
        if timed {
            assert_eq!(r.phases[0].work.evaluations, 4);
        } else {
            assert_eq!(r.phases[0].work.completed_steps, 2);
        }
    }
}
#[test]
fn invalid_bindings_fail_before_clock_lifecycle_or_factory_callbacks() {
    let p = problem(true);
    let calls = Cell::new(0);
    let condition = SearchTermination::Condition(SearchLimit::MoveEvaluations(1));
    let clock = SolverClock::new(|| panic!("clock must not run"));
    for duplicate in [false, true] {
        let bindings = if duplicate {
            vec![
                PhaseTerminationBinding::search(0, &condition),
                PhaseTerminationBinding::search(0, &condition),
            ]
        } else {
            vec![PhaseTerminationBinding::search(1, &condition)]
        };
        let mut lifecycle = |_| panic!("lifecycle must not run");
        let r = solve_stream_with_extensions(
            &p,
            || {
                calls.set(calls.get() + 1);
                let mut g = StreamGraphBuilder::<SimpleScore>::new();
                let source = g.source::<usize, (usize, Option<i64>)>();
                Ok((g.build(), source))
            },
            sequence(vec![local()]),
            SolverExtensions {
                clock: Some(&clock),
                lifecycle: Some(&mut lifecycle),
                phase_terminations: &bindings,
                ..Default::default()
            },
        );
        assert!(if duplicate {
            matches!(r, Err(SolverError::DuplicatePhaseTermination { phase: 0 }))
        } else {
            matches!(r, Err(SolverError::InvalidPhaseTermination { phase: 1 }))
        });
    }
    assert_eq!(calls.get(), 0);
}

#[test]
fn shared_evaluation_limit_still_ends_solve_after_a_phase_local_stop() {
    let p = problem(true);
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let phase = SearchTermination::Condition(SearchLimit::ScoreEvaluations(2));
    let bindings = [
        PhaseTerminationBinding::search(0, &phase),
        PhaseTerminationBinding::search(1, &phase),
    ];
    let mut plan = sequence(vec![local(), local(), local()]);
    plan.limits.max_evaluations = 3;
    let r = solve_with_extensions(
        &p,
        &n,
        plan,
        SolverExtensions {
            phase_terminations: &bindings,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.phases.len(), 2);
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
    );
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::EvaluationLimit)
    );
    assert_eq!(r.termination, SolverTermination::EvaluationLimit);
}

#[test]
fn provider_phases_use_bindings_and_reconstruction_does_not_advance_construction_steps() {
    let p = problem(false);
    let n = ConstraintNetwork::<_, SimpleScore>::new(2);
    let construction = SearchTermination::Condition(SearchLimit::ConstructionSteps(1));
    let stagnant = SearchTermination::Condition(SearchLimit::UnimprovedTime(Duration::ZERO));
    let local_moves = SearchTermination::Condition(SearchLimit::MoveEvaluations(1));
    let bindings = [
        PhaseTerminationBinding::search(0, &construction),
        PhaseTerminationBinding::search(1, &stagnant),
        PhaseTerminationBinding::search(2, &local_moves),
    ];
    let mut provider = |_: &FiniteProblem<i64>, a: &Assignment<i64>, _: u64| {
        a.values()
            .iter()
            .enumerate()
            .map(|(variable, value)| Move::Change {
                variable,
                value: Some(if *value == Some(0) { 1 } else { 0 }),
            })
            .collect::<Vec<_>>()
    };
    let mut extensions = SolverExtensions {
        phase_terminations: &bindings,
        ..Default::default()
    };
    extensions
        .move_providers
        .bind_shared([0, 2], &mut provider)
        .unwrap();
    let r = solve_with_extensions(
        &p,
        &n,
        sequence(vec![
            SolverPhase::Construction(Default::default()),
            SolverPhase::Construction(Default::default()),
            local(),
        ]),
        extensions,
    )
    .unwrap();
    assert_eq!(r.phases.len(), 3);
    assert_eq!(r.phases[0].work.construction_steps, 1);
    assert_eq!(
        r.phases[0].termination,
        PhaseTermination::Construction(ConstructionTermination::Condition)
    );
    assert_eq!(
        r.phases[1].termination,
        PhaseTermination::Construction(ConstructionTermination::Completed)
    ); // Stagnation is inapplicable to construction.
    assert_eq!(r.phases[2].work.move_evaluations, 1);
    assert_eq!(
        r.phases[2].termination,
        PhaseTermination::LocalSearch(LocalSearchTermination::StepCondition)
    );
    assert_eq!(r.termination, SolverTermination::PlanCompleted);
}
