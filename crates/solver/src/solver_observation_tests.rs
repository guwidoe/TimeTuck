//! Original controlled-time and retained-state observer checks.
use super::*;
use std::cell::Cell;
fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}
#[test]
fn observer_time_preserves_shared_stagnation_in_local_and_exact_phases() {
    for exact_phase in [false, true] {
        let elapsed = Cell::new(0);
        let mut events = Vec::new();
        let mut callback = |u: SolverUpdate<'_, Assignment<usize>, SimpleScore>| {
            events.push((u.phase, u.kind, u.score));
            if u.phase == 1 {
                elapsed.set(10);
            }
        };
        let mut observation = PlanObservation::new(Some(PlanCallback::Best(&mut callback)));
        let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
        let mut n = ConstraintNetwork::new(1);
        n.add_constraint("value", vec![0], |v| {
            SimpleScore::new(v[0].copied().unwrap() as i64)
        })
        .unwrap();
        let phases = vec![
            if exact_phase {
                SolverPhase::Exact(ExactPhaseConfig::default())
            } else {
                SolverPhase::LocalSearch(LocalSearchConfig {
                    max_steps: 10,
                    selection: MoveSelection::FirstAccepted,
                    ..Default::default()
                })
            },
            SolverPhase::Exact(ExactPhaseConfig::default()),
        ];
        let r = run_with_clock(
            SolverLimits {
                max_unimproved_time: Some(ms(5)),
                ..Default::default()
            },
            ConstructionConfig::default(),
            &phases,
            InitialState {
                assignment: p.initial_assignment(),
                complete: p.is_complete(&p.initial_assignment()),
            },
            |index, a, request, time| -> Result<_, LocalSearchError> {
                let request = request.with_time(time);
                let kind = request.kind();
                let mut notify = |u: SolutionUpdate<'_, Assignment<usize>, SimpleScore>| {
                    observation.notify(index, kind, u)
                };
                Ok(match request {
                    PhaseRequest::Custom(..) => unreachable!("fixture has no custom phase"),
                    PhaseRequest::Construction(c) => {
                        constructed!(greedy_construction_with_observer(&p, &n, c, &mut notify)?)
                    }
                    PhaseRequest::LocalSearch(c) => {
                        improved!(crate::solution_observation::local_search_from_observed(
                            time.monitor(),
                            &p,
                            &n,
                            a.unwrap(),
                            c,
                            &mut notify
                        )?)
                    }
                    PhaseRequest::Exact(c, prior) => exact::run_observed(
                        c,
                        prior,
                        time,
                        |visit| crate::search::visit_assignments(&p, visit),
                        |a| n.calculate(&p, a).map(|s| (s, 1)),
                        Some(&mut notify),
                    )?,
                })
            },
            &mut || ms(elapsed.get()),
        )
        .unwrap();
        assert_eq!(r.termination, SolverTermination::UnimprovedTimeLimit);
        assert_eq!(r.score, Some(SimpleScore::new(1)));
        assert_eq!(r.phases.len(), 2);
        assert_eq!(
            events,
            vec![
                (0, SolverPhaseKind::Construction, SimpleScore::ZERO),
                (
                    1,
                    if exact_phase {
                        SolverPhaseKind::Exact
                    } else {
                        SolverPhaseKind::LocalSearch
                    },
                    SimpleScore::new(1)
                )
            ]
        );
        if !exact_phase {
            assert_eq!(r.work.completed_steps, 1);
        }
    }
}
#[test]
fn exact_observer_elapsed_time_stops_before_next_candidate_with_best_preserved() {
    let elapsed = Cell::new(0);
    let mut events = Vec::new();
    let mut callback = |u: SolverUpdate<'_, Assignment<usize>, SimpleScore>| {
        events.push(u.score);
        if u.phase == 1 {
            elapsed.set(10);
        }
    };
    let mut observation = PlanObservation::new(Some(PlanCallback::Best(&mut callback)));
    let p = FiniteProblem::new(vec![Variable::new(vec![0, 1, 2]).with_initial(0)]).unwrap();
    let mut n = ConstraintNetwork::<usize, SimpleScore>::new(1);
    n.add_constraint("value", vec![0], |v| {
        SimpleScore::new(v[0].copied().unwrap() as i64)
    })
    .unwrap();
    let r = run_with_clock(
        SolverLimits {
            max_time: Some(ms(5)),
            ..Default::default()
        },
        ConstructionConfig::default(),
        &[SolverPhase::Exact(ExactPhaseConfig::default())],
        InitialState {
            assignment: p.initial_assignment(),
            complete: p.is_complete(&p.initial_assignment()),
        },
        |index, _, request, time| -> Result<_, LocalSearchError> {
            let request = request.with_time(time);
            let kind = request.kind();
            let mut notify = |u: SolutionUpdate<'_, Assignment<usize>, SimpleScore>| {
                observation.notify(index, kind, u)
            };
            Ok(match request {
                PhaseRequest::Custom(..) => unreachable!("fixture has no custom phase"),
                PhaseRequest::Construction(c) => {
                    constructed!(greedy_construction_with_observer(&p, &n, c, &mut notify)?)
                }
                PhaseRequest::Exact(c, prior) => {
                    let start = elapsed.get();
                    exact::run_with_clock_observed(
                        c,
                        prior,
                        time,
                        |visit| crate::search::visit_assignments(&p, visit),
                        |a| n.calculate(&p, a).map(|score| (score, 1)),
                        || ms(elapsed.get() - start),
                        Some(&mut notify),
                    )?
                }
                _ => unreachable!(),
            })
        },
        &mut || ms(elapsed.get()),
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(r.score, Some(SimpleScore::new(1)));
    assert_eq!(r.work.evaluations, 3);
    assert_eq!(r.work.move_evaluations, 2);
    assert_eq!(events, vec![SimpleScore::ZERO, SimpleScore::new(1)]);
}

#[test]
fn initialization_callback_time_stops_the_solve_before_local_search() {
    let elapsed = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![1])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let mut callback = |event: SolverEvent<'_, Assignment<i32>, SimpleScore>| {
        if let SolverEvent::FirstInitialized { .. } = event {
            elapsed.set(10);
        }
    };
    let mut observation = PlanObservation::new(Some(PlanCallback::Events(&mut callback)));
    let r = run_with_clock_observed(
        SolverLimits {
            max_time: Some(ms(5)),
            ..Default::default()
        },
        &[
            SolverPhase::Construction(ConstructionConfig::default()),
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
        ],
        || InitialState {
            assignment: p.initial_assignment(),
            complete: p.is_complete(&p.initial_assignment()),
        },
        &mut observation,
        |index, _, request, time, _| -> Result<_, SolverError<ScoringError>> {
            assert_eq!(index, 0);
            let PhaseRequest::Construction(c) = request.with_time(time) else {
                panic!("only construction can run");
            };
            Ok(constructed!(greedy_construction(&p, &n, c).map_err(
                |error| SolverError::Phase {
                    phase: index,
                    error
                }
            )?))
        },
        &mut || ms(elapsed.get()),
    )
    .unwrap();
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert!(r.complete);
    assert_eq!(r.phases.len(), 1);
    assert_eq!(r.work.evaluations, 2);
}

#[test]
fn initialization_callback_elapsed_time_precedes_strict_partial_handoff_rejection() {
    let elapsed = Cell::new(0);
    let events = Cell::new(0);
    let p = FiniteProblem::new(vec![Variable::new(vec![0usize, 1])]).unwrap();
    let n = ConstraintNetwork::<_, SimpleScore>::new(1);
    let mut callback = |e: SolverEvent<'_, Assignment<usize>, SimpleScore>| {
        if let SolverEvent::FirstInitialized { complete, .. } = e {
            assert!(!complete);
            events.set(events.get() + 1);
            elapsed.set(10);
        }
    };
    let mut observation = PlanObservation::new(Some(PlanCallback::Events(&mut callback)));
    let r = run_with_clock_observed(
        SolverLimits {
            max_time: Some(ms(5)),
            incomplete_handoff: IncompleteHandoffPolicy::Error,
            ..Default::default()
        },
        &[
            SolverPhase::Construction(ConstructionConfig {
                max_steps: Some(0),
                ..Default::default()
            }),
            SolverPhase::LocalSearch(LocalSearchConfig::default()),
        ],
        || InitialState {
            assignment: p.initial_assignment(),
            complete: false,
        },
        &mut observation,
        |index, _, request, time, _| -> Result<_, SolverError<ScoringError>> {
            assert_eq!(index, 0, "local phase must not start");
            let PhaseRequest::Construction(c) = request.with_time(time) else {
                panic!("construction only");
            };
            Ok(constructed!(greedy_construction(&p, &n, c).map_err(
                |error| SolverError::Phase {
                    phase: index,
                    error
                }
            )?))
        },
        &mut || ms(elapsed.get()),
    )
    .unwrap();
    assert_eq!(events.get(), 1);
    assert_eq!(r.termination, SolverTermination::TimeLimit);
    assert_eq!(r.work.evaluations, 1);
    assert_eq!(r.phases.len(), 1);
    assert!(!r.complete);
}
