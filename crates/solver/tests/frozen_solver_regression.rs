// Original native regressions, separate from upstream credit.
use std::cell::Cell;
use timetuck::*;

#[derive(Debug)]
struct Outcome {
    score: Option<SimpleScore>,
    complete: bool,
    work: SolverWork,
    phases: usize,
    reason: SolverTermination,
    commands: usize,
    factories: usize,
    events: Vec<(SolverPhaseKind, bool)>,
}
fn result<A>(
    r: SolverResult<A, SimpleScore>,
    commands: usize,
    factories: usize,
    events: Vec<(SolverPhaseKind, bool)>,
) -> Outcome {
    Outcome {
        score: r.score,
        complete: r.complete,
        work: r.work,
        phases: r.phases.len(),
        reason: r.termination,
        commands,
        factories,
        events,
    }
}
fn event<A>(events: &mut Vec<(SolverPhaseKind, bool)>, e: SolverEvent<'_, A, SimpleScore>) {
    match e {
        SolverEvent::Best(u) => events.push((u.kind, u.complete)),
        SolverEvent::FirstInitialized { .. } => {
            panic!("custom-only plans have no initialization boundary")
        }
    }
}
fn exercise(mode: usize, basic_free: bool, list_free: bool, limits: SolverLimits<'_>) -> Outcome {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![10, 11])
            .with_initial(10)
            .with_pinned(!basic_free),
    ])
    .unwrap();
    let l = ListProblem::new(
        vec![20],
        vec![ListEntity::new(vec![20]).with_pinned(!list_free)],
    )
    .unwrap();
    let m = MixedProblem::new(p.clone(), l.clone());
    let plan = SolverSequence {
        limits,
        phases: vec![SolverPhase::Custom(CustomPhaseConfig::default()); 2],
    };
    let mixed = MixedSolverSequence {
        limits: plan.limits,
        phases: vec![MixedSolverPhase::Custom(CustomPhaseConfig::default()); 2],
    };
    let commands = Cell::new(0);
    let factories = Cell::new(0);
    let mut events = vec![];
    match mode {
        0 => {
            let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
            let r = solve_with_commands_and_events(
                &p,
                &n,
                plan,
                |_, _| {
                    commands.set(commands.get() + 1);
                    Ok(())
                },
                |e| event(&mut events, e),
            )
            .unwrap();
            result(r, commands.get(), 0, events)
        }
        1 => {
            let n = ListConstraintNetwork::<i32, SimpleScore>::new(1, 1);
            let r = solve_list_with_commands_and_events(
                &l,
                &n,
                plan,
                |_, _| {
                    commands.set(commands.get() + 1);
                    Ok(())
                },
                |e| event(&mut events, e),
            )
            .unwrap();
            result(r, commands.get(), 0, events)
        }
        2 => {
            let n = MixedConstraintNetwork::<i32, i32, SimpleScore>::new(1, 1, 1);
            let r = solve_mixed_with_commands_and_events(
                &m,
                &n,
                mixed,
                |_, _| {
                    commands.set(commands.get() + 1);
                    Ok(())
                },
                |e| event(&mut events, e),
            )
            .unwrap();
            result(r, commands.get(), 0, events)
        }
        3 => {
            let r = solve_stream_with_commands_and_events(
                &p,
                || {
                    factories.set(factories.get() + 1);
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let s = b.source::<usize, (usize, Option<i32>)>();
                    Ok((b.build(), s))
                },
                plan,
                |_, _| {
                    commands.set(commands.get() + 1);
                    Ok(())
                },
                |e| event(&mut events, e),
            )
            .unwrap();
            result(r, commands.get(), factories.get(), events)
        }
        4 => {
            let r = solve_list_stream_with_commands_and_events(
                &l,
                || {
                    factories.set(factories.get() + 1);
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let s = ListStreamSources::new(&mut b);
                    Ok((b.build(), s))
                },
                plan,
                |_, _| {
                    commands.set(commands.get() + 1);
                    Ok(())
                },
                |e| event(&mut events, e),
            )
            .unwrap();
            result(r, commands.get(), factories.get(), events)
        }
        5 => {
            let r = solve_mixed_stream_with_commands_and_events(
                &m,
                || {
                    factories.set(factories.get() + 1);
                    let mut b = StreamGraphBuilder::<SimpleScore>::new();
                    let s = MixedStreamSources::new(&mut b);
                    Ok((b.build(), s))
                },
                mixed,
                |_, _| {
                    commands.set(commands.get() + 1);
                    Ok(())
                },
                |e| event(&mut events, e),
            )
            .unwrap();
            result(r, commands.get(), factories.get(), events)
        }
        _ => unreachable!(),
    }
}
#[test]
fn frozen_models_score_once_and_skip_commands_across_all_six_backends() {
    for mode in 0..6 {
        let r = exercise(mode, false, false, SolverLimits::default());
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert!(r.complete);
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(r.work.move_evaluations, 0);
        assert_eq!(r.phases, 0);
        assert_eq!(r.commands, 0);
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        assert_eq!(r.factories, usize::from(mode >= 3));
        assert_eq!(r.events, [(SolverPhaseKind::Initial, true)]);
    }
}
#[test]
fn either_movable_mixed_component_preserves_normal_phase_execution() {
    for mode in 0..6 {
        for (basic, list) in [(true, false), (false, true), (true, true)] {
            let movable = match mode % 3 {
                0 => basic,
                1 => list,
                _ => basic || list,
            };
            let r = exercise(mode, basic, list, SolverLimits::default());
            assert_eq!(r.commands, if movable { 2 } else { 0 });
            assert_eq!(r.phases, if movable { 2 } else { 0 });
            assert_eq!(r.work.evaluations, if movable { 2 } else { 1 });
            assert_eq!(
                r.events[0].0,
                if movable {
                    SolverPhaseKind::Custom
                } else {
                    SolverPhaseKind::Initial
                }
            );
        }
    }
}
#[test]
fn global_budget_and_cancel_gates_avoid_initial_scoring_and_factories() {
    let token = CancellationToken::new();
    token.cancel();
    for mode in 0..6 {
        for (limits, reason) in [
            (
                SolverLimits {
                    max_evaluations: 0,
                    ..Default::default()
                },
                SolverTermination::EvaluationLimit,
            ),
            (
                SolverLimits {
                    cancellation: Some(&token),
                    ..Default::default()
                },
                SolverTermination::Cancelled,
            ),
        ] {
            let r = exercise(mode, false, false, limits);
            assert_eq!(r.reason, reason);
            assert_eq!(r.score, None);
            assert!(r.complete);
            assert_eq!(r.work.evaluations, 0);
            assert_eq!(r.factories, 0);
            assert_eq!(r.commands, 0);
            assert!(r.events.is_empty());
        }
        let r = exercise(
            mode,
            false,
            false,
            SolverLimits {
                best_score_limit: Some(SimpleScore::ZERO),
                ..Default::default()
            },
        );
        assert_eq!(r.reason, SolverTermination::BestScoreLimit);
        assert_eq!(r.work.evaluations, 1);
        let r = exercise(
            mode,
            false,
            false,
            SolverLimits {
                max_move_evaluations: Some(0),
                ..Default::default()
            },
        );
        assert_eq!(r.reason, SolverTermination::MoveEvaluationLimit);
        assert_eq!(r.work.evaluations, 1);
    }
}
#[test]
fn frozen_incomplete_list_rejects_impossibility_before_budget_gates() {
    let p = ListProblem::new(vec![1, 2], vec![ListEntity::new(vec![1]).with_pinned(true)]).unwrap();
    let n = ListConstraintNetwork::<i32, SimpleScore>::new(1, 2);
    let r = solve_list(&p, &n, SolverConfig::default()).unwrap();
    assert!(!r.complete);
    assert_eq!(r.score, None);
    assert_eq!(r.termination, SolverTermination::NoCompleteAssignment);
    assert!(r.phases.is_empty());
    assert_eq!(r.work.evaluations, 0);
    let r = solve_list(
        &p,
        &n,
        SolverConfig {
            limits: SolverLimits {
                max_evaluations: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::NoCompleteAssignment);
    assert!(r.phases.is_empty());
}
#[test]
fn full_prefix_pin_still_allows_appending_unassigned_values() {
    let p = ListProblem::new(
        vec![1, 2],
        vec![ListEntity::new(vec![1]).with_pinned_prefix(1)],
    )
    .unwrap();
    let n = ListConstraintNetwork::<i32, SimpleScore>::new(1, 2);
    let r = solve_list(&p, &n, SolverConfig::default()).unwrap();
    assert!(r.complete);
    assert_eq!(r.assignment.lists(), &[vec![1, 2]]);
    assert!(!r.phases.is_empty());
}
#[test]
fn local_provider_never_opens_and_invalid_plans_still_fail() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![1]).with_initial(1).with_pinned(true),
    ])
    .unwrap();
    let n = ConstraintNetwork::<i32, SimpleScore>::new(1);
    let mut provider = |_: &FiniteProblem<i32>, _: &Assignment<i32>, _: u64| -> Vec<Move<i32>> {
        panic!("provider opened")
    };
    let mut ext = SolverExtensions::default();
    ext.move_providers.bind(1, &mut provider);
    let r = solve_with_extensions(&p, &n, SolverConfig::default(), ext).unwrap();
    assert!(r.phases.is_empty());
    assert_eq!(r.work.evaluations, 1);
    let empty = SolverSequence {
        limits: SolverLimits::default(),
        phases: vec![],
    };
    assert!(matches!(solve(&p, &n, empty), Err(SolverError::EmptyPlan)));
    let unbound = SolverSequence {
        limits: SolverLimits::default(),
        phases: vec![SolverPhase::Custom(CustomPhaseConfig::default())],
    };
    assert!(matches!(
        solve(&p, &n, unbound),
        Err(SolverError::UnboundCustomPhase { phase: 0 })
    ));
}

#[test]
fn empty_models_initialize_without_entering_a_phase_in_all_backends() {
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    let l = ListProblem::<i32>::new(vec![], vec![]).unwrap();
    let m = MixedProblem::new(p.clone(), l.clone());
    fn check<A>(r: SolverResult<A, SimpleScore>) {
        assert!(r.complete);
        assert!(r.phases.is_empty());
        assert_eq!(r.score, Some(SimpleScore::ZERO));
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(r.termination, SolverTermination::PlanCompleted);
    }
    check(solve(&p, &ConstraintNetwork::new(0), SolverConfig::default()).unwrap());
    check(
        solve_list(
            &l,
            &ListConstraintNetwork::new(0, 0),
            SolverConfig::default(),
        )
        .unwrap(),
    );
    check(
        solve_mixed(
            &m,
            &MixedConstraintNetwork::new(0, 0, 0),
            MixedSolverConfig::default(),
        )
        .unwrap(),
    );
    check(
        solve_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = b.source::<usize, (usize, Option<i32>)>();
                Ok((b.build(), s))
            },
            SolverConfig::default(),
        )
        .unwrap(),
    );
    check(
        solve_list_stream(
            &l,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = ListStreamSources::new(&mut b);
                Ok((b.build(), s))
            },
            SolverConfig::default(),
        )
        .unwrap(),
    );
    check(
        solve_mixed_stream(
            &m,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::new(&mut b);
                Ok((b.build(), s))
            },
            MixedSolverConfig::default(),
        )
        .unwrap(),
    );
}

#[test]
fn initialization_failures_do_not_claim_a_custom_phase_ran() {
    let p = FiniteProblem::<i32>::new(vec![]).unwrap();
    let wrong = ConstraintNetwork::<i32, SimpleScore>::new(1);
    assert!(matches!(
        solve(&p, &wrong, SolverConfig::default()),
        Err(SolverError::InitialScoring { .. })
    ));
    let error = solve_stream(
        &p,
        || {
            let b = StreamGraphBuilder::<SimpleScore>::new();
            let mut other = StreamGraphBuilder::<SimpleScore>::new();
            let s = other.source::<usize, (usize, Option<i32>)>();
            Ok((b.build(), s))
        },
        SolverConfig::default(),
    )
    .unwrap_err();
    assert!(matches!(error, SolverError::InitialScoring { .. }));
    assert!(error.to_string().starts_with("initial scoring failed:"));
}

#[test]
fn frozen_initialization_evaluates_real_constraints_exactly_once() {
    let p = FiniteProblem::new(vec![
        Variable::new(vec![7]).with_initial(7).with_pinned(true),
    ])
    .unwrap();
    for stream in [false, true] {
        let calls = Cell::new(0);
        let mut events = vec![];
        let r = if stream {
            solve_stream_with_observer(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = b.source::<usize, (usize, Option<i32>)>();
                    b.score("twice", s.port(), |(_, v)| {
                        calls.set(calls.get() + 1);
                        SimpleScore::new(i64::from(v.unwrap()) * 2)
                    })?;
                    Ok((b.build(), s))
                },
                SolverConfig::default(),
                |u| events.push((u.kind, u.score)),
            )
            .unwrap()
        } else {
            let mut n = ConstraintNetwork::new(1);
            n.add_constraint("twice", vec![0], |v| {
                calls.set(calls.get() + 1);
                SimpleScore::new(i64::from(*v[0].unwrap()) * 2)
            })
            .unwrap();
            solve_with_observer(&p, &n, SolverConfig::default(), |u| {
                events.push((u.kind, u.score))
            })
            .unwrap()
        };
        assert_eq!(r.score, Some(SimpleScore::new(14)));
        assert_eq!(events, [(SolverPhaseKind::Initial, SimpleScore::new(14))]);
        assert_eq!(calls.get(), 1);
        assert_eq!(r.work.constraint_evaluations, 1);
    }
}
