//! Original cross-phase observation and managed-plan integration.
use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
    time::Duration,
};
const ORDER: MixedConstructionOrder = MixedConstructionOrder::BasicThenLists;
fn search() -> LocalSearchConfig<'static> {
    LocalSearchConfig {
        max_steps: 1,
        selection: MoveSelection::FirstAccepted,
        assert_scores: true,
        ..Default::default()
    }
}
fn plan() -> SolverPlan<'static> {
    SolverPlan {
        construction: ConstructionConfig {
            pick_early: ConstructionPickEarly::FirstNonDeterioratingScore,
            assert_scores: true,
            ..Default::default()
        },
        phases: vec![
            SolverPhase::LocalSearch(search()),
            SolverPhase::LocalSearch(search()),
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ],
        ..Default::default()
    }
}
fn observed<'a>(
    mode: usize,
    c: SolverPlan<'a>,
    order: MixedConstructionOrder,
    events: &mut Vec<Update>,
) -> Outcome {
    run_scored_observed(
        (mode, false, order),
        c,
        &Cell::new(0),
        None,
        || {},
        SimpleScore::new,
        Some(&mut |u| events.push(u)),
    )
    .unwrap()
}
fn check(events: &[Update], r: &Outcome) {
    assert!(events.windows(2).all(|w| w[0].score < w[1].score));
    if let Some(score) = r.score {
        let last = events.last().unwrap();
        assert_eq!(last.score, score);
        assert_eq!(last.values, r.values);
        assert_eq!(last.complete, r.complete);
    } else {
        assert!(events.is_empty());
    }
    for e in events {
        assert_eq!(
            e.score,
            SimpleScore::new(e.values.iter().flatten().map(|&v| v as i64).sum())
                .with_structural(-(e.values.iter().filter(|v| v.is_none()).count() as i64))
        );
        assert_eq!(e.complete, e.values.iter().all(Option::is_some));
    }
}
#[test]
fn ordered_observation_preserves_outcomes_and_metadata_without_handoff_duplicates() {
    for mode in 0..6 {
        for order in [ORDER, MixedConstructionOrder::ListsThenBasic] {
            let mut events = Vec::new();
            let c = plan();
            let baseline = run_scored(
                (mode, false, order),
                c.clone(),
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
            )
            .unwrap();
            let result = observed(mode, c, order, &mut events);
            assert_eq!(result, baseline);
            check(&events, &result);
            assert_eq!(result.reason, SolverTermination::Optimal);
            assert_eq!(events[0].phase, 0);
            assert_eq!(events[0].kind, SolverPhaseKind::Construction);
            assert!(
                events
                    .iter()
                    .any(|e| e.kind == SolverPhaseKind::LocalSearch)
            );
            assert!(events.windows(2).all(|w| w[0].phase <= w[1].phase));
            for e in events {
                assert_eq!(
                    e.kind,
                    match e.phase {
                        0 => SolverPhaseKind::Construction,
                        1 | 2 => SolverPhaseKind::LocalSearch,
                        3 => SolverPhaseKind::Exact,
                        _ => panic!(),
                    }
                );
            }
        }
    }
}
#[test]
fn exact_updates_follow_prior_best_and_can_finish_partial_initialization() {
    for mode in 0..6 {
        for construction_budget in 0..=2 {
            let mut c = plan();
            c.construction.max_evaluations = construction_budget;
            c.phases = vec![SolverPhase::Exact(ExactPhaseConfig::default())];
            let mut events = Vec::new();
            let r = observed(mode, c, ORDER, &mut events);
            check(&events, &r);
            assert_eq!(r.reason, SolverTermination::Optimal);
            assert!(
                events
                    .iter()
                    .any(|e| e.phase == 1 && e.kind == SolverPhaseKind::Exact)
            );
            assert!(events.iter().filter(|e| e.phase == 1).all(|e| e.complete));
            if construction_budget == 0 {
                assert!(events.iter().all(|e| e.phase == 1));
            }
        }
        let mut c = plan();
        c.phases = vec![
            SolverPhase::LocalSearch(search()),
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ];
        let mut events = Vec::new();
        let r = observed(mode, c, ORDER, &mut events);
        check(&events, &r);
        assert!(
            events
                .iter()
                .any(|e| e.phase == 2 && e.kind == SolverPhaseKind::Exact)
        );
    }
}
#[test]
fn every_shared_budget_prefix_matches_unobserved_results_and_latest_update() {
    for mode in 0..6 {
        for max in 0..=20 {
            for moves in [false, true] {
                let mut c = plan();
                if moves {
                    c.limits.max_move_evaluations = Some(max);
                } else {
                    c.limits.max_evaluations = max;
                }
                let baseline = run_scored(
                    (mode, false, ORDER),
                    c.clone(),
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                )
                .unwrap();
                let mut events = Vec::new();
                let r = observed(mode, c, ORDER, &mut events);
                assert_eq!(r, baseline, "mode {mode}, limit {max}, moves {moves}");
                check(&events, &r);
            }
        }
    }
}
#[test]
fn observed_plans_keep_shared_stagnation_steps_and_validate_all_phases_before_events() {
    for mode in 0..6 {
        let mut c = plan();
        c.limits.max_unimproved_steps = Some(3);
        let search = LocalSearchConfig {
            max_steps: 2,
            acceptance: Acceptance::HillClimbing,
            ..search()
        };
        c.phases = vec![SolverPhase::LocalSearch(search.clone()); 4];
        let baseline = run_scored(
            (mode, false, ORDER),
            c.clone(),
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
        )
        .unwrap();
        let mut events = Vec::new();
        let r = run_scored_observed(
            (mode, false, ORDER),
            c.clone(),
            &Cell::new(0),
            None,
            || {},
            |_| SimpleScore::ZERO,
            Some(&mut |u| events.push(u)),
        )
        .unwrap();
        assert_eq!(r, baseline);
        assert_eq!(r.reason, SolverTermination::UnimprovedStepLimit);
        assert_eq!(r.work.completed_steps, 3);
        assert!(events.iter().all(|e| e.phase == 0));
        c.phases.push(SolverPhase::LocalSearch(LocalSearchConfig {
            acceptance: Acceptance::VariableTabu {
                tenure: 0,
                aspiration: false,
            },
            ..search
        }));
        let factories = Cell::new(0);
        assert_eq!(
            run_scored_observed(
                (mode, false, ORDER),
                c,
                &factories,
                None,
                || panic!("must not score"),
                SimpleScore::new,
                Some(&mut |_| panic!("invalid plan event"))
            ),
            Err(Failure::Acceptance(5))
        );
        assert_eq!(factories.get(), 0);
    }
}
#[test]
fn callback_cancellation_retains_committed_best_and_panics_are_managed_solve_errors() {
    for mode in 0..6 {
        let token = CancellationToken::new();
        let mut c = plan();
        c.limits.cancellation = Some(&token);
        let mut events = Vec::new();
        let r = run_scored_observed(
            (mode, false, ORDER),
            c,
            &Cell::new(0),
            None,
            || {},
            SimpleScore::new,
            Some(&mut |u| {
                if u.phase == 1 {
                    token.cancel();
                }
                events.push(u);
            }),
        )
        .unwrap();
        assert_eq!(r.reason, SolverTermination::Cancelled);
        check(&events, &r);
        assert_eq!(events.last().unwrap().phase, 1);
        assert!(
            catch_unwind(AssertUnwindSafe(|| run_scored_observed(
                (mode, false, ORDER),
                plan(),
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
                Some(&mut |_| panic!("plan observer"))
            )))
            .is_err()
        );
        let mut manager = SolverManager::<Outcome, Failure>::new(1).unwrap();
        let job = manager
            .submit(1, move |_| {
                run_scored_observed(
                    (mode, false, ORDER),
                    plan(),
                    &Cell::new(0),
                    None,
                    || {},
                    SimpleScore::new,
                    Some(&mut |_| panic!("plan observer")),
                )
            })
            .unwrap();
        assert!(
            matches!(job.wait_timeout(Duration::from_secs(10)).unwrap().as_ref(),Err(SolverJobError::Panicked(s)) if s=="plan observer")
        );
        manager.shutdown().unwrap();
    }
}
#[test]
fn graph_factory_failure_preserves_event_prefix_and_managed_success_delivers_final_best() {
    let mut manager = SolverManager::<Update, Failure>::new(2).unwrap();
    for mode in 0..6 {
        if mode % 2 == 1 {
            let factories = Cell::new(0);
            let mut events = Vec::new();
            assert_eq!(
                run_scored_observed(
                    (mode, false, ORDER),
                    plan(),
                    &factories,
                    Some(2),
                    || {},
                    SimpleScore::new,
                    Some(&mut |u| events.push(u))
                ),
                Err(Failure::Phase(1))
            );
            assert_eq!(factories.get(), 2);
            assert!(!events.is_empty());
            assert!(events.iter().all(|e| e.phase == 0));
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let best = seen.clone();
        let final_seen = seen.clone();
        let job = manager
            .submit_with_progress(
                mode as u64,
                move |token, p| {
                    let mut c = plan();
                    c.limits.cancellation = Some(&token);
                    let mut last = None;
                    run_scored_observed(
                        (mode, false, ORDER),
                        c,
                        &Cell::new(0),
                        None,
                        || {},
                        SimpleScore::new,
                        Some(&mut |u| {
                            last = Some(u.clone());
                            assert!(p.publish(u));
                        }),
                    )?;
                    Ok(last.unwrap())
                },
                SolverJobCallbacks::default()
                    .on_best(move |_, u: &Update| best.lock().unwrap().push(u.clone()))
                    .on_final(move |_, u| {
                        let seen = final_seen.lock().unwrap();
                        assert_eq!(seen.last(), Some(u));
                        assert!(seen.windows(2).all(|w| w[0].score < w[1].score));
                    }),
            )
            .unwrap();
        assert!(job.wait_timeout(Duration::from_secs(10)).unwrap().is_ok());
        assert!(job.callback_failures().is_empty());
    }
    manager.shutdown().unwrap();
}

#[test]
fn exact_observer_cancellation_preserves_prefix_and_final_exhaustion_precedence() {
    for mode in 0..6 {
        for final_candidate in [false, true] {
            let token = CancellationToken::new();
            let mut c = plan();
            c.limits.cancellation = Some(&token);
            c.construction.max_evaluations = 0;
            c.phases = vec![SolverPhase::Exact(ExactPhaseConfig::default())];
            let threshold = if final_candidate {
                if mode < 4 { 2 } else { 4 }
            } else {
                0
            };
            let mut events = Vec::new();
            let r = run_scored_observed(
                (mode, false, ORDER),
                c,
                &Cell::new(0),
                None,
                || {},
                SimpleScore::new,
                Some(&mut |u| {
                    assert_eq!(u.kind, SolverPhaseKind::Exact);
                    if u.score == SimpleScore::new(threshold) {
                        token.cancel();
                    }
                    events.push(u);
                }),
            )
            .unwrap();
            assert!(token.is_cancelled());
            check(&events, &r);
            assert_eq!(r.score, Some(SimpleScore::new(threshold)));
            assert_eq!(
                r.reason,
                if final_candidate {
                    SolverTermination::Optimal
                } else {
                    SolverTermination::Cancelled
                }
            );
        }
    }
}
