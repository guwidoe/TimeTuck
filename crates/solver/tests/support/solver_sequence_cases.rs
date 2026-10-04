//! Original explicit-phase sequence and initial-state regressions.
use super::*;
const ORDER: MixedConstructionOrder = MixedConstructionOrder::BasicThenLists;
fn local() -> SolverPhase<'static> {
    SolverPhase::LocalSearch(LocalSearchConfig {
        max_steps: 1,
        ..Default::default()
    })
}
fn sequence<'a>(phases: Vec<SolverPhase<'a>>) -> SolverSequence<'a> {
    SolverSequence {
        phases,
        ..Default::default()
    }
}
#[test]
fn single_local_search_starts_at_zero_without_construction_or_initialization_event() {
    for mode in 0..6 {
        let mut init = Vec::new();
        let mut best = Vec::new();
        let factories = Cell::new(0);
        let c = sequence(vec![local()]);
        let r = run_scored_events(
            (mode, true, ORDER),
            c.clone(),
            &factories,
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
                (mode, true, ORDER),
                c,
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new
            )
            .unwrap()
        );
        assert!(init.is_empty());
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.phases[0].index, 0);
        assert_eq!(
            r.phases[0].termination,
            PhaseTermination::LocalSearch(LocalSearchTermination::StepLimit)
        );
        assert_eq!(r.work.evaluations, 2);
        assert_eq!(r.work.move_evaluations, 1);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert!(r.complete);
        assert_eq!(best.len(), 2);
        assert!(
            best.iter()
                .all(|u| u.phase == 0 && u.kind == SolverPhaseKind::LocalSearch)
        );
        assert_eq!(factories.get(), mode % 2);
    }
}
#[test]
fn exact_only_proves_complete_space_without_charging_an_initial_rescore() {
    for mode in 0..6 {
        let states = if mode < 4 { 3 } else { 9 };
        for max in 0..=states + 1 {
            let mut c = sequence(vec![SolverPhase::Exact(ExactPhaseConfig::default())]);
            c.limits.max_evaluations = max;
            let mut init = Vec::new();
            let mut best = Vec::new();
            let r = run_scored_events(
                (mode, false, ORDER),
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
                    (mode, false, ORDER),
                    c,
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new
                )
                .unwrap()
            );
            assert_eq!(r.work.evaluations, max.min(states));
            assert_eq!(r.work.move_evaluations, max.min(states));
            assert_eq!(r.phases.len(), 1);
            assert_eq!(r.phases[0].index, 0);
            assert!(init.is_empty());
            assert_eq!(
                r.reason,
                if max >= states {
                    SolverTermination::Optimal
                } else {
                    SolverTermination::EvaluationLimit
                }
            );
            assert_eq!(r.complete, max > 0);
            assert_eq!(best.last().map(|u| u.score), r.score);
            if max >= states {
                assert!(r.values.iter().all(|v| *v == Some(2)));
            }
        }
    }
}
#[test]
fn zero_work_and_pre_cancelled_first_phases_retain_the_model_initial_assignment() {
    for mode in 0..6 {
        for exact in [false, true] {
            for cancelled in [false, true] {
                let token = CancellationToken::new();
                token.cancel();
                let mut c = SolverSequence {
                    phases: vec![if exact {
                        SolverPhase::Exact(ExactPhaseConfig::default())
                    } else {
                        local()
                    }],
                    ..Default::default()
                };
                if cancelled {
                    c.limits.cancellation = Some(&token);
                } else {
                    c.limits.max_evaluations = 0;
                }
                let r = run_scored_events(
                    (mode, true, ORDER),
                    c,
                    &Cell::new(0),
                    None,
                    || panic!("zero work scored"),
                    SimpleScore::new,
                    Some(&mut |_| panic!("zero work emitted best")),
                    Some(&mut |_| panic!("unexpected initialization")),
                )
                .unwrap();
                assert_eq!(r.score, None);
                assert!(r.complete);
                assert!(r.values.iter().all(|v| *v == Some(0)));
                assert_eq!(r.work, SolverWork::default());
                assert_eq!(r.phases.len(), 1);
                assert_eq!(
                    r.reason,
                    if cancelled {
                        SolverTermination::Cancelled
                    } else {
                        SolverTermination::EvaluationLimit
                    }
                );
            }
        }
    }
}
#[test]
fn first_local_rejects_incomplete_models_and_empty_sequences_fail_before_factories() {
    for mode in 0..6 {
        for max in [0, 100] {
            let mut c = sequence(vec![local()]);
            c.limits.max_evaluations = max;
            assert_eq!(
                run_scored_events(
                    (mode, false, ORDER),
                    c,
                    &Cell::new(0),
                    None,
                    || panic!("invalid state scored"),
                    SimpleScore::new,
                    None,
                    Some(&mut |_| panic!("invalid state emitted"))
                ),
                Err(Failure::Phase(0))
            );
            let factories = Cell::new(0);
            let mut c = sequence(vec![]);
            c.limits.max_evaluations = max;
            assert_eq!(
                run_scored_events(
                    (mode, true, ORDER),
                    c,
                    &factories,
                    None,
                    || panic!("empty plan scored"),
                    SimpleScore::new,
                    None,
                    Some(&mut |_| panic!("empty plan emitted"))
                ),
                Err(Failure::Empty)
            );
            assert_eq!(factories.get(), 0);
        }
    }

    #[derive(Eq, PartialEq)]
    struct MustNotClone;
    impl Clone for MustNotClone {
        fn clone(&self) -> Self {
            panic!("invalid sequence cloned model facts");
        }
    }
    let p = FiniteProblem::new(vec![
        Variable::new(vec![MustNotClone]).with_initial(MustNotClone),
    ])
    .unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    assert!(matches!(
        solve(&p, &n, sequence(vec![])),
        Err(SolverError::EmptyPlan)
    ));
    let invalid = SolverPhase::LocalSearch(LocalSearchConfig {
        acceptance: Acceptance::VariableTabu {
            tenure: 0,
            aspiration: true,
        },
        ..Default::default()
    });
    assert!(matches!(
        solve(&p, &n, sequence(vec![invalid])),
        Err(SolverError::InvalidAcceptance { phase: 0, .. })
    ));
}

#[test]
fn explicit_phase_indices_preserve_partial_handoffs_and_later_initialization_boundary() {
    for mode in 0..6 {
        let c = sequence(vec![
            SolverPhase::Exact(ExactPhaseConfig {
                max_evaluations: 0,
                ..Default::default()
            }),
            SolverPhase::Construction(ConstructionConfig::default()),
            local(),
        ]);
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
        assert!(r.complete);
        assert_eq!(r.phases.len(), 3);
        assert_eq!(r.phases[0].work, SolverWork::default());
        assert_eq!(init.len(), 1);
        assert_eq!(init[0].initialization.phase, 1);
        assert_eq!(init[0].initialization.kind, SolverPhaseKind::Construction);
        assert!(!init[0].initialization.terminated_early);
        let c = sequence(vec![
            local(),
            SolverPhase::Construction(ConstructionConfig::default()),
            local(),
        ]);
        init.clear();
        let r = run_scored_events(
            (mode, true, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            None,
            Some(&mut |u| init.push(u)),
        )
        .unwrap();
        assert_eq!(r.phases.len(), 3);
        assert!(init.is_empty());
        assert_eq!(r.phases[1].work.move_evaluations, 0);
    }
}
#[test]
fn sequence_validation_and_shared_controls_use_explicit_indices() {
    for mode in 0..6 {
        let global = CancellationToken::new();
        let other = CancellationToken::new();
        let mut c = sequence(vec![
            local(),
            SolverPhase::Construction(ConstructionConfig {
                cancellation: Some(&other),
                ..Default::default()
            }),
        ]);
        c.limits.cancellation = Some(&global);
        assert_eq!(
            run_scored(
                (mode, true, ORDER),
                c,
                &Cell::new(0),
                None,
                || panic!("invalid tokens scored"),
                SimpleScore::new
            ),
            Err(Failure::Token(1))
        );
        let c = sequence(vec![SolverPhase::LocalSearch(LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 0,
                aspiration: true,
            },
            ..Default::default()
        })]);
        assert_eq!(
            run_scored(
                (mode, true, ORDER),
                c,
                &Cell::new(0),
                None,
                || panic!("invalid acceptance scored"),
                SimpleScore::new
            ),
            Err(Failure::Acceptance(0))
        );
        let mut c = sequence(vec![local(), local()]);
        c.limits.max_unimproved_steps = Some(0);
        let r = run_scored(
            (mode, true, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::UnimprovedStepLimit);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.work.evaluations, 1);
        let mut c = sequence(vec![local(), local()]);
        c.limits.best_score_limit = Some(SimpleScore::ZERO);
        let r = run_scored(
            (mode, true, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::BestScoreLimit);
        assert_eq!(r.phases.len(), 1);
        assert_eq!(r.work.evaluations, 1);
        if mode % 2 == 1 {
            assert_eq!(
                run_scored(
                    (mode, true, ORDER),
                    sequence(vec![local(), local()]),
                    &Cell::new(0),
                    Some(1),
                    || {},
                    SimpleScore::new
                ),
                Err(Failure::Phase(0))
            );
        }
    }
}
#[test]
fn direct_exact_validates_scoped_dimensions_and_stream_sources_at_zero_budget() {
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let list = ListProblem::new(vec![7], vec![ListEntity::new(vec![])]).unwrap();
    let mixed = MixedProblem::new(p.clone(), list.clone());
    let mut c = sequence(vec![SolverPhase::Exact(ExactPhaseConfig::default())]);
    c.limits.max_evaluations = 0;
    assert!(matches!(
        solve(&p, &ConstraintNetwork::<_, SimpleScore>::new(0), c.clone()),
        Err(SolverError::Phase { phase: 0, .. })
    ));
    assert!(matches!(
        solve_list(
            &list,
            &ListConstraintNetwork::<_, SimpleScore>::new(0, 0),
            c.clone()
        ),
        Err(SolverError::Phase { phase: 0, .. })
    ));
    let m = MixedSolverSequence {
        limits: c.limits,
        phases: vec![MixedSolverPhase::Exact(ExactPhaseConfig::default())],
    };
    assert!(matches!(
        solve_mixed(
            &mixed,
            &MixedConstraintNetwork::<_, _, SimpleScore>::new(0, 0, 0),
            m.clone()
        ),
        Err(SolverError::Phase { phase: 0, .. })
    ));
    assert!(matches!(
        solve_stream(
            &p,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let source = b.source::<usize, (usize, Option<usize>)>();
                Ok((StreamGraphBuilder::<SimpleScore>::new().build(), source))
            },
            c.clone()
        ),
        Err(SolverError::Phase { phase: 0, .. })
    ));
    assert!(matches!(
        solve_list_stream(
            &list,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let sources = ListStreamSources::new(&mut b);
                Ok((StreamGraphBuilder::<SimpleScore>::new().build(), sources))
            },
            c
        ),
        Err(SolverError::Phase { phase: 0, .. })
    ));
    assert!(matches!(
        solve_mixed_stream(
            &mixed,
            || {
                let mut b = StreamGraphBuilder::<SimpleScore>::new();
                let sources = MixedStreamSources::new(&mut b);
                Ok((StreamGraphBuilder::<SimpleScore>::new().build(), sources))
            },
            m
        ),
        Err(SolverError::Phase { phase: 0, .. })
    ));
}

#[test]
fn construction_free_shared_budget_prefixes_match_observed_execution() {
    for mode in 0..6 {
        for max in 0..=18 {
            for moves in [false, true] {
                let mut c = sequence(vec![
                    local(),
                    SolverPhase::Exact(ExactPhaseConfig {
                        max_evaluations: 1,
                        ..Default::default()
                    }),
                    local(),
                    SolverPhase::Exact(ExactPhaseConfig::default()),
                ]);
                if moves {
                    c.limits.max_move_evaluations = Some(max);
                } else {
                    c.limits.max_evaluations = max;
                }
                let plain = run_scored(
                    (mode, true, ORDER),
                    c.clone(),
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                )
                .unwrap();
                let mut best = Vec::new();
                let mut init = Vec::new();
                let observed = run_scored_events(
                    (mode, true, ORDER),
                    c,
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                    Some(&mut |u| best.push(u)),
                    Some(&mut |u| init.push(u)),
                )
                .unwrap();
                assert_eq!(plain, observed);
                assert!(init.is_empty());
                assert_eq!(best.last().map(|u| u.score), plain.score);
                assert!(if moves {
                    plain.work.move_evaluations <= max
                } else {
                    plain.work.evaluations <= max
                });
                assert!(
                    plain
                        .phases
                        .iter()
                        .enumerate()
                        .all(|(index, p)| p.index == index)
                );
            }
        }
    }
}

#[test]
fn invalid_additional_policy_in_later_phase_fails_before_factories_scoring_or_events() {
    const INVALID: [Acceptance; 1] = [Acceptance::VariableTabu {
        tenure: 0,
        aspiration: true,
    }];
    for mode in 0..6 {
        let factories = Cell::new(0);
        let c = sequence(vec![
            local(),
            SolverPhase::LocalSearch(LocalSearchConfig {
                acceptance: Acceptance::HillClimbing,
                additional_acceptance: &INVALID,
                ..Default::default()
            }),
        ]);
        assert_eq!(
            run_scored_events(
                (mode, true, ORDER),
                c,
                &factories,
                None,
                || panic!("invalid extra policy scored"),
                SimpleScore::new,
                None,
                Some(&mut |_| panic!("invalid extra policy emitted"))
            ),
            Err(Failure::Acceptance(1))
        );
        assert_eq!(factories.get(), 0);
    }
}
