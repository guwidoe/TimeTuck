//! Original strict handoff contracts across all six model/backend combinations.
use super::*;
const ORDER: MixedConstructionOrder = MixedConstructionOrder::BasicThenLists;
fn strict<'a>(construction: ConstructionConfig<'a>) -> SolverPlan<'a> {
    SolverPlan {
        construction,
        limits: SolverLimits {
            incomplete_handoff: IncompleteHandoffPolicy::Error,
            ..Default::default()
        },
        phases: vec![SolverPhase::LocalSearch(LocalSearchConfig::default())],
    }
}
#[test]
fn strict_incomplete_handoffs_publish_initialization_before_error_and_skip_local_factories() {
    for mode in 0..6 {
        for construction in [
            ConstructionConfig {
                max_steps: Some(0),
                ..Default::default()
            },
            ConstructionConfig {
                max_evaluations: 0,
                ..Default::default()
            },
            ConstructionConfig {
                max_evaluations: 1,
                ..Default::default()
            },
            ConstructionConfig {
                max_move_evaluations: Some(0),
                ..Default::default()
            },
        ] {
            for zero_local in [false, true] {
                let mut c = strict(construction);
                if zero_local {
                    c.phases = vec![SolverPhase::LocalSearch(LocalSearchConfig {
                        max_steps: 0,
                        max_evaluations: 0,
                        ..Default::default()
                    })];
                }
                let mut initializations = vec![];
                let factories = Cell::new(0);
                let calls = Cell::new(0);
                let r = run_scored_events(
                    (mode, false, ORDER),
                    c.clone(),
                    &factories,
                    Some(2),
                    || calls.set(calls.get() + 1),
                    SimpleScore::new,
                    None,
                    Some(&mut |u| initializations.push(u)),
                );
                assert_eq!(r, Err(Failure::Incomplete(1)));
                assert_eq!(initializations.len(), 1);
                let e = &initializations[0];
                assert!(!e.complete);
                assert!(e.values.iter().all(Option::is_none));
                assert_eq!(e.score.is_none(), construction.max_evaluations == 0);
                assert_eq!(
                    e.initialization,
                    SolverInitialization {
                        phase: 0,
                        kind: SolverPhaseKind::Construction,
                        terminated_early: true
                    }
                );
                assert_eq!(factories.get(), if mode % 2 == 1 { 1 } else { 0 });
                assert_eq!(calls.get(), usize::from(construction.max_evaluations != 0));
                let plain = run_scored(
                    (mode, false, ORDER),
                    c.clone(),
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                );
                assert_eq!(plain, Err(Failure::Incomplete(1)));
                c.limits.incomplete_handoff = IncompleteHandoffPolicy::ReturnPartial;
                let partial = run_scored(
                    (mode, false, ORDER),
                    c,
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                )
                .unwrap();
                assert_eq!(partial.reason, SolverTermination::IncompleteConstruction);
                assert_eq!(partial.phases.len(), 1);
                assert_eq!(partial.values, e.values);
                assert_eq!(partial.score, e.score);
            }
        }
    }
}

#[test]
fn shared_stops_and_initialization_callback_cancellation_precede_strict_handoff_errors() {
    for mode in 0..6 {
        for (scores, moves, reason) in [
            (0, None, SolverTermination::EvaluationLimit),
            (1, None, SolverTermination::EvaluationLimit),
            (100, Some(0), SolverTermination::MoveEvaluationLimit),
        ] {
            let mut c = strict(ConstructionConfig {
                max_steps: Some(0),
                ..Default::default()
            });
            c.limits.max_evaluations = scores;
            c.limits.max_move_evaluations = moves;
            let mut initializations = vec![];
            let r = run_scored_events(
                (mode, false, ORDER),
                c,
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
                None,
                Some(&mut |e| initializations.push(e)),
            )
            .unwrap();
            assert_eq!(r.reason, reason);
            assert_eq!(initializations.len(), 1);
            assert!(!r.complete);
        }
        let token = CancellationToken::new();
        let mut c = strict(ConstructionConfig {
            max_steps: Some(0),
            ..Default::default()
        });
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
        assert!(!r.complete);
        let mut c = strict(ConstructionConfig {
            max_steps: Some(0),
            ..Default::default()
        });
        c.limits.max_time = Some(std::time::Duration::ZERO);
        let r = run_scored(
            (mode, false, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::TimeLimit);
    }
}

#[test]
fn strict_policy_preserves_partial_final_results_and_resumable_construction_or_exact_handoffs() {
    for mode in 0..6 {
        let mut c = strict(ConstructionConfig {
            max_steps: Some(0),
            ..Default::default()
        });
        c.phases.clear();
        let r = run_scored(
            (mode, false, ORDER),
            c.clone(),
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::IncompleteConstruction);
        assert!(!r.complete);
        c.phases = vec![
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig {
                max_steps: 0,
                ..Default::default()
            }),
        ];
        let mut init = vec![];
        let r = run_scored_events(
            (mode, false, ORDER),
            c.clone(),
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            None,
            Some(&mut |e| init.push(e)),
        )
        .unwrap();
        assert!(r.complete);
        assert_eq!(init[0].initialization.phase, 1);
        c.phases = vec![
            SolverPhase::Exact(ExactPhaseConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
        ];
        let r = run_scored(
            (mode, false, ORDER),
            c.clone(),
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
        )
        .unwrap();
        assert!(r.complete);
        assert_eq!(r.reason, SolverTermination::Optimal);
        // Repeated partial construction reports the actual destination index.
        c.phases = vec![
            SolverPhase::Construction(c.construction),
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
        ];
        assert_eq!(
            run_scored(
                (mode, false, ORDER),
                c,
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new
            ),
            Err(Failure::Incomplete(2))
        );
    }
}
