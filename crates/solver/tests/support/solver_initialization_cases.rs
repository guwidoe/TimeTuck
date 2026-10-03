//! Original initialization-boundary checks across all models and backends.
use super::*;
const ORDER: MixedConstructionOrder = MixedConstructionOrder::BasicThenLists;
fn execute(
    mode: usize,
    c: SolverPlan<'_>,
    init: &mut Vec<Initialized>,
    best: &mut Vec<Update>,
) -> Outcome {
    run_scored_events(
        (mode, false, ORDER),
        c,
        &Cell::new(0),
        None,
        || {},
        SimpleScore::new,
        Some(&mut |u| best.push(u)),
        Some(&mut |u| init.push(u)),
    )
    .unwrap()
}
#[test]
fn initialization_follows_last_construction_before_first_local_search_once() {
    for mode in 0..6 {
        for repeats in 0..=3 {
            let mut c = SolverPlan::default();
            for _ in 0..repeats {
                c.phases
                    .insert(0, SolverPhase::Construction(ConstructionConfig::default()));
            }
            c.phases.extend([
                SolverPhase::Construction(ConstructionConfig::default()),
                SolverPhase::LocalSearch(LocalSearchConfig::default()),
            ]);
            let plain = run_scored(
                (mode, false, ORDER),
                c.clone(),
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
            )
            .unwrap();
            let mut init = Vec::new();
            let mut best = Vec::new();
            let r = execute(mode, c, &mut init, &mut best);
            assert_eq!(r, plain);
            assert_eq!(init.len(), 1);
            assert_eq!(
                init[0].initialization,
                SolverInitialization {
                    phase: repeats,
                    kind: SolverPhaseKind::Construction,
                    terminated_early: false
                }
            );
            assert!(init[0].complete);
            assert_eq!(init[0].score, plain.phases[repeats].score);
            assert!(best.iter().all(|u| u.phase == 0));
        }
    }
}
#[test]
fn initialization_distinguishes_early_termination_from_completeness_and_unscored_state() {
    for mode in 0..6 {
        for max in 0..=4 {
            let mut c = SolverPlan::default();
            c.construction.max_evaluations = max;
            let mut init = Vec::new();
            let r = execute(mode, c, &mut init, &mut Vec::new());
            assert_eq!(init.len(), 1);
            assert_eq!(init[0].score, r.phases[0].score);
            assert_eq!(
                init[0].initialization.terminated_early,
                r.phases[0].termination
                    != PhaseTermination::Construction(ConstructionTermination::Completed)
            );
            assert_eq!(init[0].complete, init[0].values.iter().all(Option::is_some));
            if max == 0 {
                assert_eq!(init[0].score, None);
                assert!(!init[0].complete);
            }
            if max == 2 && mode < 4 {
                assert!(init[0].complete);
                assert!(init[0].initialization.terminated_early);
            }
        }
    }
}
#[test]
fn boundary_events_preserve_shared_budget_results_and_omit_unreached_producers() {
    for mode in 0..6 {
        for max in 0..=16 {
            for moves in [false, true] {
                let mut c = SolverPlan::default();
                c.construction.max_evaluations = 1;
                c.phases
                    .insert(0, SolverPhase::Construction(ConstructionConfig::default()));
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
                let mut init = Vec::new();
                let mut best = Vec::new();
                let r = execute(mode, c, &mut init, &mut best);
                assert_eq!(r, plain);
                assert_eq!(init.len(), usize::from(r.phases.len() > 1));
                if let Some(u) = init.first() {
                    assert_eq!(u.initialization.phase, 1);
                    assert_eq!(u.score, r.phases[1].score);
                }
            }
        }
    }
    for mode in 0..6 {
        let c = SolverPlan {
            phases: vec![SolverPhase::Construction(ConstructionConfig::default())],
            ..Default::default()
        };
        let mut init = Vec::new();
        execute(mode, c, &mut init, &mut Vec::new());
        assert!(init.is_empty());
    }
}
#[test]
fn exact_boundary_events_keep_carried_best_and_zero_work_partial_assignments() {
    for mode in 0..6 {
        for initial_budget in [0, 100] {
            for exact_budget in [0, 1, 100] {
                let c = SolverPlan {
                    construction: ConstructionConfig {
                        max_evaluations: initial_budget,
                        ..Default::default()
                    },
                    phases: vec![
                        SolverPhase::Exact(ExactPhaseConfig {
                            max_evaluations: exact_budget,
                            ..Default::default()
                        }),
                        SolverPhase::LocalSearch(LocalSearchConfig::default()),
                    ],
                    ..Default::default()
                };
                let plain = run_scored(
                    (mode, false, ORDER),
                    c.clone(),
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                )
                .unwrap();
                let mut init = Vec::new();
                let r = execute(mode, c, &mut init, &mut Vec::new());
                assert_eq!(r, plain);
                assert_eq!(init.len(), 1);
                assert_eq!(init[0].initialization.phase, 1);
                assert_eq!(init[0].initialization.kind, SolverPhaseKind::Exact);
                assert_eq!(
                    init[0].initialization.terminated_early,
                    r.phases[1].termination
                        != PhaseTermination::Exact(ExactPhaseTermination::Exhausted)
                );
                if exact_budget == 0 {
                    assert_eq!(init[0].score, r.phases[0].score);
                    assert_eq!(init[0].complete, initial_budget != 0);
                }
            }
        }
    }
}
#[test]
fn initialization_callback_cancellation_and_failures_keep_phase_boundaries() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        let mut c = SolverPlan::default();
        c.limits.cancellation = Some(&token);
        let r = run_scored_events(
            (mode, false, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            None,
            Some(&mut |_| {
                token.cancel();
            }),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        assert_eq!(r.phases.len(), 1);
        assert!(r.complete);
        if mode % 2 == 1 {
            for fail in [1, 2, 3] {
                let mut c = SolverPlan::default();
                c.phases
                    .insert(0, SolverPhase::Construction(ConstructionConfig::default()));
                let mut init = Vec::new();
                let r = run_scored_events(
                    (mode, false, ORDER),
                    c,
                    &Cell::new(0),
                    Some(fail),
                    || {},
                    SimpleScore::new,
                    None,
                    Some(&mut |u| init.push(u)),
                );
                assert_eq!(r, Err(Failure::Phase(fail - 1)));
                assert_eq!(init.len(), usize::from(fail == 3));
            }
        }
        let mut c = SolverPlan::default();
        c.phases[0] = SolverPhase::LocalSearch(LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 0,
                aspiration: true,
            },
            ..Default::default()
        });
        assert_eq!(
            run_scored_events(
                (mode, false, ORDER),
                c,
                &Cell::new(0),
                None,
                || panic!("invalid plan scored"),
                SimpleScore::new,
                None,
                Some(&mut |_| panic!("invalid plan emitted"))
            ),
            Err(Failure::Acceptance(1))
        );
    }
}
