//! Original six-path repair phase/lifecycle checks; no upstream method credit.
#![allow(dead_code)] // Shared full solver fixture also supports legacy-only helpers.
include!("solver_support/fixture.rs");
fn repair() -> RuinRecreateConfig<'static> {
    RuinRecreateConfig {
        minimum_ruined: usize::MAX,
        maximum_ruined: usize::MAX,
        max_attempts: 1,
        assert_scores: true,
        ..Default::default()
    }
}
fn phases<'a>(phases: Vec<SolverPhase<'a>>) -> SolverSequence<'a> {
    SolverSequence {
        phases,
        limits: SolverLimits::default(),
    }
}
fn check(mode: usize, initialized: bool, c: SolverSequence) -> Outcome {
    run_scored(
        (mode, initialized, MixedConstructionOrder::BasicThenLists),
        c,
        &Cell::new(0),
        None,
        || {},
        SimpleScore::new,
    )
    .unwrap()
}
#[test]
fn repair_phases_improve_all_backends_and_report_all_reconstruction_work() {
    for mode in 0..6 {
        let r = check(
            mode,
            true,
            phases(vec![
                SolverPhase::RuinRecreate(repair()),
                SolverPhase::RuinRecreate(repair()),
            ]),
        );
        let n = if mode < 4 { 1 } else { 2 };
        assert_eq!(r.score, Some(SimpleScore::new(2 * n)));
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.reason, SolverTermination::PlanCompleted);
        assert_eq!(r.work.evaluations, 2 * (2 + 3 * n as u64));
        assert_eq!(r.work.move_evaluations, 6 * n as u64);
        assert_eq!(r.work.accepted_moves, 1);
        assert_eq!(r.work.completed_steps, 1);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::RuinRecreate(RuinRecreateTermination::AttemptLimit)
        );
        assert_eq!(r.phases[1].work.accepted_moves, 0);
    }
}
#[test]
fn every_shared_score_and_move_prefix_caps_repeated_repairs() {
    for mode in 0..6 {
        for scores in 0..=20 {
            for moves in 0..=15 {
                let mut c = phases(vec![SolverPhase::RuinRecreate(repair()); 3]);
                c.limits.max_evaluations = scores;
                c.limits.max_move_evaluations = Some(moves);
                let r = check(mode, true, c);
                assert!(r.work.evaluations <= scores);
                assert!(r.work.move_evaluations <= moves);
                assert_eq!(
                    r.work.evaluations,
                    r.phases.iter().map(|p| p.work.evaluations).sum()
                );
                assert_eq!(
                    r.work.move_evaluations,
                    r.phases.iter().map(|p| p.work.move_evaluations).sum()
                );
                let per = if mode < 4 { 3 } else { 6 };
                let mut es = 0;
                let mut ms = 0;
                let mut count = 0;
                let mut best = 0_i64;
                for phase in 0..3 {
                    if phase > 0 && (es == scores || ms == moves) {
                        break;
                    }
                    count += 1;
                    if es == scores {
                        break;
                    }
                    es += 1;
                    if es == scores || ms == moves {
                        break;
                    }
                    es += 1;
                    let trial = (scores - es).min(moves - ms).min(per);
                    es += trial;
                    ms += trial;
                    if mode < 4 && trial >= 1 {
                        best = best.max(trial as i64 - 1);
                    }
                    if mode >= 4 && trial >= 4 {
                        best = best.max(trial as i64 - 2);
                    }
                }
                assert_eq!(r.score, (scores > 0).then_some(SimpleScore::new(best)));
                assert_eq!(
                    (r.work.evaluations, r.work.move_evaluations, r.phases.len()),
                    (es, ms, count),
                    "mode={mode} scores={scores} moves={moves}"
                );
            }
        }
    }
}
#[test]
fn construction_and_exact_phases_handoff_to_repair_with_initialization_events() {
    for mode in 0..6 {
        let factories = Cell::new(0);
        let mut updates = vec![];
        let mut initialized = vec![];
        let c = phases(vec![
            SolverPhase::Construction(ConstructionConfig {
                max_steps: Some(0),
                ..Default::default()
            }),
            SolverPhase::Exact(ExactPhaseConfig {
                max_evaluations: 1,
                ..Default::default()
            }),
            SolverPhase::RuinRecreate(repair()),
        ]);
        let r = run_scored_events(
            (mode, false, MixedConstructionOrder::ListsThenBasic),
            c,
            &factories,
            None,
            || {},
            SimpleScore::new,
            Some(&mut |u| updates.push(u)),
            Some(&mut |i| initialized.push(i)),
        )
        .unwrap();
        assert_eq!(r.phases.len(), 3);
        assert_eq!(initialized.len(), 1);
        assert_eq!(initialized[0].initialization.kind, SolverPhaseKind::Exact);
        assert_eq!(initialized[0].initialization.phase, 1);
        assert!(initialized[0].complete);
        assert_eq!(updates.last().unwrap().kind, SolverPhaseKind::RuinRecreate);
        assert_eq!(updates.last().unwrap().score, r.score.unwrap());
        assert!(updates.windows(2).all(|w| w[0].score < w[1].score));
        assert_eq!(factories.get(), if mode % 2 == 1 { 4 } else { 0 });
    }
}
#[test]
fn incomplete_repair_handoffs_and_all_phase_validation_precede_factories() {
    for mode in 0..6 {
        for error in [false, true] {
            let factories = Cell::new(0);
            let mut c = phases(vec![
                SolverPhase::Construction(ConstructionConfig {
                    max_steps: Some(0),
                    ..Default::default()
                }),
                SolverPhase::RuinRecreate(repair()),
            ]);
            c.limits.incomplete_handoff = if error {
                IncompleteHandoffPolicy::Error
            } else {
                IncompleteHandoffPolicy::ReturnPartial
            };
            let r = run_scored(
                (mode, false, MixedConstructionOrder::BasicThenLists),
                c,
                &factories,
                None,
                || {},
                SimpleScore::new,
            );
            if error {
                assert_eq!(r, Err(Failure::Incomplete(1)));
            } else {
                assert_eq!(r.unwrap().reason, SolverTermination::IncompleteConstruction);
            }
            assert_eq!(factories.get(), if mode % 2 == 1 { 1 } else { 0 });
        }
        let factories = Cell::new(0);
        let c = phases(vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                minimum_ruined: 0,
                ..repair()
            }),
        ]);
        assert_eq!(
            run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &factories,
                None,
                || {},
                SimpleScore::new
            ),
            Err(Failure::Repair(1))
        );
        assert_eq!(factories.get(), 0);
    }
}
#[test]
fn global_targets_feasibility_cancellation_and_zero_time_apply_to_repair() {
    for mode in 0..6 {
        let max = if mode < 4 { 2 } else { 4 };
        let mut c = phases(vec![
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                max_attempts: 10,
                ..repair()
            });
            2
        ]);
        c.limits.best_score_limit = Some(SimpleScore::new(max));
        let r = check(mode, true, c);
        assert_eq!(r.reason, SolverTermination::BestScoreLimit);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.work.accepted_moves, 1);
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair())]);
        c.limits.stop_when_feasible = true;
        let r = check(mode, true, c);
        assert_eq!(r.reason, SolverTermination::FeasibleScore);
        assert_eq!(r.work.evaluations, 1);
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair())]);
        c.limits.max_time = Some(std::time::Duration::ZERO);
        let r = check(mode, true, c);
        assert_eq!(r.reason, SolverTermination::TimeLimit);
        assert_eq!(r.work.evaluations, 1);
        let token = CancellationToken::new();
        token.cancel();
        let factories = Cell::new(0);
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair())]);
        c.limits.cancellation = Some(&token);
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &factories,
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(r.work.evaluations, 0);
        assert_eq!(factories.get(), 0);
    }
}
#[test]
fn callback_cancellation_retains_repair_best_and_suppresses_later_phases() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        let mut updates = vec![];
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair()); 2]);
        c.limits.cancellation = Some(&token);
        let r = run_scored_observed(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            Some(&mut |u| {
                updates.push(u);
                if updates.len() == 2 {
                    token.cancel();
                }
            }),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(updates.len(), 2);
        assert_eq!(updates.last().unwrap().score, r.score.unwrap());
        assert_eq!(r.work.accepted_moves, 1);
    }
}
#[test]
fn shared_stagnation_counts_only_accepted_changed_repairs() {
    for mode in 0..6 {
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair()); 2]);
        c.limits.max_unimproved_steps = Some(0);
        let r = check(mode, true, c);
        assert_eq!(r.reason, SolverTermination::UnimprovedStepLimit);
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(r.work.completed_steps, 0);
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair()); 2]);
        c.limits.max_unimproved_time = Some(std::time::Duration::ZERO);
        let r = check(mode, true, c);
        assert_eq!(r.reason, SolverTermination::UnimprovedTimeLimit);
        assert_eq!(r.work.evaluations, 1);
    }
}
#[test]
fn repair_factory_errors_keep_phase_identity_and_skip_unused_graphs() {
    for mode in [1, 3, 5] {
        for fail in [1, 2, 3] {
            let factories = Cell::new(0);
            let c = phases(vec![SolverPhase::RuinRecreate(repair()); 2]);
            let r = run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &factories,
                Some(fail),
                || {},
                SimpleScore::new,
            );
            assert_eq!(r, Err(Failure::Repair(if fail <= 2 { 0 } else { 1 })));
            assert_eq!(factories.get(), fail);
        }
        let factories = Cell::new(0);
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair())]);
        c.limits.max_evaluations = 0;
        let r = run_scored(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &factories,
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.work.evaluations, 0);
        assert_eq!(factories.get(), 0);
    }
}

#[test]
fn accepted_equal_repairs_age_shared_stagnation_but_noops_do_not() {
    for mode in 0..6 {
        for limit in [1, 2] {
            let mut c = phases(vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
                max_attempts: 3,
                ..repair()
            })]);
            c.limits.max_unimproved_steps = Some(limit);
            let r = run_scored_events_initial(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &Cell::new(0),
                None,
                || {},
                |_| SimpleScore::ZERO,
                None,
                None,
                2,
            )
            .unwrap();
            assert_eq!(r.work.completed_steps, 1);
            assert_eq!(r.work.accepted_moves, 1);
            assert_eq!(
                r.reason,
                if limit == 1 {
                    SolverTermination::UnimprovedStepLimit
                } else {
                    SolverTermination::PlanCompleted
                }
            );
            assert_eq!(r.phases.len(), 1);
        }
    }
}
#[test]
fn local_and_repair_phases_share_stagnation_without_resetting_at_handoff() {
    for mode in 0..6 {
        let mut c = phases(vec![
            SolverPhase::LocalSearch(LocalSearchConfig {
                acceptance: Acceptance::HillClimbing,
                max_steps: 1,
                ..Default::default()
            }),
            SolverPhase::RuinRecreate(repair()),
            SolverPhase::RuinRecreate(repair()),
        ]);
        c.limits.max_unimproved_steps = Some(2);
        let r = run_scored_events_initial(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
            None,
            None,
            2,
        )
        .unwrap();
        // The plan retains its equal-score initial assignment (value 2), so both
        // improvement phases independently change it; global history must not reset.
        assert_eq!(r.reason, SolverTermination::UnimprovedStepLimit);
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.work.completed_steps, 2);
    }
}
#[test]
fn repair_phase_observer_time_expires_global_stagnation_before_reconstruction() {
    for mode in 0..6 {
        let mut c = phases(vec![SolverPhase::RuinRecreate(repair()); 2]);
        c.limits.max_unimproved_time = Some(std::time::Duration::from_millis(1));
        let r = run_scored_observed(
            (mode, true, MixedConstructionOrder::BasicThenLists),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            Some(&mut |_| std::thread::sleep(std::time::Duration::from_millis(3))),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::UnimprovedTimeLimit);
        assert_eq!(r.work.evaluations, 1);
        assert_eq!(r.work.completed_steps, 0);
        assert_eq!(r.phases.len(), 1);
    }
}
#[test]
fn repair_token_conflicts_and_bad_acceptance_are_rejected_before_any_work() {
    for mode in 0..6 {
        let a = CancellationToken::new();
        let b = CancellationToken::new();
        let factories = Cell::new(0);
        let mut c = phases(vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
            cancellation: Some(&a),
            ..repair()
        })]);
        c.limits.cancellation = Some(&b);
        assert_eq!(
            run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &factories,
                None,
                || {},
                SimpleScore::new
            ),
            Err(Failure::Token(0))
        );
        let c = phases(vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                acceptance: Acceptance::LateAcceptance {
                    history_size: 0,
                    hill_climbing: false,
                },
                ..repair()
            }),
        ]);
        assert_eq!(
            run_scored(
                (mode, true, MixedConstructionOrder::BasicThenLists),
                c,
                &factories,
                None,
                || {},
                SimpleScore::new
            ),
            Err(Failure::Acceptance(1))
        );
        assert_eq!(factories.get(), 0);
    }
}
#[test]
fn managed_jobs_preserve_repair_configuration_and_isolate_candidate_overrides() {
    let sequence = phases(vec![SolverPhase::RuinRecreate(repair())]);
    let factory = SolverFactory::new(sequence.clone(), |p: FiniteProblem<i64>, c, progress| {
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| {
            SimpleScore::new(v[0].copied().unwrap_or(0))
        })
        .unwrap();
        let r = solve(&p, &n, c).map_err(|e| e.to_string())?;
        progress.record_work(r.work);
        Ok::<_, String>(r)
    })
    .unwrap();
    let mut manager = factory.build_manager(2).unwrap();
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let limited = manager
        .job_builder(1)
        .with_problem(p.clone())
        .with_limits(SolverLimits {
            max_move_evaluations: Some(1),
            ..Default::default()
        })
        .run()
        .unwrap();
    let full = manager.job_builder(2).with_problem(p).run().unwrap();
    let a = limited
        .wait_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let a = a.as_ref().as_ref().unwrap();
    let b = full
        .wait_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let b = b.as_ref().as_ref().unwrap();
    assert_eq!(a.termination, SolverTermination::MoveEvaluationLimit);
    assert_eq!(a.score, Some(SimpleScore::ZERO));
    assert_eq!(a.work.move_evaluations, 1);
    assert_eq!(b.score, Some(SimpleScore::new(2)));
    assert_eq!(b.work.move_evaluations, 3);
    assert_eq!(factory.configuration().phases, sequence.phases);
    assert_eq!(full.configuration().phases, sequence.phases);
    assert_eq!(limited.statistics().work, Some(a.work));
    assert_eq!(full.statistics().work, Some(b.work));
    manager.shutdown().unwrap();
    let token = Box::leak(Box::new(CancellationToken::new()));
    let bad = phases(vec![SolverPhase::RuinRecreate(RuinRecreateConfig {
        cancellation: Some(token),
        ..repair()
    })]);
    assert!(matches!(
        SolverFactory::new(bad, |_: (), _, _| Ok::<_, String>(())),
        Err(SolverFactoryError::PhaseCancellationToken { phase: 0 })
    ));
}

#[test]
fn mixed_repair_phase_preserves_component_order_for_coupled_scores() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![0, 1]).with_initial(0)]).unwrap(),
        ListProblem::new(
            vec![7],
            vec![ListEntity::new(vec![7]), ListEntity::new(vec![])],
        )
        .unwrap(),
    );
    let score = |b: Option<i32>, o: Option<usize>| {
        SimpleScore::new(match (b, o) {
            (Some(1), Some(1)) => 10,
            (Some(0), _) => 5,
            (None, Some(1)) => 4,
            _ => 0,
        })
    };
    let mut n = MixedConstraintNetwork::new(1, 2, 1);
    n.add_constraint(
        "coupled",
        vec![MixedScope::Basic(0), MixedScope::ListValue(0)],
        |f| {
            let (MixedFact::Basic(b), MixedFact::ListValue { element, .. }) = (&f[0], &f[1]) else {
                unreachable!()
            };
            score(b.copied(), element.as_ref().map(|e| e.position.entity))
        },
    )
    .unwrap();
    for order in [
        MixedConstructionOrder::BasicThenLists,
        MixedConstructionOrder::ListsThenBasic,
    ] {
        let c = MixedSolverSequence {
            limits: SolverLimits::default(),
            phases: vec![MixedSolverPhase::RuinRecreate(MixedRuinRecreateConfig {
                search: repair(),
                order,
            })],
        };
        let a = solve_mixed(&p, &n, c.clone()).unwrap();
        let b = solve_mixed_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::new();
                let s = MixedStreamSources::<i32, i32>::new(&mut b);
                let joined = b.join_equal(s.basic.port(), s.lists.values.port(), |_| 0, |_| 0)?;
                b.score("coupled", &joined, |((_, v), l)| {
                    score(*v, l.position.map(|p| p.entity))
                })?;
                Ok((b.build(), s))
            },
            c,
        )
        .unwrap();
        assert_eq!(a.assignment, b.assignment);
        assert_eq!(a.score, b.score);
        assert_eq!(
            a.score,
            Some(SimpleScore::new(
                if order == MixedConstructionOrder::BasicThenLists {
                    5
                } else {
                    10
                }
            ))
        );
    }
}
#[test]
fn owned_decimal_phase_targets_preserve_sub_float_improvements() {
    let scores: Vec<SimpleDecimalScore> = [
        "9007199254740992.000000000000000001",
        "9007199254740992.000000000000000002",
    ]
    .iter()
    .map(|s| s.parse().unwrap())
    .collect();
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<usize, SimpleDecimalScore>::new(1);
    n.add_constraint("exact", vec![0], |v| {
        v[0].map_or(SimpleDecimalScore::ZERO, |v| scores[*v].clone())
    })
    .unwrap();
    let c = SolverSequence {
        limits: SolverLimits {
            best_score_limit: Some(scores[1].clone()),
            ..Default::default()
        },
        phases: vec![
            SolverPhase::RuinRecreate(RuinRecreateConfig {
                max_attempts: 10,
                ..Default::default()
            });
            2
        ],
    };
    let a = solve(&p, &n, c.clone()).unwrap();
    let b = solve_stream(
        &p,
        || {
            let mut b = StreamGraphBuilder::new();
            let s = b.source::<usize, (usize, Option<usize>)>();
            b.score("exact", s.port(), |(_, v)| {
                v.map_or(SimpleDecimalScore::ZERO, |v| scores[v].clone())
            })?;
            Ok((b.build(), s))
        },
        c,
    )
    .unwrap();
    assert_eq!(a.score, Some(scores[1].clone()));
    assert_eq!(a, b);
    assert_eq!(a.phases.len(), 1);
    assert_eq!(a.termination, SolverTermination::BestScoreLimit);
}
#[test]
fn managed_consumers_receive_real_repair_phase_origins_and_initialization() {
    use std::sync::{Arc, Mutex};
    let c = SolverPlan {
        construction: ConstructionConfig {
            max_steps: Some(0),
            ..Default::default()
        },
        phases: vec![SolverPhase::RuinRecreate(repair())],
        ..Default::default()
    };
    let factory = SolverFactory::new(c, |p: FiniteProblem<i64>, c, progress| {
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| {
            SimpleScore::new(v[0].copied().unwrap_or(0))
        })
        .unwrap();
        let r = solve_with_events(&p, &n, c, |event| match event {
            SolverEvent::Best(u) => {
                progress.publish_from(
                    u.score,
                    SolverEventProducer {
                        phase: u.phase,
                        kind: u.kind,
                    },
                );
            }
            SolverEvent::FirstInitialized {
                score,
                initialization,
                ..
            } => {
                progress.publish_first_initialized(score.unwrap(), initialization);
            }
        })
        .map_err(|e| e.to_string())?;
        progress.record_work(r.work);
        Ok::<_, String>(r.score.unwrap())
    })
    .unwrap();
    let origins = Arc::new(Mutex::new(vec![]));
    let seen = origins.clone();
    let init = Arc::new(Mutex::new(vec![]));
    let initialized = init.clone();
    let mut manager = factory.build_manager(1).unwrap();
    let job = manager
        .job_builder(1)
        .with_problem(
            FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap(),
        )
        .with_callbacks(
            SolverJobCallbacks::<SimpleScore, String>::default()
                .on_best_event(move |_, event| {
                    seen.lock().unwrap().push((*event.solution, event.producer));
                })
                .on_first_initialized(move |_, score, origin| {
                    initialized.lock().unwrap().push((*score, origin));
                }),
        )
        .run()
        .unwrap();
    assert_eq!(
        *job.wait_timeout(std::time::Duration::from_secs(10))
            .unwrap()
            .as_ref()
            .as_ref()
            .unwrap(),
        SimpleScore::new(2)
    );
    manager.shutdown().unwrap();
    assert_eq!(
        origins.lock().unwrap().last(),
        Some(&(
            SimpleScore::new(2),
            Some(SolverEventProducer {
                phase: 1,
                kind: SolverPhaseKind::RuinRecreate
            })
        ))
    );
    assert_eq!(
        init.lock().unwrap().as_slice(),
        &[(
            SimpleScore::ZERO,
            SolverInitialization {
                phase: 0,
                kind: SolverPhaseKind::Construction,
                terminated_early: false
            }
        )]
    );
    assert!(job.callback_failures().is_empty());
}
