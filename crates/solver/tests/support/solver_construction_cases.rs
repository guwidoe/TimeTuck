//! Original repeated-construction and handoff regressions.
use super::*;
const ORDER: MixedConstructionOrder = MixedConstructionOrder::BasicThenLists;
fn construction(max_evaluations: u64) -> ConstructionConfig<'static> {
    ConstructionConfig {
        max_evaluations,
        assert_scores: true,
        ..Default::default()
    }
}
fn plan() -> SolverPlan<'static> {
    SolverPlan {
        construction: construction(1),
        phases: vec![
            SolverPhase::Construction(construction(2)),
            SolverPhase::Construction(construction(100)),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 0,
                ..Default::default()
            }),
        ],
        ..Default::default()
    }
}
#[test]
fn repeated_construction_resumes_without_replacing_prior_choices() {
    for mode in 0..6 {
        for order in [ORDER, MixedConstructionOrder::ListsThenBasic] {
            let mut events = Vec::new();
            let r = run_scored_observed(
                (mode, false, order),
                plan(),
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
                Some(&mut |u| events.push(u)),
            )
            .unwrap();
            assert_eq!(r.reason, SolverTermination::PlanCompleted);
            assert_eq!(r.phases.len(), 4);
            assert!(r.complete);
            let expected = if mode < 4 {
                vec![Some(0)]
            } else if order == ORDER {
                vec![Some(0), Some(2)]
            } else {
                vec![Some(2), Some(0)]
            };
            assert_eq!(r.values, expected);
            assert_eq!(r.phases[0].work.evaluations, 1);
            assert_eq!(r.phases[1].work.evaluations, 2);
            assert_eq!(r.phases[2].work.evaluations, if mode < 4 { 1 } else { 4 });
            assert_eq!(r.phases[3].work.evaluations, 1);
            assert_eq!(events.len(), if mode < 4 { 2 } else { 3 });
            assert_eq!(events[0].phase, 0);
            assert_eq!(events[1].phase, 1);
            assert!(
                events
                    .iter()
                    .all(|e| e.kind == SolverPhaseKind::Construction)
            );
            assert!(events.windows(2).all(|w| w[0].score < w[1].score));
            assert_eq!(events.last().unwrap().values, r.values);
        }
    }
}
#[test]
fn repeated_construction_observers_preserve_all_shared_budget_prefixes() {
    for mode in 0..6 {
        for max in 0..=20 {
            for moves in [false, true] {
                let mut c = plan();
                c.phases
                    .push(SolverPhase::Exact(ExactPhaseConfig::default()));
                if moves {
                    c.limits.max_move_evaluations = Some(max);
                } else {
                    c.limits.max_evaluations = max;
                }
                let plain = run_scored(
                    (mode, false, ORDER),
                    c.clone(),
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                )
                .unwrap();
                let mut events = Vec::new();
                let observed = run_scored_observed(
                    (mode, false, ORDER),
                    c,
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                    Some(&mut |u| events.push(u)),
                )
                .unwrap();
                assert_eq!(plain, observed, "{mode} {max} {moves}");
                assert!(if moves {
                    plain.work.move_evaluations <= max
                } else {
                    plain.work.evaluations <= max
                });
                assert_eq!(events.last().map(|u| u.score), plain.score);
                if let Some(last) = events.last() {
                    assert_eq!(last.values, plain.values);
                }
            }
        }
    }
}
#[test]
fn repeated_construction_keeps_improved_state_and_shared_stagnation_steps() {
    for mode in 0..6 {
        let mut c = plan();
        c.construction = construction(100);
        let search = LocalSearchConfig {
            max_steps: 1,
            selection: MoveSelection::FirstAccepted,
            acceptance: Acceptance::HillClimbing,
            ..Default::default()
        };
        c.phases = vec![
            SolverPhase::LocalSearch(search.clone()),
            SolverPhase::Construction(construction(100)),
            SolverPhase::LocalSearch(search.clone()),
            SolverPhase::LocalSearch(search),
        ];
        c.limits.max_unimproved_steps = Some(2);
        let r = run_scored(
            (mode, true, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::UnimprovedStepLimit);
        assert_eq!(r.work.completed_steps, 2);
        assert_eq!(r.phases.len(), 4);
        assert_eq!(r.phases[2].work.evaluations, 1);
        assert_eq!(r.phases[2].work.move_evaluations, 0);
        let mut c = plan();
        c.construction = construction(2);
        c.phases = vec![
            SolverPhase::Construction(construction(100)),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 1,
                selection: MoveSelection::FirstAccepted,
                ..Default::default()
            }),
            SolverPhase::Construction(construction(100)),
        ];
        let r = run_scored(
            (mode, false, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert!(r.complete);
        assert_eq!(r.phases[2].score, r.phases[3].score);
        assert_eq!(r.phases[3].work.evaluations, 1);
        assert_eq!(r.phases[3].work.move_evaluations, 0);
    }
}
#[test]
fn repeated_construction_validates_future_tokens_and_keeps_failure_prefixes() {
    for mode in 0..6 {
        let global = CancellationToken::new();
        let different = CancellationToken::new();
        let mut c = plan();
        c.limits.cancellation = Some(&global);
        c.phases[1] = SolverPhase::Construction(ConstructionConfig {
            cancellation: Some(&different),
            ..construction(100)
        });
        let mut events = Vec::new();
        let factories = Cell::new(0);
        assert_eq!(
            run_scored_observed(
                (mode, false, ORDER),
                c,
                &factories,
                None,
                || panic!("invalid plan scored"),
                SimpleScore::new,
                Some(&mut |u| events.push(u))
            ),
            Err(Failure::Token(2))
        );
        assert!(events.is_empty());
        assert_eq!(factories.get(), 0);
        let mut c = plan();
        c.limits.cancellation = Some(&global);
        let r = run_scored_observed(
            (mode, false, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            Some(&mut |u| {
                if u.phase == 1 {
                    global.cancel();
                }
            }),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(r.phases.len(), 2);
        assert_eq!(r.phases[1].work.evaluations, 2);
        if mode % 2 == 1 {
            events.clear();
            assert_eq!(
                run_scored_observed(
                    (mode, false, ORDER),
                    plan(),
                    &Cell::new(0),
                    Some(3),
                    || {},
                    SimpleScore::new,
                    Some(&mut |u| events.push(u))
                ),
                Err(Failure::Phase(2))
            );
            assert_eq!(events.len(), 2);
            assert_eq!(events.last().unwrap().phase, 1);
        }
    }
}
#[test]
fn each_mixed_construction_phase_selects_its_own_component_order() {
    let p = MixedProblem::new(
        FiniteProblem::new(vec![Variable::new(vec![1, 2])]).unwrap(),
        ListProblem::new(vec![7], vec![ListEntity::new(vec![])]).unwrap(),
    );
    for stream in [false, true] {
        let c = MixedSolverPlan {
            construction: MixedConstructionConfig {
                construction: construction(1),
                order: ORDER,
            },
            phases: vec![
                MixedSolverPhase::Construction(MixedConstructionConfig {
                    construction: construction(2),
                    order: MixedConstructionOrder::ListsThenBasic,
                }),
                MixedSolverPhase::Construction(MixedConstructionConfig {
                    construction: construction(100),
                    order: ORDER,
                }),
            ],
            ..Default::default()
        };
        let mut events = Vec::new();
        let mut observe = |u: SolverUpdate<'_, MixedAssignment<i32, i32>, SimpleScore>| {
            events.push((u.phase, u.assignment.clone()))
        };
        let r = if stream {
            solve_mixed_stream_with_observer(
                &p,
                || {
                    let mut b = StreamGraphBuilder::new();
                    let s = MixedStreamSources::new(&mut b);
                    Ok((b.build(), s))
                },
                c,
                &mut observe,
            )
            .unwrap()
        } else {
            solve_mixed_with_observer(&p, &MixedConstraintNetwork::new(1, 1, 1), c, &mut observe)
                .unwrap()
        };
        assert!(r.complete);
        assert_eq!(events.len(), 3);
        assert_eq!(events[1].0, 1);
        assert_eq!(events[1].1.basic().values(), &[None]);
        assert_eq!(
            events[1].1.lists().location(&7),
            Some(ListPosition {
                entity: 0,
                index: 0
            })
        );
        assert_eq!(events[2].0, 2);
    }
}

#[test]
fn construction_step_limits_resume_across_phases_and_emit_the_actual_boundary() {
    for mode in 0..6 {
        for order in [ORDER, MixedConstructionOrder::ListsThenBasic] {
            let limited = ConstructionConfig {
                max_steps: Some(1),
                ..construction(100)
            };
            let c = SolverPlan {
                construction: ConstructionConfig {
                    max_steps: Some(0),
                    ..limited
                },
                phases: vec![
                    SolverPhase::Construction(limited),
                    SolverPhase::Construction(limited),
                    SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 0,
                        ..Default::default()
                    }),
                ],
                ..Default::default()
            };
            let mut init = Vec::new();
            let mut best = Vec::new();
            let r = run_scored_events(
                (mode, false, order),
                c.clone(),
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
                Some(&mut |u| best.push(u)),
                Some(&mut |u| init.push(u)),
            )
            .unwrap();
            assert_eq!(
                r,
                run_scored(
                    (mode, false, order),
                    c,
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new
                )
                .unwrap()
            );
            assert!(r.complete);
            assert_eq!(r.reason, SolverTermination::PlanCompleted);
            assert_eq!(r.phases.len(), 4);
            assert_eq!(
                r.phases[0].termination,
                PhaseTermination::Construction(ConstructionTermination::StepLimit)
            );
            assert_eq!(r.phases[0].work.evaluations, 1);
            assert_eq!(
                r.phases[1].termination,
                PhaseTermination::Construction(if mode < 4 {
                    ConstructionTermination::Completed
                } else {
                    ConstructionTermination::StepLimit
                })
            );
            assert_eq!(r.phases[1].work.evaluations, 4);
            assert_eq!(r.phases[2].work.evaluations, if mode < 4 { 1 } else { 4 });
            assert_eq!(
                r.phases[2].termination,
                PhaseTermination::Construction(ConstructionTermination::Completed)
            );
            assert!(r.values.iter().all(|v| *v == Some(2)));
            // Construction counts remain separate from local-search stagnation.
            assert_eq!(r.work.completed_steps, 0);
            assert_eq!(r.work.accepted_moves, 0);
            assert_eq!(init.len(), 1);
            assert_eq!(
                init[0].initialization,
                SolverInitialization {
                    phase: 2,
                    kind: SolverPhaseKind::Construction,
                    terminated_early: false
                }
            );
            assert_eq!(init[0].values, r.values);
            assert!(init[0].complete);
            assert_eq!(best.len(), if mode < 4 { 2 } else { 3 });
        }
    }
}

#[test]
fn construction_step_stops_report_partial_initialization_before_local_or_exact_handoffs() {
    for mode in 0..6 {
        for steps in 0..=1 {
            for exact in [false, true] {
                let c = SolverPlan {
                    construction: ConstructionConfig {
                        max_steps: Some(steps),
                        ..construction(100)
                    },
                    phases: if exact {
                        vec![
                            SolverPhase::Exact(ExactPhaseConfig::default()),
                            SolverPhase::LocalSearch(LocalSearchConfig::default()),
                        ]
                    } else {
                        vec![SolverPhase::LocalSearch(LocalSearchConfig {
                            max_steps: 0,
                            ..Default::default()
                        })]
                    },
                    ..Default::default()
                };
                let mut init = Vec::new();
                let r = run_scored_events(
                    (mode, false, ORDER),
                    c,
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                    None,
                    Some(&mut |u| init.push(u)),
                )
                .unwrap();
                assert_eq!(init.len(), 1);
                if exact {
                    assert_eq!(r.reason, SolverTermination::Optimal);
                    assert_eq!(
                        init[0].initialization,
                        SolverInitialization {
                            phase: 1,
                            kind: SolverPhaseKind::Exact,
                            terminated_early: false
                        }
                    );
                    assert!(r.complete);
                    assert!(init[0].complete);
                } else {
                    let complete = mode < 4 && steps == 1;
                    assert_eq!(
                        r.reason,
                        if complete {
                            SolverTermination::PlanCompleted
                        } else {
                            SolverTermination::IncompleteConstruction
                        }
                    );
                    assert_eq!(r.complete, complete);
                    assert_eq!(init[0].complete, complete);
                    assert_eq!(r.phases.len(), if complete { 2 } else { 1 });
                    assert_eq!(
                        init[0].initialization,
                        SolverInitialization {
                            phase: 0,
                            kind: SolverPhaseKind::Construction,
                            terminated_early: !complete
                        }
                    );
                    assert_eq!(init[0].values, r.values);
                    assert_eq!(init[0].score, r.score);
                }
            }
        }
    }
}
